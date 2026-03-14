//! Daemon connection and auto-start logic.
//!
//! On app startup we try to connect to an already-running daemon. If that
//! fails, we auto-start it and retry. The same daemon binary is used on all
//! platforms — on Windows it's spawned via `wsl.exe`, on macOS/Linux it runs
//! natively.
//!
//! Bootstrap events are emitted through the structured JSONL sink so they're
//! persisted even when stderr isn't visible in GUI apps.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::provider::daemon_client::DaemonProvider;
use crate::provider::path;
use serde_json::{Map, Value};

fn blog(log_path: &Path, msg: &str) {
    tracing::info!("{msg}");
    let mut fields = Map::new();
    fields.insert(
        "log_path".to_string(),
        Value::String(log_path.display().to_string()),
    );
    crate::commands::logging::emit_global(
        "info",
        "bootstrap",
        "daemon.bootstrap",
        Some(msg.to_string()),
        fields,
    );
}

fn bwarn(log_path: &Path, msg: &str) {
    tracing::warn!("{msg}");
    let mut fields = Map::new();
    fields.insert(
        "log_path".to_string(),
        Value::String(log_path.display().to_string()),
    );
    crate::commands::logging::emit_global(
        "warn",
        "bootstrap",
        "daemon.bootstrap",
        Some(msg.to_string()),
        fields,
    );
}

/// Validate a WSL distro name against a safe pattern.
///
/// Accepts alphanumeric characters, hyphens, underscores, and dots.
/// Rejects empty strings and anything with shell metacharacters.
pub fn validate_wsl_distro(distro: &str) -> Result<(), String> {
    if distro.is_empty() {
        return Err("WSL distro name is empty".to_string());
    }
    if !distro
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!(
            "WSL distro name contains invalid characters: {distro:?}"
        ));
    }
    Ok(())
}

/// Create a `Command` for `wsl.exe` that won't flash a console window.
///
/// On Windows, console subsystem processes (like wsl.exe) create a visible
/// window by default. `CREATE_NO_WINDOW` prevents this.
pub fn wsl_command() -> std::process::Command {
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
const STARTUP_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

/// Interval between TCP connection attempts while waiting for daemon startup.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Whether the current platform runs the daemon natively (macOS, Linux)
/// vs. via WSL (Windows).
pub fn is_native_daemon() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
}

/// Try to connect to an existing daemon, or start one if needed.
///
/// **Windows**: Uses WSL to communicate with the daemon running inside Linux.
/// If `wsl_distro` is None, no WSL projects exist — returns None immediately.
///
/// **macOS/Linux**: The daemon runs natively. `wsl_distro` is ignored.
///
/// `log_path` is the path to `taurhaus.log.jsonl` for bootstrap logging context.
pub fn try_connect_daemon(
    wsl_distro: Option<&str>,
    port: u16,
    log_path: &Path,
) -> Option<DaemonProvider> {
    // On Windows, we need a WSL distro to know where to spawn the daemon.
    // On macOS/Linux, the daemon runs natively — no distro needed.
    let distro = if is_native_daemon() {
        // Synthetic value — not used for WSL commands on native platforms.
        "native"
    } else {
        match wsl_distro {
            Some(d) => d,
            None => {
                tracing::debug!("No WSL projects registered, skipping daemon");
                return None;
            }
        }
    };

    if !is_native_daemon() {
        if let Err(e) = validate_wsl_distro(distro) {
            bwarn(log_path, &format!("Invalid WSL distro: {e}"));
            return None;
        }
    }

    blog(
        log_path,
        &format!(
            "Checking for daemon on port {port} ({})",
            if is_native_daemon() {
                "native".to_string()
            } else {
                format!("distro: {distro}")
            }
        ),
    );

    // Try connecting to an already-running daemon.
    if let Some(provider) = try_connect(port) {
        blog(
            log_path,
            &format!("Connected to existing daemon on port {port}"),
        );
        return Some(provider);
    }

    // Daemon not running — try to auto-start it.
    blog(
        log_path,
        &format!(
            "Daemon not reachable, attempting auto-start {}",
            if is_native_daemon() {
                "(native)"
            } else {
                "via wsl.exe"
            }
        ),
    );
    match try_start_daemon(distro, port, log_path) {
        Ok(()) => {
            blog(
                log_path,
                &format!("Daemon spawned, polling for connectivity (up to {STARTUP_TIMEOUT:?})"),
            );
            if let Some(provider) = poll_until_reachable(port, STARTUP_TIMEOUT) {
                blog(log_path, "Connected to auto-started daemon");
                return Some(provider);
            }
            bwarn(
                log_path,
                "Daemon process started but not reachable within timeout",
            );
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to auto-start daemon: {e}"));
        }
    }

    bwarn(
        log_path,
        "Daemon not available — will use local provider fallback",
    );
    None
}

pub enum StartupDaemonValidation {
    Healthy,
    RestartedStaleBinary,
}

/// Try to restart the daemon process (called by health check on disconnect).
pub fn try_restart_daemon(distro: &str, port: u16) -> Result<(), std::io::Error> {
    try_restart_daemon_with(distro, port, try_start_daemon)
}

pub fn validate_startup_daemon_binary(
    provider: &DaemonProvider,
    wsl_distro: Option<&str>,
    port: u16,
    log_path: &Path,
) -> Result<StartupDaemonValidation, std::io::Error> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (provider, wsl_distro, port, log_path);
        return Ok(StartupDaemonValidation::Healthy);
    }

    #[cfg(target_os = "linux")]
    {
        if !is_native_daemon() {
            let _ = (provider, wsl_distro, port, log_path);
            return Ok(StartupDaemonValidation::Healthy);
        }

        let Some(pid) = crate::platform::listening_process_on_port(port) else {
            bwarn(
                log_path,
                &format!(
                    "Could not resolve daemon PID for port {port}; skipping binary staleness check"
                ),
            );
            return Ok(StartupDaemonValidation::Healthy);
        };

        let running_exe = crate::platform::process_exe(pid).ok_or_else(|| {
            std::io::Error::other(format!(
                "Failed to read /proc/{pid}/exe for connected daemon"
            ))
        })?;
        let expected_binary = daemon_binary_path(wsl_distro.unwrap_or("native"))?;
        let expected_path = PathBuf::from(&expected_binary);
        if !startup_daemon_binary_is_stale(&running_exe, &expected_path) {
            return Ok(StartupDaemonValidation::Healthy);
        }
        let running_exe_display = running_exe.to_string_lossy().to_string();

        blog(
            log_path,
            &format!(
                "Detected stale daemon binary at startup: pid={pid}, running_exe={running_exe_display}, expected_exe={expected_binary}"
            ),
        );

        provider.disconnect("stale_startup_binary");
        terminate_pid_gracefully(pid, log_path)?;
        try_restart_daemon(wsl_distro.unwrap_or("native"), port)?;
        reconnect_existing_provider_until_reachable(provider, port)?;

        blog(
            log_path,
            "Reconnected to fresh daemon binary after stale daemon eviction",
        );
        Ok(StartupDaemonValidation::RestartedStaleBinary)
    }
}

#[cfg(target_os = "linux")]
fn startup_daemon_binary_is_stale(running_exe: &Path, expected_path: &Path) -> bool {
    let running_exe_display = running_exe.to_string_lossy();
    if running_exe_display.ends_with(" (deleted)") {
        return true;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if let (Ok(running_meta), Ok(expected_meta)) = (
            std::fs::metadata(running_exe),
            std::fs::metadata(expected_path),
        ) {
            return running_meta.dev() != expected_meta.dev()
                || running_meta.ino() != expected_meta.ino();
        }
    }

    running_exe
        .canonicalize()
        .unwrap_or_else(|_| running_exe.to_path_buf())
        != expected_path
            .canonicalize()
            .unwrap_or_else(|_| expected_path.to_path_buf())
}

fn try_restart_daemon_with<F>(distro: &str, port: u16, starter: F) -> Result<(), std::io::Error>
where
    F: FnOnce(&str, u16, &Path) -> Result<(), std::io::Error>,
{
    if !is_native_daemon() {
        validate_wsl_distro(distro).map_err(std::io::Error::other)?;
    }
    tracing::info!(port, distro, "Attempting daemon restart");
    let log_path = health_check_log_path();
    starter(distro, port, &log_path)
}

/// Resolve the daemon binary path.
///
/// **macOS/Linux**: `~/.local/bin/taurhaus-daemon`
/// **Windows**: Resolves via WSL `$HOME`.
fn daemon_binary_path(distro: &str) -> Result<String, std::io::Error> {
    if is_native_daemon() {
        let home = dirs::home_dir()
            .ok_or_else(|| std::io::Error::other("Could not determine home directory"))?;
        Ok(home
            .join(".local/bin/taurhaus-daemon")
            .to_string_lossy()
            .to_string())
    } else {
        let home = resolve_wsl_home(distro)?;
        Ok(format!("{home}/.local/bin/taurhaus-daemon"))
    }
}

/// Resolve the WSL user's home directory by running `echo $HOME` inside WSL.
fn resolve_wsl_home(distro: &str) -> Result<String, std::io::Error> {
    let mut command = wsl_command();
    command
        .args(["-d", distro, "--", "sh", "-c", "echo $HOME"])
        .stdin(std::process::Stdio::null());
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        STARTUP_COMMAND_TIMEOUT,
        "wsl echo $HOME",
    )?;

    if !output.status.success() {
        return Err(std::io::Error::other(
            "Failed to resolve WSL home directory",
        ));
    }

    let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if home.is_empty() {
        return Err(std::io::Error::other("WSL $HOME is empty"));
    }
    Ok(home)
}

/// Spawn the daemon process.
///
/// **macOS/Linux**: Spawns the daemon binary directly.
/// **Windows**: Spawns via `wsl.exe` as a long-lived child process.
fn try_start_daemon(distro: &str, port: u16, log_path: &Path) -> Result<(), std::io::Error> {
    let binary_path = daemon_binary_path(distro)?;

    if is_native_daemon() {
        try_start_daemon_native(&binary_path, port, log_path)
    } else {
        try_start_daemon_wsl(distro, &binary_path, port, log_path)
    }
}

/// Spawn the daemon natively (macOS/Linux).
fn try_start_daemon_native(
    binary_path: &str,
    port: u16,
    log_path: &Path,
) -> Result<(), std::io::Error> {
    // Verify the daemon binary exists.
    blog(
        log_path,
        &format!("Checking daemon binary exists at {binary_path}"),
    );
    let path = Path::new(binary_path);
    if !path.exists() {
        let msg = format!(
            "taurhaus-daemon not found at {binary_path}. Install with: just install-daemon"
        );
        bwarn(log_path, &msg);
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, msg));
    }

    blog(
        log_path,
        &format!("Spawning: {binary_path} --port {port} (native daemon)"),
    );

    let mut cmd = std::process::Command::new(binary_path);
    cmd.args(["--port", &port.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(data_dir) = daemon_data_dir_env_value(log_path) {
        cmd.env("TAURHAUS_DATA_DIR", data_dir);
    }
    let child = cmd.spawn();

    match child {
        Ok(_child) => {
            blog(log_path, "Native daemon process spawned");
            Ok(())
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to spawn daemon: {e}"));
            Err(e)
        }
    }
}

/// Spawn the daemon via `wsl.exe` (Windows).
fn try_start_daemon_wsl(
    distro: &str,
    binary_path: &str,
    port: u16,
    log_path: &Path,
) -> Result<(), std::io::Error> {
    // Verify the daemon binary exists inside WSL.
    blog(
        log_path,
        &format!("Checking daemon binary exists at {binary_path}"),
    );
    let mut check = wsl_command();
    check
        .args(["-d", distro, "--", "test", "-x", binary_path])
        .stdin(std::process::Stdio::null());
    let check = crate::process_utils::run_command_with_timeout(
        &mut check,
        STARTUP_COMMAND_TIMEOUT,
        "wsl test -x taurhaus-daemon",
    );

    match check {
        Ok(output) if !output.status.success() => {
            let msg =
                format!("taurhaus-daemon not found at {binary_path}. Run: just install-daemon");
            bwarn(log_path, &msg);
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
    blog(
        log_path,
        &format!(
            "Spawning: wsl -d {distro} -- taurhaus-daemon --port {port} (long-lived wsl.exe child)"
        ),
    );

    let mut cmd = wsl_command();
    cmd.arg("-d").arg(distro).arg("--");
    if let Some(data_dir) = daemon_data_dir_env_value_for_launch(log_path, false) {
        cmd.arg("env").arg(format!("TAURHAUS_DATA_DIR={data_dir}"));
    }
    cmd.arg(binary_path)
        .arg("--port")
        .arg(port.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let child = cmd.spawn();

    match child {
        Ok(_child) => {
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

pub(crate) fn reconnect_existing_provider_until_reachable(
    provider: &DaemonProvider,
    port: u16,
) -> Result<(), std::io::Error> {
    let start = Instant::now();
    loop {
        if provider.reconnect().is_ok() {
            return Ok(());
        }
        if start.elapsed() >= STARTUP_TIMEOUT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Timed out reconnecting daemon on port {port} after stale restart"),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(target_os = "linux")]
fn terminate_pid_gracefully(pid: u32, log_path: &Path) -> Result<(), std::io::Error> {
    blog(log_path, &format!("Stopping stale daemon pid {pid}"));
    let term_status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !term_status.success() && crate::platform::process_exists(pid) {
        return Err(std::io::Error::other(format!(
            "Failed to send SIGTERM to stale daemon pid {pid}"
        )));
    }

    for _ in 0..20 {
        if !crate::platform::process_exists(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let kill_status = std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !kill_status.success() && crate::platform::process_exists(pid) {
        return Err(std::io::Error::other(format!(
            "Failed to send SIGKILL to stale daemon pid {pid}"
        )));
    }

    Ok(())
}

fn daemon_data_dir_env_value(log_path: &Path) -> Option<String> {
    daemon_data_dir_env_value_for_launch(log_path, is_native_daemon())
}

fn daemon_data_dir_env_value_for_launch(log_path: &Path, daemon_is_native: bool) -> Option<String> {
    let raw = daemon_data_dir_raw(log_path)?;
    if daemon_is_native {
        return Some(raw);
    }
    path::to_linux(&raw).or(Some(raw))
}

fn daemon_data_dir_raw(log_path: &Path) -> Option<String> {
    if let Some(parent) = log_path.parent() {
        let value = parent.to_string_lossy().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }

    let raw = log_path.to_string_lossy();
    raw.strip_suffix("\\taurhaus.log.jsonl")
        .or_else(|| raw.strip_suffix("/taurhaus.log.jsonl"))
        .map(|value| value.to_string())
}

/// Ensure the taurhaus tmux session exists.
///
/// **macOS/Linux**: Runs tmux directly (natively installed).
/// **Windows**: Runs tmux inside WSL via `wsl.exe`.
///
/// Creates a dedicated named session (`taurhaus`) so our CLI tool windows
/// don't interfere with the user's own tmux sessions. `tmux new-session`
/// implicitly starts the server if needed.
///
/// Failure is non-fatal — the daemon-side code also creates the session
/// on demand when launching a tool.
pub fn ensure_tmux_session(distro: &str, log_path: &Path) {
    use crate::session_scanner::control::TMUX_SESSION_NAME;

    if is_native_daemon() {
        ensure_tmux_session_native(TMUX_SESSION_NAME, log_path);
    } else {
        ensure_tmux_session_wsl(distro, TMUX_SESSION_NAME, log_path);
    }
}

fn ensure_tmux_session_native(session_name: &str, log_path: &Path) {
    blog(
        log_path,
        &format!("Ensuring tmux session '{session_name}' exists (native)"),
    );

    let mut check = std::process::Command::new("tmux");
    check
        .args(["has-session", "-t", session_name])
        .stdin(std::process::Stdio::null());
    let check = crate::process_utils::run_command_with_timeout(
        &mut check,
        STARTUP_COMMAND_TIMEOUT,
        "tmux has-session",
    );

    if let Ok(output) = &check {
        if output.status.success() {
            blog(
                log_path,
                &format!("tmux session '{session_name}' already exists"),
            );
            return;
        }
    }

    let mut result = std::process::Command::new("tmux");
    result
        .args(["new-session", "-d", "-s", session_name])
        .stdin(std::process::Stdio::null());
    let result = crate::process_utils::run_command_with_timeout(
        &mut result,
        STARTUP_COMMAND_TIMEOUT,
        "tmux new-session",
    );

    match result {
        Ok(output) if output.status.success() => {
            blog(log_path, &format!("Created tmux session '{session_name}'"));
        }
        Ok(output) => {
            bwarn(
                log_path,
                &format!("tmux new-session exited with status {:?}", output.status),
            );
        }
        Err(e) => {
            bwarn(log_path, &format!("Failed to create tmux session: {e}"));
        }
    }
}

fn ensure_tmux_session_wsl(distro: &str, session_name: &str, log_path: &Path) {
    if let Err(e) = validate_wsl_distro(distro) {
        bwarn(log_path, &format!("Invalid WSL distro for tmux: {e}"));
        return;
    }

    blog(
        log_path,
        &format!("Ensuring tmux session '{session_name}' exists"),
    );

    let mut check = wsl_command();
    check
        .args([
            "-d",
            distro,
            "--",
            "tmux",
            "has-session",
            "-t",
            session_name,
        ])
        .stdin(std::process::Stdio::null());
    let check = crate::process_utils::run_command_with_timeout(
        &mut check,
        STARTUP_COMMAND_TIMEOUT,
        "wsl tmux has-session",
    );

    if let Ok(output) = &check {
        if output.status.success() {
            blog(
                log_path,
                &format!("tmux session '{session_name}' already exists"),
            );
            return;
        }
    }

    let mut result = wsl_command();
    result
        .args([
            "-d",
            distro,
            "--",
            "tmux",
            "new-session",
            "-d",
            "-s",
            session_name,
        ])
        .stdin(std::process::Stdio::null());
    let result = crate::process_utils::run_command_with_timeout(
        &mut result,
        STARTUP_COMMAND_TIMEOUT,
        "wsl tmux new-session",
    );

    match result {
        Ok(output) if output.status.success() => {
            blog(log_path, &format!("Created tmux session '{session_name}'"));
        }
        Ok(output) => {
            bwarn(
                log_path,
                &format!("tmux new-session exited with status {:?}", output.status),
            );
        }
        Err(e) => {
            bwarn(
                log_path,
                &format!("Failed to create tmux session via wsl.exe: {e}"),
            );
        }
    }
}

/// Best-effort log path for the health check (which doesn't receive the
/// app's log path). Falls back to the known Windows app data location.
fn health_check_log_path() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata)
            .join("com.taurhaus.dev")
            .join(crate::commands::logging::JSONL_LOG_FILE_NAME)
    } else {
        PathBuf::from(format!(
            "/tmp/{}",
            crate::commands::logging::JSONL_LOG_FILE_NAME
        ))
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
    use crate::provider::local::LocalProvider;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct TestDaemon {
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

    fn reserve_free_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    fn spawn_test_daemon(port: u16, startup_delay: Duration) -> TestDaemon {
        let heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let shutdown = Arc::new(AtomicBool::new(false));
        let config = crate::daemon::server::DaemonConfig {
            port,
            bind_addr: "127.0.0.1".to_string(),
            idle_timeout_secs: None,
            auth_token: None,
        };
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            if startup_delay > Duration::ZERO {
                std::thread::sleep(startup_delay);
            }
            crate::daemon::server::run(&config, shutdown_clone, Arc::new(LocalProvider))
        });
        TestDaemon {
            shutdown,
            _heavy_guard: heavy_guard,
            handle: Some(handle),
        }
    }

    fn test_log_path() -> PathBuf {
        std::env::temp_dir().join("taurhaus-test.log")
    }

    #[test]
    fn no_distro_on_windows_returns_none() {
        // On native platforms (macOS/Linux), None distro still connects.
        // On Windows, None means no WSL projects → skip daemon.
        if !is_native_daemon() {
            let result = try_connect_daemon(None, DEFAULT_PORT, &test_log_path());
            assert!(result.is_none());
        }
    }

    #[test]
    fn connects_to_running_daemon() {
        let port = reserve_free_port();
        let daemon = spawn_test_daemon(port, Duration::ZERO);
        std::thread::sleep(Duration::from_millis(100));

        // On native platforms, distro is ignored — daemon connects directly.
        // On Windows/Linux-in-WSL, we'd need a valid distro.
        let distro = if is_native_daemon() {
            None
        } else {
            Some("Ubuntu")
        };
        let result = try_connect_daemon(distro, port, &test_log_path());
        assert!(result.is_some());

        daemon.shutdown.store(true, Ordering::Relaxed);
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
        let port = reserve_free_port();
        let daemon = spawn_test_daemon(port, Duration::from_millis(300));

        let result = poll_until_reachable(port, Duration::from_secs(3));
        assert!(
            result.is_some(),
            "Should connect to daemon that started after a delay"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn poll_until_reachable_times_out() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let start = Instant::now();
        let result = poll_until_reachable(port, Duration::from_secs(1));
        assert!(result.is_none(), "Should timeout when no daemon starts");
        assert!(
            start.elapsed() >= Duration::from_secs(1),
            "Should wait full timeout"
        );
    }

    #[test]
    fn reconnect_existing_provider_until_reachable_succeeds_when_daemon_starts() {
        let port = reserve_free_port();
        let provider = DaemonProvider::new_disconnected(&format!("127.0.0.1:{port}"));

        let daemon = spawn_test_daemon(port, Duration::from_millis(300));
        let start = Instant::now();
        let result = reconnect_existing_provider_until_reachable(&provider, port);

        assert!(
            result.is_ok(),
            "Should reconnect once the delayed daemon becomes reachable"
        );
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "Readiness reconnect should not wait for an unnecessary fixed delay"
        );
        assert!(
            provider.is_connected(),
            "Provider should be marked connected"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn validate_distro_accepts_valid_names() {
        assert!(validate_wsl_distro("Ubuntu").is_ok());
        assert!(validate_wsl_distro("Ubuntu-22.04").is_ok());
        assert!(validate_wsl_distro("Debian_11").is_ok());
        assert!(validate_wsl_distro("kali-linux").is_ok());
    }

    #[test]
    fn validate_distro_rejects_invalid_names() {
        assert!(validate_wsl_distro("").is_err());
        assert!(validate_wsl_distro("foo bar").is_err());
        assert!(validate_wsl_distro("foo;rm -rf /").is_err());
        assert!(validate_wsl_distro("test$(whoami)").is_err());
        assert!(validate_wsl_distro("test`id`").is_err());
    }

    // -----------------------------------------------------------------------
    // Platform dispatch tests
    // -----------------------------------------------------------------------

    #[test]
    fn is_native_daemon_matches_platform() {
        let native = is_native_daemon();
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert!(native, "macOS and Linux should use native daemon");
        }
        if cfg!(target_os = "windows") {
            assert!(!native, "Windows should use WSL daemon");
        }
    }

    #[test]
    fn daemon_binary_path_resolves_on_native() {
        if !is_native_daemon() {
            return; // WSL path resolution needs real WSL — skip on Windows
        }
        let path = daemon_binary_path("anything").unwrap();
        assert!(
            path.ends_with("/.local/bin/taurhaus-daemon"),
            "Native daemon path should end with ~/.local/bin/taurhaus-daemon, got: {path}"
        );
        assert!(
            path.starts_with('/'),
            "Native daemon path should be absolute, got: {path}"
        );
    }

    #[test]
    fn native_daemon_connects_with_none_distro() {
        // On native platforms, try_connect_daemon works with None distro
        // (uses synthetic "native" internally). Verify by connecting to
        // a real test server — if None distro caused early return, this
        // would fail.
        if !is_native_daemon() {
            return; // Only relevant on macOS/Linux
        }

        let port = reserve_free_port();
        let daemon = spawn_test_daemon(port, Duration::ZERO);
        std::thread::sleep(Duration::from_millis(100));

        // Key assertion: None distro doesn't cause early return on native.
        let result = try_connect_daemon(None, port, &test_log_path());
        assert!(
            result.is_some(),
            "None distro should still connect on native platforms"
        );

        daemon.shutdown.store(true, Ordering::Relaxed);
    }

    #[test]
    fn native_daemon_skips_wsl_validation() {
        // On native platforms, try_restart_daemon should NOT validate
        // the distro string against WSL name rules.
        if is_native_daemon() {
            let mut starter_called = false;
            let result = try_restart_daemon_with("native", 0, |distro, port, _log_path| {
                starter_called = true;
                assert_eq!(distro, "native");
                assert_eq!(port, 0);
                Err(std::io::Error::other("simulated starter error"))
            });

            assert!(starter_called, "starter should be called on native");
            assert!(result.is_err(), "mocked starter error should propagate");
            let err = result.err().unwrap().to_string();
            assert!(
                !err.contains("invalid"),
                "Should not fail on distro validation, got: {err}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn startup_daemon_binary_is_stale_for_deleted_inode_and_mismatched_binary() {
        // Regression: startup previously trusted any responding daemon on the
        // fast path, including deleted-inode or old-binary processes.
        let tempdir = tempfile::tempdir().unwrap();
        let expected = tempdir.path().join("taurhaus-daemon");
        let alternate = tempdir.path().join("taurhaus-daemon.old");
        std::fs::write(&expected, "expected").unwrap();
        std::fs::write(&alternate, "alternate").unwrap();

        assert!(
            !startup_daemon_binary_is_stale(&expected, &expected),
            "matching binary path should be accepted"
        );
        assert!(
            startup_daemon_binary_is_stale(&alternate, &expected),
            "different binary path should be rejected"
        );
        assert!(
            startup_daemon_binary_is_stale(
                &PathBuf::from(format!("{} (deleted)", expected.display())),
                &expected
            ),
            "deleted-inode binary should be rejected"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn startup_daemon_binary_is_stale_for_replaced_binary_with_same_visible_path() {
        use std::os::fd::AsRawFd;

        let tempdir = tempfile::tempdir().unwrap();
        let expected = tempdir.path().join("taurhaus-daemon");
        let replacement = tempdir.path().join("taurhaus-daemon.new");
        std::fs::write(&expected, "old").unwrap();
        let old_handle = std::fs::File::open(&expected).unwrap();

        std::fs::write(&replacement, "new").unwrap();
        std::fs::rename(&replacement, &expected).unwrap();

        let running_via_fd = PathBuf::from(format!("/proc/self/fd/{}", old_handle.as_raw_fd()));
        assert!(
            startup_daemon_binary_is_stale(&running_via_fd, &expected),
            "replaced binary should be rejected even when the visible install path is reused"
        );
    }

    #[test]
    fn health_check_log_path_resolves() {
        // Smoke test: should always return a path, never panic.
        let path = health_check_log_path();
        assert!(
            !path.as_os_str().is_empty(),
            "Health check log path should not be empty"
        );
    }

    #[test]
    fn daemon_data_dir_env_value_uses_parent_dir_for_native_launch() {
        let log_path = PathBuf::from("/tmp/taurhaus/taurhaus.log.jsonl");
        let resolved = daemon_data_dir_env_value_for_launch(&log_path, true);
        assert_eq!(resolved, Some("/tmp/taurhaus".to_string()));
    }

    #[test]
    fn daemon_data_dir_env_value_converts_windows_path_for_wsl_launch() {
        let log_path =
            PathBuf::from(r"C:\Users\me\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl");
        let resolved = daemon_data_dir_env_value_for_launch(&log_path, false);
        assert_eq!(
            resolved,
            Some("/mnt/c/Users/me/AppData/Roaming/com.taurhaus.dev".to_string())
        );
    }
}
