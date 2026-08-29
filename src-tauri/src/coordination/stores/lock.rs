//! Advisory file locks for store concurrency safety.

use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::Duration;

use fs2::FileExt;

use crate::coordination::errors::CoordinationError;

const LOCK_FILENAME: &str = ".lock";
const INODE_RETRY_LIMIT: usize = 50;
const READ_RETRY_BACKOFFS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(500),
];

fn is_windows_unsupported_lock_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
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
/// The lock is held for the lifetime of the returned `File`.
/// On drop, the lock is automatically released.
pub fn acquire_team_lock(teams_dir: &Path, team_name: &str) -> Result<File, CoordinationError> {
    let team_dir = teams_dir.join(team_name);
    fs::create_dir_all(&team_dir)?;

    let lock_path = team_dir.join(LOCK_FILENAME);
    let file = File::create(&lock_path).map_err(CoordinationError::Io)?;
    match file.lock_exclusive() {
        Ok(()) => {}
        Err(err) if is_windows_unsupported_lock_error(&err) => {
            tracing::warn!(
                team_name = team_name,
                lock_path = %lock_path.display(),
                "advisory file locks are unsupported for this Windows path; continuing without lock"
            );
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    }
    Ok(file)
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
        file.read_to_string(&mut contents)?;
        Ok(contents)
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
    use std::sync::{Arc, Barrier};
    use std::thread;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn unsupported_lock_error_detection_is_platform_aware() {
        let err = std::io::Error::from_raw_os_error(1);
        assert_eq!(
            is_windows_unsupported_lock_error(&err),
            cfg!(target_os = "windows")
        );
    }

    #[test]
    fn non_unsupported_lock_error_is_rejected() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(!is_windows_unsupported_lock_error(&err));
    }

    #[test]
    fn windows_lock_violation_is_a_transient_file_lock() {
        // Regression: 694b130 introduced target-file locks but omitted Windows
        // ERROR_LOCK_VIOLATION (33) from the unlocked-reader retry policy.
        assert!(is_transient_file_lock_error(
            &std::io::Error::from_raw_os_error(33)
        ));
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
