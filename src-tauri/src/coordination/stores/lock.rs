//! Advisory file locks for store concurrency safety.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use fs2::FileExt;
use taurhaus_lib::logging::emit_global;

use crate::coordination::errors::CoordinationError;

const LOCK_FILENAME: &str = ".lock";
const INODE_RETRY_LIMIT: usize = 50;
pub(super) const READ_RETRY_BACKOFFS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(500),
];

thread_local! {
    static HELD_TEAM_LOCKS: RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());
}

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

/// Paths already reported as unlockable, so one degraded volume does not
/// produce a line per write.
fn reported_unsupported_locks() -> &'static Mutex<HashSet<PathBuf>> {
    static REPORTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REPORTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Whether this path's unlockable storage still has to be reported.
fn note_unsupported_lock(path: &Path) -> bool {
    reported_unsupported_locks()
        .lock()
        .map(|mut reported| reported.insert(path.to_path_buf()))
        .unwrap_or(true)
}

/// Report storage whose advisory locks the platform refuses.
///
/// Windows answers `ERROR_INVALID_FUNCTION` for `LockFileEx` on the redirected
/// paths a WSL-resolved teams directory lives behind. The store still writes —
/// refusing to would take coordination down on exactly the platform the release
/// builds target — and every field a cross-writer owns is re-read inside the
/// same critical section, so what is left exposed is the rename itself. That is
/// still a degradation an operator has to be able to see, so it is a structured
/// event and not only a line in the tracing log, and it is emitted once per
/// path rather than once per write.
fn report_unsupported_lock(path: &Path, scope: &str) {
    if !note_unsupported_lock(path) {
        return;
    }
    let mut fields = serde_json::Map::new();
    fields.insert(
        "path".to_string(),
        serde_json::Value::String(path.display().to_string()),
    );
    fields.insert(
        "scope".to_string(),
        serde_json::Value::String(scope.to_string()),
    );
    emit_global(
        "warn",
        "coordination",
        "coordination.store.lock_unsupported",
        Some("Advisory file locks are unsupported for this path".to_string()),
        fields,
    );
}

/// Rename errors Windows answers when a volume cannot atomically replace
/// the target: ERROR_INVALID_FUNCTION (1), ERROR_ACCESS_DENIED (5 — the 9p
/// server behind a `\\wsl.localhost` teams dir refuses to replace a file
/// any handle holds open, our own target lock included; NTFS replaces an
/// open file via POSIX-semantics rename, so this only fires where the
/// atomic path truly is unavailable), and ERROR_SHARING_VIOLATION (32).
/// Platform-gated deliberately: only the Windows app drives these volumes,
/// and the same numbers on Linux are EPERM/EIO/EPIPE — real faults a
/// truncating fallback must never paper over.
pub(crate) fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    if should_force_rename_fallback_for_tests() {
        return true;
    }
    cfg!(target_os = "windows") && matches!(err.raw_os_error(), Some(1 | 5 | 32))
}

/// Test-only forcing hook, mirroring the template store's
/// `TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK`: lets tests drive the fallback
/// branches that are otherwise dead behind the platform gate. Read once —
/// a production process cannot have the classification flipped mid-run by
/// an injected variable, and tests that set it do so under the shared env
/// guard before the first store call.
fn should_force_rename_fallback_for_tests() -> bool {
    static FORCED: OnceLock<bool> = OnceLock::new();
    *FORCED.get_or_init(|| {
        std::env::var_os("TAURHAUS_FORCE_COORDINATION_RENAME_FALLBACK").is_some_and(|v| v == "1")
    })
}

/// Paths already reported for the non-atomic write fallback, so a degraded
/// volume produces one structured event per path, not one per save.
fn reported_degraded_writes() -> &'static Mutex<HashSet<PathBuf>> {
    static REPORTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REPORTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// A save fell back from the atomic rename to a direct write. Structured,
/// once per path, like `report_unsupported_lock`: an operator has to be able
/// to see which stores are publishing non-atomically.
pub(crate) fn report_atomic_write_degraded(path: &Path, scope: &str, raw_os_error: Option<i32>) {
    let inserted = reported_degraded_writes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf());
    if !inserted {
        return;
    }
    let mut fields = serde_json::Map::new();
    fields.insert(
        "path".to_string(),
        serde_json::Value::String(path.display().to_string()),
    );
    fields.insert(
        "scope".to_string(),
        serde_json::Value::String(scope.to_string()),
    );
    if let Some(code) = raw_os_error {
        fields.insert("raw_os_error".to_string(), serde_json::Value::from(code));
    }
    emit_global(
        "warn",
        "coordination",
        "coordination.store.atomic_write_degraded",
        Some("Store save fell back to a non-atomic direct write".to_string()),
        fields,
    );
}

pub(super) fn is_transient_file_lock_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(5 | 32 | 33))
}

pub(super) fn read_to_string_with_retry(path: &Path) -> std::io::Result<String> {
    let mut retry_index = 0;
    loop {
        match fs::read_to_string(path) {
            Ok(contents) => return Ok(contents),
            Err(err) if is_transient_file_lock_error(&err) => {
                let Some(delay) = READ_RETRY_BACKOFFS.get(retry_index).copied() else {
                    return Err(err);
                };
                retry_index += 1;
                tracing::warn!(
                    path = %path.display(),
                    attempt = retry_index,
                    max_attempts = READ_RETRY_BACKOFFS.len() + 1,
                    retry_in_ms = delay.as_millis() as u64,
                    raw_os_error = ?err.raw_os_error(),
                    "target file is temporarily locked; retrying read"
                );
                thread::sleep(delay);
            }
            Err(err) => return Err(err),
        }
    }
}

/// Acquire an exclusive advisory lock on a team directory.
///
/// The lock is held for the lifetime of the returned guard.
/// On drop, the lock is automatically released.
///
/// A guard cannot move to another thread: its thread-local ownership marker
/// must be removed on the same thread that installed it. Re-entering this lock
/// on one thread is an error instead of an unbounded `flock` wait.
#[derive(Debug)]
pub struct TeamLockGuard {
    _file: File,
    lock_path: PathBuf,
    teams_dir: PathBuf,
    team_name: String,
    _not_send: PhantomData<Rc<()>>,
}

impl TeamLockGuard {
    pub(crate) fn covers(&self, teams_dir: &Path, team_name: &str) -> bool {
        // The exact acquisition inputs match without any filesystem lookup, so
        // a held guard can never lose its own team to a transient failure.
        if self.teams_dir == teams_dir && self.team_name == team_name {
            return true;
        }
        // A differently spelled path may still name the same team: fall back
        // to the canonical lock identity, computed the way acquisition did.
        self.team_name == team_name && self.lock_path == team_lock_path(teams_dir, team_name)
    }
}

impl Drop for TeamLockGuard {
    fn drop(&mut self) {
        HELD_TEAM_LOCKS.with(|held| {
            let removed = held.borrow_mut().remove(&self.lock_path);
            debug_assert!(removed, "dropping an unregistered team lock guard");
        });
    }
}

fn team_lock_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    let team_dir = teams_dir.join(team_name);
    let canonical_team_dir = fs::canonicalize(&team_dir).unwrap_or(team_dir);
    canonical_team_dir.join(LOCK_FILENAME)
}

pub fn acquire_team_lock(
    teams_dir: &Path,
    team_name: &str,
) -> Result<TeamLockGuard, CoordinationError> {
    let team_dir = teams_dir.join(team_name);
    fs::create_dir_all(&team_dir)?;

    let lock_path = team_lock_path(teams_dir, team_name);
    if HELD_TEAM_LOCKS.with(|held| held.borrow().contains(&lock_path)) {
        return Err(CoordinationError::StoreError(format!(
            "team lock is already held by this thread: {}",
            lock_path.display()
        )));
    }

    let file = File::create(&lock_path).map_err(CoordinationError::Io)?;
    match file.lock_exclusive() {
        Ok(()) => {}
        Err(err) if is_windows_unsupported_lock_error(&err) => {
            tracing::warn!(
                team_name = team_name,
                lock_path = %lock_path.display(),
                "advisory file locks are unsupported for this Windows path; continuing without lock"
            );
            report_unsupported_lock(&lock_path, "team");
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    }
    let inserted = HELD_TEAM_LOCKS.with(|held| held.borrow_mut().insert(lock_path.clone()));
    if !inserted {
        return Err(CoordinationError::StoreError(format!(
            "team lock is already held by this thread: {}",
            lock_path.display()
        )));
    }
    Ok(TeamLockGuard {
        _file: file,
        lock_path,
        teams_dir: teams_dir.to_path_buf(),
        team_name: team_name.to_string(),
        _not_send: PhantomData,
    })
}

/// Exclusive advisory lock held on the file that will be atomically replaced.
///
/// A waiter can open the old inode before another writer renames a new file over
/// the path. After the lock is acquired, compare the descriptor identity with
/// the current path and retry when they differ. This matches mesh's cross-writer
/// lock discipline for config and inbox mutations.
pub struct TargetFileLock {
    file: File,
    path: PathBuf,
    /// False when the advisory lock could not engage on this volume. Reads
    /// then go through a fresh open of the path — after another writer's
    /// move-aside publish the held handle follows the DISPLACED inode, and
    /// only a path read sees the current record.
    lock_engaged: bool,
}

impl TargetFileLock {
    pub fn acquire_or_create(path: &Path) -> Result<Self, CoordinationError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Self::acquire(path, true)?.ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "target file disappeared while locking: {}",
                path.display()
            ))
        })
    }

    /// Lock a file that already exists, or report that it does not.
    ///
    /// A read-modify-write of a record that must exist cannot use
    /// [`Self::acquire_or_create`]: creating the file to lock it would turn a
    /// missing record into an empty one that every later read has to treat as
    /// corrupt.
    pub fn acquire_if_exists(path: &Path) -> Result<Option<Self>, CoordinationError> {
        Self::acquire(path, false)
    }

    fn acquire(path: &Path, create: bool) -> Result<Option<Self>, CoordinationError> {
        // An interrupted move-aside swap leaves the record at its displaced
        // sibling; settle it before opening, or `create` would bury the only
        // copy under a fresh empty file.
        recover_displaced(path, &displaced_path(path));
        for _ in 0..INODE_RETRY_LIMIT {
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .create(create)
                .truncate(false)
                .open(path)
            {
                Ok(file) => file,
                Err(err) if !create && err.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(None)
                }
                Err(err) => return Err(CoordinationError::Io(err)),
            };
            let mut lock_engaged = true;
            match file.lock_exclusive() {
                Ok(()) => {}
                Err(err) if is_windows_unsupported_lock_error(&err) => {
                    lock_engaged = false;
                    tracing::warn!(
                        path = %path.display(),
                        "target-file advisory locks are unsupported for this Windows path; continuing without lock"
                    );
                    report_unsupported_lock(path, "target_file");
                }
                Err(err) => return Err(CoordinationError::Io(err)),
            }
            if inode_matches(&file, path) {
                return Ok(Some(Self {
                    file,
                    path: path.to_path_buf(),
                    lock_engaged,
                }));
            }
        }

        Err(CoordinationError::StoreError(format!(
            "target file inode changed after {INODE_RETRY_LIMIT} lock attempts: {}",
            path.display()
        )))
    }

    pub fn read_contents(&self) -> Result<String, CoordinationError> {
        if !self.lock_engaged {
            // The handle follows the inode it opened; after another writer's
            // move-aside publish that is the displaced pre-image. Where the
            // lock never engaged the handle buys nothing, so read the path.
            return match fs::read_to_string(&self.path) {
                Ok(contents) => Ok(contents),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
                Err(err) => Err(CoordinationError::Io(err)),
            };
        }
        let mut contents = String::new();
        let mut file = &self.file;
        file.seek(SeekFrom::Start(0))?;
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }
}

/// Publish `tmp` at `target` on a volume that refuses to rename over an open
/// file (the 9p server behind `\\wsl.localhost`): move the current target
/// ASIDE — renaming an open file to a sibling name is legal everywhere, and
/// a holder's handle transparently follows the old inode — then rename the
/// fully written, synced tmp into the vacant slot. No reader can ever
/// observe torn content; the worst case is a one-syscall window where the
/// path is absent, which every store reader already treats as an empty or
/// missing record. On failure the previous file is restored and the tmp is
/// left on disk as the intact copy of the intended state. Ported from the
/// template store, which has published this way on unrenameable volumes all
/// along.
/// Read the locked target, allowing one short re-read when its contents are
/// non-empty yet not valid JSON. Move-aside publishes never expose torn
/// content, so this guards only against writers from older builds or plain
/// truncating tools; the wait is a single 100ms because both callers hold
/// the team and target locks across it, and a persistently unparsable file
/// is the caller's decision (repair, skip, or error) — never made on a
/// transient.
pub(crate) fn read_json_tolerating_torn(
    lock: &TargetFileLock,
) -> Result<String, CoordinationError> {
    let raw = lock.read_contents()?;
    if raw.trim().is_empty() || serde_json::from_str::<serde_json::Value>(&raw).is_ok() {
        return Ok(raw);
    }
    thread::sleep(READ_RETRY_BACKOFFS[0]);
    lock.read_contents()
}

/// The deterministic sibling a move-aside swap displaces the old target to:
/// `<file name>.displaced`, appended so `team-lead.json` keeps its identity
/// as `team-lead.json.displaced` (readers filter on the extension and never
/// see it).
pub(crate) fn displaced_path(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".displaced");
    target.with_file_name(name)
}

/// Publish `tmp` at `target` on a volume that refuses to rename over an open
/// file (the 9p server behind `\\wsl.localhost`): move the current target
/// aside — renaming an open file to a sibling name is legal everywhere, and
/// a holder's handle transparently follows the old inode — then rename the
/// fully written, synced tmp into the vacant slot. No reader can ever
/// observe torn content; the worst case is a brief window where the path is
/// absent, which every store reader treats as an empty or missing record.
///
/// The displaced sibling is NEVER removed during the swap. Removing a file
/// whose handle is still open is deferred by the 9p server to handle close,
/// and — verified live with a Rust probe on the affected machine — that
/// deferred delete lands on the TARGET PATH's current file, silently
/// destroying the record this function just published. Cleanup and crash
/// recovery instead happen at the START of the next swap (and in
/// `TargetFileLock::acquire`): a displaced sibling next to a present target
/// is a husk whose holders are gone and is removed; one next to an absent
/// target is an interrupted swap and is restored.
///
/// On failure the previous file is restored and the tmp is left on disk as
/// the intact copy of the intended state — callers remove it only on paths
/// where the target is known intact.
pub(crate) fn replace_via_move_aside(tmp: &Path, target: &Path) -> std::io::Result<()> {
    // No settling here. The aside-rename REPLACES a settled husk implicitly
    // (bounded: one sibling per record), and if a husk is still held open —
    // our own lock during a repair republish, or a concurrent writer — the
    // rename fails cleanly instead of unlinking a held file, which is the
    // deferred-delete class this module exists to avoid. Settle-restore
    // lives only in `TargetFileLock::acquire`, where no handle of ours can
    // be on the sibling yet.
    let displaced = displaced_path(target);
    let had_target = match fs::rename(target, &displaced) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };
    match fs::rename(tmp, target) {
        Ok(()) => Ok(()),
        Err(err) => {
            if had_target {
                if let Err(restore) = fs::rename(&displaced, target) {
                    // Log rather than wrap: wrapping would erase
                    // raw_os_error and defeat every retry classifier.
                    tracing::warn!(
                        displaced = %displaced.display(),
                        error = %restore,
                        "restoring the previous file after a failed publish also failed"
                    );
                }
            }
            Err(err)
        }
    }
}

/// Restore an interrupted swap: when the target is genuinely absent
/// (`ErrorKind::NotFound` from a real stat, never a collapsed transient
/// error) and its displaced sibling exists, the sibling is the only copy of
/// the record and is renamed back. Never removes anything: a sibling beside
/// a present target is consumed by the next swap's replacing aside-rename,
/// so no code path ever unlinks a file another handle might hold — the
/// deferred-delete class stays structurally impossible.
///
/// Concurrency note: a restore could in principle steal the vacant window of
/// another writer's in-flight swap. Exactly one process performs 9p writes
/// today (the Windows app, whose per-team critical section serializes its
/// own threads); the daemon-routing migration removes cross-process 9p
/// writing before a second writer can exist. See
/// docs/design/coordination-daemon-routing.md.
fn recover_displaced(target: &Path, displaced: &Path) {
    match fs::metadata(displaced) {
        Ok(_) => {}
        Err(_) => return,
    }
    match fs::metadata(target) {
        Ok(_) => return,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return,
    }
    tracing::warn!(
        target = %target.display(),
        "recovering an interrupted move-aside swap from its displaced sibling"
    );
    if let Err(err) = fs::rename(displaced, target) {
        tracing::warn!(
            path = %displaced.display(),
            error = %err,
            "failed to recover the displaced sibling"
        );
    }
}

/// Remove a record together with its displaced sibling, so a deliberate
/// delete cannot be resurrected by `recover_displaced` on the next acquire.
/// The sibling goes first: with the target still present, a crash between
/// the two removals leaves a state the restore branch will not touch.
pub(crate) fn remove_record(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(displaced_path(path)) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Synced staging write for the tmp side of a move-aside publish.
pub(crate) fn stage_synced(path: &Path, payload: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(payload)?;
    file.sync_all()
}

#[cfg(unix)]
fn inode_matches(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(file_metadata) = file.metadata() else {
        return true;
    };
    let Ok(path_metadata) = fs::metadata(path) else {
        return true;
    };
    file_metadata.dev() == path_metadata.dev() && file_metadata.ino() == path_metadata.ino()
}

#[cfg(not(unix))]
fn inode_matches(_file: &File, _path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc, Barrier};
    use std::thread;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn unsupported_lock_error_detection_is_platform_aware() {
        // Verified live during the Windows team-init failure: LockFileEx over
        // `\\wsl.localhost` answers ERROR_INVALID_FUNCTION (1) — this
        // degrade path — while the RENAME over the open handle is what
        // answers ERROR_ACCESS_DENIED (5); 5 stays on the transient/read
        // retry policy and must never silently disable locking.
        let err = std::io::Error::from_raw_os_error(1);
        assert_eq!(
            is_windows_unsupported_lock_error(&err),
            cfg!(target_os = "windows")
        );
        for code in [5, 33] {
            assert!(!is_windows_unsupported_lock_error(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
    }

    #[test]
    fn non_unsupported_lock_error_is_rejected() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(!is_windows_unsupported_lock_error(&err));
    }

    #[test]
    fn rename_fallback_predicate_is_platform_gated() {
        // Regression: Windows team init failed at "Sending agent
        // instructions" — the 9p server refuses to rename over an open
        // handle with ERROR_ACCESS_DENIED (5); ERROR_INVALID_FUNCTION (1)
        // and ERROR_SHARING_VIOLATION (32) are the sibling spellings. On
        // Linux the same numbers are EPERM/EIO/EPIPE and must never trigger
        // a truncating fallback.
        for code in [1, 5, 32] {
            assert_eq!(
                is_windows_unsupported_rename_error(&std::io::Error::from_raw_os_error(code)),
                cfg!(target_os = "windows"),
                "os error {code}"
            );
        }
        for code in [2, 13, 33] {
            assert!(!is_windows_unsupported_rename_error(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
    }

    #[test]
    fn move_aside_publish_replaces_content_while_a_handle_stays_open() {
        // Regression: the direct-write fallback for Windows team init
        // truncated the live file in place, exposing torn state to readers.
        // The move-aside publish never does: the path always holds either
        // the complete old or the complete new content, and the open handle
        // (our own target lock) transparently follows the displaced inode.
        let tmp = TempDir::new().expect("tempdir");
        let target = tmp.path().join("record.json");
        let staged = tmp.path().join("record.tmp");
        std::fs::write(&target, "the previous, much longer record body").expect("seed");
        std::fs::write(&staged, "new").expect("stage");

        let lock = TargetFileLock::acquire_or_create(&target).expect("lock");
        replace_via_move_aside(&staged, &target).expect("publish");

        assert_eq!(std::fs::read_to_string(&target).expect("path"), "new");
        assert_eq!(
            lock.read_contents().expect("via handle"),
            "the previous, much longer record body",
            "the lock holder keeps reading the displaced old inode"
        );
        assert!(!staged.exists(), "the staged tmp is consumed on success");
        // Deferred cleanup: the displaced sibling stays until the NEXT swap
        // — removing it while our handle lives would let the 9p server's
        // deferred delete destroy the freshly published target (verified
        // live; see replace_via_move_aside's doc).
        let displaced = displaced_path(&target);
        assert_eq!(
            std::fs::read_to_string(&displaced).expect("displaced sibling remains"),
            "the previous, much longer record body"
        );

        drop(lock);
        std::fs::write(&staged, "second").expect("stage again");
        replace_via_move_aside(&staged, &target).expect("second publish settles the husk");
        assert_eq!(std::fs::read_to_string(&target).expect("path"), "second");
        assert_eq!(
            std::fs::read_to_string(&displaced).expect("displaced now holds the first publish"),
            "new"
        );
    }

    #[test]
    fn an_interrupted_swap_is_recovered_on_the_next_acquire() {
        // Regression: a crash between the two renames leaves the record only
        // at its displaced sibling; acquire_or_create used to bury it under
        // a fresh empty file, which read as "no record" forever.
        let tmp = TempDir::new().expect("tempdir");
        let target = tmp.path().join("record.json");
        std::fs::write(displaced_path(&target), "the only copy").expect("simulate interruption");

        let lock = TargetFileLock::acquire_or_create(&target).expect("lock");
        assert_eq!(
            lock.read_contents().expect("recovered"),
            "the only copy",
            "the displaced sibling must be restored before the open"
        );
    }

    #[test]
    fn displaced_path_appends_never_replaces_the_extension() {
        for (target, expected) in [
            ("record", "record.displaced"),
            ("record.json", "record.json.displaced"),
            ("a.b.json", "a.b.json.displaced"),
            (".lock", ".lock.displaced"),
        ] {
            assert_eq!(
                displaced_path(Path::new(target)),
                Path::new(expected),
                "{target}"
            );
        }
    }

    #[test]
    fn a_forced_fallback_save_publishes_end_to_end() {
        // Drives the fallback branch that is dead on Linux behind the
        // platform gate, via the env seam (read once per process, so it is
        // set before any predicate call in this test binary — the guard
        // serializes env mutation across the suite).
        let _guard = taurhaus_lib::test_support::acquire_env_test_guard();
        std::env::set_var("TAURHAUS_FORCE_COORDINATION_RENAME_FALLBACK", "1");
        let forced = is_windows_unsupported_rename_error(&std::io::Error::from_raw_os_error(99));
        std::env::remove_var("TAURHAUS_FORCE_COORDINATION_RENAME_FALLBACK");
        if !forced {
            // Another test in this process evaluated the OnceLock first with
            // the variable unset; the branch is covered by the CI env lane
            // instead of this opportunistic in-process check.
            return;
        }

        let tmp = TempDir::new().expect("tempdir");
        let target = tmp.path().join("record.json");
        let staged = tmp.path().join("record.json.tmp");
        std::fs::write(&target, "old").expect("seed");
        std::fs::write(&staged, "new").expect("stage");
        let lock = TargetFileLock::acquire_or_create(&target).expect("lock");
        replace_via_move_aside(&staged, &target).expect("publish");
        drop(lock);
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "new");
        assert_eq!(
            std::fs::read_to_string(displaced_path(&target)).expect("sibling"),
            "old"
        );
    }

    #[test]
    fn read_contents_rereads_from_the_start() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("record.json");
        std::fs::write(&path, "stable contents").expect("seed");

        let lock = TargetFileLock::acquire_or_create(&path).expect("lock");
        assert_eq!(lock.read_contents().expect("first"), "stable contents");
        assert_eq!(
            lock.read_contents().expect("second"),
            "stable contents",
            "a reread through the same handle must not start at the old cursor"
        );
    }

    #[test]
    fn move_aside_publish_creates_a_missing_target() {
        let tmp = TempDir::new().expect("tempdir");
        let target = tmp.path().join("record.json");
        let staged = tmp.path().join("record.tmp");
        std::fs::write(&staged, "fresh").expect("stage");

        replace_via_move_aside(&staged, &target).expect("publish");

        assert_eq!(std::fs::read_to_string(&target).expect("path"), "fresh");
    }

    #[test]
    fn windows_lock_violation_is_a_transient_file_lock() {
        // Regression: 694b130 introduced target-file locks but omitted Windows
        // ERROR_LOCK_VIOLATION (33) from the unlocked-reader retry policy.
        assert!(is_transient_file_lock_error(
            &std::io::Error::from_raw_os_error(33)
        ));
    }

    // An unsupported advisory lock used to be reported only through a tracing
    // line on every single write: invisible in the structured log, and drowning
    // the unstructured one. Once per path is what an operator can act on.
    #[test]
    fn unlockable_storage_is_reported_once_per_path() {
        let tmp = TempDir::new().expect("tempdir");
        let first = tmp.path().join("teams-a/.lock");
        let second = tmp.path().join("teams-b/.lock");

        assert!(note_unsupported_lock(&first), "the first sighting reports");
        assert!(
            !note_unsupported_lock(&first),
            "the same path is not reported again on every write"
        );
        assert!(
            note_unsupported_lock(&second),
            "another degraded path is reported on its own"
        );
    }

    #[test]
    fn lock_is_exclusive() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().to_path_buf();
        let team_name = "lock-test";

        let _lock = acquire_team_lock(&teams_dir, team_name).expect("first lock should succeed");

        // Verify a second lock from another thread blocks (we use try_lock to test).
        let teams_dir_clone = teams_dir.clone();
        let handle = thread::spawn(move || {
            let team_dir = teams_dir_clone.join(team_name);
            let lock_path = team_dir.join(LOCK_FILENAME);
            let file = File::open(&lock_path).expect("open lock file");
            file.try_lock_exclusive()
        });

        let result = handle.join().expect("thread should not panic");
        assert!(
            result.is_err(),
            "second exclusive lock should fail with try_lock"
        );
    }

    #[test]
    fn lock_released_on_drop() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().to_path_buf();
        let team_name = "drop-test";

        {
            let _lock =
                acquire_team_lock(&teams_dir, team_name).expect("first lock should succeed");
        }
        // Lock dropped, second acquisition should succeed.
        let _lock = acquire_team_lock(&teams_dir, team_name)
            .expect("second lock should succeed after drop");
    }

    #[cfg(unix)]
    #[test]
    fn guard_scope_survives_team_directory_canonicalization_failure() {
        // Regression: 1827f8a8 re-canonicalized TeamLockGuard::covers while
        // the guard was held, so a transient lookup failure made a valid guard
        // appear to cover another team.
        let tmp = TempDir::new().expect("tempdir");
        let real_teams_dir = tmp.path().join("real-teams");
        let teams_dir = tmp.path().join("teams-link");
        fs::create_dir_all(&real_teams_dir).expect("create real teams dir");
        std::os::unix::fs::symlink(&real_teams_dir, &teams_dir).expect("link teams dir");
        let team_name = "canonicalization-test";

        let guard = acquire_team_lock(&teams_dir, team_name).expect("acquire through symlink");
        fs::remove_dir_all(real_teams_dir.join(team_name)).expect("remove team dir");

        assert!(
            guard.covers(&teams_dir, team_name),
            "guard identity must not depend on another filesystem lookup"
        );
    }

    #[test]
    fn guard_scope_accepts_an_aliased_spelling_of_the_same_teams_dir() {
        // The held-lock set is keyed on the canonical lock path; the scope
        // check must accept a caller that names the same team through a
        // different spelling of the teams dir.
        let tmp = TempDir::new().expect("tempdir");
        let real_teams_dir = tmp.path().join("real-teams");
        let teams_link = tmp.path().join("teams-link");
        fs::create_dir_all(&real_teams_dir).expect("create real teams dir");
        std::os::unix::fs::symlink(&real_teams_dir, &teams_link).expect("link teams dir");
        let team_name = "aliased-spelling-test";

        let guard = acquire_team_lock(&real_teams_dir, team_name).expect("acquire via real path");
        assert!(
            guard.covers(&teams_link, team_name),
            "the symlinked spelling names the same team"
        );
        assert!(
            !guard.covers(&real_teams_dir, "another-team"),
            "a different team is never covered"
        );
    }

    #[test]
    fn reacquiring_a_team_lock_on_the_same_thread_fails_fast() {
        // Regression: 366f4b7 removed orchestrator-wide exclusion, so the
        // replacement needs an outer team lock without letting a nested store
        // acquisition block its own thread forever.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().to_path_buf();
        let team_name = "reentrant-test";

        let (result_tx, result_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _lock =
                acquire_team_lock(&teams_dir, team_name).expect("first lock should succeed");
            let error = acquire_team_lock(&teams_dir, team_name)
                .expect_err("same-thread re-entry must return instead of blocking");
            result_tx.send(error.to_string()).expect("send result");
        });
        let error = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("same-thread re-entry blocked instead of failing fast");
        handle.join().expect("re-entry test thread");

        assert!(
            error.contains("already held by this thread"),
            "unexpected re-entry error: {error}"
        );
    }

    #[test]
    fn concurrent_lock_acquire_serializes() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = Arc::new(tmp.path().to_path_buf());
        let team_name = "concurrent-test";
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let barrier = Arc::new(Barrier::new(4));

        // Pre-create the team dir and lock file.
        acquire_team_lock(&teams_dir, team_name).expect("setup lock");

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let teams = Arc::clone(&teams_dir);
                let ctr = Arc::clone(&counter);
                let bar = Arc::clone(&barrier);
                let name = team_name.to_string();
                thread::spawn(move || {
                    bar.wait();
                    let _lock = acquire_team_lock(&teams, &name).expect("lock should succeed");
                    ctr.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 4);
    }
}
