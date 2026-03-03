use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::RecommendedWatcher;

use crate::daemon::protocol::{self, DaemonEvent, DaemonRequest, DaemonResponse};
use crate::provider::local::LocalProvider;

/// Default port for the daemon.
pub const DEFAULT_PORT: u16 = 17233;

/// Maximum allowed length for a single request line (1 MB).
///
/// Normal requests are typically < 10 KB. This limit prevents unbounded
/// memory allocation from malicious or misbehaving clients.
const MAX_REQUEST_LINE_LEN: usize = 1_048_576;

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

/// Run the daemon server. Blocks until `shutdown` is set to true or idle timeout elapses.
pub fn run(config: &DaemonConfig, shutdown: Arc<AtomicBool>) -> std::io::Result<()> {
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

    tracing::info!(port = config.port, "daemon listening");

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                tracing::info!(%addr, "client connected");
                last_activity.store(epoch_secs(), Ordering::Relaxed);
                let shutdown_clone = shutdown.clone();
                let start = start_time;
                let activity = last_activity.clone();
                let token = auth_token.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(
                        stream,
                        start,
                        &shutdown_clone,
                        &activity,
                        token.as_deref(),
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

    tracing::info!("daemon shutting down");
    Ok(())
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
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let writer = Arc::new(Mutex::new(stream));
    let provider = LocalProvider;
    let mut active_watches: HashMap<String, RecommendedWatcher> = HashMap::new();
    let git_debounce: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

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
            &provider,
            start_time,
            &writer,
            &mut active_watches,
            &git_debounce,
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
    drop(active_watches);
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
/// Silently drops the event if the writer lock is poisoned or the write fails
/// (the connection will be cleaned up by the handler thread).
pub(crate) fn push_event(writer: &Arc<Mutex<TcpStream>>, event: &DaemonEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(json.as_bytes());
            let _ = w.write_all(b"\n");
            let _ = w.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

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

    fn start_server(config: DaemonConfig) -> TestServer {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let shutdown = Arc::new(AtomicBool::new(false));
        let port = config.port;
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || run(&config, shutdown_clone));
        let server = TestServer {
            port,
            shutdown,
            _heavy_guard: heavy_guard,
            handle: Some(handle),
        };

        // Poll until the server is accepting connections (up to 2s).
        // A fixed sleep was flaky under parallel test load.
        for _ in 0..40 {
            if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
                return server;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
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
    fn server_wait_session_updates_returns_typed_payload() {
        let server = start_test_server();
        let port = server.port;

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
                timeout_ms: 0,
            },
        );
        let resp = send_request(&mut stream, &mut reader, &req);
        assert!(resp.is_ok(), "response: {:?}", resp);

        let payload: protocol::WaitSessionUpdatesResult =
            serde_json::from_value(resp.result.unwrap()).unwrap();
        assert!(!payload.changed);
        assert!(payload.version <= u64::MAX);

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
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
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
        let handle = std::thread::spawn(move || run(&config, shutdown_clone));

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
}
