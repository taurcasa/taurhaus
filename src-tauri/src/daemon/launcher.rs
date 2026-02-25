//! Daemon connection and auto-start logic.
//!
//! On app startup, if WSL projects exist in the database, we try to connect
//! to an already-running daemon. If that fails, we auto-start the daemon
//! via `wsl.exe` and retry the connection.
//!
//! Bootstrap logs are written to `taurhaus.log` via `bootstrap_log` so they're
//! visible on Windows (where Rust tracing to stderr is invisible in GUI apps).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::provider::daemon_client::DaemonProvider;

/// Write a timestamped line to the app log file.
/// Dual-writes to both tracing (for dev builds) and the log file (for Windows).
fn blog(log_path: &Path, msg: &str) {
    tracing::info!("{msg}");
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path) {
        let ts = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "[{ts}] [INF] [bootstrap] {msg}");
    }
}

fn bwarn(log_path: &Path, msg: &str) {
    tracing::warn!("{msg}");
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(log_path) {
        let ts = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "[{ts}] [WRN] [bootstrap] {msg}");
    }
}

/// Create a `Command` for `wsl.exe` that won't flash a console window.
///
/// On Windows, console subsystem processes (like wsl.exe) create a visible
/// window by default. `CREATE_NO_WINDOW` prevents this.
fn wsl_command() -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new("wsl");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

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
///
/// `log_path` is the path to `taurhaus.log` for bootstrap logging.
pub fn try_connect_daemon(
    wsl_distro: Option<&str>,
    port: u16,
    log_path: &Path,
) -> Option<DaemonProvider> {
    let distro = match wsl_distro {
        Some(d) => d,
        None => {
            tracing::debug!("No WSL projects registered, skipping daemon");
            return None;
        }
    };

    blog(log_path, &format!("Checking for WSL daemon on port {port} (distro: {distro})"));

    // Try connecting to an already-running daemon.
    if let Some(provider) = try_connect(port) {
        blog(log_path, &format!("Connected to existing daemon on port {port}"));
        return Some(provider);
    }

    // Daemon not running — try to auto-start it.
    blog(log_path, "Daemon not reachable, attempting auto-start via wsl.exe");
    match try_start_daemon(distro, port, log_path) {
        Ok(()) => {
            // Daemon started, poll until reachable.
            blog(log_path, &format!("Daemon spawned, polling for connectivity (up to {STARTUP_TIMEOUT:?})"));
            if let Some(provider) = poll_until_reachable(port, STARTUP_TIMEOUT) {
                blog(log_path, "Connected to auto-started daemon");
                return Some(provider);
            }
            bwarn(log_path, "Daemon process started but not reachable within timeout");
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to auto-start daemon: {e}"));
        }
    }

    bwarn(log_path, "Daemon not available — WSL projects will use local provider fallback");
    None
}

/// Try to restart the daemon process (called by health check on disconnect).
pub fn try_restart_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    tracing::info!(port, distro, "Attempting daemon restart via wsl.exe");
    // Health check doesn't have log_path — use a fallback location.
    let log_path = health_check_log_path();
    try_start_daemon(distro, port, &log_path)
}

/// Spawn the daemon process via `wsl.exe`.
fn try_start_daemon(distro: &str, port: u16, log_path: &Path) -> Result<(), std::io::Error> {
    // First verify the daemon binary exists inside WSL.
    blog(log_path, "Checking daemon binary exists at ~/.local/bin/taurhaus-daemon");
    let check = wsl_command()
        .args([
            "-d",
            distro,
            "--",
            "test",
            "-x",
            "/home/mstie/.local/bin/taurhaus-daemon",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match check {
        Ok(output) if !output.status.success() => {
            let msg = "taurhaus-daemon not found at ~/.local/bin/taurhaus-daemon. Run: just install-daemon";
            bwarn(log_path, msg);
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, msg));
        }
        Err(e) => {
            let msg = format!("Failed to check daemon binary via wsl.exe: {e}");
            bwarn(log_path, &msg);
            return Err(std::io::Error::new(e.kind(), msg));
        }
        _ => {
            blog(log_path, "Daemon binary found");
        }
    }

    // Spawn the daemon directly — no shell wrapper.
    //
    // WSL kills background children when wsl.exe exits (WSL#4649), so we
    // DON'T use sh -c "... &" or nohup. Instead, we run the daemon as a
    // direct child of wsl.exe and intentionally never call .wait() on
    // the Rust side. This keeps wsl.exe alive as a parent process for the
    // entire app lifetime, which keeps the daemon alive inside WSL.
    //
    // The wsl.exe process is lightweight (~1MB RSS) and exits automatically
    // when the daemon terminates.
    blog(log_path, &format!(
        "Spawning: wsl -d {distro} -- taurhaus-daemon --port {port} (long-lived wsl.exe child)"
    ));

    let child = wsl_command()
        .args([
            "-d", distro, "--",
            "/home/mstie/.local/bin/taurhaus-daemon",
            "--port", &port.to_string(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(_child) => {
            // Intentionally don't wait — wsl.exe stays alive as the daemon's
            // parent. The child handle is dropped but the process continues.
            blog(log_path, "Daemon wsl.exe process spawned (not waiting)");
            Ok(())
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to spawn wsl.exe: {e}"));
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
pub fn ensure_tmux_server(distro: &str, log_path: &Path) {
    blog(log_path, "Ensuring tmux server is running");

    let result = wsl_command()
        .args(["-d", distro, "--", "tmux", "start-server"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match result {
        Ok(output) if output.status.success() => {
            blog(log_path, "tmux server is running");
        }
        Ok(output) => {
            bwarn(log_path, &format!("tmux start-server exited with status {:?}", output.status));
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to run tmux start-server via wsl.exe: {e}"));
        }
    }
}

/// Best-effort log path for the health check (which doesn't receive the
/// app's log path). Falls back to the known Windows app data location.
fn health_check_log_path() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata)
            .join("com.taurhaus.dev")
            .join("taurhaus.log")
    } else {
        PathBuf::from("/tmp/taurhaus-bootstrap.log")
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

    fn test_log_path() -> PathBuf {
        std::env::temp_dir().join("taurhaus-test.log")
    }

    #[test]
    fn no_distro_returns_none_immediately() {
        let result = try_connect_daemon(None, DEFAULT_PORT, &test_log_path());
        assert!(result.is_none());
    }

    #[test]
    fn connects_to_running_daemon() {
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

        let result = try_connect_daemon(Some("Ubuntu"), port, &test_log_path());
        assert!(result.is_some());

        shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn dead_port_returns_none() {
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
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let start = Instant::now();
        let result = poll_until_reachable(port, Duration::from_secs(1));
        assert!(result.is_none(), "Should timeout when no daemon starts");
        assert!(start.elapsed() >= Duration::from_secs(1), "Should wait full timeout");
    }
}
