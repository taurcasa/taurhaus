use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::daemon::protocol::{self, DaemonRequest, DaemonResponse};
use crate::errors::AppError;

/// Dedicated daemon connection for versioned session-activity updates.
///
/// Uses long-poll requests (`wait_session_updates`) so the app can stay
/// event-driven above the daemon while scanner polling remains daemon-owned.
pub struct DaemonSessionListener {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
    auth_token: Option<String>,
}

impl DaemonSessionListener {
    pub fn connect(addr: &str) -> Result<Self, AppError> {
        let stream = TcpStream::connect(addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Session listener connect to {addr} failed: {e}"),
            ))
        })?;
        stream.set_nodelay(true).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("Session listener TCP_NODELAY setup failed for {addr}: {e}"),
            ))
        })?;
        let reader = BufReader::new(stream.try_clone().map_err(AppError::Io)?);
        let auth_token = crate::daemon::auth::read_auth_token();

        Ok(Self {
            stream,
            reader,
            next_id: 1,
            auth_token,
        })
    }

    pub fn wait_for_updates(
        &mut self,
        since_version: u64,
        timeout: Duration,
    ) -> Result<protocol::WaitSessionUpdatesResult, AppError> {
        let id = format!("su{}", self.next_id);
        self.next_id += 1;

        let request = DaemonRequest::new(
            &id,
            protocol::method::WAIT_SESSION_UPDATES,
            protocol::WaitSessionUpdatesParams {
                since_version,
                timeout_ms: timeout.as_millis() as u64,
            },
        )
        .with_auth(self.auth_token.clone());

        let json = serde_json::to_string(&request).map_err(|e| {
            AppError::InvalidPath(format!(
                "Serialize wait_session_updates request failed: {e}"
            ))
        })?;
        self.stream
            .write_all(json.as_bytes())
            .map_err(AppError::Io)?;
        self.stream.write_all(b"\n").map_err(AppError::Io)?;
        self.stream.flush().map_err(AppError::Io)?;

        self.stream
            .set_read_timeout(Some(timeout + Duration::from_secs(2)))
            .map_err(AppError::Io)?;

        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(AppError::Io)?;
        if line.trim().is_empty() {
            return Err(AppError::InvalidPath(
                "Daemon returned empty wait_session_updates response".to_string(),
            ));
        }

        let response: DaemonResponse = serde_json::from_str(&line).map_err(|e| {
            AppError::InvalidPath(format!("Parse wait_session_updates response failed: {e}"))
        })?;

        if let Some(err) = response.error {
            return Err(AppError::InvalidPath(format!(
                "Daemon wait_session_updates error [{}]: {}",
                err.code, err.message
            )));
        }

        let result = response.result.unwrap_or(serde_json::Value::Null);
        serde_json::from_value(result).map_err(|e| {
            AppError::InvalidPath(format!(
                "Deserialize wait_session_updates result failed: {e}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DaemonConfig;
    use crate::provider::local::LocalProvider;
    use std::net::TcpStream;
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
    fn session_listener_connect_enables_tcp_nodelay() {
        let daemon = start_daemon();
        let listener = DaemonSessionListener::connect(&format!("127.0.0.1:{}", daemon.port))
            .expect("connect session listener");
        assert!(listener.stream.nodelay().unwrap());
    }
}
