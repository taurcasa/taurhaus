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

/// A daemon stand-in that answers `ping` with a chosen protocol version.
///
/// `daemon::server::run_for_test` always reports the current
/// `PROTOCOL_VERSION`, so it cannot play the older daemon the app has to
/// refuse. This stub answers `ping` with `protocol_version` and every other
/// method with `default_result`.
#[cfg(test)]
pub(crate) struct StubDaemon {
    addr: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(test)]
impl StubDaemon {
    /// Start a stub on an ephemeral loopback port.
    pub(crate) fn start(protocol_version: u32, default_result: serde_json::Value) -> Self {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
        let addr = listener.local_addr().expect("stub daemon addr").to_string();
        listener
            .set_nonblocking(true)
            .expect("stub daemon nonblocking accept");
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();

        let handle = std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let stop = stop.clone();
                        let default_result = default_result.clone();
                        std::thread::spawn(move || {
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                            let Ok(mut writer) = stream.try_clone() else {
                                return;
                            };
                            let mut reader = BufReader::new(stream);
                            let mut line = String::new();
                            while !stop.load(Ordering::Relaxed) {
                                line.clear();
                                match reader.read_line(&mut line) {
                                    Ok(0) => return,
                                    Ok(_) => {}
                                    Err(error)
                                        if matches!(
                                            error.kind(),
                                            std::io::ErrorKind::WouldBlock
                                                | std::io::ErrorKind::TimedOut
                                        ) =>
                                    {
                                        continue
                                    }
                                    Err(_) => return,
                                }
                                let Ok(request) =
                                    serde_json::from_str::<serde_json::Value>(line.trim())
                                else {
                                    continue;
                                };
                                let result =
                                    if request["method"] == crate::daemon::protocol::method::PING {
                                        serde_json::json!({
                                            "version": env!("CARGO_PKG_VERSION"),
                                            "protocol_version": protocol_version,
                                            "uptime_secs": 0,
                                            "data_root": "",
                                        })
                                    } else {
                                        default_result.clone()
                                    };
                                let response = serde_json::json!({
                                    "id": request["id"].as_str().unwrap_or("stub"),
                                    "result": result,
                                });
                                if writer.write_all(response.to_string().as_bytes()).is_err()
                                    || writer.write_all(b"\n").is_err()
                                    || writer.flush().is_err()
                                {
                                    return;
                                }
                            }
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            }
        });

        Self {
            addr,
            shutdown,
            handle: Some(handle),
        }
    }

    pub(crate) fn addr(&self) -> &str {
        &self.addr
    }
}

#[cfg(test)]
impl Drop for StubDaemon {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
