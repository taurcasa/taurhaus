use tauri::{Emitter, Manager, State};

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::launcher::{validate_wsl_distro, wsl_command};
use crate::daemon::protocol::PROTOCOL_VERSION;
use crate::daemon::server::DEFAULT_PORT;
use crate::models::{DaemonInstallStatus, DaemonStatus, OperationResult};
use crate::ProviderState;

const BUNDLED_VERSION: &str = env!("CARGO_PKG_VERSION");
const WSL_INSTALL_RESTART_MARKER: &str = "__TAURHAUS_DAEMON_WAS_RUNNING__=";

/// Get the current platform identifier.
///
/// Returns "macos", "linux", or "windows". Used by the frontend to show
/// platform-appropriate UI (e.g., wizard text about WSL vs native daemon).
#[tauri::command]
pub fn get_platform() -> String {
    let span = IpcCommandSpan::start("get_platform");
    let platform = if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "linux".to_string()
    };
    span.complete();
    platform
}

/// Get the current daemon connection status.
#[tauri::command]
pub fn get_daemon_status(provider: State<'_, ProviderState>) -> Result<DaemonStatus, String> {
    let span = IpcCommandSpan::start("get_daemon_status");
    // This command is used by splash startup and should never queue behind a
    // long-running shared daemon RPC such as git reseed or runtime snapshot.
    // Report connection state from the provider immediately; richer ping-based
    // metadata can be fetched on non-critical paths if we ever need it.
    let result = Ok(daemon_status_snapshot(&provider));
    span.finish_result(&result);
    result
}

fn daemon_status_snapshot(provider: &ProviderState) -> DaemonStatus {
    let status = match provider.daemon.as_ref() {
        None => "not_configured",
        Some(daemon) if daemon.is_connected() && daemon.is_busy() => "busy",
        Some(daemon) if daemon.is_connected() => "connected",
        Some(_) => "disconnected",
    };

    DaemonStatus {
        status: status.to_string(),
        version: None,
        protocol_version: 0,
        expected_protocol_version: PROTOCOL_VERSION,
        uptime_secs: None,
        port: DEFAULT_PORT,
        wsl_distro: provider.wsl_distro.clone(),
    }
}

/// Manually start the daemon process.
#[tauri::command]
pub fn start_daemon(
    provider: State<'_, ProviderState>,
    app: tauri::AppHandle,
) -> Result<OperationResult, String> {
    let span = IpcCommandSpan::start("start_daemon");
    let distro = match provider.wsl_distro.as_deref().ok_or_else(|| {
        if crate::daemon::launcher::is_native_daemon() {
            "No daemon configuration available".to_string()
        } else {
            "No WSL distro configured".to_string()
        }
    }) {
        Ok(distro) => distro,
        Err(error) => {
            span.fail_msg(&error);
            return Err(error);
        }
    };

    let port = DEFAULT_PORT;

    if let Err(error) = crate::daemon::launcher::try_restart_daemon(distro, port)
        .map_err(|e| format!("Failed to start daemon: {e}"))
    {
        span.fail_msg(&error);
        return Err(error);
    }

    // Wait a moment, then try to reconnect
    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(ref daemon) = provider.daemon {
        if daemon.reconnect().is_ok() {
            if let Err(error) = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "connected" }),
            ) {
                tracing::warn!(
                    error = %error,
                    "Failed to emit daemon-status event after reconnect"
                );
            }
            let result = Ok(OperationResult::success("Daemon started and connected"));
            span.finish_result(&result);
            return result;
        }
    }

    let result = Ok(OperationResult::success(
        "Daemon process started (not yet connected)",
    ));
    span.finish_result(&result);
    result
}

// ---------------------------------------------------------------------------
// Daemon auto-install commands (FirstRunWizard + startup update check)
// ---------------------------------------------------------------------------

/// Detect the default WSL distro name.
///
/// Runs `wsl -l -q` and returns the first line (the default distro).
/// Returns None if WSL is not available or no distro is configured.
fn detect_default_distro() -> Result<Option<String>, String> {
    let output = wsl_command()
        .args(["--list", "--quiet"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run wsl.exe: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(parse_distro_from_wsl_output(&output.stdout))
}

fn parse_distro_from_wsl_output(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .map(|l| l.replace('\0', "").trim().to_string())
        .find(|l| !l.is_empty())
}

/// Check whether the daemon binary is installed and compare versions.
///
/// On macOS/Linux: checks `~/.local/bin/taurhaus-daemon` directly.
/// On Windows: checks inside WSL.
///
/// Used by FirstRunWizard and startup update detection.
#[tauri::command]
pub fn check_daemon_install_status() -> Result<DaemonInstallStatus, String> {
    let span = IpcCommandSpan::start("check_daemon_install_status");
    let result = read_daemon_install_status();
    span.finish_result(&result);
    result
}

pub(crate) fn ensure_bundled_daemon_installed(
    app: &tauri::AppHandle,
) -> Result<Option<OperationResult>, String> {
    let status = read_daemon_install_status()?;
    if !daemon_install_required(&status) {
        return Ok(None);
    }

    install_bundled_daemon(app).map(Some)
}

fn read_daemon_install_status() -> Result<DaemonInstallStatus, String> {
    if crate::daemon::launcher::is_native_daemon() {
        check_daemon_install_native()
    } else {
        check_daemon_install_wsl()
    }
}

/// Native daemon check (macOS/Linux): just stat the binary and run --version.
fn check_daemon_install_native() -> Result<DaemonInstallStatus, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let binary = home.join(".local/bin/taurhaus-daemon");

    if !binary.exists() {
        return Ok(DaemonInstallStatus {
            installed: false,
            version: None,
            bundled_version: BUNDLED_VERSION.to_string(),
            needs_update: false,
            wsl_available: true, // "available" means daemon CAN run — true on native
            error: None,
        });
    }

    let version_output = std::process::Command::new(&binary)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    let version = match version_output {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            raw.trim()
                .strip_prefix("taurhaus-daemon ")
                .map(|v| v.trim().to_string())
        }
        _ => None,
    };

    let needs_update = match &version {
        Some(v) => semver_less_than(v, BUNDLED_VERSION),
        None => true,
    };

    Ok(DaemonInstallStatus {
        installed: true,
        version,
        bundled_version: BUNDLED_VERSION.to_string(),
        needs_update,
        wsl_available: true,
        error: None,
    })
}

/// WSL daemon check (Windows): probe WSL, detect distro, check binary inside WSL.
fn check_daemon_install_wsl() -> Result<DaemonInstallStatus, String> {
    // Step 1: Check WSL availability
    let wsl_check = wsl_command()
        .arg("--status")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match wsl_check {
        Err(_) => {
            return Ok(DaemonInstallStatus {
                installed: false,
                version: None,
                bundled_version: BUNDLED_VERSION.to_string(),
                needs_update: false,
                wsl_available: false,
                error: Some("WSL is not installed".to_string()),
            });
        }
        Ok(output) if !output.status.success() => {
            return Ok(DaemonInstallStatus {
                installed: false,
                version: None,
                bundled_version: BUNDLED_VERSION.to_string(),
                needs_update: false,
                wsl_available: false,
                error: Some("WSL is not available".to_string()),
            });
        }
        _ => {}
    }

    // Step 2: Detect default distro
    let distro = match detect_default_distro()? {
        Some(d) => d,
        None => {
            return Ok(DaemonInstallStatus {
                installed: false,
                version: None,
                bundled_version: BUNDLED_VERSION.to_string(),
                needs_update: false,
                wsl_available: true,
                error: Some("No WSL distro configured".to_string()),
            });
        }
    };

    if let Err(e) = validate_wsl_distro(&distro) {
        return Ok(DaemonInstallStatus {
            installed: false,
            version: None,
            bundled_version: BUNDLED_VERSION.to_string(),
            needs_update: false,
            wsl_available: true,
            error: Some(format!("Invalid WSL distro name: {e}")),
        });
    }

    // Step 3: Check if binary exists
    let exists = wsl_command()
        .args([
            "-d",
            &distro,
            "--",
            "test",
            "-f",
            "$HOME/.local/bin/taurhaus-daemon",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !exists {
        return Ok(DaemonInstallStatus {
            installed: false,
            version: None,
            bundled_version: BUNDLED_VERSION.to_string(),
            needs_update: false,
            wsl_available: true,
            error: None,
        });
    }

    // Step 4: Get installed version
    let version_output = wsl_command()
        .args([
            "-d",
            &distro,
            "--",
            "$HOME/.local/bin/taurhaus-daemon",
            "--version",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    let version = match version_output {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            raw.trim()
                .strip_prefix("taurhaus-daemon ")
                .map(|v| v.trim().to_string())
        }
        _ => None,
    };

    let needs_update = match &version {
        Some(v) => semver_less_than(v, BUNDLED_VERSION),
        None => true,
    };

    Ok(DaemonInstallStatus {
        installed: true,
        version,
        bundled_version: BUNDLED_VERSION.to_string(),
        needs_update,
        wsl_available: true,
        error: None,
    })
}

/// Install (or update) the daemon binary from bundled app resources.
///
/// On macOS/Linux: copies directly to `~/.local/bin/taurhaus-daemon`.
/// On Windows: copies into the default WSL distro.
#[tauri::command]
pub fn install_daemon(app: tauri::AppHandle) -> Result<OperationResult, String> {
    let span = IpcCommandSpan::start("install_daemon");
    let result = install_bundled_daemon(&app);
    span.finish_result(&result);
    result
}

fn install_bundled_daemon(app: &tauri::AppHandle) -> Result<OperationResult, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource directory: {e}"))?;
    let bundled_binary = resource_dir.join("resources").join("taurhaus-daemon");

    if !bundled_binary.exists() {
        return Err(format!(
            "Bundled daemon binary not found at {}",
            bundled_binary.display()
        ));
    }

    if crate::daemon::launcher::is_native_daemon() {
        install_daemon_native(&bundled_binary)
    } else {
        install_daemon_wsl(&bundled_binary)
    }
}

/// Install daemon natively (macOS/Linux): copy binary + chmod + verify.
fn install_daemon_native(bundled_binary: &std::path::Path) -> Result<OperationResult, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let target_dir = home.join(".local/bin");
    let target_path = target_dir.join("taurhaus-daemon");

    // Create target directory
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create ~/.local/bin: {e}"))?;

    // Copy binary
    std::fs::copy(bundled_binary, &target_path)
        .map_err(|e| format!("Failed to copy daemon binary: {e}"))?;

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set executable permission: {e}"))?;
    }

    // On macOS, re-sign the binary after copying.
    // Cargo's linker-signed adhoc binaries get invalidated on copy;
    // macOS Sequoia+ enforces code signature validity and kills unsigned binaries.
    #[cfg(target_os = "macos")]
    {
        let sign = std::process::Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&target_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output();
        match sign {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to sign daemon binary: {stderr}"));
            }
            Err(e) => {
                return Err(format!("Failed to run codesign: {e}"));
            }
        }
    }

    // Verify installation
    let verify = std::process::Command::new(&target_path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match verify {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let version = raw.trim();
            Ok(OperationResult::success(format!(
                "Daemon installed successfully: {version}"
            )))
        }
        Ok(_) => Err(
            "Daemon was copied but --version check failed. The binary may be corrupted."
                .to_string(),
        ),
        Err(e) => Err(format!("Daemon was copied but verification failed: {e}")),
    }
}

/// Install daemon via WSL (Windows): copy into WSL distro + chmod + verify.
fn install_daemon_wsl(bundled_binary: &std::path::Path) -> Result<OperationResult, String> {
    let distro = detect_default_distro()?.ok_or("No WSL distro configured")?;
    validate_wsl_distro(&distro).map_err(|e| format!("Invalid distro: {e}"))?;

    // Translate Windows/WSL paths to Linux where possible; keep native paths.
    let bundled_binary_str = bundled_binary.to_string_lossy();
    let wsl_source_path = crate::provider::path::to_linux(&bundled_binary_str)
        .unwrap_or_else(|| bundled_binary_str.to_string());

    let output = wsl_command()
        .args([
            "-d",
            &distro,
            "--",
            "sh",
            "-lc",
            install_daemon_wsl_script(),
            "taurhaus-install",
            &wsl_source_path,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to install daemon in WSL: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to install daemon in WSL: {stderr}"));
    }

    let result = parse_wsl_install_output(&output.stdout)?;

    if result.daemon_was_running {
        crate::daemon::launcher::try_restart_daemon(&distro, DEFAULT_PORT)
            .map_err(|e| format!("Daemon installed but restart failed: {e}"))?;
    }

    let message = if result.daemon_was_running {
        format!(
            "Daemon installed successfully: {} (daemon restarted)",
            result.version
        )
    } else {
        format!("Daemon installed successfully: {}", result.version)
    };

    Ok(OperationResult::success(message))
}

#[derive(Debug)]
struct WslInstallResult {
    version: String,
    daemon_was_running: bool,
}

fn install_daemon_wsl_script() -> &'static str {
    r#"set -eu
source_path="$1"
target_dir="$HOME/.local/bin"
target_path="$target_dir/taurhaus-daemon"
temp_path="$target_dir/.taurhaus-daemon.new.$$"
pattern='[t]aurhaus-daemon([[:space:]]|$)'
was_running=0

mkdir -p "$target_dir"

if pgrep -f "$pattern" >/dev/null 2>&1; then
  was_running=1
  pids="$(pgrep -f "$pattern" || true)"
  if [ -n "$pids" ]; then
    kill -TERM $pids || true
    for _ in $(seq 1 50); do
      if ! pgrep -f "$pattern" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    if pgrep -f "$pattern" >/dev/null 2>&1; then
      kill -KILL $pids || true
    fi
  fi
fi

cp "$source_path" "$temp_path"
chmod +x "$temp_path"
mv -f "$temp_path" "$target_path"
"$target_path" --version
printf '%s%s\n' "${WSL_INSTALL_RESTART_MARKER:-__TAURHAUS_DAEMON_WAS_RUNNING__=}" "$was_running"
"#
}

fn parse_wsl_install_output(stdout: &[u8]) -> Result<WslInstallResult, String> {
    let text = String::from_utf8_lossy(stdout);
    let mut version = None;
    let mut daemon_was_running = false;

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_RESTART_MARKER) {
            daemon_was_running = raw == "1";
            continue;
        }
        if version.is_none() {
            version = Some(line.to_string());
        }
    }

    let version = version.ok_or_else(|| {
        "WSL install completed but no daemon version was returned for verification".to_string()
    })?;

    Ok(WslInstallResult {
        version,
        daemon_was_running,
    })
}

/// Simple semver less-than comparison.
///
/// Compares major.minor.patch numerically. Returns true if `a` < `b`.
/// Falls back to string comparison if parsing fails.
fn semver_less_than(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };

    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a < b,
        _ => a < b, // String comparison as fallback
    }
}

fn daemon_install_required(status: &DaemonInstallStatus) -> bool {
    status.wsl_available && (!status.installed || status.needs_update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    fn canonical_linux_path(path: &std::path::Path) -> String {
        let path_str = path.to_string_lossy();
        crate::provider::path::to_linux(&path_str).unwrap_or_else(|| path_str.to_string())
    }

    #[test]
    fn semver_comparison() {
        assert!(semver_less_than("0.3.1", "0.3.2"));
        assert!(semver_less_than("0.2.9", "0.3.0"));
        assert!(semver_less_than("0.3.2", "1.0.0"));
        assert!(!semver_less_than("0.3.2", "0.3.2"));
        assert!(!semver_less_than("0.3.3", "0.3.2"));
        assert!(!semver_less_than("1.0.0", "0.9.9"));
    }

    #[test]
    fn semver_comparison_prerelease_and_malformed_edges() {
        // Falls back to string comparison when strict x.y.z parsing fails.
        assert!(!semver_less_than("0.3.2-alpha", "0.3.2"));
        assert!(semver_less_than("0.3.2", "0.3.2-alpha"));

        assert!(!semver_less_than("abc", "abc"));
        assert!(semver_less_than("abc", "abd"));
        assert!(semver_less_than("", "0.0.0"));
        assert!(!semver_less_than("", ""));
        assert!(semver_less_than("1.2", "1.2.3"));
        assert!(semver_less_than("1.2.3.4", "2.0.0"));
        assert!(!semver_less_than("1.2.3", "1.2.3"));
    }

    #[test]
    fn windows_path_translation() {
        let win = std::path::PathBuf::from(
            "C:\\Users\\mstie\\AppData\\Local\\com.taurhaus.dev\\resources\\taurhaus-daemon",
        );
        assert_eq!(
            canonical_linux_path(&win),
            "/mnt/c/Users/mstie/AppData/Local/com.taurhaus.dev/resources/taurhaus-daemon"
        );
    }

    #[test]
    fn windows_path_translation_edge_cases() {
        let d_drive = std::path::PathBuf::from("D:\\Work\\Agent Mesh\\taurhaus-daemon");
        assert_eq!(
            canonical_linux_path(&d_drive),
            "/mnt/d/Work/Agent Mesh/taurhaus-daemon"
        );

        let e_drive = std::path::PathBuf::from("e:\\Users\\foo\\bar baz\\daemon.exe");
        assert_eq!(
            canonical_linux_path(&e_drive),
            "/mnt/e/Users/foo/bar baz/daemon.exe"
        );

        let unc = std::path::PathBuf::from("\\\\server\\share\\daemon.exe");
        assert_eq!(canonical_linux_path(&unc), "\\\\server\\share\\daemon.exe");
    }

    #[test]
    fn unix_path_passthrough() {
        let unix = std::path::PathBuf::from("/home/mstie/.local/bin/taurhaus-daemon");
        let result = canonical_linux_path(&unix);
        assert_eq!(result, "/home/mstie/.local/bin/taurhaus-daemon");
    }

    #[test]
    fn parse_distro_from_wsl_output_handles_utf8() {
        let raw = b"Ubuntu\n";
        assert_eq!(
            parse_distro_from_wsl_output(raw),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn parse_distro_from_wsl_output_handles_utf16le_null_bytes() {
        let raw = b"U\0b\0u\0n\0t\0u\0\n\0";
        assert_eq!(
            parse_distro_from_wsl_output(raw),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn parse_distro_from_wsl_output_empty_and_whitespace() {
        assert_eq!(parse_distro_from_wsl_output(b""), None);
        assert_eq!(parse_distro_from_wsl_output(b"   \n\t\n"), None);
    }

    #[test]
    fn parse_distro_from_wsl_output_returns_first_non_empty_line() {
        let raw = b"\nUbuntu-22.04\nDebian\n";
        assert_eq!(
            parse_distro_from_wsl_output(raw),
            Some("Ubuntu-22.04".to_string())
        );
    }

    #[test]
    fn parse_wsl_install_output_reads_version_and_restart_marker() {
        let raw = b"taurhaus-daemon 0.5.3\n__TAURHAUS_DAEMON_WAS_RUNNING__=1\n";
        let result = parse_wsl_install_output(raw).expect("parsed");
        assert_eq!(result.version, "taurhaus-daemon 0.5.3");
        assert!(result.daemon_was_running);
    }

    #[test]
    fn parse_wsl_install_output_requires_version_line() {
        let err = parse_wsl_install_output(b"__TAURHAUS_DAEMON_WAS_RUNNING__=0\n")
            .expect_err("missing version should fail");
        assert!(err.contains("no daemon version"));
    }

    #[test]
    fn install_daemon_wsl_script_uses_atomic_swap_and_running_daemon_coordination() {
        let script = install_daemon_wsl_script();
        assert!(script.contains("kill -TERM"));
        assert!(script.contains("kill -KILL"));
        assert!(script.contains("mv -f \"$temp_path\" \"$target_path\""));
        assert!(script.contains("\"$target_path\" --version"));
    }

    #[test]
    fn daemon_install_required_when_binary_missing() {
        let status = DaemonInstallStatus {
            installed: false,
            version: None,
            bundled_version: "0.5.10".to_string(),
            needs_update: false,
            wsl_available: true,
            error: None,
        };

        assert!(daemon_install_required(&status));
    }

    #[test]
    fn daemon_install_required_when_binary_is_outdated() {
        let status = DaemonInstallStatus {
            installed: true,
            version: Some("0.5.9".to_string()),
            bundled_version: "0.5.10".to_string(),
            needs_update: true,
            wsl_available: true,
            error: None,
        };

        assert!(daemon_install_required(&status));
    }

    #[test]
    fn daemon_install_required_skips_when_environment_unavailable() {
        let status = DaemonInstallStatus {
            installed: false,
            version: None,
            bundled_version: "0.5.10".to_string(),
            needs_update: false,
            wsl_available: false,
            error: Some("WSL is not available".to_string()),
        };

        assert!(!daemon_install_required(&status));
    }

    #[test]
    fn daemon_status_snapshot_returns_connected_without_waiting_for_daemon_ping() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let accept_thread = std::thread::spawn(move || {
            let _stream = listener.accept().expect("accept client");
            std::thread::sleep(Duration::from_secs(2));
        });

        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&addr.to_string())
                    .expect("connect daemon provider"),
            ),
            wsl_distro: Some("Ubuntu".to_string()),
        };

        let started = Instant::now();
        let status = daemon_status_snapshot(&provider);
        let elapsed = started.elapsed();

        assert_eq!(status.status, "connected");
        assert_eq!(status.wsl_distro.as_deref(), Some("Ubuntu"));
        assert!(
            elapsed < Duration::from_millis(100),
            "status snapshot should not wait on daemon I/O; took {elapsed:?}"
        );

        drop(provider);
        accept_thread.join().expect("accept thread joined");
    }

    #[test]
    fn daemon_status_snapshot_reports_busy_without_treating_it_as_disconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr");
        let accept_thread = std::thread::spawn(move || {
            let _stream = listener.accept().expect("accept client");
            std::thread::sleep(Duration::from_secs(2));
        });

        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::connect(&addr.to_string())
                    .expect("connect daemon provider"),
            ),
            wsl_distro: Some("Ubuntu".to_string()),
        };

        std::thread::scope(|scope| {
            let daemon = provider.daemon.as_ref().expect("daemon provider");
            let _busy_thread = scope.spawn(|| {
                let request = crate::daemon_api::protocol::DaemonRequest::ping("busy-status");
                let _ = daemon.send_status_request(&request);
            });
            std::thread::sleep(Duration::from_millis(100));

            let status = daemon_status_snapshot(&provider);
            assert_eq!(status.status, "busy");
            assert_eq!(status.wsl_distro.as_deref(), Some("Ubuntu"));
        });

        accept_thread.join().expect("accept thread joined");
    }
}
