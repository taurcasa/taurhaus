use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::sync::{LazyLock, Mutex, MutexGuard};

static HEAVY_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static GLOBAL_LOG_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static COMPACTION_EXTRACTOR_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Guard that serializes heavy integration-style tests (daemon sockets,
/// filesystem watchers) both within a process and across test binaries.
pub struct HeavyTestGuard {
    _in_process: MutexGuard<'static, ()>,
    lock_file: File,
}

/// Guard that serializes tests mutating the process-global structured log sink.
pub struct GlobalLogTestGuard {
    _in_process: MutexGuard<'static, ()>,
}

/// Guard that serializes tests mutating the process-global compaction extractor.
pub struct CompactionExtractorTestGuard {
    _in_process: MutexGuard<'static, ()>,
}

impl Drop for HeavyTestGuard {
    fn drop(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

/// Acquire the shared heavy-test lock.
///
/// Uses an in-process mutex plus an OS file lock in tempdir so only one heavy
/// test runs at a time even when multiple Rust test binaries are active.
pub fn acquire_heavy_test_guard() -> HeavyTestGuard {
    let in_process = HEAVY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let lock_path = std::env::temp_dir().join("taurhaus-heavy-tests.lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap_or_else(|e| {
            panic!(
                "failed to open heavy test lock file at {:?}: {e}",
                lock_path
            )
        });
    lock_file.lock_exclusive().unwrap_or_else(|e| {
        panic!(
            "failed to lock heavy test lock file at {:?}: {e}",
            lock_path
        )
    });

    HeavyTestGuard {
        _in_process: in_process,
        lock_file,
    }
}

/// Acquire the shared guard for tests that install a process-global log sink.
pub fn acquire_global_log_test_guard() -> GlobalLogTestGuard {
    let in_process = GLOBAL_LOG_TEST_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    GlobalLogTestGuard {
        _in_process: in_process,
    }
}

/// Acquire the shared guard for tests that start or stop the compaction extractor service.
pub fn acquire_compaction_extractor_test_guard() -> CompactionExtractorTestGuard {
    let in_process = COMPACTION_EXTRACTOR_TEST_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    CompactionExtractorTestGuard {
        _in_process: in_process,
    }
}
