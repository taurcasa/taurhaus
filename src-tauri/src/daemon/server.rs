use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Config, Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::daemon::protocol::{self, DaemonEvent, DaemonRequest, DaemonResponse};
use crate::fs::watcher::{classify_event, EventClass};
use crate::provider::local::LocalProvider;
use crate::provider::ProjectProvider;

/// Default port for the daemon.
pub const DEFAULT_PORT: u16 = 17233;

/// Configuration for the daemon server.
pub struct DaemonConfig {
    pub port: u16,
    pub bind_addr: String,
    /// Auto-shutdown after this many seconds with no client activity.
    /// `None` disables idle timeout.
    pub idle_timeout_secs: Option<u64>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
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

/// Run the daemon server. Blocks until `shutdown` is set to true or idle timeout elapses.
pub fn run(config: &DaemonConfig, shutdown: Arc<AtomicBool>) -> std::io::Result<()> {
    let listener = TcpListener::bind(format!("{}:{}", config.bind_addr, config.port))?;
    listener.set_nonblocking(true)?;

    let start_time = Instant::now();
    let last_activity = Arc::new(AtomicU64::new(epoch_secs()));

    tracing::info!(port = config.port, "daemon listening");

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                tracing::info!(%addr, "client connected");
                last_activity.store(epoch_secs(), Ordering::Relaxed);
                let shutdown_clone = shutdown.clone();
                let start = start_time;
                let activity = last_activity.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, start, &shutdown_clone, &activity) {
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

    tracing::info!("daemon shutting down");
    Ok(())
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
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let reader = BufReader::new(stream.try_clone()?);
    let writer = Arc::new(Mutex::new(stream));
    let provider = LocalProvider;
    let mut active_watches: HashMap<String, RecommendedWatcher> = HashMap::new();
    let git_debounce: Arc<Mutex<HashMap<String, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for line_result in reader.lines() {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
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

        let response = match request.method.as_str() {
            protocol::method::WATCH => {
                handle_watch(
                    &request.id,
                    &request.params,
                    &writer,
                    &mut active_watches,
                    &git_debounce,
                )
            }
            protocol::method::UNWATCH => {
                handle_unwatch(&request.id, &request.params, &mut active_watches)
            }
            _ => dispatch(&request, &provider, start_time),
        };

        write_locked(&writer, &response)?;
        last_activity.store(epoch_secs(), Ordering::Relaxed);

        // Handle shutdown method
        if request.method == protocol::method::SHUTDOWN {
            shutdown.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Drop watches before writer — stops watcher callbacks before closing stream
    drop(active_watches);
    Ok(())
}

/// Write a NDJSON response through a shared (locked) writer.
fn write_locked(
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
    let json = serde_json::to_string(response).map_err(|e| {
        std::io::Error::other(e)
    })?;
    writer.write_all(json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Push a DaemonEvent to a client through a shared writer.
///
/// Silently drops the event if the writer lock is poisoned or the write fails
/// (the connection will be cleaned up by the handler thread).
fn push_event(writer: &Arc<Mutex<TcpStream>>, event: &DaemonEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(json.as_bytes());
            let _ = w.write_all(b"\n");
            let _ = w.flush();
        }
    }
}

/// Dispatch a request to the appropriate handler.
fn dispatch(
    request: &DaemonRequest,
    provider: &LocalProvider,
    start_time: Instant,
) -> DaemonResponse {
    tracing::info!(method = %request.method, id = %request.id, "Received request");
    match request.method.as_str() {
        protocol::method::PING => handle_ping(&request.id, start_time),
        protocol::method::GIT_STATUS => handle_git_status(&request.id, &request.params, provider),
        protocol::method::GIT_LOG => handle_git_log(&request.id, &request.params, provider),
        protocol::method::GIT_LATEST_COMMIT_TIME => {
            handle_git_latest_commit_time(&request.id, &request.params, provider)
        }
        protocol::method::FILE_TREE => handle_file_tree(&request.id, &request.params, provider),
        protocol::method::READ_FILE => handle_read_file(&request.id, &request.params, provider),
        protocol::method::READ_README => handle_read_readme(&request.id, &request.params, provider),
        protocol::method::READ_ASSET => handle_read_asset(&request.id, &request.params, provider),
        protocol::method::SCAN_SESSIONS => {
            handle_scan_sessions(&request.id, &request.params, provider)
        }
        protocol::method::LIST_CLAUDE_SESSIONS => {
            handle_list_claude_sessions(&request.id)
        }
        protocol::method::LAUNCH_SESSION => {
            handle_launch_session(&request.id, &request.params)
        }
        protocol::method::STOP_SESSION => {
            handle_stop_session(&request.id, &request.params)
        }
        protocol::method::NAVIGATE_TO_SESSION => {
            handle_navigate_to_session(&request.id, &request.params)
        }
        protocol::method::SHUTDOWN => {
            DaemonResponse::ok(&request.id, serde_json::json!({"ok": true}))
        }
        _ => DaemonResponse::err(
            &request.id,
            "UNKNOWN_METHOD",
            format!("Unknown method: {}", request.method),
        ),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn handle_ping(id: &str, start_time: Instant) -> DaemonResponse {
    DaemonResponse::ok(
        id,
        protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: start_time.elapsed().as_secs(),
        },
    )
}

fn handle_git_status(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.git_status(&params.path) {
        Ok(status) => DaemonResponse::ok(id, status),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

fn handle_git_log(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::GitLogParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.all_commits(&params.path, params.limit, params.offset) {
        Ok(commits) => DaemonResponse::ok(id, commits),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

fn handle_git_latest_commit_time(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.latest_commit_time(&params.path) {
        Ok(time) => DaemonResponse::ok(
            id,
            protocol::LatestCommitTimeResult {
                timestamp: time.map(|t| t.to_rfc3339()),
            },
        ),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

fn handle_file_tree(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.file_tree(&params.path) {
        Ok(tree) => DaemonResponse::ok(id, tree),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

fn handle_read_file(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_file(&params.path, &params.relative) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

fn handle_read_readme(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_readme(&params.path) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

fn handle_read_asset(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_asset(&params.path, &params.relative) {
        Ok(bytes) => {
            let data = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &bytes,
            );
            DaemonResponse::ok(id, protocol::ReadAssetResult { data })
        }
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

fn handle_scan_sessions(
    id: &str,
    params: &serde_json::Value,
    provider: &LocalProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.scan_session_files(&params.path) {
        Ok(paths) => DaemonResponse::ok(
            id,
            protocol::ScanSessionsResult {
                paths: paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            },
        ),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Command Center — session management handlers
// ---------------------------------------------------------------------------

fn handle_list_claude_sessions(id: &str) -> DaemonResponse {
    let sessions = crate::session_scanner::scan_sessions();
    DaemonResponse::ok(id, sessions)
}

fn handle_launch_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::LaunchSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::launch_in_tmux(&params.project_path, params.mode) {
        Ok((window, pane)) => DaemonResponse::ok(
            id,
            protocol::LaunchSessionResult {
                tmux_window: window,
                tmux_pane: pane,
            },
        ),
        Err(e) => DaemonResponse::err(id, "LAUNCH_ERROR", e),
    }
}

fn handle_stop_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::StopSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::stop_session(&params.tmux_pane) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "STOP_ERROR", e),
    }
}

fn handle_navigate_to_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::NavigateToSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::navigate_to_pane(
        &params.tmux_session,
        &params.tmux_window,
        &params.tmux_pane,
    ) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "NAVIGATE_ERROR", e),
    }
}

// ---------------------------------------------------------------------------
// Watch / Unwatch handlers
// ---------------------------------------------------------------------------

/// Duration to debounce git internal events pushed to clients.
const WATCH_GIT_DEBOUNCE_SECS: u64 = 2;

/// Handle a `watch` request: start an inotify/notify watcher for the path
/// and push classified events to the client as DaemonEvents.
fn handle_watch(
    id: &str,
    params: &serde_json::Value,
    writer: &Arc<Mutex<TcpStream>>,
    active_watches: &mut HashMap<String, RecommendedWatcher>,
    git_debounce: &Arc<Mutex<HashMap<String, Instant>>>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let path = PathBuf::from(&params.path);
    if !path.is_dir() {
        return DaemonResponse::err(
            id,
            "NOT_FOUND",
            format!("Path does not exist or is not a directory: {}", params.path),
        );
    }

    // Already watching this path?
    if active_watches.contains_key(&params.path) {
        return DaemonResponse::ok(id, protocol::WatchResult { ok: true });
    }

    let writer_clone = writer.clone();
    let watch_path = params.path.clone();
    let debounce_clone = git_debounce.clone();

    let watcher_result = RecommendedWatcher::new(
        move |res: Result<NotifyEvent, notify::Error>| {
            if let Ok(event) = res {
                forward_watch_event(&writer_clone, &watch_path, &debounce_clone, event);
            }
        },
        Config::default(),
    );

    let mut watcher = match watcher_result {
        Ok(w) => w,
        Err(e) => return DaemonResponse::err(id, "WATCH_ERROR", e.to_string()),
    };

    if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
        return DaemonResponse::err(id, "WATCH_ERROR", e.to_string());
    }

    tracing::info!(path = %params.path, "Started watching directory");
    active_watches.insert(params.path, watcher);
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Handle an `unwatch` request: stop watching the specified path.
fn handle_unwatch(
    id: &str,
    params: &serde_json::Value,
    active_watches: &mut HashMap<String, RecommendedWatcher>,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    if active_watches.remove(&params.path).is_some() {
        tracing::info!(path = %params.path, "Stopped watching directory");
    }
    DaemonResponse::ok(id, protocol::WatchResult { ok: true })
}

/// Classify a notify event and push the appropriate DaemonEvent to the client.
///
/// Uses the same `classify_event` logic as the local `ProjectWatcher` to ensure
/// consistent event classification between local and daemon-forwarded watching.
fn forward_watch_event(
    writer: &Arc<Mutex<TcpStream>>,
    project_path: &str,
    debounce: &Arc<Mutex<HashMap<String, Instant>>>,
    event: NotifyEvent,
) {
    // Only care about create, modify, remove events
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return,
    }

    let project_root = Path::new(project_path);
    let mut regular_files = Vec::new();

    for path in &event.paths {
        let Some(class) = classify_event(project_root, path) else {
            continue;
        };

        match class {
            EventClass::GitInternal => {
                // Debounce: only emit if enough time has passed
                if let Ok(mut state) = debounce.lock() {
                    let now = Instant::now();
                    let should_emit = state
                        .get(project_path)
                        .is_none_or(|last| {
                            now.duration_since(*last)
                                >= Duration::from_secs(WATCH_GIT_DEBOUNCE_SECS)
                        });

                    if should_emit {
                        state.insert(project_path.to_string(), now);
                        push_event(
                            writer,
                            &DaemonEvent::new(
                                protocol::event::GIT_CHANGED,
                                protocol::GitChangedData {
                                    path: project_path.to_string(),
                                },
                            ),
                        );
                    }
                }
            }
            EventClass::SessionFile => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    let relative = path
                        .strip_prefix(project_root)
                        .map(|r| r.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string());
                    push_event(
                        writer,
                        &DaemonEvent::new(
                            protocol::event::SESSION_FILE_CREATED,
                            protocol::SessionFileCreatedData {
                                path: project_path.to_string(),
                                file: relative,
                            },
                        ),
                    );
                }
            }
            EventClass::GitignoreChange => {
                // Gitignore changes don't have a dedicated daemon event type.
                // They're treated as regular file changes for now.
                regular_files.push(
                    path.strip_prefix(project_root)
                        .map(|r| r.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string()),
                );
            }
            EventClass::RegularFile => {
                regular_files.push(
                    path.strip_prefix(project_root)
                        .map(|r| r.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string()),
                );
            }
        }
    }

    if !regular_files.is_empty() {
        push_event(
            writer,
            &DaemonEvent::new(
                protocol::event::FILE_CHANGED,
                protocol::FileChangedData {
                    path: project_path.to_string(),
                    files: regular_files,
                },
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    fn start_test_server() -> (u16, Arc<AtomicBool>) {
        let shutdown = Arc::new(AtomicBool::new(false));
        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
        };
        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            let _ = run(&config, shutdown_clone);
        });

        // Give the server a moment to start
        std::thread::sleep(std::time::Duration::from_millis(100));
        (port, shutdown)
    }

    fn send_request(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>, req: &DaemonRequest) -> DaemonResponse {
        let json = serde_json::to_string(req).unwrap();
        stream.write_all(json.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    #[test]
    fn server_responds_to_ping() {
        let (port, shutdown) = start_test_server();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::ping("r1");
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(resp.is_ok());
        assert_eq!(resp.id, "r1");

        let result: protocol::PingResult =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert_eq!(result.version, env!("CARGO_PKG_VERSION"));

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_returns_error_for_unknown_method() {
        let (port, shutdown) = start_test_server();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new("r1", "nonexistent_method", serde_json::Value::Null);
        let resp = send_request(&mut stream, &mut reader, &req);

        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "UNKNOWN_METHOD");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_git_status_on_test_repo() {
        let (port, shutdown) = start_test_server();

        // Create a test repo
        let dir = tempfile::TempDir::new().unwrap();
        let _repo = git2::Repository::init(dir.path()).unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
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

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_file_tree() {
        let (port, shutdown) = start_test_server();

        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "world").unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
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

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_read_file() {
        let (port, shutdown) = start_test_server();

        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
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

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_multiple_requests_on_same_connection() {
        let (port, shutdown) = start_test_server();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send two pings
        let r1 = send_request(&mut stream, &mut reader, &DaemonRequest::ping("p1"));
        assert!(r1.is_ok());
        assert_eq!(r1.id, "p1");

        let r2 = send_request(&mut stream, &mut reader, &DaemonRequest::ping("p2"));
        assert!(r2.is_ok());
        assert_eq!(r2.id, "p2");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_handles_malformed_json() {
        let (port, shutdown) = start_test_server();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        // Send malformed JSON
        stream.write_all(b"not valid json\n").unwrap();
        stream.flush().unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let resp: DaemonResponse = serde_json::from_str(&line).unwrap();
        assert!(!resp.is_ok());
        assert_eq!(resp.error.unwrap().code, "PARSE_ERROR");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn server_shutdown_method() {
        let (port, shutdown) = start_test_server();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let req = DaemonRequest::new("s1", protocol::method::SHUTDOWN, serde_json::Value::Null);
        let resp = send_request(&mut stream, &mut reader, &req);
        assert!(resp.is_ok());

        // The shutdown flag should be set
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn server_idle_timeout_shuts_down() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: Some(1), // 1 second timeout
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            run(&config, shutdown_clone)
        });

        // Wait for the server to start, then let it idle for >1s
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Verify server is running (can connect)
        let _stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        drop(_stream);

        // Wait for idle timeout to trigger
        std::thread::sleep(std::time::Duration::from_millis(1500));

        // Server thread should have exited
        let result = handle.join().unwrap();
        assert!(result.is_ok());
    }
}
