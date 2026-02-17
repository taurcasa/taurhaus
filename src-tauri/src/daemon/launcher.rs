//! Daemon auto-start and connection logic.
//!
//! On app startup, if WSL projects exist in the database, we try to connect
//! to the daemon. If it's not running, we attempt to start it and retry.

use std::time::{Duration, Instant};

use crate::provider::daemon_client::DaemonProvider;

/// Maximum time to wait for the daemon to start and become connectable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Interval between connection retries after starting the daemon.
const RETRY_INTERVAL: Duration = Duration::from_millis(500);

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

    // Try connecting to existing daemon
    if let Some(provider) = try_connect(port) {
        tracing::info!(port, "Connected to existing daemon");
        return Some(provider);
    }

    // Try starting daemon
    tracing::info!(port, distro, "No daemon running, attempting to start");
    if let Err(e) = start_daemon(distro, port) {
        tracing::warn!(error = %e, "Failed to start daemon");
        return None;
    }

    // Retry connection with backoff
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    while Instant::now() < deadline {
        std::thread::sleep(RETRY_INTERVAL);
        if let Some(provider) = try_connect(port) {
            tracing::info!(port, "Connected to newly-started daemon");
            return Some(provider);
        }
    }

    tracing::warn!(port, "Daemon started but connection failed within timeout");
    None
}

/// Try to restart the daemon process (used by health check on disconnect).
///
/// Just starts the process — caller is responsible for reconnecting.
pub fn try_restart_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    tracing::info!(distro, port, "Restarting daemon process");
    start_daemon(distro, port)
}

/// Attempt a single TCP connection to the daemon.
fn try_connect(port: u16) -> Option<DaemonProvider> {
    let addr = format!("127.0.0.1:{port}");
    DaemonProvider::connect(&addr).ok()
}

/// Start the daemon process.
///
/// On Windows: launches `wsl.exe -d <distro>` to run the daemon inside WSL.
/// On Linux (dev): launches the daemon binary directly.
#[cfg(target_os = "windows")]
fn start_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    let daemon_cmd = format!(
        "$HOME/.local/bin/taurhaus-daemon --port {port}"
    );
    tracing::debug!(distro, cmd = %daemon_cmd, "Launching daemon via wsl.exe");

    std::process::Command::new("wsl.exe")
        .args(["-d", distro, "--", "sh", "-c", &daemon_cmd])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn start_daemon(_distro: &str, port: u16) -> Result<(), std::io::Error> {
    tracing::debug!(port, "Launching daemon directly (Linux dev mode)");

    std::process::Command::new("taurhaus-daemon")
        .args(["--port", &port.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::server::DEFAULT_PORT;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

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
