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
const READ_RETRY_BACKOFFS: [Duration; 3] = [
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
    cfg!(target_os = "windows") && matches!(err.raw_os_error(), Some(1 | 5 | 32))
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

/// Synced direct write for fallbacks at sites that hold no target lock.
pub(crate) fn write_direct_synced(path: &Path, payload: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(payload)?;
    file.sync_all()
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
            match file.lock_exclusive() {
                Ok(()) => {}
                Err(err) if is_windows_unsupported_lock_error(&err) => {
                    tracing::warn!(
                        path = %path.display(),
                        "target-file advisory locks are unsupported for this Windows path; continuing without lock"
                    );
                    report_unsupported_lock(path, "target_file");
                }
                Err(err) => return Err(CoordinationError::Io(err)),
            }
            if inode_matches(&file, path) {
                return Ok(Some(Self { file }));
            }
        }

        Err(CoordinationError::StoreError(format!(
            "target file inode changed after {INODE_RETRY_LIMIT} lock attempts: {}",
            path.display()
        )))
    }

    pub fn read_contents(&self) -> Result<String, CoordinationError> {
        let mut contents = String::new();
        let mut file = &self.file;
        file.seek(SeekFrom::Start(0))?;
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    /// Replace the locked target's contents through the very handle that
    /// owns the lock — the direct-write fallback for volumes that refuse to
    /// rename over an open file. A second handle would be blocked by our own
    /// byte-range lock where locking works, and is pointless where it
    /// degraded; this handle works in both worlds. Not atomic: a concurrent
    /// reader on a lock-degraded volume can observe a torn state, which the
    /// stores' readers treat as a transient before quarantining anything.
    pub fn overwrite(&self, payload: &[u8]) -> std::io::Result<()> {
        let mut file = &self.file;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(payload)?;
        self.file.set_len(payload.len() as u64)?;
        self.file.sync_all()
    }
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
    fn overwrite_replaces_longer_content_through_the_lock_handle() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("record.json");
        std::fs::write(&path, "the previous, much longer record body").expect("seed");

        let lock = TargetFileLock::acquire_or_create(&path).expect("lock");
        lock.overwrite(b"short").expect("overwrite");

        assert_eq!(lock.read_contents().expect("read back"), "short");
        drop(lock);
        assert_eq!(std::fs::read_to_string(&path).expect("reread"), "short");
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
