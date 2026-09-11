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
    static HOST_OPERATION_HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Stable shared exclusion inode for an owned host's bounded operations.
///
/// Acquire after releasing data locks, never together with terminal exclusion.
/// Contract v1 permits short runtime snapshots under this guard, but no data
/// lock may span host I/O. Callers must give every read/write the remaining
/// submission deadline; this guard must never span a model turn.
#[derive(Debug)]
pub struct HostOperationLock {
    _file: File,
    deadline: std::time::Instant,
    _not_send: PhantomData<Rc<()>>,
}

#[cfg(test)]
thread_local! {
    /// Test seam: a shorter launch deadline so a live-child timeout case takes seconds.
    pub(crate) static LAUNCH_DEADLINE_OVERRIDE: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

impl HostOperationLock {
    pub fn acquire(
        root: &Path,
        team: &str,
        member: &str,
        wait: Duration,
    ) -> Result<Self, CoordinationError> {
        crate::coordination::validation::validate_team_name(team)?;
        crate::coordination::validation::validate_member_name(member)?;
        if HOST_OPERATION_HELD.get()
            || taurhaus_lib::platform::terminal_io::active()
            || HELD_TEAM_LOCKS.with(|locks| !locks.borrow().is_empty())
        {
            return Err(CoordinationError::Conflict(
                "host-operation lock must be last and alone".into(),
            ));
        }
        let directory = root.join(team).join("state/app-server");
        fs::create_dir_all(&directory)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join(format!("{member}.lock")))?;
        let deadline = std::time::Instant::now() + wait.min(Duration::from_secs(2));
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => break,
                Err(error) if terminal_lock_contended(&error, cfg!(target_os = "windows")) => {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(CoordinationError::Conflict(
                            "host operation deferred: lock busy".into(),
                        ));
                    }
                    thread::sleep(remaining.min(Duration::from_millis(10)));
                }
                // Unsupported flock is a hard refusal, never unlocked I/O.
                Err(error) => return Err(error.into()),
            }
        }
        HOST_OPERATION_HELD.set(true);
        Ok(Self {
            _file: file,
            deadline: std::time::Instant::now() + Duration::from_secs(5),
            _not_send: PhantomData,
        })
    }

    /// Cold process startup has its own bound; ordinary submissions stay at five seconds.
    pub fn acquire_for_launch(
        root: &Path,
        team: &str,
        member: &str,
    ) -> Result<Self, CoordinationError> {
        let mut guard = Self::acquire(root, team, member, Duration::from_secs(2))?;
        #[cfg(test)]
        let budget = LAUNCH_DEADLINE_OVERRIDE
            .with(|d| d.get())
            .unwrap_or(Duration::from_secs(30));
        #[cfg(not(test))]
        let budget = Duration::from_secs(30);
        guard.deadline = std::time::Instant::now() + budget;
        Ok(guard)
    }

    /// Background activity reads never consume a user-operation deadline.
    pub fn acquire_for_activity(
        root: &Path,
        team: &str,
        member: &str,
    ) -> Result<Self, CoordinationError> {
        let mut guard = Self::acquire(root, team, member, Duration::ZERO)?;
        guard.deadline = std::time::Instant::now() + Duration::from_millis(250);
        Ok(guard)
    }

    pub fn remaining(&self) -> Result<Duration, CoordinationError> {
        let remaining = self
            .deadline
            .saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(CoordinationError::Conflict(
                "host operation deadline expired".into(),
            ));
        }
        Ok(remaining)
    }
}

impl Drop for HostOperationLock {
    fn drop(&mut self) {
        HOST_OPERATION_HELD.set(false);
        // Test forks can retain this descriptor briefly; release fixture exclusion now.
        #[cfg(test)]
        let _ = fs2::FileExt::unlock(&self._file);
        // Closing the descriptor releases exclusion. Never unlink its inode.
    }
}

/// Permanent per-member inode. Drop closes it; it is never unlinked.
#[derive(Debug)]
pub struct TerminalLock {
    _file: File,
    holder: PathBuf,
    _not_send: PhantomData<Rc<()>>,
}

impl TerminalLock {
    pub fn acquire(
        root: &Path,
        team: &str,
        member: &str,
        op: &str,
        epoch: u64,
        wait: Duration,
    ) -> Result<Self, CoordinationError> {
        crate::coordination::validation::validate_team_name(team)?;
        crate::coordination::validation::validate_member_name(member)?;
        if HOST_OPERATION_HELD.get()
            || taurhaus_lib::platform::terminal_io::active()
            || HELD_TEAM_LOCKS.with(|h| !h.borrow().is_empty())
        {
            return Err(CoordinationError::Conflict(
                "terminal lock must be last and alone".into(),
            ));
        }
        let dir = root.join(team).join("state/terminal");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{member}.lock"));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        let deadline = std::time::Instant::now() + wait.min(Duration::from_secs(2));
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => break,
                Err(e) if terminal_lock_contended(&e, cfg!(target_os = "windows")) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(CoordinationError::Conflict(
                            "terminal write deferred: lock busy".into(),
                        ));
                    }
                    thread::sleep(
                        Duration::from_millis(10)
                            .min(deadline.saturating_duration_since(std::time::Instant::now())),
                    );
                }
                Err(e) => {
                    if note_unsupported_lock(&path) {
                        tracing::error!(team, member, error = %e, "terminal write disabled: flock unavailable");
                        let fields = serde_json::json!({"team": team, "member": member, "error": e.to_string()});
                        emit_global(
                            "error",
                            "coordination",
                            "coordination.terminal.unavailable",
                            Some("Terminal writes require an engaged flock".into()),
                            fields.as_object().unwrap().clone(),
                        );
                    }
                    return Err(e.into());
                }
            }
        }
        let holder = dir.join(format!("{member}.holder.json"));
        // Overwrite a crashed owner's diagnostic only after acquiring ownership.
        fs::write(&holder, serde_json::to_vec(&serde_json::json!({"owner": "taurhaus", "op": op, "epoch": epoch, "since": chrono::Utc::now()})).map_err(|e| CoordinationError::StoreError(e.to_string()))?)?;
        taurhaus_lib::platform::terminal_io::enter(file.try_clone()?);
        Ok(Self {
            _file: file,
            holder,
            _not_send: PhantomData,
        })
    }
}
impl Drop for TerminalLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.holder);
        taurhaus_lib::platform::terminal_io::leave();
    }
}

pub fn terminal_write<T>(
    root: &Path,
    team: &str,
    member: &str,
    op: &str,
    write: impl FnOnce() -> Result<T, CoordinationError>,
) -> Result<T, CoordinationError> {
    let epoch = match super::runtime::MemberRuntimeStore::load(root, team, member) {
        Ok(r) => r.attachment_generation,
        Err(CoordinationError::NotFound(_)) => 0,
        Err(e) => return Err(e),
    };
    let _guard = TerminalLock::acquire(root, team, member, op, epoch, Duration::from_secs(2))?;
    write()
}

/// Stop IPC carries a pane, so resolve the managed record before taking its
/// lock. An unreadable runtime is an error, never an unlocked fallback.
pub fn terminal_write_for_pane<T>(
    pane: &str,
    op: &str,
    write: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    terminal_write_for_pane_at_root(
        &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        pane,
        op,
        write,
    )
}

pub(crate) fn terminal_write_for_pane_at_root<T>(
    teams_dir: &Path,
    pane: &str,
    op: &str,
    write: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    terminal_write_for_pane_with_runtime(
        teams_dir,
        pane,
        op,
        &crate::coordination::runtime::SystemCoordinationRuntime,
        write,
    )
}

fn terminal_write_for_pane_with_runtime<T>(
    teams_dir: &Path,
    pane: &str,
    op: &str,
    runtime: &dyn crate::coordination::runtime::CoordinationRuntime,
    write: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let registry = super::team_roots::TeamRootRegistry::new(teams_dir.to_path_buf());
    if let Some((root, team, member)) =
        resolve_terminal_member_with_runtime(&registry, pane, runtime)?
    {
        // Windows app fallback must not create lock/holder state on the UNC volume.
        #[cfg(target_os = "windows")]
        {
            let _ = (&root, &team, &member, op);
            return Err("terminal write deferred: managed stop requires the native daemon".into());
        }
        #[cfg(not(target_os = "windows"))]
        return terminal_write(&root, &team, &member, op, || {
            // Liveness can change health while we wait for exclusion. Resolve
            // ownership from the current binding under the terminal lock.
            let record = super::runtime::MemberRuntimeStore::load(&root, &team, &member)?;
            if record.pane_id.as_deref() != Some(pane) {
                return Err(CoordinationError::Conflict(
                    "terminal write deferred: pane binding changed".into(),
                ));
            }
            if record.health == crate::coordination::domain::HealthState::SessionDead {
                use crate::coordination::runtime::{pane_belongs_to_member, PaneOwnership};
                let owned = runtime.live_pane(pane)?.is_some_and(|live| {
                    live.is_dead || pane_belongs_to_member(&record, &live) == PaneOwnership::Owned
                });
                if !owned {
                    return Err(CoordinationError::Conflict(
                        "terminal write deferred: dead record no longer owns pane".into(),
                    ));
                }
            }
            write().map_err(CoordinationError::Backend)
        })
        .map_err(|e| e.to_string());
    }
    write()
}

/// Shared resolution fence: a positive binding wins over unrelated unreadable records.
#[cfg(test)]
pub(crate) fn resolve_terminal_member(
    registry: &super::team_roots::TeamRootRegistry,
    pane: &str,
) -> Result<Option<(PathBuf, String, String)>, String> {
    resolve_terminal_member_with_runtime(
        registry,
        pane,
        &crate::coordination::runtime::SystemCoordinationRuntime,
    )
}

fn resolve_terminal_member_with_runtime(
    registry: &super::team_roots::TeamRootRegistry,
    pane: &str,
    runtime: &dyn crate::coordination::runtime::CoordinationRuntime,
) -> Result<Option<(PathBuf, String, String)>, String> {
    let deferred = |e| format!("terminal write deferred: attachment lookup: {e}");
    let mut uncertain = false;
    for (root, team) in registry.team_locations().map_err(deferred)? {
        let records = match super::runtime::MemberRuntimeStore::load_all(&root, &team) {
            Ok(records) => records,
            Err(_) => {
                super::runtime::log_runtime_record_skipped(
                    &team,
                    "<inventory>",
                    "runtime_unreadable",
                );
                Vec::new()
            }
        };
        for (member, record) in &records {
            if record.pane_id.as_deref() == Some(pane) {
                if record.health == crate::coordination::domain::HealthState::SessionDead {
                    use crate::coordination::runtime::{pane_belongs_to_member, PaneOwnership};
                    if !runtime
                        .live_pane(pane)
                        .map_err(deferred)?
                        .is_some_and(|live| {
                            live.is_dead
                                || pane_belongs_to_member(record, &live) == PaneOwnership::Owned
                        })
                    {
                        // A proven-foreign dead binding is not this pane's owner:
                        // keep scanning for the record that is. It does not make
                        // the inventory uncertain — with no owner at all the pane
                        // is unmanaged and takes the unlocked write, as before.
                        continue;
                    }
                }
                return Ok(Some((root, team, member.clone())));
            }
        }
        // Config's derived pane binding survives a missing/stale runtime record.
        // Read the wire field: TeamConfig deliberately drops tmuxPaneId.
        let config_path = root.join(&team).join("config.json");
        let config = read_to_string_with_retry(&config_path)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    read_to_string_with_retry(&displaced_path(&config_path))
                } else {
                    Err(e)
                }
            })
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        if let Some(members) = config.as_ref().and_then(|c| c["members"].as_array()) {
            uncertain |= members
                .iter()
                .any(|m| m["tmuxPaneId"].as_str() == Some(pane));
        } else if let Ok(names) = super::runtime::MemberRuntimeStore::list(&root, &team) {
            super::runtime::log_runtime_record_skipped(&team, "<inventory>", "config_unreadable");
            // Without config, an unreadable named member can still own this pane.
            // A half-deleted directory with no member evidence cannot block all stops.
            uncertain |= names
                .iter()
                .any(|name| !records.iter().any(|(n, _)| n == name));
        }
    }
    if uncertain {
        return Err("terminal write deferred: attachment inventory incomplete".into());
    }
    Ok(None)
}

fn terminal_lock_contended(error: &std::io::Error, windows: bool) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock || (windows && error.raw_os_error() == Some(33))
    // ERROR_LOCK_VIOLATION
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

pub(crate) fn read_to_string_with_retry(path: &Path) -> std::io::Result<String> {
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
    if taurhaus_lib::platform::terminal_io::active() {
        return Err(CoordinationError::Conflict("terminal lock is held".into()));
    }
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
        if taurhaus_lib::platform::terminal_io::active() {
            return Err(CoordinationError::Conflict("terminal lock is held".into()));
        }
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
    #[test]
    fn hosted_fixture_guard_releases_inherited_lock_descriptor() {
        // Regression: cadd533e released flock only on final close. Parallel
        // fixture forks can retain the open file description until exec.
        let tmp = tempfile::tempdir().unwrap();
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let _inherited = guard._file.try_clone().unwrap();
        drop(guard);
        assert!(HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn host_operation_lock_excludes_mesh_and_never_replaces_inode() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("team/state/app-server/seat.lock");
        let guard = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        let inode = std::fs::metadata(&path).unwrap().ino();
        let mesh = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(
            fs2::FileExt::try_lock_exclusive(&mesh).is_err(),
            "mesh must defer"
        );
        assert!(
            TerminalLock::acquire(tmp.path(), "team", "seat", "test", 0, Duration::ZERO).is_err()
        );
        drop(guard);
        fs2::FileExt::try_lock_exclusive(&mesh).unwrap();
        let started = std::time::Instant::now();
        assert!(
            HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::from_millis(20))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        fs2::FileExt::unlock(&mesh).unwrap();
        let _guard =
            HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert_eq!(std::fs::metadata(path).unwrap().ino(), inode);
    }

    #[test]
    fn host_operation_lock_is_acquired_after_data_locks_are_released() {
        let tmp = tempfile::TempDir::new().unwrap();
        let team = acquire_team_lock(tmp.path(), "team").unwrap();
        assert!(HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).is_err());
        drop(team);
        let _host = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert!(HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).is_err());
    }
    use std::sync::{mpsc, Arc, Barrier};
    use std::thread;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn recycled_pane_stop_skips_stale_dead_record_and_locks_live_owner() {
        // Regression: 0d5c16bf let the first stale dead binding shadow a recycled
        // pane's live owner, so the in-lock ownership check refused its stop.
        use super::super::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
        use crate::coordination::{domain::HealthState, runtime::RecordingCoordinationRuntime};
        let tmp = TempDir::new().unwrap();
        TeamConfigStore::save(tmp.path(), "team", &serde_json::from_value(
            serde_json::json!({"schema_version": 3, "name": "team", "created_at": chrono::Utc::now(), "members": []})
        ).unwrap()).unwrap();
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%1", true);
        runtime.set_pane_identity("%1", Some(901), Some(42));
        for (pid, start) in [(902, 42), (901, 43)] {
            for (member, health, pid, start) in [
                ("a-stale", HealthState::SessionDead, pid, start),
                ("z-owner", HealthState::Healthy, 901, 42),
            ] {
                MemberRuntimeStore::save(
                    tmp.path(),
                    "team",
                    member,
                    &MemberRuntimeRecord {
                        health,
                        pane_id: Some("%1".into()),
                        pane_pid: Some(pid),
                        pane_start_time: Some(start),
                        terminal_contract: 1,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            terminal_write_for_pane_with_runtime(tmp.path(), "%1", "stop", &runtime, || {
                let owner_lock =
                    File::open(tmp.path().join("team/state/terminal/z-owner.lock")).unwrap();
                assert!(
                    owner_lock.try_lock_exclusive().is_err(),
                    "stop must hold the live owner's lock"
                );
                assert!(!tmp.path().join("team/state/terminal/a-stale.lock").exists());
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn resume_dead_pane_cleanup_is_admitted_before_recreate() {
        // Regression: 0d5c16bf refused SessionDead terminal writes for remain-on-exit
        // panes, so resume created a replacement while leaking the old dead pane.
        use super::super::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
        use crate::coordination::domain::{HealthState, Member};
        use crate::coordination::runtime::{
            resolve_or_create_pane_for_member, CoordinationRuntime, RecordingCoordinationRuntime,
            RuntimeCall,
        };
        use crate::session_scanner::cli_tool::CliTool;
        struct CleanupRuntime<'a> {
            root: &'a Path,
            inner: RecordingCoordinationRuntime,
        }
        impl CoordinationRuntime for CleanupRuntime<'_> {
            fn create_aitx_pane(
                &self,
                project_id: &str,
                tmux_layout: &str,
            ) -> Result<String, CoordinationError> {
                self.inner.create_aitx_pane(project_id, tmux_layout)
            }
            fn send_tmux_keys_with_enter(
                &self,
                pane_id: &str,
                keys: &str,
            ) -> Result<(), CoordinationError> {
                self.inner.send_tmux_keys_with_enter(pane_id, keys)
            }
            fn detect_session_id(
                &self,
                pane_id: &str,
                cli_tool: CliTool,
            ) -> Result<Option<String>, CoordinationError> {
                self.inner.detect_session_id(pane_id, cli_tool)
            }
            fn join_mesh(
                &self,
                team_name: &str,
                member_name: &str,
                project_id: &str,
                member_type: &str,
                model: &str,
                claude_dir: &str,
            ) -> Result<(), CoordinationError> {
                self.inner.join_mesh(
                    team_name,
                    member_name,
                    project_id,
                    member_type,
                    model,
                    claude_dir,
                )
            }
            fn spawn_mesh_daemon(
                &self,
                pane_id: &str,
                team_name: &str,
                member_name: &str,
            ) -> Result<u32, CoordinationError> {
                self.inner
                    .spawn_mesh_daemon(pane_id, team_name, member_name)
            }
            fn pane_belongs_to_project(
                &self,
                pane_id: &str,
                project_id: &str,
            ) -> Result<bool, CoordinationError> {
                self.inner.pane_belongs_to_project(pane_id, project_id)
            }
            fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
                self.inner.pane_exists(pane_id)
            }
            fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
                self.inner.pane_is_dead(pane_id)
            }
            fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
                self.inner.pane_is_shell(pane_id)
            }
            fn pane_current_command(
                &self,
                pane_id: &str,
            ) -> Result<Option<String>, CoordinationError> {
                self.inner.pane_current_command(pane_id)
            }
            fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
                terminal_write_for_pane_with_runtime(
                    self.root,
                    pane_id,
                    "teardown",
                    &self.inner,
                    || {
                        assert!(taurhaus_lib::platform::terminal_io::active());
                        self.inner
                            .kill_aitx_pane(pane_id)
                            .map_err(|e| e.to_string())
                    },
                )
                .map_err(CoordinationError::Backend)
            }
            fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
                self.inner.terminate_process_by_pid(pid)
            }
            fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
                self.inner.is_process_running_by_pid(pid)
            }
        }
        let tmp = TempDir::new().unwrap();
        TeamConfigStore::save(tmp.path(), "team", &serde_json::from_value(
            serde_json::json!({"schema_version": 3, "name": "team", "created_at": chrono::Utc::now(), "members": []})
        ).unwrap()).unwrap();
        let runtime = CleanupRuntime {
            root: tmp.path(),
            inner: RecordingCoordinationRuntime::default(),
        };
        runtime.inner.set_pane_exists("%1", true);
        runtime.inner.set_pane_dead("%1", true);
        runtime.inner.set_pane_ownership("%1", true);
        // A dead pane may no longer expose a process identity.
        let record = MemberRuntimeRecord {
            health: HealthState::SessionDead,
            pane_id: Some("%1".into()),
            pane_pid: Some(901),
            pane_start_time: Some(42),
            terminal_contract: 1,
            ..Default::default()
        };
        MemberRuntimeStore::save(tmp.path(), "team", "seat", &record).unwrap();
        let member: Member = serde_json::from_value(serde_json::json!({
            "name": "seat", "role": "agent", "cli_tool": "codex", "project_path": tmp.path()
        }))
        .unwrap();
        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .unwrap();
        assert!(resolution.created_new_pane);
        assert!(
            !runtime.inner.pane_exists("%1").unwrap(),
            "resume leaked the dead pane"
        );
        let changes: Vec<_> = runtime
            .inner
            .calls()
            .into_iter()
            .filter(|call| {
                matches!(
                    call,
                    RuntimeCall::KillPane { .. } | RuntimeCall::CreatePane { .. }
                )
            })
            .collect();
        assert!(
            matches!(&changes[..], [RuntimeCall::KillPane { pane_id }, RuntimeCall::CreatePane { .. }] if pane_id == "%1")
        );
    }

    #[test]
    fn pane_addressed_stop_locks_dead_but_live_owned_pane() {
        // Regression: 8f99dd9a excluded dead health before locking, although
        // offline liveness can mark a still-live owned pane dead.
        use super::super::{MemberRuntimeRecord, MemberRuntimeStore};
        use crate::coordination::runtime::RecordingCoordinationRuntime;
        let tmp = TempDir::new().unwrap();
        super::super::TeamConfigStore::save(tmp.path(), "team", &serde_json::from_value(
            serde_json::json!({"schema_version": 3, "name": "team", "created_at": chrono::Utc::now(), "members": []})
        ).unwrap()).unwrap();
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%1", true);
        runtime.set_pane_identity("%1", Some(901), Some(42));
        MemberRuntimeStore::save(
            tmp.path(),
            "team",
            "seat",
            &MemberRuntimeRecord {
                health: crate::coordination::domain::HealthState::SessionDead,
                pane_id: Some("%1".into()),
                pane_pid: Some(901),
                pane_start_time: Some(42),
                terminal_contract: 1,
                ..Default::default()
            },
        )
        .unwrap();
        terminal_write_for_pane_with_runtime(tmp.path(), "%1", "stop", &runtime, || {
            assert!(
                taurhaus_lib::platform::terminal_io::active(),
                "owned stop must hold terminal lock"
            );
            Ok(())
        })
        .unwrap();
        // Round-6 review: a pane whose only claimant is a dead record proven
        // not to own it is unmanaged — the write proceeds without a member lock
        // instead of being refused as an incomplete inventory.
        for (pid, start) in [(902, 42), (901, 43)] {
            runtime.set_pane_identity("%1", Some(pid), Some(start));
            let mut wrote = false;
            terminal_write_for_pane_with_runtime(tmp.path(), "%1", "stop", &runtime, || {
                wrote = true;
                assert!(
                    !taurhaus_lib::platform::terminal_io::active(),
                    "an unmanaged pane takes no member terminal lock"
                );
                Ok(())
            })
            .unwrap();
            assert!(wrote, "unmanaged pane must be written");
        }
    }

    #[test]
    fn terminal_lookup_skips_unrelated_corruption_but_defers_unknown_attachment() {
        // Regression: 1127823e aborted every stop on one corrupt record, then
        // wrote unlocked when a managed record was transiently absent.
        use super::super::runtime::{MemberRuntimeRecord, MemberRuntimeStore};
        let tmp = TempDir::new().unwrap();
        MemberRuntimeStore::save(
            tmp.path(),
            "team",
            "seat",
            &MemberRuntimeRecord {
                health: crate::coordination::domain::HealthState::Healthy,
                pane_id: Some("%1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        fs::write(tmp.path().join("team/runtime/broken.json"), "{").unwrap();
        terminal_write_for_pane_at_root(tmp.path(), "%1", "stop", || {
            assert!(taurhaus_lib::platform::terminal_io::active());
            Ok(())
        })
        .unwrap();
        fs::remove_file(tmp.path().join("team/runtime/seat.json")).unwrap();
        let result = terminal_write_for_pane_at_root(tmp.path(), "%1", "stop", || {
            panic!("unresolved pane must not be written")
        });
        let result: Result<(), String> = result;
        assert!(result.unwrap_err().contains("terminal write deferred"));
    }

    #[test]
    fn pane_stop_reports_unrelated_unreadable_teams_once_and_defers_own_runtime() {
        // Regression: 1127823e made corrupt inventories global; 8ac90621 only added logging.
        use super::super::runtime::{MemberRuntimeRecord, MemberRuntimeStore};
        let _guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("events.jsonl");
        let sink = taurhaus_lib::logging::LogFileState::new(log_path.clone()).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        for broken in ["a-config", "b-runtime"] {
            fs::create_dir_all(tmp.path().join(broken)).unwrap();
            fs::write(tmp.path().join(broken).join("config.json"), "{").unwrap();
        }
        fs::write(tmp.path().join("b-runtime/runtime"), "not a directory").unwrap();
        MemberRuntimeStore::save(
            tmp.path(),
            "target",
            "seat",
            &MemberRuntimeRecord {
                health: crate::coordination::domain::HealthState::Healthy,
                pane_id: Some("%1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        fs::write(
            tmp.path().join("target/config.json"),
            serde_json::json!({
                "members": [{"name": "seat", "tmuxPaneId": "%1"}]
            })
            .to_string(),
        )
        .unwrap();
        for _ in 0..2 {
            terminal_write_for_pane_at_root(tmp.path(), "%99", "stop", || {
                assert!(!taurhaus_lib::platform::terminal_io::active());
                Ok(())
            })
            .unwrap();
            terminal_write_for_pane_at_root(tmp.path(), "%1", "stop", || {
                assert!(taurhaus_lib::platform::terminal_io::active());
                Ok(())
            })
            .unwrap();
        }
        sink.flush_for_test().unwrap();
        let events: Vec<serde_json::Value> = fs::read_to_string(log_path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        for (team, reason) in [
            ("a-config", "config_unreadable"),
            ("b-runtime", "runtime_unreadable"),
        ] {
            assert_eq!(
                events
                    .iter()
                    .filter(|e| e["event"] == "coordination.runtime.record_skipped"
                        && e["level"] == "WARN"
                        && e["team"] == team
                        && e["reason"] == reason)
                    .count(),
                1
            );
        }
        fs::write(tmp.path().join("target/runtime/seat.json"), "{").unwrap();
        let result: Result<(), String> =
            terminal_write_for_pane_at_root(tmp.path(), "%1", "stop", || {
                panic!("own unreadable runtime must defer")
            });
        assert!(result
            .unwrap_err()
            .contains("attachment inventory incomplete"));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_child_receives_the_locked_file_as_stdin() {
        // Regression: c7226a4d selected supervision by the library cfg(test),
        // so the integration harness re-executed itself instead of the transport.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("team/state/terminal/seat.lock");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "inherited-inode\n").unwrap();
        let _guard =
            TerminalLock::acquire(tmp.path(), "team", "seat", "test", 1, Duration::ZERO).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let supervisor = tmp.path().join("supervisor");
        fs::write(
            &supervisor,
            "#!/bin/sh\n[ \"$1\" = --terminal-child ] || exit 2\nshift 2\nexec \"$@\"\n",
        )
        .unwrap();
        fs::set_permissions(&supervisor, fs::Permissions::from_mode(0o700)).unwrap();
        let output =
            taurhaus_lib::platform::terminal_io::with_child_executable(&supervisor, || {
                taurhaus_lib::platform::terminal_io::output(
                    std::process::Command::new("/bin/sh")
                        .args(["-c", "read value; printf %s \"$value\""]),
                )
                .unwrap()
            });
        assert!(output.status.success());
        assert_eq!(output.stdout, b"inherited-inode");
    }

    #[test]
    fn terminal_lock_bounds_wait_and_preserves_inode_and_diagnostics() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("team/state/terminal/seat.lock");
        let holder = tmp.path().join("team/state/terminal/seat.holder.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let fake = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .unwrap();
        fake.lock_exclusive().unwrap();
        let started = std::time::Instant::now();
        assert!(TerminalLock::acquire(
            tmp.path(),
            "team",
            "seat",
            "launch",
            1,
            Duration::from_millis(20)
        )
        .is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!holder.exists());
        fake.unlock().unwrap();
        fs::write(&holder, "crashed holder").unwrap();
        {
            let _guard =
                TerminalLock::acquire(tmp.path(), "team", "seat", "launch", 1, Duration::ZERO)
                    .unwrap();
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&holder).unwrap()).unwrap();
            assert_eq!(value["owner"], "taurhaus");
            assert_eq!(value["op"], "launch");
            assert_eq!(value["epoch"], 1);
            assert!(fake.try_lock_exclusive().is_err());
            assert!(
                acquire_team_lock(tmp.path(), "team").is_err(),
                "terminal lock must be last and alone"
            );
        }
        assert!(!holder.exists());
        assert!(path.exists());
        assert!(
            fake.try_lock_exclusive().is_ok(),
            "same inode remains lockable"
        );
    }

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
