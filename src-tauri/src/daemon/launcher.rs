//! Daemon connection and auto-start logic.
//!
//! On app startup, if WSL projects exist in the database, we try to connect
//! to an already-running daemon. If that fails, we auto-start the daemon
//! via `wsl.exe` and retry the connection.

use std::time::{Duration, Instant};

use crate::provider::daemon_client::DaemonProvider;

/// Max time to wait for daemon to become reachable after spawning.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Interval between TCP connection attempts while waiting for daemon startup.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Try to connect to an existing daemon, or start one if needed.
///
/// - If `wsl_distro` is None, no WSL projects exist — returns None immediately.
/// - First tries a TCP connection to the daemon port.
/// - If that fails, attempts to auto-start the daemon via `wsl.exe` and retries.
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
    if let Some(provider) = try_connect(port) {
        tracing::info!(port, "Connected to existing daemon");
        return Some(provider);
    }

    // Daemon not running — try to auto-start it.
    tracing::info!(port, distro, "Daemon not reachable, attempting auto-start");
    match try_start_daemon(distro, port) {
        Ok(()) => {
            // Daemon started, poll until reachable.
            if let Some(provider) = poll_until_reachable(port, STARTUP_TIMEOUT) {
                tracing::info!(port, "Connected to auto-started daemon");
                return Some(provider);
            }
            tracing::warn!(port, "Daemon process started but not reachable within timeout");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to auto-start daemon");
        }
    }

    tracing::warn!(
        port,
        distro,
        "Daemon not available — WSL projects will use local provider fallback"
    );
    None
}

/// Try to restart the daemon process (called by health check on disconnect).
///
/// Spawns `wsl.exe -d {distro} -- taurhaus-daemon --port {port}` and waits
/// for it to become reachable.
pub fn try_restart_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    tracing::info!(port, distro, "Attempting daemon restart via wsl.exe");
    try_start_daemon(distro, port)
}

/// Spawn the daemon process via `wsl.exe`.
///
/// The daemon binary must be installed at `~/.local/bin/taurhaus-daemon`
/// (via `just install-daemon`). The process is spawned detached — it
/// continues running after this function returns.
fn try_start_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    let daemon_bin = "~/.local/bin/taurhaus-daemon";

    // Use wsl.exe to launch the daemon inside the WSL distro.
    // We wrap in a shell to expand ~ and background the process so wsl.exe
    // returns immediately instead of blocking until the daemon exits.
    let child = std::process::Command::new("wsl")
        .args([
            "-d",
            distro,
            "--",
            "sh",
            "-c",
            &format!(
                "nohup {daemon_bin} --port {port} > /dev/null 2>&1 &"
            ),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(mut c) => {
            // Wait for the shell wrapper to exit (it returns immediately
            // after backgrounding the daemon).
            let _ = c.wait();
            tracing::info!(port, distro, "Daemon spawn command completed");
            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to spawn wsl.exe");
            Err(e)
        }
    }
}

/// Poll for TCP connectivity until the daemon is reachable or timeout expires.
fn poll_until_reachable(port: u16, timeout: Duration) -> Option<DaemonProvider> {
    let start = Instant::now();
    loop {
        if let Some(provider) = try_connect(port) {
            return Some(provider);
        }
        if start.elapsed() >= timeout {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Ensure tmux server is running inside WSL.
///
/// `tmux start-server` is idempotent — if the server is already running,
/// this is a no-op. Failure is non-fatal.
pub fn ensure_tmux_server(distro: &str) {
    tracing::info!(distro, "Ensuring tmux server is running");

    let result = std::process::Command::new("wsl")
        .args(["-d", distro, "--", "tmux", "start-server"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match result {
        Ok(output) if output.status.success() => {
            tracing::info!("tmux server is running");
        }
        Ok(output) => {
            tracing::warn!(
                status = ?output.status,
                "tmux start-server exited with non-zero status"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to run tmux start-server via wsl.exe");
        }
    }
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

        let result = try_connect(port);
        assert!(result.is_none());
    }

    #[test]
    fn poll_until_reachable_succeeds_when_daemon_starts() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        // Start daemon after a short delay (simulating startup time)
        let shutdown_clone = shutdown.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let config = crate::daemon::server::DaemonConfig {
                port,
                bind_addr: "127.0.0.1".to_string(),
                idle_timeout_secs: None,
            };
            let _ = crate::daemon::server::run(&config, shutdown_clone);
        });

        let result = poll_until_reachable(port, Duration::from_secs(3));
        assert!(result.is_some(), "Should connect to daemon that started after a delay");

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn poll_until_reachable_times_out() {
        // Use a port that nothing will ever listen on
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let start = Instant::now();
        let result = poll_until_reachable(port, Duration::from_secs(1));
        assert!(result.is_none(), "Should timeout when no daemon starts");
        assert!(start.elapsed() >= Duration::from_secs(1), "Should wait full timeout");
    }
}
