//! Daemon connection logic.
//!
//! On app startup, if WSL projects exist in the database, we try to connect
//! to an already-running daemon. The daemon must be started separately
//! (e.g. `just run-daemon`).

use crate::provider::daemon_client::DaemonProvider;

/// Try to connect to an existing daemon, or start one if needed.
///
/// - If `wsl_distro` is None, no WSL projects exist — returns None immediately.
/// - First tries a TCP connection to the daemon port.
/// - If that fails, tries to start the daemon and retries connection.
/// - Returns None if daemon can't be reached within the timeout.
pub fn try_connect_daemon(
    wsl_distro: Option<&str>,
    port: u16,
) -> Option<DaemonProvider> {
    let distro = match wsl_distro {
        Some(d) => d,
        None => {
            tracing::debug!("No WSL projects registered, skipping daemon");
            return None;
        }
    };

    tracing::info!(port, distro, "Checking for WSL daemon");

    // Try connecting to an already-running daemon.
    // We don't auto-start — the daemon should be launched separately
    // (e.g. `just run-daemon` or installed via `just install-daemon`).
    if let Some(provider) = try_connect(port) {
        tracing::info!(port, "Connected to existing daemon");
        return Some(provider);
    }

    tracing::warn!(port, distro, "Daemon not reachable — WSL projects will use local provider fallback. Start the daemon with: just run-daemon");
    None
}

/// Daemon restart is disabled — auto-start via wsl.exe is not used during development.
/// The health check calls this on disconnect; we just log and report failure.
pub fn try_restart_daemon(_distro: &str, port: u16) -> Result<(), std::io::Error> {
    tracing::warn!(port, "Daemon disconnected. Restart it manually: just run-daemon");
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "auto-start disabled"))
}

/// Attempt a single TCP connection to the daemon.
fn try_connect(port: u16) -> Option<DaemonProvider> {
    let addr = format!("127.0.0.1:{port}");
    DaemonProvider::connect(&addr).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DEFAULT_PORT;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn no_distro_returns_none_immediately() {
        // Should return instantly, no network call
        let result = try_connect_daemon(None, DEFAULT_PORT);
        assert!(result.is_none());
    }

    #[test]
    fn connects_to_running_daemon() {
        // Start a real daemon
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = crate::daemon::server::DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
        };
        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            let _ = crate::daemon::server::run(&config, shutdown_clone);
        });
        std::thread::sleep(Duration::from_millis(100));

        let result = try_connect_daemon(Some("Ubuntu"), port);
        assert!(result.is_some());

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn dead_port_returns_none() {
        // Use a port that nothing is listening on
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        // This will try to connect, fail, try to start daemon (which also fails
        // since there's no taurhaus-daemon on PATH in test), and timeout.
        // We override the timeout behavior by testing try_connect directly.
        let result = try_connect(port);
        assert!(result.is_none());
    }
}
