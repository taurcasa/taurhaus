use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::daemon::protocol::{self, DaemonEvent, DaemonRequest, DaemonResponse};
use crate::project_provider::ProjectProvider;

/// Default port for the daemon.
pub const DEFAULT_PORT: u16 = 17233;

/// Maximum allowed length for a single request line (1 MB).
///
/// Normal requests are typically < 10 KB. This limit prevents unbounded
/// memory allocation from malicious or misbehaving clients.
const MAX_REQUEST_LINE_LEN: usize = 1_048_576;
static DROPPED_PUSH_EVENT_COUNT: AtomicU64 = AtomicU64::new(0);
static ACTIVE_CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);
static WATCH_TELEMETRY_DIRTY: AtomicBool = AtomicBool::new(true);
const TELEMETRY_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

/// Configuration for the daemon server.
pub struct DaemonConfig {
    pub port: u16,
    pub bind_addr: String,
    /// Auto-shutdown after this many seconds with no client activity.
    /// `None` disables idle timeout.
    pub idle_timeout_secs: Option<u64>,
    /// Auth token. When `Some`, every request must include a matching `auth` field.
    /// When `None`, authentication is disabled (for tests/backward compat).
    pub auth_token: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: None,
        }
    }
}

/// Current epoch seconds (for idle timeout tracking).
fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn configure_accepted_stream(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_nodelay(true)
}

pub(crate) fn active_connection_count() -> u64 {
    ACTIVE_CONNECTION_COUNT.load(Ordering::Relaxed) as u64
}

pub(crate) fn mark_daemon_watch_telemetry_dirty() {
    WATCH_TELEMETRY_DIRTY.store(true, Ordering::Relaxed);
}

fn emit_daemon_watch_telemetry(
    reason: &str,
    watch_registry: &crate::daemon::watch::SharedDaemonWatchRegistry,
) {
    crate::inotify_diagnostics::emit_daemon_telemetry_with_counts(
        reason,
        Some(active_connection_count()),
        Some(watch_registry.physical_watch_registration_count() as u64),
        Some(watch_registry.logical_subscription_count() as u64),
    );
}

/// Run the daemon server. Blocks until `shutdown` is set to true or idle timeout elapses.
pub fn run(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
) -> std::io::Result<()> {
    // The daemon owns the Claude status-line bridge on every platform: on
    // Windows the config dirs live in WSL, where only the daemon can reach
    // them, and on native hosts a single owner is what keeps the app and the
    // daemon from rewriting each other's script with their own executable.
    // Passed as a closure rather than called here so `run_for_test` — which
    // must never write into real config dirs — shares the same startup.
    run_with_installer(
        config,
        shutdown,
        provider,
        None,
        install_claude_usage_statusline,
    )
}

fn install_claude_usage_statusline() {
    let Ok(exe) = std::env::current_exe() else {
        tracing::debug!("Claude usage status line skipped: the daemon has no resolvable path");
        return;
    };
    crate::session_scanner::claude_statusline::install_statusline_for_detected_accounts(&exe);
}

/// Start the daemon, with the status-line install running beside the listener.
///
/// Never in front of it: the install calls `CliVersions::current`, which probes
/// `codex --version` and `claude --version` with a five second timeout each,
/// and `daemon::launcher` gives the whole daemon five seconds to answer a TCP
/// connect. One hung probe would cost a healthy daemon its startup.
fn run_with_installer<F>(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    compaction_teams_dir: Option<std::path::PathBuf>,
    installer: F,
) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    if let Err(error) = std::thread::Builder::new()
        .name("claude-usage-statusline".to_string())
        .spawn(installer)
    {
        tracing::warn!(error = %error, "Claude usage status line install not spawned");
    }
    run_with_compaction_teams_dir(config, shutdown, provider, compaction_teams_dir)
}

fn run_with_compaction_teams_dir(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    compaction_teams_dir: Option<std::path::PathBuf>,
) -> std::io::Result<()> {
    crate::daemon::compaction::reset_requested_mode(crate::models::CodexCompactionMode::Transcript);
    let session_hub = crate::daemon::session_activity::SessionActivityHub::global();
    let _ = session_hub.wait_for_update(0, 0, Duration::from_millis(750));

    // On macOS, use SO_REUSEADDR so we can rebind immediately after the previous
    // daemon dies. Linux does not need this for our listener pattern, and enabling
    // it there can permit duplicate listeners on the same port.
    let listener = {
        let addr: std::net::SocketAddr = format!("{}:{}", config.bind_addr, config.port)
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;
        #[cfg(target_os = "macos")]
        socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        let listener: TcpListener = socket.into();
        listener
    };
    listener.set_nonblocking(true)?;

    let start_time = Instant::now();
    let last_activity = Arc::new(AtomicU64::new(epoch_secs()));
    let auth_token: Option<Arc<str>> = config.auth_token.as_deref().map(Arc::from);
    let watch_registry =
        crate::daemon::watch::SharedDaemonWatchRegistry::new().map_err(std::io::Error::other)?;

    let compaction_shutdown = shutdown.clone();
    let compaction_handle = std::thread::spawn(move || {
        crate::daemon::compaction::run_mode_controller(compaction_teams_dir, compaction_shutdown);
    });

    tracing::info!(port = config.port, "daemon listening");
    emit_daemon_watch_telemetry("startup", &watch_registry);

    let telemetry_shutdown = shutdown.clone();
    let telemetry_registry = watch_registry.clone();
    let telemetry_handle = std::thread::spawn(move || {
        let mut last_heartbeat = Instant::now();
        while !telemetry_shutdown.load(Ordering::Relaxed) {
            let dirty = WATCH_TELEMETRY_DIRTY.swap(false, Ordering::Relaxed);
            let heartbeat_due = last_heartbeat.elapsed() >= TELEMETRY_HEARTBEAT_INTERVAL;

            if dirty {
                emit_daemon_watch_telemetry("state_changed", &telemetry_registry);
                last_heartbeat = Instant::now();
            } else if heartbeat_due {
                emit_daemon_watch_telemetry("periodic", &telemetry_registry);
                last_heartbeat = Instant::now();
            }

            std::thread::sleep(Duration::from_secs(1));
        }
    });

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                if let Err(e) = configure_accepted_stream(&stream) {
                    tracing::warn!(%addr, error = %e, "failed to configure accepted stream");
                    continue;
                }
                tracing::info!(%addr, "client connected");
                last_activity.store(epoch_secs(), Ordering::Relaxed);
                let shutdown_clone = shutdown.clone();
                let start = start_time;
                let activity = last_activity.clone();
                let token = auth_token.clone();
                let provider = provider.clone();
                let watch_registry = watch_registry.clone();
                ACTIVE_CONNECTION_COUNT.fetch_add(1, Ordering::Relaxed);
                mark_daemon_watch_telemetry_dirty();
                std::thread::spawn(move || {
                    struct ActiveConnectionGuard;

                    impl Drop for ActiveConnectionGuard {
                        fn drop(&mut self) {
                            ACTIVE_CONNECTION_COUNT.fetch_sub(1, Ordering::Relaxed);
                            mark_daemon_watch_telemetry_dirty();
                        }
                    }

                    let _active_connection_guard = ActiveConnectionGuard;
                    if let Err(e) = handle_connection(
                        stream,
                        start,
                        &shutdown_clone,
                        &activity,
                        token.as_deref(),
                        provider,
                        watch_registry,
                    ) {
                        tracing::warn!(error = %e, "connection handler error");
                    }
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Check idle timeout
                if let Some(timeout) = config.idle_timeout_secs {
                    let last = last_activity.load(Ordering::Relaxed);
                    if epoch_secs().saturating_sub(last) > timeout {
                        tracing::info!(timeout, "Idle timeout reached, shutting down");
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                tracing::error!(error = %e, "accept error");
            }
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = compaction_handle.join();
    let _ = telemetry_handle.join();
    tracing::info!("daemon shutting down");
    Ok(())
}

#[cfg(test)]
pub(crate) fn run_for_test(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
) -> std::io::Result<()> {
    run_for_test_with_installer(config, shutdown, provider, || {})
}

/// `run_for_test`, with the status-line install a test wants to time.
#[cfg(test)]
pub(crate) fn run_for_test_with_installer<F>(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    installer: F,
) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    // Regression: 9f723d3 removed the off-WSL compaction gate, so in-process
    // daemon tests began rewriting the developer's real Claude teams state.
    let claude_root = tempfile::tempdir()?;
    run_with_installer(
        config,
        shutdown,
        provider,
        Some(claude_root.path().join("teams")),
        installer,
    )
}

/// Read a newline-terminated line from a `BufReader`, respecting a max byte limit.
///
/// Returns:
/// - `Ok(Some(line))` — successfully read a line (newline stripped)
/// - `Ok(None)` — EOF (client disconnected)
/// - `Err(InvalidData)` — line exceeded `max_len` bytes
/// - `Err(other)` — propagated I/O error (timeout, etc.)
fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    max_len: usize,
) -> std::io::Result<Option<String>> {
    use std::io::BufRead as _;

    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            // EOF
            return if line.is_empty() {
                Ok(None)
            } else {
                // Partial line at EOF — try to return it
                break;
            };
        }

        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            let chunk = &available[..=pos];
            if line.len() + chunk.len() > max_len {
                reader.consume(pos + 1); // drain through the newline to resync
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "request line too large",
                ));
            }
            line.extend_from_slice(chunk);
            reader.consume(pos + 1);
            break;
        } else {
            let len = available.len();
            if line.len() + len > max_len {
                reader.consume(len);
                drain_until_newline(reader);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "request line too large",
                ));
            }
            line.extend_from_slice(available);
            reader.consume(len);
        }
    }

    // Strip trailing \r\n
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }

    String::from_utf8(line).map(Some).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "request line is not valid UTF-8",
        )
    })
}

/// Drain bytes until the next newline to resync the stream after an oversized line.
fn drain_until_newline(reader: &mut BufReader<TcpStream>) {
    use std::io::BufRead as _;

    loop {
        match reader.fill_buf() {
            Ok([]) => break,
            Ok(buf) => {
                if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    reader.consume(pos + 1);
                    break;
                }
                let len = buf.len();
                reader.consume(len);
            }
            Err(_) => break,
        }
    }
}

/// Handle a single client connection: read NDJSON requests, dispatch, respond.
///
/// Uses a shared writer (`Arc<Mutex<TcpStream>>`) so that watch event callbacks
/// can push events to the client on the same connection.
fn handle_connection(
    stream: TcpStream,
    start_time: Instant,
    shutdown: &AtomicBool,
    last_activity: &AtomicU64,
    auth_token: Option<&str>,
    provider: Arc<dyn ProjectProvider>,
    watch_registry: Arc<crate::daemon::watch::SharedDaemonWatchRegistry>,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let writer = Arc::new(Mutex::new(stream));
    let project_task_scan_cache = crate::daemon::handlers::ProjectTaskScanCacheState::default();
    let mut watch_runtime = crate::daemon::watch::WatchRuntime::new(watch_registry);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let line = match read_bounded_line(&mut reader, MAX_REQUEST_LINE_LEN) {
            Ok(Some(l)) => l,
            Ok(None) => break, // EOF — client disconnected
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::InvalidData => {
                // Line exceeded max length
                let resp = DaemonResponse::err(
                    "",
                    "REQUEST_TOO_LARGE",
                    format!("Request line exceeds {MAX_REQUEST_LINE_LEN} byte limit"),
                );
                write_locked(&writer, &resp)?;
                continue;
            }
            Err(e) => return Err(e),
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: DaemonRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = DaemonResponse::err("", "PARSE_ERROR", format!("Invalid JSON: {e}"));
                write_locked(&writer, &resp)?;
                continue;
            }
        };

        // Validate auth token if the server was started with one
        if let Some(expected) = auth_token {
            if let Err(msg) = crate::daemon::auth::validate_token(expected, request.auth.as_deref())
            {
                let resp = DaemonResponse::err(&request.id, "AUTH_FAILED", msg);
                write_locked(&writer, &resp)?;
                continue;
            }
        }

        let response = crate::daemon::handlers::dispatch(
            &request,
            provider.as_ref(),
            start_time,
            &writer,
            &mut watch_runtime,
            &project_task_scan_cache,
        );

        write_locked(&writer, &response)?;
        last_activity.store(epoch_secs(), Ordering::Relaxed);

        // Handle shutdown method
        if request.method == protocol::method::SHUTDOWN {
            shutdown.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Drop watches before writer — stops watcher callbacks before closing stream
    watch_runtime.clear();
    Ok(())
}

/// Write a NDJSON response through a shared (locked) writer.
pub(crate) fn write_locked(
    writer: &Arc<Mutex<TcpStream>>,
    response: &DaemonResponse,
) -> std::io::Result<()> {
    let mut w = writer
        .lock()
        .map_err(|_| std::io::Error::other("Writer lock poisoned"))?;
    write_response(&mut w, response)
}

/// Write a single NDJSON response line.
fn write_response(writer: &mut TcpStream, response: &DaemonResponse) -> std::io::Result<()> {
    let json = serde_json::to_string(response).map_err(std::io::Error::other)?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Push a DaemonEvent to a client through a shared writer.
///
pub(crate) fn push_event(writer: &Arc<Mutex<TcpStream>>, event: &DaemonEvent) {
    match serde_json::to_string(event) {
        Ok(json) => match writer.lock() {
            Ok(mut w) => {
                if let Err(error) = w.write_all(json.as_bytes()) {
                    log_dropped_push_event(event, "write", Some(&error.to_string()));
                    return;
                }
                if let Err(error) = w.write_all(b"\n") {
                    log_dropped_push_event(event, "newline", Some(&error.to_string()));
                    return;
                }
                if let Err(error) = w.flush() {
                    log_dropped_push_event(event, "flush", Some(&error.to_string()));
                }
            }
            Err(_) => {
                log_dropped_push_event(event, "writer_lock_poisoned", None);
            }
        },
        Err(error) => log_dropped_push_event(event, "serialize", Some(&error.to_string())),
    }
}

fn log_dropped_push_event(event: &DaemonEvent, stage: &str, error: Option<&str>) {
    let dropped_count = DROPPED_PUSH_EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    tracing::warn!(
        event = %event.event,
        stage,
        dropped_count,
        error = error.unwrap_or("n/a"),
        "dropping daemon push event"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::local::LocalProvider;
    use serde_json::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::sync::LazyLock;

    static LOG_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct TestServer {
        port: u16,
        shutdown: Arc<AtomicBool>,
        _heavy_guard: crate::test_support::HeavyTestGuard,
        _extractor_guard: crate::test_support::CompactionExtractorTestGuard,
        handle: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn wait_for_server_accepting(port: u16, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        false
    }

    fn start_server(config: DaemonConfig) -> TestServer {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        start_server_with_heavy_guard(config, heavy_guard)
    }

    fn start_server_with_heavy_guard(
        config: DaemonConfig,
        heavy_guard: crate::test_support::HeavyTestGuard,
    ) -> TestServer {
        let extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();
        let shutdown = Arc::new(AtomicBool::new(false));
        let port = config.port;
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run_for_test(&config, shutdown_clone, Arc::new(LocalProvider))
        });
        let server = TestServer {
            port,
            shutdown,
            _heavy_guard: heavy_guard,
            _extractor_guard: extractor_guard,
            handle: Some(handle),
        };

        // Poll until the server is accepting connections (up to 2s).
        // A fixed sleep was flaky under parallel test load.
        if wait_for_server_accepting(port, std::time::Duration::from_secs(2)) {
            return server;
        }
        panic!("test server on port {port} did not start within 2s");
    }

    fn start_test_server() -> TestServer {
        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        start_server(DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: None,
        })
    }

    fn start_test_server_with_heavy_guard(
        heavy_guard: crate::test_support::HeavyTestGuard,
    ) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        start_server_with_heavy_guard(
            DaemonConfig {
                port,
                bind_addr: "127.0.0.1".to_string(),
                idle_timeout_secs: None,
                auth_token: None,
            },
            heavy_guard,
        )
    }

    // Regression: 79be608 installed the Claude status-line bridge from `run`,
    // synchronously, before the listener existed. That install calls
    // `CliVersions::current`, which probes `codex --version` and
    // `claude --version` with a five second timeout each, while
    // `daemon::launcher` gives the whole daemon five seconds to become
    // reachable — so one hung CLI probe cost an otherwise healthy daemon its
    // startup and pushed the app onto the local fallback.
    #[test]
    fn a_slow_status_line_install_never_delays_the_listener() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let shutdown = Arc::new(AtomicBool::new(false));
        let installed = Arc::new(AtomicBool::new(false));
        let install_flag = installed.clone();
        let server_shutdown = shutdown.clone();
        let handle = std::thread::spawn(move || {
            let config = DaemonConfig {
                port,
                bind_addr: "127.0.0.1".to_string(),
                idle_timeout_secs: None,
                auth_token: None,
            };
            run_for_test_with_installer(
                &config,
                server_shutdown,
                Arc::new(LocalProvider),
                move || {
                    std::thread::sleep(Duration::from_secs(3));
                    install_flag.store(true, Ordering::Relaxed);
                },
            )
        });

        let reachable = wait_for_server_accepting(port, Duration::from_millis(1500));
        shutdown.store(true, Ordering::Relaxed);
        let _ = handle.join();

        assert!(
            reachable,
            "the daemon must bind before the status-line install, not after it"
        );
    }

    fn send_request(
        stream: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        req: &DaemonRequest,
    ) -> DaemonResponse {
        let json = serde_json::to_string(req).unwrap();
        stream.write_all(json.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    #[test]
    fn configure_accepted_stream_enables_tcp_nodelay() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || TcpStream::connect(addr).unwrap());
        let (stream, _) = listener.accept().unwrap();
        let _client = client.join().unwrap();

        configure_accepted_stream(&stream).unwrap();
        assert!(stream.nodelay().unwrap());
    }

    #[test]
    fn server_responds_to_ping() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::ping("r1");
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok());
        assert_eq!(resp.id, "r1");

        let result: protocol::PingResult = serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(result.version, env!("CARGO_PKG_VERSION"));

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_start_eagerly_emits_session_scan_cycles() {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _global_log_guard = crate::test_support::acquire_global_log_test_guard();
        let _log_guard = LOG_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let log_dir = tempfile::TempDir::new().expect("tempdir");
        let log_path = log_dir.path().join("taurhaus.log.jsonl");
        let log_state =
            crate::commands::logging::LogFileState::new(log_path.clone()).expect("log state");
        crate::commands::logging::install_global_sink(&log_state);

        let server = start_test_server_with_heavy_guard(heavy_guard);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut observed = false;

        while std::time::Instant::now() < deadline {
            if let Ok(contents) = std::fs::read_to_string(&log_path) {
                if contents.contains("\"event\":\"session_scanner.scan.completed\"") {
                    observed = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        server.shutdown.store(true, Ordering::Relaxed);
        assert!(
            observed,
            "daemon should emit session_scanner.scan.completed without waiting for a client request"
        );
    }

    #[test]
    fn server_emits_state_changed_inotify_telemetry_after_watch_registration() {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _global_log_guard = crate::test_support::acquire_global_log_test_guard();
        let _log_guard = LOG_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let log_dir = tempfile::TempDir::new().expect("tempdir");
        let log_path = log_dir.path().join("taurhaus.log.jsonl");
        let log_state =
            crate::commands::logging::LogFileState::new(log_path.clone()).expect("log state");
        crate::commands::logging::install_global_sink(&log_state);
        let (telemetry_tx, telemetry_rx) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(telemetry_tx);

        let project_dir = tempfile::TempDir::new().expect("project tempdir");
        let watched = project_dir.path().join("watched");
        std::fs::create_dir_all(&watched).expect("watched dir");

        let server = start_test_server_with_heavy_guard(heavy_guard);
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("set read timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

        let response = send_request(
            &mut stream,
            &mut reader,
            &DaemonRequest::new(
                "watch-1",
                protocol::method::WATCH,
                protocol::PathParams {
                    path: watched.to_string_lossy().to_string(),
                },
            ),
        );
        assert!(response.is_ok(), "watch response: {response:?}");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut telemetry = None;
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let Ok(record) =
                telemetry_rx.recv_timeout(remaining.min(std::time::Duration::from_millis(250)))
            else {
                continue;
            };
            let fields = record
                .get("fields")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if record.get("event") == Some(&Value::String("inotify.telemetry".to_string()))
                && fields.get("reason") == Some(&Value::String("state_changed".to_string()))
                && fields
                    .get("logical_watch_subscriptions")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    > 0
                && fields
                    .get("physical_watch_registrations")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    > 0
            {
                telemetry = Some((record, fields));
                break;
            }
        }

        server.shutdown.store(true, Ordering::Relaxed);
        crate::commands::logging::clear_test_tap();

        let (telemetry, fields) = telemetry.unwrap_or_else(|| {
            panic!(
                "timed out waiting for state_changed inotify telemetry in {}",
                log_path.display()
            )
        });
        assert_eq!(telemetry["component"], "daemon");
        assert_eq!(fields["reason"], "state_changed");
    }

    #[test]
    fn server_wait_session_updates_returns_typed_payload() {
        let server = start_test_server();
        let port = server.port;
        let expected_version = crate::daemon::session_activity::SessionActivityHub::global()
            .snapshot()
            .version;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new(
            "su1",
            protocol::method::WAIT_SESSION_UPDATES,
            protocol::WaitSessionUpdatesParams {
                since_version: u64::MAX,
                since_degraded_revision: u64::MAX,
                timeout_ms: 0,
            },
        );
        let resp = send_request(&mut stream, &mut reader, &req);
        assert!(resp.is_ok(), "response: {:?}", resp);

        let payload: protocol::WaitSessionUpdatesResult =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!payload.changed);
        assert!(
            payload.version >= expected_version,
            "session activity version should be monotonic"
        );

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_returns_error_for_unknown_method() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new("r1", "nonexistent_method", serde_json::Value::Null);
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "UNKNOWN_METHOD");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_git_status_on_test_repo() {
        let server = start_test_server();
        let port = server.port;

        // Create a test repo
        let dir = tempfile::TempDir::new().unwrap();
        let _repo = git2::Repository::init(dir.path()).unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new(
            "r2",
            protocol::method::GIT_STATUS,
            protocol::PathParams {
                path: dir.path().to_str().unwrap().to_string(),
            },
        );
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok(), "response: {:?}", resp);
        let status: crate::models::GitStatus =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!status.is_dirty);

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_file_tree() {
        let server = start_test_server();
        let port = server.port;

        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "world").unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new(
            "r3",
            protocol::method::FILE_TREE,
            protocol::PathParams {
                path: dir.path().to_str().unwrap().to_string(),
            },
        );
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok());
        let tree: Vec<crate::models::FileTreeNode> =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!tree.is_empty());

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_read_file() {
        let server = start_test_server();
        let port = server.port;

        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new(
            "r4",
            protocol::method::READ_FILE,
            protocol::ReadFileParams {
                path: dir.path().to_str().unwrap().to_string(),
                relative: "test.rs".to_string(),
            },
        );
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok());
        let content: crate::models::FileContent =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(content.content, "fn main() {}");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_multiple_requests_on_same_connection() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send two pings
        let r1 = send_request(&mut stream, &mut reader, &DaemonRequest::ping("p1"));
        assert!(r1.is_ok());
        assert_eq!(r1.id, "p1");

        let r2 = send_request(&mut stream, &mut reader, &DaemonRequest::ping("p2"));
        assert!(r2.is_ok());
        assert_eq!(r2.id, "p2");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_malformed_json() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send malformed JSON
        stream.write_all(b"not valid json\n").unwrap();
        stream.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: DaemonResponse = serde_json::from_str(&line).unwrap();
        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "PARSE_ERROR");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_shutdown_method() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new("s1", protocol::method::SHUTDOWN, serde_json::Value::Null);
        let resp = send_request(&mut stream, &mut reader, &req);
        assert!(resp.is_ok());

        // The shutdown flag should be set
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(server.shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn server_idle_timeout_shuts_down() {
        // Regression: a fixed 100ms startup sleep was not enough under load,
        // causing occasional ConnectionRefused before the listener was ready.
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: Some(1), // 1 second timeout
            auth_token: None,
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run_for_test(&config, shutdown_clone, Arc::new(LocalProvider))
        });

        assert!(
            wait_for_server_accepting(port, std::time::Duration::from_secs(2)),
            "idle-timeout test server on port {port} did not start within 2s"
        );

        // Verify server is running (can connect)
        let _stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        drop(_stream);

        // Wait for idle timeout to trigger
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // Server thread should have exited
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn server_rejects_oversized_request() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send a line that exceeds MAX_REQUEST_LINE_LEN (1 MB)
        // We only need slightly over the limit to trigger rejection
        let oversized = "x".repeat(MAX_REQUEST_LINE_LEN + 100);
        stream.write_all(oversized.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: DaemonResponse = serde_json::from_str(&line).unwrap();
        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "REQUEST_TOO_LARGE");

        // Connection should still work for normal requests afterward
        let r = send_request(&mut stream, &mut reader, &DaemonRequest::ping("p1"));
        assert!(r.is_ok());

        server.shutdown.store(true, Ordering::Relaxed);
    }

    // -----------------------------------------------------------------------
    // Auth token tests
    // -----------------------------------------------------------------------

    fn start_authed_server(token: &str) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        start_server(DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: Some(token.to_string()),
        })
    }

    #[test]
    fn server_rejects_missing_auth_token() {
        let server = start_authed_server("secret-token-123");
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send request without auth
        let req = DaemonRequest::ping("r1");
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "AUTH_FAILED");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_rejects_wrong_auth_token() {
        let server = start_authed_server("secret-token-123");
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let mut req = DaemonRequest::ping("r1");
        req.auth = Some("wrong-token".to_string());
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "AUTH_FAILED");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_accepts_correct_auth_token() {
        let server = start_authed_server("secret-token-123");
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let mut req = DaemonRequest::ping("r1");
        req.auth = Some("secret-token-123".to_string());
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok());
        assert_eq!(resp.id, "r1");

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_dispatches_all_registered_methods_without_unknown_method() {
        let server = start_test_server();
        let port = server.port;

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let methods = [
            protocol::method::PING,
            protocol::method::GIT_STATUS,
            protocol::method::GIT_LOG,
            protocol::method::GIT_LATEST_COMMIT_TIME,
            protocol::method::FILE_TREE,
            protocol::method::READ_FILE,
            protocol::method::READ_README,
            protocol::method::READ_ASSET,
            protocol::method::SCAN_SESSIONS,
            protocol::method::LIST_DISPLAY_SESSIONS,
            protocol::method::LIST_RUNTIME_SESSIONS,
            protocol::method::WAIT_SESSION_UPDATES,
            protocol::method::LAUNCH_SESSION,
            protocol::method::STOP_SESSION,
            protocol::method::NAVIGATE_TO_SESSION,
            protocol::method::GET_PROJECT_TASKS,
            protocol::method::GIT_COMMITS_IN_RANGE,
            protocol::method::GIT_COMMIT_FILES,
            protocol::method::GIT_COMMIT_DIFF,
            protocol::method::WATCH,
            protocol::method::UNWATCH,
        ];

        for (idx, method) in methods.into_iter().enumerate() {
            let req =
                DaemonRequest::new(format!("dispatch-{idx}"), method, serde_json::Value::Null);
            let resp = send_request(&mut stream, &mut reader, &req);
            assert_eq!(resp.id, format!("dispatch-{idx}"));
            assert!(
                resp.error
                    .as_ref()
                    .is_none_or(|error| error.code != "UNKNOWN_METHOD"),
                "dispatch returned UNKNOWN_METHOD for {method}"
            );
        }

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_allows_new_connection_after_client_disconnect() {
        let server = start_test_server();
        let port = server.port;

        {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            let resp = send_request(&mut stream, &mut reader, &DaemonRequest::ping("first"));
            assert!(resp.is_ok());
        }

        {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            let resp = send_request(&mut stream, &mut reader, &DaemonRequest::ping("second"));
            assert!(resp.is_ok());
        }

        server.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn push_event_ignores_lock_poisoning_and_write_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let _client = TcpStream::connect(addr).expect("connect client");
        let (server_stream, _) = listener.accept().expect("accept");
        let writer = Arc::new(Mutex::new(server_stream));

        let poison_target = writer.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poison_target.lock().expect("lock");
            panic!("poison writer lock");
        }));

        let event = DaemonEvent {
            event: "git_changed".to_string(),
            data: serde_json::json!({"path": "/tmp/project"}),
        };

        let poisoned_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            push_event(&writer, &event);
        }));
        assert!(
            poisoned_result.is_ok(),
            "push_event should not panic on poisoned lock"
        );

        // Create a fresh writer and close its peer so writes fail.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind second");
        let addr = listener.local_addr().expect("addr second");
        let client = TcpStream::connect(addr).expect("connect second client");
        let (server_stream, _) = listener.accept().expect("accept second");
        drop(client);
        let writer = Arc::new(Mutex::new(server_stream));

        let write_fail_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            push_event(&writer, &event);
        }));
        assert!(
            write_fail_result.is_ok(),
            "push_event should not panic when stream writes fail"
        );
    }
}
