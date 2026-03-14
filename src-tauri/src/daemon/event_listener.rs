use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crate::daemon::protocol::{self, DaemonMessage, DaemonRequest, DaemonResponse};
use crate::errors::AppError;
use crate::fs::watcher::WatchEvent;

const DEFAULT_WATCH_HANDSHAKE_TIMEOUT_SECS: u64 = 10;
const WATCH_HANDSHAKE_TIMEOUT_ENV: &str = "TAURHAUS_DAEMON_WATCH_TIMEOUT_SECS";
static DROPPED_DAEMON_EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

fn watch_handshake_timeout() -> Duration {
    std::env::var(WATCH_HANDSHAKE_TIMEOUT_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_WATCH_HANDSHAKE_TIMEOUT_SECS))
}

/// Listens for push events from the daemon over a dedicated TCP connection.
///
/// Opens a separate connection from the main `DaemonProvider` (which is used
/// for request/response operations). Sends `watch` commands for WSL projects,
/// then reads events in a blocking loop, converting daemon events to
/// `WatchEvent`s and forwarding them to the app's event channel.
pub struct DaemonEventListener {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    event_tx: mpsc::Sender<WatchEvent>,
    /// Mapping from watched Linux path → project_id.
    path_to_project: HashMap<String, String>,
    next_id: u64,
    wsl_distro: Option<String>,
    /// Auth token read from the daemon's token file.
    auth_token: Option<String>,
}

impl DaemonEventListener {
    /// Connect to the daemon for event listening.
    pub fn connect(addr: &str, event_tx: mpsc::Sender<WatchEvent>) -> Result<Self, AppError> {
        Self::connect_with_distro(addr, event_tx, None)
    }

    pub fn connect_with_distro(
        addr: &str,
        event_tx: mpsc::Sender<WatchEvent>,
        wsl_distro: Option<&str>,
    ) -> Result<Self, AppError> {
        let stream = TcpStream::connect(addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Event listener connect to {addr} failed: {e}"),
            ))
        })?;
        stream.set_nodelay(true).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Event listener TCP_NODELAY setup failed for {addr}: {e}"),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);

        // Read auth token (falls back to WSL on Windows)
        let auth_token = crate::daemon::auth::read_auth_token_for_distro(wsl_distro);

        Ok(Self {
            stream,
            reader,
            event_tx,
            path_to_project: HashMap::new(),
            next_id: 1,
            wsl_distro: wsl_distro.map(ToOwned::to_owned),
            auth_token,
        })
    }

    /// Send a watch command for a project. The daemon will start watching the
    /// directory and push events back on this connection.
    pub fn watch(&mut self, project_id: &str, linux_path: &str) -> Result<(), AppError> {
        let id = format!("ew{}", self.next_id);
        self.next_id += 1;
        let handshake_timeout = watch_handshake_timeout();

        let request = DaemonRequest::new(
            &id,
            protocol::method::WATCH,
            protocol::PathParams {
                path: linux_path.to_string(),
            },
        )
        .with_auth(self.load_auth_token_if_missing());

        let json = serde_json::to_string(&request)
            .map_err(|e| AppError::InvalidPath(format!("Serialize watch request failed: {e}")))?;
        self.stream
            .write_all(json.as_bytes())
            .map_err(AppError::Io)?;
        self.stream.write_all(b"\n").map_err(AppError::Io)?;
        self.stream.flush().map_err(AppError::Io)?;
        self.stream
            .set_read_timeout(Some(handshake_timeout))
            .map_err(|error| {
                AppError::Io(std::io::Error::new(
                    error.kind(),
                    format!(
                        "Set watch handshake timeout failed for project {project_id} ({linux_path}): {error}"
                    ),
                ))
            })?;

        // Read lines until we get the watch response. The daemon may push events
        // on this connection before the response arrives (e.g. if a previously
        // watched project fires an event between our request and its response).
        let mut line = String::new();
        let response: DaemonResponse = loop {
            line.clear();
            self.reader.read_line(&mut line).map_err(|error| {
                AppError::Io(std::io::Error::new(
                    error.kind(),
                    format!(
                        "Read watch response failed for project {project_id} ({linux_path}) within {}s: {error}",
                        handshake_timeout.as_secs()
                    ),
                ))
            })?;
            match serde_json::from_str::<DaemonMessage>(line.trim()) {
                Ok(DaemonMessage::Response(r)) => break r,
                Ok(DaemonMessage::Event(event)) => {
                    // Forward the event — it's from a previously registered watch
                    self.handle_event(event);
                }
                Err(e) => {
                    return Err(AppError::InvalidPath(format!(
                        "Parse watch response failed: {e}"
                    )));
                }
            }
        };
        self.stream.set_read_timeout(None).map_err(|error| {
            AppError::Io(std::io::Error::new(
                error.kind(),
                format!(
                    "Reset watch handshake timeout failed for project {project_id} ({linux_path}): {error}"
                ),
            ))
        })?;

        if !response.is_ok() {
            return Err(AppError::InvalidPath(format!(
                "Daemon watch failed: {}",
                response
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "unknown".into())
            )));
        }

        // Canonicalize for matching against daemon events (which use canonical paths,
        // critical on macOS where /var → /private/var).
        let canonical = std::path::Path::new(linux_path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| linux_path.to_string());
        self.path_to_project
            .insert(canonical, project_id.to_string());
        tracing::info!(project_id, linux_path, "Daemon watch registered");
        Ok(())
    }

    pub fn unwatch(&mut self, linux_path: &str) -> Result<(), AppError> {
        let id = format!("eu{}", self.next_id);
        self.next_id += 1;
        let handshake_timeout = watch_handshake_timeout();

        let request = DaemonRequest::new(
            &id,
            protocol::method::UNWATCH,
            protocol::PathParams {
                path: linux_path.to_string(),
            },
        )
        .with_auth(self.load_auth_token_if_missing());

        let json = serde_json::to_string(&request)
            .map_err(|e| AppError::InvalidPath(format!("Serialize unwatch request failed: {e}")))?;
        self.stream
            .write_all(json.as_bytes())
            .map_err(AppError::Io)?;
        self.stream.write_all(b"\n").map_err(AppError::Io)?;
        self.stream.flush().map_err(AppError::Io)?;
        self.stream
            .set_read_timeout(Some(handshake_timeout))
            .map_err(|error| {
                AppError::Io(std::io::Error::new(
                    error.kind(),
                    format!("Set unwatch handshake timeout failed for path {linux_path}: {error}"),
                ))
            })?;

        let mut line = String::new();
        let response: DaemonResponse = loop {
            line.clear();
            self.reader.read_line(&mut line).map_err(|error| {
                AppError::Io(std::io::Error::new(
                    error.kind(),
                    format!(
                        "Read unwatch response failed for path {linux_path} within {}s: {error}",
                        handshake_timeout.as_secs()
                    ),
                ))
            })?;
            match serde_json::from_str::<DaemonMessage>(line.trim()) {
                Ok(DaemonMessage::Response(r)) => break r,
                Ok(DaemonMessage::Event(event)) => self.handle_event(event),
                Err(e) => {
                    return Err(AppError::InvalidPath(format!(
                        "Parse unwatch response failed: {e}"
                    )));
                }
            }
        };
        self.stream.set_read_timeout(None).map_err(|error| {
            AppError::Io(std::io::Error::new(
                error.kind(),
                format!("Reset unwatch handshake timeout failed for {linux_path}: {error}"),
            ))
        })?;

        if !response.is_ok() {
            return Err(AppError::InvalidPath(format!(
                "Daemon unwatch failed: {}",
                response
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "unknown".into())
            )));
        }

        let canonical = std::path::Path::new(linux_path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| linux_path.to_string());
        self.path_to_project.remove(&canonical);
        tracing::info!(linux_path, "Daemon watch unregistered");
        Ok(())
    }

    /// Run the event loop. Blocks until the connection drops.
    ///
    /// Call this on a background thread after all `watch()` calls are done.
    /// Events are forwarded to the `event_tx` channel as `WatchEvent`s.
    pub fn run(self) {
        self.run_inner(None);
    }

    /// Run the event loop until the connection drops or `stop_signal` is set.
    pub fn run_until_stopped(self, stop_signal: Arc<AtomicBool>) {
        self.run_inner(Some(stop_signal));
    }

    pub fn pump_once(&mut self, timeout: Duration) -> Result<bool, AppError> {
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| {
                AppError::Io(std::io::Error::new(
                    error.kind(),
                    format!("Failed to set daemon event listener timeout: {error}"),
                ))
            })?;

        let mut line = String::new();
        let outcome = match self.reader.read_line(&mut line) {
            Ok(0) => Ok(false),
            Ok(_) => {
                self.handle_line(&line);
                Ok(true)
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(true)
            }
            Err(error) => Err(AppError::Io(error)),
        };

        self.stream.set_read_timeout(None).map_err(|error| {
            AppError::Io(std::io::Error::new(
                error.kind(),
                format!("Failed to clear daemon event listener timeout: {error}"),
            ))
        })?;
        outcome
    }

    fn run_inner(mut self, stop_signal: Option<Arc<AtomicBool>>) {
        if let Err(error) = self.stream.set_read_timeout(Some(Duration::from_secs(5))) {
            tracing::warn!(error = %error, "Failed to set daemon event listener read timeout");
        }

        let mut line = String::new();
        loop {
            if stop_signal
                .as_ref()
                .is_some_and(|signal| signal.load(Ordering::Relaxed))
            {
                break;
            }
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => break, // Connection closed
                Ok(_) => self.handle_line(&line),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    continue;
                }
                Err(_) => break,
            }
        }

        tracing::info!("Daemon event listener disconnected");
    }

    fn handle_line(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        let msg: DaemonMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("Event listener: unparseable line: {e}");
                return;
            }
        };

        match msg {
            DaemonMessage::Event(event) => self.handle_event(event),
            DaemonMessage::Response(_) => {
                // Stray response — ignore (watch responses are handled synchronously)
            }
        }
    }

    fn handle_event(&self, event: protocol::DaemonEvent) {
        if let Some(watch_event) = convert_daemon_event(event, &self.path_to_project) {
            if let Err(error) = self.event_tx.send(watch_event) {
                let dropped_count = DROPPED_DAEMON_EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!(
                    error = %error,
                    dropped_count,
                    "dropping daemon event: failed to forward to app channel"
                );
            }
        }
    }

    fn load_auth_token_if_missing(&mut self) -> Option<String> {
        if self.auth_token.is_none() {
            self.auth_token =
                crate::daemon::auth::read_auth_token_for_distro(self.wsl_distro.as_deref());
        }
        self.auth_token.clone()
    }
}

/// Convert a daemon push event into a `WatchEvent`, using the path→project_id
/// mapping to tag events with the correct project.
///
/// Returns `None` if the event's path doesn't match any watched project.
fn convert_daemon_event(
    event: protocol::DaemonEvent,
    path_to_project: &HashMap<String, String>,
) -> Option<WatchEvent> {
    let event_name = event.event.clone();
    match event.event.as_str() {
        protocol::event::GIT_CHANGED => {
            let data: protocol::GitChangedData = match serde_json::from_value(event.data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    log_dropped_daemon_event(
                        &event_name,
                        "decode_payload",
                        None,
                        Some(&error.to_string()),
                    );
                    return None;
                }
            };
            let project_id = match path_to_project.get(&data.path) {
                Some(id) => id,
                None => {
                    log_dropped_daemon_event(
                        &event_name,
                        "unmapped_path",
                        Some(data.path.as_str()),
                        None,
                    );
                    return None;
                }
            };
            Some(WatchEvent::GitChanged {
                project_id: project_id.clone(),
            })
        }
        protocol::event::FILE_CHANGED => {
            let data: protocol::FileChangedData = match serde_json::from_value(event.data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    log_dropped_daemon_event(
                        &event_name,
                        "decode_payload",
                        None,
                        Some(&error.to_string()),
                    );
                    return None;
                }
            };
            let project_id = match path_to_project.get(&data.path) {
                Some(id) => id,
                None => {
                    log_dropped_daemon_event(
                        &event_name,
                        "unmapped_path",
                        Some(data.path.as_str()),
                        None,
                    );
                    return None;
                }
            };
            let project_root = PathBuf::from(&data.path);
            let paths: Vec<PathBuf> = data.files.iter().map(|f| project_root.join(f)).collect();
            Some(WatchEvent::FileChanged {
                project_id: project_id.clone(),
                paths,
            })
        }
        protocol::event::SESSION_FILE_CREATED => {
            let data: protocol::SessionFileCreatedData = match serde_json::from_value(event.data) {
                Ok(parsed) => parsed,
                Err(error) => {
                    log_dropped_daemon_event(
                        &event_name,
                        "decode_payload",
                        None,
                        Some(&error.to_string()),
                    );
                    return None;
                }
            };
            let project_id = match path_to_project.get(&data.path) {
                Some(id) => id,
                None => {
                    log_dropped_daemon_event(
                        &event_name,
                        "unmapped_path",
                        Some(data.path.as_str()),
                        None,
                    );
                    return None;
                }
            };
            let full_path = PathBuf::from(&data.path).join(&data.file);
            Some(WatchEvent::SessionFileCreated {
                project_id: project_id.clone(),
                path: full_path,
            })
        }
        _ => {
            tracing::debug!(event = event.event, "Unknown daemon event type");
            None
        }
    }
}

fn log_dropped_daemon_event(event: &str, stage: &str, path: Option<&str>, error: Option<&str>) {
    let dropped_count = DROPPED_DAEMON_EVENT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    tracing::warn!(
        event,
        stage,
        path = path.unwrap_or("n/a"),
        error = error.unwrap_or("n/a"),
        dropped_count,
        "dropping daemon event"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonConfig;
    use crate::provider::local::LocalProvider;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct TestDaemon {
        port: u16,
        shutdown: Arc<AtomicBool>,
        _heavy_guard: crate::test_support::HeavyTestGuard,
        handle: Option<std::thread::JoinHandle<std::io::Result<()>>>,
    }

    impl Drop for TestDaemon {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn start_daemon() -> TestDaemon {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: None,
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            crate::daemon::server::run(&config, shutdown_clone, Arc::new(LocalProvider))
        });
        wait_for_port(port, Duration::from_secs(3));
        TestDaemon {
            port,
            shutdown,
            _heavy_guard: heavy_guard,
            handle: Some(handle),
        }
    }

    fn wait_for_port(port: u16, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("daemon did not accept connections on port {port} before timeout");
    }

    #[test]
    fn event_listener_connects_and_watches() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let (tx, _rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{}", daemon.port), tx).unwrap();
        assert!(listener.stream.nodelay().unwrap());
        let result = listener.watch("p1", dir.path().to_str().unwrap());
        assert!(result.is_ok());

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn event_listener_watch_nonexistent_fails() {
        let daemon = start_daemon();
        let (tx, _rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{}", daemon.port), tx).unwrap();
        let result = listener.watch("p1", "/nonexistent/path/that/does/not/exist");
        assert!(result.is_err());

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn event_listener_receives_file_change() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let (tx, rx) = mpsc::channel();

        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{}", daemon.port), tx).unwrap();
        listener.watch("p1", dir.path().to_str().unwrap()).unwrap();

        // Start the event loop on a background thread
        let listener_handle = std::thread::spawn(move || {
            listener.run();
        });

        // Create a file in the watched directory
        // FSEvents on macOS needs more setup time than inotify on Linux
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        // Wait for the event (FSEvents on macOS can have higher latency than inotify)
        let event = rx.recv_timeout(Duration::from_secs(10));
        assert!(
            event.is_ok(),
            "Should receive a file change event within 10s"
        );

        let event = event.unwrap();
        match event {
            WatchEvent::FileChanged { project_id, paths } => {
                assert_eq!(project_id, "p1");
                assert!(!paths.is_empty());
            }
            other => {
                // Depending on timing, we might get a GitChanged first if .git exists
                // That's also acceptable
                tracing::debug!("Received: {other:?}");
            }
        }

        daemon.shutdown.store(true, Ordering::Relaxed);
        // Event loop will exit when daemon connection drops
        let _ = listener_handle.join();
    }

    #[test]
    fn event_listener_receives_git_change() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();

        // Create a git repo structure so .git/HEAD changes are classified as GitInternal
        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main").unwrap();

        let (tx, rx) = mpsc::channel();
        let mut listener =
            DaemonEventListener::connect(&format!("127.0.0.1:{}", daemon.port), tx).unwrap();
        listener.watch("p1", dir.path().to_str().unwrap()).unwrap();

        let listener_handle = std::thread::spawn(move || {
            listener.run();
        });

        // Modify a git internal file
        // FSEvents on macOS needs more setup time than inotify on Linux
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature").unwrap();

        // Collect events (FSEvents on macOS can have higher latency than inotify)
        let mut got_git = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(WatchEvent::GitChanged { project_id }) => {
                    assert_eq!(project_id, "p1");
                    got_git = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => continue,
            }
        }
        assert!(got_git, "Should receive a GitChanged event");

        daemon.shutdown.store(true, Ordering::Relaxed);
        let _ = listener_handle.join();
    }

    #[test]
    fn convert_git_changed_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::GIT_CHANGED,
            protocol::GitChangedData {
                path: "/home/user/project".to_string(),
            },
        );
        let result = convert_daemon_event(event, &mapping);
        assert!(matches!(
            result,
            Some(WatchEvent::GitChanged { ref project_id }) if project_id == "proj-1"
        ));
    }

    #[test]
    fn convert_file_changed_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::FILE_CHANGED,
            protocol::FileChangedData {
                path: "/home/user/project".to_string(),
                files: vec!["src/main.rs".to_string()],
            },
        );
        let result = convert_daemon_event(event, &mapping);
        match result {
            Some(WatchEvent::FileChanged { project_id, paths }) => {
                assert_eq!(project_id, "proj-1");
                assert_eq!(paths.len(), 1);
                assert!(paths[0].to_string_lossy().contains("src/main.rs"));
            }
            other => panic!("Expected FileChanged, got: {other:?}"),
        }
    }

    #[test]
    fn convert_session_file_created_event() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent::new(
            protocol::event::SESSION_FILE_CREATED,
            protocol::SessionFileCreatedData {
                path: "/home/user/project".to_string(),
                file: "docs/sessions/session-2026-01-15.md".to_string(),
            },
        );
        let result = convert_daemon_event(event, &mapping);
        match result {
            Some(WatchEvent::SessionFileCreated { project_id, path }) => {
                assert_eq!(project_id, "proj-1");
                assert!(path.to_string_lossy().contains("session-2026-01-15.md"));
            }
            other => panic!("Expected SessionFileCreated, got: {other:?}"),
        }
    }

    #[test]
    fn convert_event_unknown_path_returns_none() {
        let mapping = HashMap::new(); // empty — no projects registered

        let event = protocol::DaemonEvent::new(
            protocol::event::GIT_CHANGED,
            protocol::GitChangedData {
                path: "/unknown/path".to_string(),
            },
        );
        assert!(convert_daemon_event(event, &mapping).is_none());
    }

    #[test]
    fn convert_unknown_event_type_returns_none() {
        let mut mapping = HashMap::new();
        mapping.insert("/home/user/project".to_string(), "proj-1".to_string());

        let event = protocol::DaemonEvent {
            event: "unknown_event".to_string(),
            data: serde_json::json!({"path": "/home/user/project"}),
        };
        assert!(convert_daemon_event(event, &mapping).is_none());
    }
}
