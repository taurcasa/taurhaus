use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::daemon::protocol::{self, DaemonRequest, DaemonResponse};
use crate::errors::AppError;
use crate::models::{Commit, FileContent, FileTreeNode, GitStatus};
use crate::provider::path as wsl_path;
use crate::provider::ProjectProvider;

/// Timeout for requests that involve git operations (may be slow).
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for file read operations.
const FILE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for health check pings.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// A ProjectProvider that forwards all operations to the WSL daemon via TCP.
///
/// Automatically translates Windows UNC paths to Linux-native paths before
/// sending requests. Tracks connection state so the provider router can
/// fall back to LocalProvider when the daemon is down.
pub struct DaemonProvider {
    stream: Mutex<Option<TcpStream>>,
    reader: Mutex<Option<BufReader<TcpStream>>>,
    addr: String,
    next_id: AtomicU64,
    connected: AtomicBool,
}

impl DaemonProvider {
    /// Create a new DaemonProvider connected to the given address.
    pub fn connect(addr: &str) -> Result<Self, AppError> {
        let stream = TcpStream::connect(addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to connect to daemon at {addr}: {e}"),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);

        Ok(Self {
            stream: Mutex::new(Some(stream)),
            reader: Mutex::new(Some(reader)),
            addr: addr.to_string(),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(true),
        })
    }

    /// Whether the daemon connection is alive.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// The address this provider connects to.
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Send a ping to the daemon and return Ok if it responds.
    pub fn ping(&self) -> Result<(), AppError> {
        let id = self.next_id();
        let request = DaemonRequest::ping(&id);
        let response = self.send_request(&request, PING_TIMEOUT)?;
        if response.is_ok() {
            Ok(())
        } else {
            Err(AppError::InvalidPath("Daemon ping failed".to_string()))
        }
    }

    /// Send a request for status/admin purposes (e.g., ping from IPC commands).
    ///
    /// Unlike the trait methods, this returns the raw DaemonResponse so callers
    /// can inspect version/uptime without deserializing a specific type.
    pub fn send_status_request(
        &self,
        request: &DaemonRequest,
    ) -> Result<DaemonResponse, AppError> {
        self.send_request(request, PING_TIMEOUT)
    }

    /// Reconnect to the daemon at the stored address.
    ///
    /// Replaces the TCP stream and reader. On success, marks the provider
    /// as connected.
    pub fn reconnect(&self) -> Result<(), AppError> {
        let stream = TcpStream::connect(&self.addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to reconnect to daemon at {}: {e}", self.addr),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);

        // Replace the stream and reader under locks
        if let Ok(mut guard) = self.stream.lock() {
            *guard = Some(stream);
        }
        if let Ok(mut guard) = self.reader.lock() {
            *guard = Some(reader);
        }

        self.connected.store(true, Ordering::Relaxed);
        tracing::debug!(addr = %self.addr, "Daemon reconnected");
        Ok(())
    }

    /// Mark the provider as disconnected (clears the TCP stream).
    fn mark_disconnected(&self) {
        self.connected.store(false, Ordering::Relaxed);
        // Clear the stream/reader so future calls fail fast
        if let Ok(mut guard) = self.stream.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.reader.lock() {
            *guard = None;
        }
    }

    /// Generate a unique request ID.
    fn next_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("r{id}")
    }

    /// Translate a project path to a Linux-native path for the daemon.
    /// If it's already a Linux path, return as-is.
    fn translate_path(path: &str) -> String {
        wsl_path::wsl_unc_to_linux(path).unwrap_or_else(|| path.to_string())
    }

    /// Send a request and receive the response.
    /// On I/O error, marks the provider as disconnected.
    fn send_request(
        &self,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        let result = self.send_request_inner(request, timeout);
        if let Err(AppError::Io(_)) = &result {
            tracing::warn!("Daemon I/O error, marking disconnected");
            self.mark_disconnected();
        }
        result
    }

    fn send_request_inner(
        &self,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        let mut stream_guard = self.stream.lock().map_err(|_| {
            AppError::InvalidPath("Daemon connection lock poisoned".to_string())
        })?;
        let mut reader_guard = self.reader.lock().map_err(|_| {
            AppError::InvalidPath("Daemon reader lock poisoned".to_string())
        })?;

        let stream = stream_guard.as_mut().ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Daemon not connected",
            ))
        })?;
        let reader = reader_guard.as_mut().ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Daemon not connected",
            ))
        })?;

        stream.set_read_timeout(Some(timeout)).map_err(AppError::Io)?;

        let json = serde_json::to_string(request).map_err(|e| {
            AppError::InvalidPath(format!("Failed to serialize request: {e}"))
        })?;
        stream.write_all(json.as_bytes()).map_err(AppError::Io)?;
        stream.write_all(b"\n").map_err(AppError::Io)?;
        stream.flush().map_err(AppError::Io)?;

        let mut line = String::new();
        reader.read_line(&mut line).map_err(AppError::Io)?;

        let response: DaemonResponse = serde_json::from_str(&line).map_err(|e| {
            AppError::InvalidPath(format!("Failed to parse daemon response: {e}"))
        })?;

        Ok(response)
    }

    /// Send a request and extract the result, converting daemon errors to AppError.
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: impl serde::Serialize,
        timeout: Duration,
    ) -> Result<T, AppError> {
        let id = self.next_id();
        let request = DaemonRequest::new(id, method, params);
        let response = self.send_request(&request, timeout)?;

        if let Some(err) = response.error {
            return Err(AppError::InvalidPath(format!(
                "Daemon error [{}]: {}",
                err.code, err.message
            )));
        }

        // Use Value::Null for missing results — allows Option<T> return types to
        // deserialize correctly (e.g. read_readme returning None for no README).
        // Non-nullable types will still produce a deserialization error.
        let result = response.result.unwrap_or(serde_json::Value::Null);

        serde_json::from_value(result).map_err(|e| {
            AppError::InvalidPath(format!("Failed to deserialize daemon result: {e}"))
        })
    }
}

impl ProjectProvider for DaemonProvider {
    fn git_status(&self, project_path: &str) -> Result<GitStatus, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::GIT_STATUS,
            protocol::PathParams { path },
            GIT_TIMEOUT,
        )
    }

    fn recent_commits(&self, project_path: &str, limit: usize) -> Result<Vec<Commit>, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::GIT_LOG,
            protocol::GitLogParams {
                path,
                limit,
                offset: 0,
            },
            GIT_TIMEOUT,
        )
    }

    fn all_commits(
        &self,
        project_path: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Commit>, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::GIT_LOG,
            protocol::GitLogParams {
                path,
                limit,
                offset,
            },
            GIT_TIMEOUT,
        )
    }

    fn latest_commit_time(&self, project_path: &str) -> Result<Option<DateTime<Utc>>, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::LatestCommitTimeResult = self.call(
            protocol::method::GIT_LATEST_COMMIT_TIME,
            protocol::PathParams { path },
            GIT_TIMEOUT,
        )?;
        match result.timestamp {
            Some(ts) => {
                let dt = ts.parse::<DateTime<Utc>>().map_err(|e| {
                    AppError::InvalidPath(format!("Invalid timestamp from daemon: {e}"))
                })?;
                Ok(Some(dt))
            }
            None => Ok(None),
        }
    }

    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::FILE_TREE,
            protocol::PathParams { path },
            FILE_TIMEOUT,
        )
    }

    fn read_file(
        &self,
        project_path: &str,
        relative_path: &str,
    ) -> Result<FileContent, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::READ_FILE,
            protocol::ReadFileParams {
                path,
                relative: relative_path.to_string(),
            },
            FILE_TIMEOUT,
        )
    }

    fn read_readme(&self, project_path: &str) -> Result<Option<FileContent>, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::READ_README,
            protocol::PathParams { path },
            FILE_TIMEOUT,
        )
    }

    fn read_asset(&self, project_path: &str, relative_path: &str) -> Result<Vec<u8>, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::ReadAssetResult = self.call(
            protocol::method::READ_ASSET,
            protocol::ReadFileParams {
                path,
                relative: relative_path.to_string(),
            },
            FILE_TIMEOUT,
        )?;
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &result.data)
            .map_err(|e| AppError::InvalidPath(format!("Failed to decode base64: {e}")))
    }

    fn scan_session_files(&self, project_path: &str) -> Result<Vec<PathBuf>, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::ScanSessionsResult = self.call(
            protocol::method::SCAN_SESSIONS,
            protocol::PathParams { path },
            FILE_TIMEOUT,
        )?;
        Ok(result.paths.into_iter().map(PathBuf::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonConfig;
    use git2::{Repository, Signature};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    /// Start a test daemon server and return port + shutdown handle.
    fn start_daemon() -> (u16, Arc<AtomicBool>) {
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
        };
        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            let _ = crate::daemon::server::run(&config, shutdown_clone);
        });
        std::thread::sleep(Duration::from_millis(100));
        (port, shutdown)
    }

    fn init_test_repo(dir: &std::path::Path) {
        let repo = Repository::init(dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        let sig = Signature::now("Test", "test@test.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();
    }

    #[test]
    fn daemon_provider_ping_via_git_status() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        init_test_repo(dir.path());

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let status = provider.git_status(path).unwrap();

        assert!(status.branch.is_some());
        assert!(!status.is_dirty);

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_recent_commits() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        init_test_repo(dir.path());

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let commits = provider.recent_commits(path, 10).unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Initial commit");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_file_tree() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "world").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let tree = provider.file_tree(path).unwrap();

        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_file() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let content = provider.read_file(path, "main.rs").unwrap();

        assert_eq!(content.content, "fn main() {}");
        assert_eq!(content.language, Some("rust".to_string()));

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_readme() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let readme = provider.read_readme(path).unwrap();

        assert!(readme.is_some());
        assert_eq!(readme.unwrap().content, "# Hello");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_asset() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let data = vec![0x89, 0x50, 0x4e, 0x47]; // PNG magic bytes
        std::fs::write(dir.path().join("icon.png"), &data).unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let bytes = provider.read_asset(path, "icon.png").unwrap();

        assert_eq!(bytes, data);

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_scan_sessions() {
        let (port, shutdown) = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let handoffs = dir.path().join(".claude").join("handoffs");
        std::fs::create_dir_all(&handoffs).unwrap();
        std::fs::write(handoffs.join("session.md"), "# Session").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let path = dir.path().to_str().unwrap();
        let files = provider.scan_session_files(path).unwrap();

        assert_eq!(files.len(), 1);

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_handles_errors() {
        let (port, shutdown) = start_daemon();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        let result = provider.git_status("/nonexistent/path");

        assert!(result.is_err());

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_translates_wsl_paths() {
        // Static method test — no daemon needed
        let linux = DaemonProvider::translate_path(r"\\wsl$\Ubuntu\home\user\projects");
        assert_eq!(linux, "/home/user/projects");

        let local = DaemonProvider::translate_path("/home/user/projects");
        assert_eq!(local, "/home/user/projects");
    }

    #[test]
    fn connect_to_nonexistent_daemon_fails() {
        let result = DaemonProvider::connect("127.0.0.1:1");
        assert!(result.is_err());
    }

    #[test]
    fn is_connected_true_initially() {
        let (port, shutdown) = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        assert!(provider.is_connected());
        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn marks_disconnected_on_daemon_crash() {
        let (port, shutdown) = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();

        // Verify connection works
        assert!(provider.ping().is_ok());
        assert!(provider.is_connected());

        // Kill the daemon — handler has 1s read timeout before noticing shutdown
        shutdown.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(1500));

        // The first ping after shutdown may still succeed (handler processes it
        // before exiting). Send up to two pings to ensure disconnection.
        let _ = provider.ping();
        if provider.is_connected() {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = provider.ping();
        }
        assert!(!provider.is_connected());
    }

    #[test]
    fn reconnect_after_daemon_restart() {
        let (port, shutdown) = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{port}")).unwrap();
        assert!(provider.ping().is_ok());

        // Kill daemon — wait for handler to exit
        shutdown.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(1500));
        let _ = provider.ping();
        if provider.is_connected() {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = provider.ping();
        }
        assert!(!provider.is_connected());

        // Start a new daemon on the same port
        let shutdown2 = Arc::new(AtomicBool::new(false));
        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
        };
        let shutdown2_clone = shutdown2.clone();
        std::thread::spawn(move || {
            let _ = crate::daemon::server::run(&config, shutdown2_clone);
        });
        std::thread::sleep(Duration::from_millis(200));

        // Reconnect should succeed
        assert!(provider.reconnect().is_ok());
        assert!(provider.is_connected());
        assert!(provider.ping().is_ok());

        shutdown2.store(true, Ordering::Relaxed);
    }
}
