use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::daemon::protocol::{self, DaemonEvent, DaemonRequest, DaemonResponse};
use crate::project_provider::ProjectProvider;

/// Default port for the daemon.
pub const DEFAULT_PORT: u16 = 17233;
/// App-only override used by isolated E2E workers and explicit launchers.
pub const DAEMON_PORT_OVERRIDE_ENV: &str = "TAURHAUS_DAEMON_PORT";

/// Port the app and its daemon launcher must agree on for this process.
pub fn app_daemon_port() -> u16 {
    std::env::var(DAEMON_PORT_OVERRIDE_ENV)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT)
}

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
    // Passed as a closure rather than called here so `run_for_test` never
    // writes into real config dirs.
    run_with_legacy_cleanup(config, shutdown, provider, retire_legacy_bridge, true)
}

fn retire_legacy_bridge() {
    crate::session_scanner::accounts::legacy_statusline::retire_once();
}

/// Start the daemon, with legacy bridge cleanup running beside the listener.
fn run_with_legacy_cleanup<F>(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    cleanup: F,
    schedule_background_passes: bool,
) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    let listener = bind_listener(config)?;
    run_on_bound_listener(
        config,
        listener,
        shutdown,
        provider,
        cleanup,
        schedule_background_passes,
    )
}

/// Run the daemon startup path on a listener the caller already owns.
fn run_on_bound_listener<F>(
    config: &DaemonConfig,
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    cleanup: F,
    schedule_background_passes: bool,
) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    if let Err(error) = std::thread::Builder::new()
        .name("claude-statusline-retire".to_string())
        .spawn(cleanup)
    {
        tracing::warn!(error = %error, "Legacy Claude status line cleanup not spawned");
    }
    let session_hub = crate::daemon::session_activity::SessionActivityHub::global();
    let _ = session_hub.wait_for_update(0, 0, Duration::from_millis(750));

    #[cfg(feature = "mesh-bridged-backend")]
    let coordination_state =
        Arc::new(crate::coordination::state::CoordinationState::for_process_default());
    #[cfg(feature = "mesh-bridged-backend")]
    let launch_settings = crate::daemon::background_scheduler::LaunchSettingsStore::default();
    #[cfg(feature = "mesh-bridged-backend")]
    let deadline_scheduler = schedule_background_passes.then(|| {
        // The hub above owns and refreshes the member-activity snapshots that
        // the shared deadline pass reads. Register only after that activity
        // source is live so the pass keeps its existing input seam.
        crate::daemon::deadline_scheduler::DeadlineScheduler::start(
            coordination_state.clone(),
            shutdown.clone(),
        )
    });
    #[cfg(feature = "mesh-bridged-backend")]
    let background_scheduler = schedule_background_passes.then(|| {
        crate::daemon::background_scheduler::BackgroundScheduler::start(
            coordination_state.clone(),
            launch_settings.clone(),
            shutdown.clone(),
        )
    });
    #[cfg(not(feature = "mesh-bridged-backend"))]
    let _ = schedule_background_passes;

    let result = serve(
        config,
        listener,
        shutdown.clone(),
        provider,
        #[cfg(feature = "mesh-bridged-backend")]
        coordination_state,
        #[cfg(feature = "mesh-bridged-backend")]
        launch_settings,
    );
    shutdown.store(true, Ordering::Relaxed);
    #[cfg(feature = "mesh-bridged-backend")]
    if let Some(scheduler) = deadline_scheduler {
        scheduler.join();
    }
    #[cfg(feature = "mesh-bridged-backend")]
    if let Some(scheduler) = background_scheduler {
        scheduler.join();
    }
    result
}

/// Bind and listen on the daemon's port, without serving on it yet.
///
/// Separated from [`serve`] so a caller can bind on its own thread and hand
/// the socket over: from the moment this returns, the port accepts. The daemon
/// binds it inline; a test does it to be sure its port is up before it starts
/// connecting.
fn bind_listener(config: &DaemonConfig) -> std::io::Result<TcpListener> {
    // On macOS, use SO_REUSEADDR so we can rebind immediately after the previous
    // daemon dies. Linux does not need this for our listener pattern, and enabling
    // it there can permit duplicate listeners on the same port.
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
    Ok(socket.into())
}

/// Serve on an already-bound listener until `shutdown` is set or idle timeout.
fn serve(
    config: &DaemonConfig,
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    #[cfg(feature = "mesh-bridged-backend")] coordination_state: Arc<
        crate::coordination::state::CoordinationState,
    >,
    #[cfg(feature = "mesh-bridged-backend")]
    launch_settings: crate::daemon::background_scheduler::LaunchSettingsStore,
) -> std::io::Result<()> {
    listener.set_nonblocking(true)?;

    let start_time = Instant::now();
    let last_activity = Arc::new(AtomicU64::new(epoch_secs()));
    let auth_token: Option<Arc<str>> = config.auth_token.as_deref().map(Arc::from);
    #[cfg(feature = "mesh-bridged-backend")]
    let coordination_run_registry =
        crate::daemon::coordination_runs::CoordinationRunRegistry::default();
    #[cfg(feature = "mesh-bridged-backend")]
    let initialize_service = Arc::new(
        crate::daemon::initialize_runs::InitializeTeamService::for_process_default(
            coordination_state.clone(),
            coordination_run_registry.clone(),
        ),
    );
    #[cfg(feature = "mesh-bridged-backend")]
    let member_operations_service = Arc::new(
        crate::daemon::member_runs::MemberOperationsService::for_process_default(
            coordination_state.clone(),
            coordination_run_registry.clone(),
        ),
    );
    #[cfg(feature = "mesh-bridged-backend")]
    let team_operations_service = Arc::new(
        crate::daemon::team_runs::TeamOperationsService::for_process_default(
            coordination_state.clone(),
            coordination_run_registry.clone(),
        ),
    );
    #[cfg(feature = "mesh-bridged-backend")]
    let roster_operations_service = Arc::new(
        crate::daemon::roster_runs::RosterOperationsService::for_process_default(
            coordination_state.clone(),
            coordination_run_registry.clone(),
        ),
    );
    #[cfg(feature = "mesh-bridged-backend")]
    let effort_operations_service = Arc::new(
        crate::daemon::effort_runs::EffortOperationsService::for_process_default(
            coordination_state.clone(),
            coordination_run_registry.clone(),
        ),
    );
    let watch_registry =
        crate::daemon::watch::SharedDaemonWatchRegistry::new().map_err(std::io::Error::other)?;

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
                let services = ConnectionServices {
                    provider: provider.clone(),
                    watch_registry: watch_registry.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    initialize_service: initialize_service.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    member_operations_service: member_operations_service.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    team_operations_service: team_operations_service.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    roster_operations_service: roster_operations_service.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    effort_operations_service: effort_operations_service.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    launch_settings: launch_settings.clone(),
                    #[cfg(feature = "mesh-bridged-backend")]
                    coordination_state: coordination_state.clone(),
                };
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
                        services,
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
    let _ = telemetry_handle.join();
    tracing::info!("daemon shutting down");
    Ok(())
}

/// Bind a test daemon's listener on the caller's thread.
///
/// A test that hands the socket to its serving thread has no window in which
/// its helper has returned and the port is not up yet — the window a fixed
/// sleep used to cover, badly.
#[cfg(test)]
pub(crate) fn bind_listener_for_test(config: &DaemonConfig) -> std::io::Result<TcpListener> {
    bind_listener(config)
}

/// Serve a test daemon on a listener [`bind_listener_for_test`] already bound.
#[cfg(test)]
pub(crate) fn serve_for_test(
    config: &DaemonConfig,
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
) -> std::io::Result<()> {
    serve(
        config,
        listener,
        shutdown,
        provider,
        #[cfg(feature = "mesh-bridged-backend")]
        Arc::new(crate::coordination::state::CoordinationState::for_process_default()),
        #[cfg(feature = "mesh-bridged-backend")]
        crate::daemon::background_scheduler::LaunchSettingsStore::default(),
    )
}

#[cfg(test)]
pub(crate) fn run_for_test(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
) -> std::io::Result<()> {
    run_for_test_with_legacy_cleanup(config, shutdown, provider, || {})
}

/// `run_for_test`, with legacy cleanup a test wants to time.
#[cfg(test)]
pub(crate) fn run_for_test_with_legacy_cleanup<F>(
    config: &DaemonConfig,
    shutdown: Arc<AtomicBool>,
    provider: Arc<dyn ProjectProvider>,
    cleanup: F,
) -> std::io::Result<()>
where
    F: FnOnce() + Send + 'static,
{
    run_with_legacy_cleanup(config, shutdown, provider, cleanup, false)
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
struct ConnectionServices {
    provider: Arc<dyn ProjectProvider>,
    watch_registry: Arc<crate::daemon::watch::SharedDaemonWatchRegistry>,
    #[cfg(feature = "mesh-bridged-backend")]
    initialize_service: Arc<crate::daemon::initialize_runs::InitializeTeamService>,
    #[cfg(feature = "mesh-bridged-backend")]
    member_operations_service: Arc<crate::daemon::member_runs::MemberOperationsService>,
    #[cfg(feature = "mesh-bridged-backend")]
    team_operations_service: Arc<crate::daemon::team_runs::TeamOperationsService>,
    #[cfg(feature = "mesh-bridged-backend")]
    roster_operations_service: Arc<crate::daemon::roster_runs::RosterOperationsService>,
    #[cfg(feature = "mesh-bridged-backend")]
    effort_operations_service: Arc<crate::daemon::effort_runs::EffortOperationsService>,
    #[cfg(feature = "mesh-bridged-backend")]
    launch_settings: crate::daemon::background_scheduler::LaunchSettingsStore,
    #[cfg(feature = "mesh-bridged-backend")]
    coordination_state: Arc<crate::coordination::state::CoordinationState>,
}

fn handle_connection(
    stream: TcpStream,
    start_time: Instant,
    shutdown: &AtomicBool,
    last_activity: &AtomicU64,
    auth_token: Option<&str>,
    services: ConnectionServices,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let writer = Arc::new(Mutex::new(stream));
    let project_task_scan_cache = crate::daemon::handlers::ProjectTaskScanCacheState::default();
    let mut watch_runtime =
        crate::daemon::watch::WatchRuntime::new(services.watch_registry.clone());

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
            services.provider.as_ref(),
            start_time,
            &writer,
            &mut watch_runtime,
            &project_task_scan_cache,
            #[cfg(feature = "mesh-bridged-backend")]
            (
                services.initialize_service.as_ref(),
                services.member_operations_service.as_ref(),
                services.team_operations_service.as_ref(),
                services.roster_operations_service.as_ref(),
                services.effort_operations_service.as_ref(),
                &services.launch_settings,
                services.coordination_state.as_ref(),
            ),
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

    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    // Regression: commit fc896344 isolated E2E data roots but left the app on
    // port 17233, where its auth failure could restart the operator's daemon.
    #[test]
    fn app_daemon_port_honors_the_worker_override() {
        let _env_guard = crate::test_support::acquire_env_test_guard();
        let _env = EnvRestore::set("TAURHAUS_DAEMON_PORT", "29441");

        assert_eq!(app_daemon_port(), 29441);
    }

    struct TestServer {
        port: u16,
        shutdown: Arc<AtomicBool>,
        _heavy_guard: crate::test_support::HeavyTestGuard,
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
        start_server_with_heavy_guard_and_probe(config, heavy_guard, |_| {})
    }

    // Regression: commit 831571dac released the selected ephemeral port before
    // the serving thread rebound it. Keep the listener owned across the handoff
    // so parallel daemon tests cannot take the same kernel resource.
    fn start_server_with_heavy_guard_and_probe(
        mut config: DaemonConfig,
        heavy_guard: crate::test_support::HeavyTestGuard,
        probe: impl FnOnce(u16),
    ) -> TestServer {
        let listener = bind_listener_for_test(&config).expect("bind test daemon listener");
        let port = listener.local_addr().expect("test daemon address").port();
        config.port = port;
        probe(port);

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run_on_bound_listener(
                &config,
                listener,
                shutdown_clone,
                Arc::new(LocalProvider),
                || {},
                false,
            )
        });
        TestServer {
            port,
            shutdown,
            _heavy_guard: heavy_guard,
            handle: Some(handle),
        }
    }

    fn start_test_server() -> TestServer {
        start_test_server_with_port_probe(|_| {})
    }

    fn start_test_server_with_port_probe(probe: impl FnOnce(u16)) -> TestServer {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        start_server_with_heavy_guard_and_probe(
            DaemonConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
                idle_timeout_secs: None,
                auth_token: None,
            },
            heavy_guard,
            probe,
        )
    }

    fn start_test_server_with_heavy_guard(
        heavy_guard: crate::test_support::HeavyTestGuard,
    ) -> TestServer {
        start_server_with_heavy_guard(
            DaemonConfig {
                port: 0,
                bind_addr: "127.0.0.1".to_string(),
                idle_timeout_secs: None,
                auth_token: None,
            },
            heavy_guard,
        )
    }

    // Regression: commit 831571dac made the fixture release its ephemeral port
    // during handoff, letting a parallel listener steal it.
    #[test]
    fn test_server_fixture_keeps_ephemeral_port_owned_during_handoff() {
        let server = start_test_server_with_port_probe(|port| {
            let error = TcpListener::bind(("127.0.0.1", port))
                .expect_err("the test fixture must retain ownership of its selected port");
            assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
        });

        assert_ne!(server.port, 0);
    }

    // Regression: 79be608 installed the Claude status-line bridge from `run`,
    // synchronously, before the listener existed. That work probed
    // `claude --version` with a five second timeout, while `daemon::launcher`
    // gives the whole daemon five seconds to become reachable — so one hung
    // CLI probe cost an otherwise healthy daemon its startup and pushed the
    // app onto the local fallback.
    #[test]
    fn slow_legacy_cleanup_never_delays_the_listener() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        // Bound listener retained across the handoff (see the idle-timeout
        // test's comment); the timing closure rides run_on_bound_listener's
        // cleanup argument, which is the same path production takes after
        // run() binds.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

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
            run_on_bound_listener(
                &config,
                listener,
                server_shutdown,
                Arc::new(LocalProvider),
                move || {
                    std::thread::sleep(Duration::from_secs(3));
                    install_flag.store(true, Ordering::Relaxed);
                },
                false,
            )
        });

        let reachable = wait_for_server_accepting(port, Duration::from_secs(2));
        shutdown.store(true, Ordering::Relaxed);
        let _ = handle.join();

        assert!(
            reachable,
            "the daemon must bind before legacy cleanup completes"
        );
    }

    // Regression: c3db92f5 built separate coordination states for deadline
    // work and initialization, leaving their orchestrator locks independent.
    #[test]
    fn daemon_coordination_workers_share_one_process_state() {
        let source = include_str!("server.rs");
        let runtime = &source[..source.find("#[cfg(test)]").unwrap_or(source.len())];
        assert_eq!(
            runtime
                .matches("CoordinationState::for_process_default()")
                .count(),
            1,
            "the daemon must build exactly one process-wide coordination state \
             so its workers share one orchestrator critical section"
        );
    }

    // Regression: f8d08a21 gave roster operations a private run registry,
    // unlike every earlier daemon-owned coordination service.
    #[test]
    fn daemon_coordination_workers_share_one_process_run_registry() {
        let source = include_str!("server.rs");
        let runtime = &source[..source.find("#[cfg(test)]").unwrap_or(source.len())];
        assert_eq!(
            runtime.matches("coordination_run_registry.clone()").count(),
            5,
            "all five coordination services must receive the process-wide run registry"
        );
    }

    // Regression: 34fdeead proved an isolated scheduler test does not pin its
    // production registration. Protocol 21 must start the self-heal/effort arm
    // from the real daemon entry point.
    #[test]
    fn production_daemon_run_registers_the_background_scheduler() {
        let source = include_str!("server.rs");
        let runtime = &source[..source.find("#[cfg(test)]").unwrap_or(source.len())];
        assert!(
            runtime.contains("BackgroundScheduler::start"),
            "the production daemon must register the protocol-21 scheduler arm"
        );
    }

    // Regression: 34fdeead added a daemon deadline scheduler but only tested
    // the scheduler in isolation. Removing `run`'s production registration
    // would therefore leave both the app and daemon with deadline work disabled.
    #[test]
    // This test deliberately exercises production `run`, which binds its own
    // listener — so the reserve-and-release window below is unavoidable here;
    // the heavy guard is what covers it against other guarded fixtures.
    fn production_daemon_run_registers_and_fires_background_schedulers() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _env_guard = crate::test_support::acquire_env_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let claude_dir = temp.path().join("claude");
        let data_dir = temp.path().join("data");
        std::fs::create_dir_all(claude_dir.join("teams")).expect("teams dir");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let _claude_env = EnvRestore::set(
            "TAURHAUS_CLAUDE_DIR",
            claude_dir.to_str().expect("utf-8 claude dir"),
        );
        let _data_env = EnvRestore::set(
            "TAURHAUS_DATA_DIR",
            data_dir.to_str().expect("utf-8 data dir"),
        );
        let log_state =
            crate::commands::logging::LogFileState::new(data_dir.join("taurhaus.log.jsonl"))
                .expect("log state");
        crate::commands::logging::install_global_sink(&log_state);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(event_tx);

        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
        let port = listener.local_addr().expect("listener address").port();
        drop(listener);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run(
                &DaemonConfig {
                    port,
                    bind_addr: "127.0.0.1".to_string(),
                    idle_timeout_secs: None,
                    auth_token: None,
                },
                server_shutdown,
                Arc::new(LocalProvider),
            )
        });

        // Measured ~6.3s on an idle host; generous for CI/loaded hosts.
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut deadline_pass = None;
        let mut self_heal_pass = None;
        while deadline_pass.is_none() || self_heal_pass.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let record = event_rx
                .recv_timeout(remaining)
                .expect("production daemon background passes within thirty seconds");
            match record["event"].as_str() {
                Some("deadline.pass.completed") => deadline_pass = Some(record),
                Some("self_heal.pass.completed") => self_heal_pass = Some(record),
                _ => {}
            }
        }
        shutdown.store(true, Ordering::Relaxed);
        handle
            .join()
            .expect("server thread")
            .expect("server result");
        crate::commands::logging::clear_test_tap();

        let deadline_pass = deadline_pass.expect("deadline pass");
        let self_heal_pass = self_heal_pass.expect("self-heal pass");
        assert_eq!(deadline_pass["component"], "coordination");
        assert_eq!(deadline_pass["fields"]["teams_scanned"], 0);
        assert_eq!(self_heal_pass["component"], "coordination");
        assert_eq!(self_heal_pass["fields"]["teams_scanned"], 0);
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

    // Regression: commit 3d45cba4 moved the shared fixture directly onto
    // `serve_for_test`, bypassing the daemon startup path that starts eager
    // session scans and making this guard depend on sibling-test side effects.
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
    fn retired_stop_member_methods_return_unknown_method() {
        // Regression: 03eb3a2c made remove-member the app's roster-removal path
        // but left both superseded stop-member methods callable in the daemon.
        let server = start_test_server();
        let port = server.port;
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        for (index, method) in [
            "coordination.stop_member",
            "coordination.stop_member_status",
        ]
        .into_iter()
        .enumerate()
        {
            let request = DaemonRequest::new(format!("retired-stop-{index}"), method, Value::Null);
            let response = send_request(&mut stream, &mut reader, &request);
            assert_eq!(
                response.error.expect("retired method error").code,
                "UNKNOWN_METHOD"
            );
        }

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
        let shutdown = Arc::new(AtomicBool::new(false));
        // Keep the bound listener across the handoff: releasing the port and
        // rebinding raced concurrent ephemeral binds now that the suite runs
        // parallel (the listener-flake class this suite's fix retired).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: Some(1), // 1 second timeout
            auth_token: None,
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run_on_bound_listener(
                &config,
                listener,
                shutdown_clone,
                Arc::new(LocalProvider),
                || {},
                false,
            )
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
        start_server(DaemonConfig {
            port: 0,
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
            protocol::method::COORDINATION_INITIALIZE_TEAM,
            protocol::method::COORDINATION_INITIALIZE_STATUS,
            protocol::method::COORDINATION_ADD_AGENT,
            protocol::method::COORDINATION_ADD_AGENT_STATUS,
            protocol::method::COORDINATION_RESUME_MEMBER,
            protocol::method::COORDINATION_RESUME_MEMBER_STATUS,
            protocol::method::COORDINATION_RESUME_TEAM,
            protocol::method::COORDINATION_RESUME_TEAM_STATUS,
            protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT,
            protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT_STATUS,
            protocol::method::COORDINATION_REONBOARD,
            protocol::method::COORDINATION_REONBOARD_STATUS,
            protocol::method::COORDINATION_CREATE_TEAM,
            protocol::method::COORDINATION_CREATE_TEAM_STATUS,
            protocol::method::COORDINATION_DISBAND_TEAM,
            protocol::method::COORDINATION_DISBAND_TEAM_STATUS,
            protocol::method::COORDINATION_ADD_MEMBER,
            protocol::method::COORDINATION_ADD_MEMBER_STATUS,
            protocol::method::COORDINATION_REMOVE_MEMBER,
            protocol::method::COORDINATION_REMOVE_MEMBER_STATUS,
            protocol::method::COORDINATION_PUT_LAUNCH_SETTINGS,
            protocol::method::COORDINATION_APPLY_TASK_EFFORT,
            protocol::method::COORDINATION_APPLY_TASK_EFFORT_STATUS,
            protocol::method::COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS,
            protocol::method::COORDINATION_RECONCILE_LIVE_PRESENCE,
            protocol::method::COORDINATION_SET_ACTIVE_PROJECT_TEAM,
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
