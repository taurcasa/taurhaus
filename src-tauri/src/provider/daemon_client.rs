use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::daemon_api::protocol::{self, DaemonRequest, DaemonResponse};
use crate::daemon_api::{emit_daemon_connection_event, is_timeout_transport_error, DaemonRpcSpan};
use crate::errors::AppError;
use crate::models::{
    Commit, CommitFile, DiffHunk, FileContent, FileTreeNode, GitRangeResult, GitStatus,
};
use crate::project_provider::ProjectProvider;
use crate::provider::path as wsl_path;

/// Timeout for requests that involve git operations (may be slow).
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for file read operations.
const FILE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for health check pings.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for establishing a fresh daemon TCP connection on localhost.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(350);

/// Minimum interval between inline reconnection attempts.
const RECONNECT_COOLDOWN: Duration = Duration::from_secs(5);

/// A ProjectProvider that forwards all operations to the WSL daemon via TCP.
///
/// Automatically translates Windows UNC paths to Linux-native paths before
/// sending requests. Tracks connection state so the provider router can
/// fall back to LocalProvider when the daemon is down.
/// A connected TCP stream paired with its buffered reader.
struct Connection {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

/// Number of pooled TCP connections to the daemon.
///
/// Eliminates "connection busy" contention during startup bursts where
/// multiple IPC commands fire in parallel. Slot 0 is connected eagerly;
/// slots 1..N connect lazily on first use.
const POOL_SIZE: usize = 3;

#[cfg(test)]
pub(crate) fn status_pool_size_for_tests() -> usize {
    POOL_SIZE
}

pub struct DaemonProvider {
    pool: Vec<Mutex<Option<Connection>>>,
    addr: String,
    wsl_distro: Option<String>,
    next_id: AtomicU64,
    connected: AtomicBool,
    /// Timestamp of the last inline reconnection attempt (for rate limiting).
    last_reconnect_attempt: Mutex<Option<Instant>>,
    /// Auth token read from the daemon's token file.
    /// Wrapped in Mutex so `reconnect()` can refresh it when the daemon restarts.
    auth_token: Mutex<Option<String>>,
}

impl DaemonProvider {
    /// Read the daemon's auth token from the well-known file path.
    /// Falls back to reading via WSL on Windows.
    fn read_auth_token_for_distro(wsl_distro: Option<&str>) -> Option<String> {
        crate::daemon_api::read_auth_token_for_distro(wsl_distro)
    }

    fn connect_stream(addr: &str) -> Result<TcpStream, AppError> {
        let resolved_addr = addr.to_socket_addrs().map_err(|error| {
            AppError::DaemonTransport(format!("Failed to resolve daemon address {addr}: {error}"))
        })?;
        let mut last_error = None;
        let mut attempted = false;
        for socket_addr in resolved_addr {
            attempted = true;
            match TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT) {
                Ok(stream) => {
                    stream.set_nodelay(true).map_err(|error| {
                        AppError::DaemonTransport(format!(
                            "Failed to configure daemon TCP_NODELAY at {addr}: {error}"
                        ))
                    })?;
                    return Ok(stream);
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }

        let error = last_error.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                format!("no socket addresses resolved for {addr}"),
            )
        });
        if !attempted {
            return Err(AppError::DaemonTransport(format!(
                "Failed to connect to daemon at {addr}: {error}"
            )));
        }

        Err(AppError::DaemonTransport(format!(
            "Failed to connect to daemon at {addr} within {} ms: {error}",
            CONNECT_TIMEOUT.as_millis()
        )))
    }

    /// Create a new DaemonProvider connected to the given address.
    pub fn connect(addr: &str) -> Result<Self, AppError> {
        Self::connect_with_distro(addr, None)
    }

    pub fn connect_with_distro(addr: &str, wsl_distro: Option<&str>) -> Result<Self, AppError> {
        let connect_started_at = Instant::now();
        let stream = Self::connect_stream(addr)?;
        let reader = BufReader::new(stream.try_clone().map_err(|error| {
            AppError::DaemonTransport(format!("Failed to clone daemon stream at {addr}: {error}"))
        })?);

        let mut pool = Vec::with_capacity(POOL_SIZE);
        pool.push(Mutex::new(Some(Connection { stream, reader })));
        for _ in 1..POOL_SIZE {
            pool.push(Mutex::new(None));
        }

        let provider = Self {
            pool,
            addr: addr.to_string(),
            wsl_distro: wsl_distro.map(ToOwned::to_owned),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(true),
            last_reconnect_attempt: Mutex::new(None),
            auth_token: Mutex::new(None),
        };
        emit_daemon_connection_event(
            "info",
            "daemon.connection.established",
            addr,
            Some("initial_connect"),
            Some(connect_started_at.elapsed().as_millis() as u64),
        );
        Ok(provider)
    }

    /// Create a DaemonProvider that is initially disconnected.
    ///
    /// Useful when the daemon isn't available at startup — the health check
    /// can call `reconnect()` later when the daemon becomes reachable.
    pub fn new_disconnected(addr: &str) -> Self {
        Self::new_disconnected_with_distro(addr, None)
    }

    pub fn new_disconnected_with_distro(addr: &str, wsl_distro: Option<&str>) -> Self {
        let mut pool = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            pool.push(Mutex::new(None));
        }
        Self {
            pool,
            addr: addr.to_string(),
            wsl_distro: wsl_distro.map(ToOwned::to_owned),
            next_id: AtomicU64::new(1),
            connected: AtomicBool::new(false),
            last_reconnect_attempt: Mutex::new(None),
            auth_token: Mutex::new(None),
        }
    }

    /// Whether the daemon connection is alive.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Whether all pooled daemon connections are currently occupied.
    ///
    /// This is a local provider-state probe only. Busy connections are still
    /// healthy connected transports and should not be conflated with disconnect.
    pub fn is_busy(&self) -> bool {
        if !self.is_connected() {
            return false;
        }

        for slot in &self.pool {
            match slot.try_lock() {
                Ok(_) => return false,
                Err(TryLockError::WouldBlock) => continue,
                Err(TryLockError::Poisoned(_)) => return false,
            }
        }
        true
    }

    /// The address this provider connects to.
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Send a ping to the daemon and return Ok if it responds.
    pub fn ping(&self) -> Result<(), AppError> {
        self.ping_info_with_timeout(PING_TIMEOUT).map(|_| ())
    }

    /// Ping the daemon and return its protocol version (0 if old daemon).
    pub fn ping_protocol_version(&self) -> Result<u32, AppError> {
        self.ping_info_with_timeout(PING_TIMEOUT)
            .map(|info| info.protocol_version)
    }

    pub fn ping_info_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<protocol::PingResult, AppError> {
        let id = self.next_id();
        let request = DaemonRequest::ping(&id);
        let response = self.send_request(&request, timeout)?;
        if !response.is_ok() {
            return Err(AppError::DaemonProtocol("Daemon ping failed".to_string()));
        }
        response
            .result
            .and_then(|value| serde_json::from_value::<protocol::PingResult>(value).ok())
            .ok_or_else(|| {
                AppError::DaemonProtocol("Daemon ping payload decode failed".to_string())
            })
    }

    /// Send a request for status/admin purposes (e.g., ping from IPC commands).
    ///
    /// Unlike the trait methods, this returns the raw DaemonResponse so callers
    /// can inspect version/uptime without deserializing a specific type.
    ///
    /// Tries each pool slot without blocking. If all slots are busy, blocks on
    /// slot 0 as a last resort (bounded by the request timeout).
    pub fn send_status_request(&self, request: &DaemonRequest) -> Result<DaemonResponse, AppError> {
        let rpc_span = DaemonRpcSpan::start(request, 0);
        let result = self.send_with_pool(request, PING_TIMEOUT);
        match &result {
            Ok(response) => {
                if let Some(error) = response.error.as_ref() {
                    rpc_span.failed(&error.code, &error.message);
                } else {
                    rpc_span.response("ok");
                }
            }
            Err(AppError::DaemonTransport(message)) if is_timeout_transport_error(message) => {
                rpc_span.timeout();
            }
            Err(AppError::DaemonProtocol(message)) => {
                rpc_span.failed("DAEMON_PROTOCOL_ERROR", message);
            }
            Err(AppError::DaemonTransport(message)) => {
                rpc_span.failed("DAEMON_TRANSPORT_ERROR", message);
            }
            Err(_) => {
                rpc_span.failed("DAEMON_RPC_ERROR", "daemon rpc call failed");
            }
        }
        if let Err(AppError::DaemonTransport(message)) = &result {
            if !crate::daemon_api::is_busy_transport_error(message) {
                tracing::warn!("Daemon I/O error, marking disconnected");
                self.mark_disconnected(message);
            }
        }
        result
    }

    /// Lazily establish a TCP connection in an empty pool slot.
    fn lazy_connect(&self, slot: &mut Option<Connection>) -> Result<(), AppError> {
        if slot.is_some() {
            return Ok(());
        }
        let stream = Self::connect_stream(&self.addr)?;
        let reader =
            BufReader::new(stream.try_clone().map_err(|e| {
                AppError::DaemonTransport(format!("Failed to clone pool stream: {e}"))
            })?);
        *slot = Some(Connection { stream, reader });
        Ok(())
    }

    /// Try each pool slot without blocking, lazily connecting empty slots.
    /// If all slots are busy, block on slot 0 as a last resort.
    fn send_with_pool(
        &self,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        // Try each slot without blocking
        for slot in &self.pool {
            match slot.try_lock() {
                Ok(mut guard) => {
                    if guard.is_none() && self.is_connected() {
                        if let Err(e) = self.lazy_connect(&mut guard) {
                            tracing::debug!(error = %e, "Lazy pool slot connection failed, trying next");
                            continue;
                        }
                    }
                    return self.send_request_with_guard(&mut guard, request, timeout);
                }
                Err(TryLockError::WouldBlock) => continue,
                Err(TryLockError::Poisoned(_)) => continue,
            }
        }

        // All slots busy — block on slot 0
        let mut guard = self.pool[0]
            .lock()
            .map_err(|_| AppError::DaemonTransport("Pool slot 0 lock poisoned".to_string()))?;
        if guard.is_none() && self.is_connected() {
            self.lazy_connect(&mut guard)?;
        }
        self.send_request_with_guard(&mut guard, request, timeout)
    }

    /// Reconnect to the daemon at the stored address.
    ///
    /// Replaces the TCP stream and reader. On success, marks the provider
    /// as connected.
    pub fn reconnect(&self) -> Result<(), AppError> {
        let reconnect_started_at = Instant::now();
        emit_daemon_connection_event(
            "info",
            "daemon.connection.reconnecting",
            &self.addr,
            Some("reconnect_requested"),
            None,
        );
        let stream = Self::connect_stream(&self.addr)?;
        let reader = BufReader::new(stream.try_clone().map_err(|error| {
            AppError::DaemonTransport(format!(
                "Failed to clone daemon stream while reconnecting at {}: {error}",
                self.addr
            ))
        })?);

        // Reconnect slot 0 (primary); clear secondary slots for lazy reconnect.
        match self.pool[0].lock() {
            Ok(mut guard) => {
                *guard = Some(Connection { stream, reader });
                self.connected.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::warn!(error = %e, "daemon reconnect: pool slot 0 mutex poisoned, connection not stored");
            }
        }
        for slot in &self.pool[1..] {
            if let Ok(mut guard) = slot.lock() {
                *guard = None;
            }
        }

        // Clear cached auth token — the daemon may have restarted with a new one,
        // and the next request should reload it lazily.
        match self.auth_token.lock() {
            Ok(mut guard) => {
                *guard = None;
            }
            Err(e) => {
                tracing::warn!(error = %e, "daemon reconnect: auth_token mutex poisoned");
            }
        }

        tracing::debug!(addr = %self.addr, "Daemon reconnected");
        emit_daemon_connection_event(
            "info",
            "daemon.connection.established",
            &self.addr,
            Some("reconnect"),
            Some(reconnect_started_at.elapsed().as_millis() as u64),
        );
        Ok(())
    }

    /// Reconnect and confirm the daemon speaks this app's exact protocol.
    ///
    /// Reachability is not adoption. Since PR 8 the hub's versioned snapshot is
    /// the app's only tmux-focus transport, and a daemon from before it answers
    /// TCP perfectly well while omitting the focus fields — they decode as
    /// `None`, so the foreground indicator simply never lights. Startup gates on
    /// the protocol; every reconnect after startup has to gate on the same
    /// thing, or a daemon started by hand walks straight around it.
    ///
    /// A mismatch disconnects immediately, which is what the health monitor's
    /// restart path is waiting to see.
    pub fn reconnect_checked(&self) -> Result<(), AppError> {
        self.reconnect()?;

        let version = match self.ping_protocol_version() {
            Ok(version) => version,
            Err(error) => {
                self.mark_disconnected("reconnect_ping_failed");
                return Err(error);
            }
        };

        let expected = protocol::PROTOCOL_VERSION;
        if version != expected {
            self.mark_disconnected("reconnect_protocol_mismatch");
            return Err(AppError::DaemonProtocol(format!(
                "daemon protocol mismatch after reconnect at {}: running={version}, expected={expected}",
                self.addr
            )));
        }

        Ok(())
    }

    /// Attempt to reconnect, but only if the cooldown period has elapsed.
    ///
    /// Returns `true` if reconnection succeeded, `false` if skipped (too soon),
    /// failed, or reached a daemon speaking another protocol. Safe to call on
    /// every poll — the rate limiter prevents thundering-herd reconnection
    /// attempts.
    pub fn try_reconnect(&self) -> bool {
        // Rate-limit: skip if we attempted recently
        {
            let guard = self
                .last_reconnect_attempt
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(last) = *guard {
                if last.elapsed() < RECONNECT_COOLDOWN {
                    return false;
                }
            }
        }

        // Record this attempt
        {
            let mut guard = self
                .last_reconnect_attempt
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *guard = Some(Instant::now());
        }

        match self.reconnect_checked() {
            Ok(()) => {
                tracing::info!(addr = %self.addr, "Inline reconnect succeeded");
                true
            }
            Err(e) => {
                tracing::debug!(addr = %self.addr, error = %e, "Inline reconnect failed");
                false
            }
        }
    }

    /// Mark the provider as disconnected (clears all pooled connections).
    fn mark_disconnected(&self, reason: &str) {
        self.connected.store(false, Ordering::Relaxed);
        for slot in &self.pool {
            let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
            *guard = None;
        }
        emit_daemon_connection_event(
            "warn",
            "daemon.connection.lost",
            &self.addr,
            Some(reason),
            None,
        );
    }

    /// Public disconnect hook for lifecycle management paths that need to
    /// drop the current daemon connection before forcing a restart.
    pub fn disconnect(&self, reason: &str) {
        self.mark_disconnected(reason);
    }

    /// Generate a unique request ID.
    fn next_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("r{id}")
    }

    /// Translate a project path to a Linux-native path for the daemon.
    /// Handles WSL UNC paths (`\\wsl$\...`) and Windows drive paths (`D:\...`).
    /// If it's already a Linux path, return as-is.
    fn translate_path(path: &str) -> String {
        wsl_path::to_linux(path).unwrap_or_else(|| path.to_string())
    }

    /// Send a request and receive the response.
    /// On I/O error, marks the provider as disconnected.
    fn send_request(
        &self,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        let rpc_span = DaemonRpcSpan::start(request, 0);
        let result = self.send_request_inner(request, timeout);
        match &result {
            Ok(response) => {
                if let Some(error) = response.error.as_ref() {
                    rpc_span.failed(&error.code, &error.message);
                } else {
                    rpc_span.response("ok");
                }
            }
            Err(AppError::DaemonTransport(message)) if is_timeout_transport_error(message) => {
                rpc_span.timeout();
            }
            Err(AppError::DaemonProtocol(message)) => {
                rpc_span.failed("DAEMON_PROTOCOL_ERROR", message);
            }
            Err(AppError::DaemonTransport(message)) => {
                rpc_span.failed("DAEMON_TRANSPORT_ERROR", message);
            }
            Err(_) => {
                rpc_span.failed("DAEMON_RPC_ERROR", "daemon rpc call failed");
            }
        }
        if let Err(AppError::DaemonTransport(message)) = &result {
            tracing::warn!("Daemon I/O error, marking disconnected");
            self.mark_disconnected(message);
        }
        result
    }

    fn send_request_inner(
        &self,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        self.send_with_pool(request, timeout)
    }

    fn send_request_with_guard(
        &self,
        conn_guard: &mut Option<Connection>,
        request: &DaemonRequest,
        timeout: Duration,
    ) -> Result<DaemonResponse, AppError> {
        let conn = conn_guard
            .as_mut()
            .ok_or_else(|| AppError::DaemonTransport("Daemon not connected".to_string()))?;

        let stream = &mut conn.stream;
        let reader = &mut conn.reader;

        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            AppError::DaemonTransport(format!(
                "Failed to set daemon read timeout ({}s): {error}",
                timeout.as_secs()
            ))
        })?;

        let auth = self.load_auth_token_if_missing();
        let response = self.send_request_once(stream, reader, request, timeout, auth.clone())?;
        if !is_auth_failed_response(&response) {
            return Ok(response);
        }

        self.clear_auth_token_cache();
        let refreshed_auth = self.load_auth_token_if_missing();
        if refreshed_auth == auth {
            return Ok(response);
        }

        self.send_request_once(stream, reader, request, timeout, refreshed_auth)
    }

    fn load_auth_token_if_missing(&self) -> Option<String> {
        let mut guard = self.auth_token.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Self::read_auth_token_for_distro(self.wsl_distro.as_deref());
        }
        guard.clone()
    }

    fn clear_auth_token_cache(&self) {
        let mut guard = self.auth_token.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }

    fn send_request_once(
        &self,
        stream: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        request: &DaemonRequest,
        timeout: Duration,
        auth: Option<String>,
    ) -> Result<DaemonResponse, AppError> {
        let mut authed_request = request.clone();
        authed_request.auth = auth;

        let json = serde_json::to_string(&authed_request)
            .map_err(|e| AppError::DaemonProtocol(format!("Failed to serialize request: {e}")))?;
        stream.write_all(json.as_bytes()).map_err(|error| {
            AppError::DaemonTransport(format!("Failed to write daemon request: {error}"))
        })?;
        stream.write_all(b"\n").map_err(|error| {
            AppError::DaemonTransport(format!("Failed to terminate daemon request line: {error}"))
        })?;
        stream.flush().map_err(|error| {
            AppError::DaemonTransport(format!("Failed to flush daemon request: {error}"))
        })?;

        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                    AppError::DaemonTransport(format!(
                        "Daemon request timed out after {}s: {error}",
                        timeout.as_secs()
                    ))
                }
                _ => AppError::DaemonTransport(format!("Failed to read daemon response: {error}")),
            })?;

        serde_json::from_str(&line)
            .map_err(|e| AppError::DaemonProtocol(format!("Failed to parse daemon response: {e}")))
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
            return Err(AppError::DaemonProtocol(format!(
                "Daemon error [{}]: {}",
                err.code, err.message
            )));
        }

        // Use Value::Null for missing results — allows Option<T> return types to
        // deserialize correctly (e.g. read_readme returning None for no README).
        // Non-nullable types will still produce a deserialization error.
        let result = response.result.unwrap_or(serde_json::Value::Null);

        serde_json::from_value(result).map_err(|e| {
            AppError::DaemonProtocol(format!("Failed to deserialize daemon result: {e}"))
        })
    }
}

fn is_auth_failed_response(response: &DaemonResponse) -> bool {
    response
        .error
        .as_ref()
        .is_some_and(|error| error.code == "AUTH_FAILED")
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
                    AppError::DaemonProtocol(format!("Invalid timestamp from daemon: {e}"))
                })?;
                Ok(Some(dt))
            }
            None => Ok(None),
        }
    }

    fn commits_in_range(
        &self,
        project_path: &str,
        after: &str,
        before: &str,
        commit_limit: Option<usize>,
    ) -> Result<GitRangeResult, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::GitCommitsInRangeResult = self.call(
            protocol::method::GIT_COMMITS_IN_RANGE,
            protocol::GitCommitsInRangeParams {
                path,
                after: after.to_string(),
                before: before.to_string(),
                commit_limit,
            },
            GIT_TIMEOUT,
        )?;
        Ok(GitRangeResult {
            commits: result.commits,
            files: result.files,
            truncated: result.truncated,
            total_count: result.total_count,
        })
    }

    fn commit_files(&self, project_path: &str, hash: &str) -> Result<Vec<CommitFile>, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::GitCommitFilesResult = self.call(
            protocol::method::GIT_COMMIT_FILES,
            protocol::GitCommitFilesParams {
                path,
                hash: hash.to_string(),
            },
            GIT_TIMEOUT,
        )?;
        Ok(result.files)
    }

    fn commit_diff(
        &self,
        project_path: &str,
        hash: &str,
        file_path: &str,
    ) -> Result<Vec<DiffHunk>, AppError> {
        let path = Self::translate_path(project_path);
        let result: protocol::GitCommitDiffResult = self.call(
            protocol::method::GIT_COMMIT_DIFF,
            protocol::GitCommitDiffParams {
                path,
                hash: hash.to_string(),
                file_path: file_path.to_string(),
            },
            GIT_TIMEOUT,
        )?;
        Ok(result.hunks)
    }

    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>, AppError> {
        let path = Self::translate_path(project_path);
        self.call(
            protocol::method::FILE_TREE,
            protocol::PathParams { path },
            FILE_TIMEOUT,
        )
    }

    fn read_file(&self, project_path: &str, relative_path: &str) -> Result<FileContent, AppError> {
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
            .map_err(|e| AppError::DaemonProtocol(format!("Failed to decode base64: {e}")))
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
    use crate::commands::logging::{install_global_sink, LogFileState};
    use crate::daemon::server::DaemonConfig;
    use crate::provider::local::LocalProvider;
    use git2::{Repository, Signature};
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
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

    fn start_daemon_on_port_with_guard(
        port: u16,
        heavy_guard: crate::test_support::HeavyTestGuard,
    ) -> TestDaemon {
        let shutdown = Arc::new(AtomicBool::new(false));
        let config = DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: None,
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            crate::daemon::server::run_for_test(&config, shutdown_clone, Arc::new(LocalProvider))
        });
        wait_for_port(port, Duration::from_secs(3));
        TestDaemon {
            port,
            shutdown,
            _heavy_guard: heavy_guard,
            handle: Some(handle),
        }
    }

    fn start_daemon_on_port(port: u16) -> TestDaemon {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        start_daemon_on_port_with_guard(port, heavy_guard)
    }

    /// Start a test daemon server with an ephemeral port.
    fn start_daemon() -> TestDaemon {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        start_daemon_on_port(port)
    }

    fn wait_for_port(port: u16, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("daemon did not accept connections on port {port} before timeout");
    }

    fn read_lines(path: &Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn wait_for_lines(path: &Path, expected_minimum: usize) -> Vec<String> {
        for _ in 0..100 {
            let lines = read_lines(path);
            if lines.len() >= expected_minimum {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        read_lines(path)
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
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        init_test_repo(dir.path());

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let status = provider.git_status(path).unwrap();

        assert!(status.branch.is_some());
        assert!(!status.is_dirty);

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_recent_commits() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        init_test_repo(dir.path());

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let commits = provider.recent_commits(path, 10).unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Initial commit");

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_file_tree() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "world").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let tree = provider.file_tree(path).unwrap();

        let names: Vec<&str> = tree.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_file() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let content = provider.read_file(path, "main.rs").unwrap();

        assert_eq!(content.content, "fn main() {}");
        assert_eq!(content.language, Some("rust".to_string()));

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_readme() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let readme = provider.read_readme(path).unwrap();

        assert!(readme.is_some());
        assert_eq!(readme.unwrap().content, "# Hello");

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_asset() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let data = vec![0x89, 0x50, 0x4e, 0x47]; // PNG magic bytes
        std::fs::write(dir.path().join("icon.png"), &data).unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let bytes = provider.read_asset(path, "icon.png").unwrap();

        assert_eq!(bytes, data);

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_read_asset_rejects_oversized_file() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let oversized = dir.path().join("huge.png");
        let file = std::fs::File::create(&oversized).unwrap();
        file.set_len(5 * 1024 * 1024 + 1).unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let err = provider
            .read_asset(path, "huge.png")
            .expect_err("oversized asset should be rejected");

        assert!(err
            .to_string()
            .contains("Asset too large to display (>5 MB)"));

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_scan_sessions() {
        let daemon = start_daemon();
        let dir = tempfile::TempDir::new().unwrap();
        let handoffs = dir.path().join(".claude").join("handoffs");
        std::fs::create_dir_all(&handoffs).unwrap();
        std::fs::write(handoffs.join("session.md"), "# Session").unwrap();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let path = dir.path().to_str().unwrap();
        let files = provider.scan_session_files(path).unwrap();

        assert_eq!(files.len(), 1);

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn daemon_provider_handles_errors() {
        let daemon = start_daemon();

        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        let result = provider.git_status("/nonexistent/path");

        assert!(result.is_err());

        daemon.shutdown.store(true, Ordering::Relaxed);
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
    fn daemon_provider_translates_windows_drive_paths() {
        let linux = DaemonProvider::translate_path(r"D:\projects\foo");
        assert_eq!(linux, "/mnt/d/projects/foo");

        let linux = DaemonProvider::translate_path(r"C:\Users\me\code");
        assert_eq!(linux, "/mnt/c/Users/me/code");
    }

    #[test]
    fn connect_to_nonexistent_daemon_fails() {
        let result = DaemonProvider::connect("127.0.0.1:1");
        assert!(result.is_err());
    }

    #[test]
    fn is_connected_true_initially() {
        let daemon = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        assert!(provider.is_connected());
        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn marks_disconnected_on_daemon_crash() {
        let daemon = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();

        // Verify connection works
        assert!(provider.ping().is_ok());
        assert!(provider.is_connected());

        // Kill the daemon — handler has 1s read timeout before noticing shutdown
        daemon.shutdown.store(true, Ordering::Relaxed);
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
        let daemon = start_daemon();
        let port = daemon.port;
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        {
            let conn = provider.pool[0].lock().unwrap();
            let stream = conn
                .as_ref()
                .expect("connection")
                .stream
                .try_clone()
                .unwrap();
            assert!(stream.nodelay().unwrap());
        }
        assert!(provider.ping().is_ok());

        // Kill daemon — wait for handler to exit
        daemon.shutdown.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(1500));
        let _ = provider.ping();
        if provider.is_connected() {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = provider.ping();
        }
        assert!(!provider.is_connected());
        drop(daemon);

        // Start a new daemon on the same port
        let daemon2 = start_daemon_on_port(port);

        // Reconnect should succeed
        assert!(provider.reconnect().is_ok());
        assert!(provider.is_connected());
        {
            let conn = provider.pool[0].lock().unwrap();
            let stream = conn
                .as_ref()
                .expect("connection")
                .stream
                .try_clone()
                .unwrap();
            assert!(stream.nodelay().unwrap());
        }
        assert!(provider.ping().is_ok());

        daemon2.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn status_request_succeeds_via_pool_when_one_slot_busy() {
        let daemon = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();

        // Hold slot 0 — request should succeed via a lazy-connected secondary slot
        let _busy_guard = provider.pool[0].try_lock().expect("lock slot 0");
        let request = DaemonRequest::ping("pool");
        let result = provider.send_status_request(&request);
        assert!(
            result.is_ok(),
            "request should succeed via another pool slot"
        );
        assert!(
            provider.is_connected(),
            "pool fallback should not disconnect provider"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn busy_probe_reports_busy_only_when_all_slots_held() {
        let daemon = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();

        // Holding one slot: not busy (other slots available)
        let guard0 = provider.pool[0].try_lock().expect("lock slot 0");
        assert!(
            !provider.is_busy(),
            "single slot held should not report busy with pool"
        );
        assert!(
            provider.is_connected(),
            "held slot should not imply disconnect"
        );

        // Hold all slots: now busy
        let guard1 = provider.pool[1].try_lock().expect("lock slot 1");
        let guard2 = provider.pool[2].try_lock().expect("lock slot 2");
        assert!(provider.is_busy(), "all slots held should report busy");

        // Release all — no longer busy
        drop(guard0);
        drop(guard1);
        drop(guard2);
        assert!(
            !provider.is_busy(),
            "busy probe should clear after all slots released"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn connect_leaves_auth_token_unloaded_until_first_request() {
        let daemon = start_daemon();
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();

        assert!(
            provider
                .auth_token
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none(),
            "startup fast path should not eagerly read daemon auth state"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn disconnected_provider_starts_without_auth_token_lookup() {
        let provider = DaemonProvider::new_disconnected("127.0.0.1:1");

        assert!(
            provider
                .auth_token
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none(),
            "startup fallback provider should defer auth token lookup"
        );
    }

    #[test]
    fn try_reconnect_rate_limits() {
        // No daemon running — every try_reconnect will fail, but we can verify
        // rate limiting: the second call within the cooldown should return false
        // immediately without attempting a connection.
        let provider = DaemonProvider::new_disconnected("127.0.0.1:1");

        // First attempt: should try (and fail, but that's fine)
        let result1 = provider.try_reconnect();
        assert!(!result1); // fails because no daemon

        // Second attempt immediately: should be rate-limited (returns false fast)
        let start = Instant::now();
        let result2 = provider.try_reconnect();
        assert!(!result2);
        // Should return very quickly (< 100ms), not spending time on TCP connect
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn emits_daemon_connection_lifecycle_events_on_connect_reconnect_and_disconnect() {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("daemon-connection-lifecycle.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let daemon = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            start_daemon_on_port_with_guard(port, heavy_guard)
        };
        let provider = DaemonProvider::connect(&format!("127.0.0.1:{}", daemon.port)).unwrap();
        assert!(provider.reconnect().is_ok());
        provider.mark_disconnected("test_disconnect");

        let mut events: Vec<serde_json::Value> = Vec::new();
        for _ in 0..100 {
            let lines = wait_for_lines(&log_path, 1);
            events = lines
                .iter()
                .map(|line| serde_json::from_str(line).expect("valid json"))
                .collect();
            if events
                .iter()
                .any(|value| value["event"] == "daemon.connection.established")
                && events
                    .iter()
                    .any(|value| value["event"] == "daemon.connection.reconnecting")
                && events
                    .iter()
                    .any(|value| value["event"] == "daemon.connection.lost")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(events
            .iter()
            .any(|value| value["event"] == "daemon.connection.established"));
        assert!(events
            .iter()
            .any(|value| value["event"] == "daemon.connection.reconnecting"));
        assert!(events
            .iter()
            .any(|value| value["event"] == "daemon.connection.lost"));

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    // Regression: 07ab6c5 deleted the hook -> file -> inotify focus chain, leaving
    // the daemon's versioned snapshot as the app's only focus transport, and
    // a0f3545 put the protocol gate on the health monitor's recovery path only.
    // `reconnect`/`try_reconnect` still counted a reachable socket as success, so
    // every inline and manual reconnect (runtime snapshot IPC, task sync, the
    // Start Daemon button) could adopt a daemon that predates hub-owned focus.
    #[test]
    fn a_checked_reconnect_drops_a_daemon_that_speaks_another_protocol() {
        let _guard = crate::test_support::acquire_heavy_test_guard();
        let stub = crate::test_support::StubDaemon::start(
            protocol::PROTOCOL_VERSION - 1,
            serde_json::Value::Null,
        );
        let provider = DaemonProvider::new_disconnected(stub.addr());

        let error = provider
            .reconnect_checked()
            .expect_err("an outdated daemon must not be adopted");

        assert!(
            error.to_string().contains("protocol"),
            "error should name the protocol mismatch: {error}"
        );
        assert!(
            !provider.is_connected(),
            "a mismatched daemon must be dropped immediately, not on some later poll"
        );
        assert!(
            !provider.try_reconnect(),
            "try_reconnect must apply the same gate"
        );
    }

    #[test]
    fn a_checked_reconnect_keeps_a_daemon_speaking_this_protocol() {
        let _guard = crate::test_support::acquire_heavy_test_guard();
        let stub = crate::test_support::StubDaemon::start(
            protocol::PROTOCOL_VERSION,
            serde_json::Value::Null,
        );
        let provider = DaemonProvider::new_disconnected(stub.addr());

        provider.reconnect_checked().expect("matching daemon");

        assert!(provider.is_connected());
    }
}
