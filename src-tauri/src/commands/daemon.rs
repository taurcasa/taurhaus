use tauri::{Emitter, Manager, State};

use crate::daemon::launcher::{validate_wsl_distro, wsl_command};
use crate::daemon::protocol::{self, PingResult, PROTOCOL_VERSION};
use crate::daemon::server::DEFAULT_PORT;
use crate::models::{DaemonInstallStatus, DaemonStatus};
use crate::ProviderState;

const BUNDLED_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Get the current platform identifier.
///
/// Returns "macos", "linux", or "windows". Used by the frontend to show
/// platform-appropriate UI (e.g., wizard text about WSL vs native daemon).
#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "linux".to_string()
    }
}

/// Get the current daemon connection status.
#[tauri::command]
pub fn get_daemon_status(provider: State<'_, ProviderState>) -> Result<DaemonStatus, String> {
    let port = DEFAULT_PORT;

    let Some(ref daemon) = provider.daemon else {
        return Ok(DaemonStatus {
            status: "not_configured".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        });
    };

    if !daemon.is_connected() {
        return Ok(DaemonStatus {
            status: "disconnected".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        });
    }

    // Try a ping to get version and uptime
    let id = "status-ping";
    let request = protocol::DaemonRequest::ping(id);
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            let ping: Option<PingResult> = response
                .result
                .and_then(|v| serde_json::from_value(v).ok());
            Ok(DaemonStatus {
                status: "connected".to_string(),
                version: ping.as_ref().map(|p| p.version.clone()),
                protocol_version: ping.as_ref().map(|p| p.protocol_version).unwrap_or(0),
                expected_protocol_version: PROTOCOL_VERSION,
                uptime_secs: ping.as_ref().map(|p| p.uptime_secs),
                port,
                wsl_distro: provider.wsl_distro.clone(),
            })
        }
        _ => Ok(DaemonStatus {
            status: "disconnected".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        }),
    }
}

/// Manually start the daemon process.
#[tauri::command]
pub fn start_daemon(
    provider: State<'_, ProviderState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let distro = provider
        .wsl_distro
        .as_deref()
        .ok_or_else(|| {
            if crate::daemon::launcher::is_native_daemon() {
                "No daemon configuration available".to_string()
            } else {
                "No WSL distro configured".to_string()
            }
        })?;

    let port = DEFAULT_PORT;

    crate::daemon::launcher::try_restart_daemon(distro, port)
        .map_err(|e| format!("Failed to start daemon: {e}"))?;

    // Wait a moment, then try to reconnect
    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(ref daemon) = provider.daemon {
        if daemon.reconnect().is_ok() {
            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "connected" }),
            );
            return Ok("Daemon started and connected".to_string());
        }
    }

    Ok("Daemon process started (not yet connected)".to_string())
}

/// Manually stop the daemon process.
#[tauri::command]
pub fn stop_daemon(
    provider: State<'_, ProviderState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let Some(ref daemon) = provider.daemon else {
        return Err("No daemon configured".to_string());
    };

    if !daemon.is_connected() {
        return Ok("Daemon already disconnected".to_string());
    }

    // Send shutdown command
    let id = "manual-shutdown";
    let request = protocol::DaemonRequest::new(id, protocol::method::SHUTDOWN, serde_json::Value::Null);
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "disconnected" }),
            );
            Ok("Daemon stopped".to_string())
        }
        Ok(response) => Err(format!(
            "Shutdown failed: {}",
            response.error.map(|e| e.message).unwrap_or_default()
        )),
        Err(e) => Err(format!("Failed to send shutdown: {e}")),
    }
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

    // wsl -l -q output on Windows uses UTF-16LE with null bytes between chars.
    // Try UTF-8 first, then try cleaning null bytes.
    let raw = String::from_utf8_lossy(&output.stdout);
    let first_line = raw
        .lines()
        .map(|l| l.replace('\0', "").trim().to_string())
        .find(|l| !l.is_empty());

    Ok(first_line)
}

/// Check whether the daemon binary is installed and compare versions.
///
/// On macOS/Linux: checks `~/.local/bin/taurhaus-daemon` directly.
/// On Windows: checks inside WSL.
///
/// Used by FirstRunWizard and startup update detection.
#[tauri::command]
pub fn check_daemon_install_status() -> Result<DaemonInstallStatus, String> {
    if crate::daemon::launcher::is_native_daemon() {
        check_daemon_install_native()
    } else {
        check_daemon_install_wsl()
    }
}

/// Native daemon check (macOS/Linux): just stat the binary and run --version.
fn check_daemon_install_native() -> Result<DaemonInstallStatus, String> {
    let home = dirs::home_dir()
        .ok_or("Could not determine home directory")?;
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
        .args(["-d", &distro, "--", "test", "-f", "$HOME/.local/bin/taurhaus-daemon"])
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
        .args(["-d", &distro, "--", "$HOME/.local/bin/taurhaus-daemon", "--version"])
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
pub fn install_daemon(app: tauri::AppHandle) -> Result<String, String> {
    // Resolve bundled binary path from Tauri resources
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
fn install_daemon_native(bundled_binary: &std::path::Path) -> Result<String, String> {
    let home = dirs::home_dir()
        .ok_or("Could not determine home directory")?;
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
            Ok(format!("Daemon installed successfully: {version}"))
        }
        Ok(_) => {
            Err("Daemon was copied but --version check failed. The binary may be corrupted.".to_string())
        }
        Err(e) => {
            Err(format!("Daemon was copied but verification failed: {e}"))
        }
    }
}

/// Install daemon via WSL (Windows): copy into WSL distro + chmod + verify.
fn install_daemon_wsl(bundled_binary: &std::path::Path) -> Result<String, String> {
    let distro = detect_default_distro()?
        .ok_or("No WSL distro configured")?;
    validate_wsl_distro(&distro)
        .map_err(|e| format!("Invalid distro: {e}"))?;

    // Translate Windows path to WSL-accessible /mnt/... path.
    let wsl_source_path = windows_to_wsl_path(bundled_binary)?;

    // Create target directory
    let mkdir = wsl_command()
        .args(["-d", &distro, "--", "mkdir", "-p", "$HOME/.local/bin"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to create target directory: {e}"))?;

    if !mkdir.status.success() {
        let stderr = String::from_utf8_lossy(&mkdir.stderr);
        return Err(format!("Failed to create ~/.local/bin: {stderr}"));
    }

    // Copy binary
    let cp = wsl_command()
        .args([
            "-d", &distro, "--",
            "cp", &wsl_source_path, "$HOME/.local/bin/taurhaus-daemon",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to copy daemon binary: {e}"))?;

    if !cp.status.success() {
        let stderr = String::from_utf8_lossy(&cp.stderr);
        return Err(format!("Failed to copy daemon binary: {stderr}"));
    }

    // Set executable permissions
    let chmod = wsl_command()
        .args(["-d", &distro, "--", "chmod", "+x", "$HOME/.local/bin/taurhaus-daemon"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to set permissions: {e}"))?;

    if !chmod.status.success() {
        let stderr = String::from_utf8_lossy(&chmod.stderr);
        return Err(format!("Failed to set executable permission: {stderr}"));
    }

    // Verify installation
    let verify = wsl_command()
        .args(["-d", &distro, "--", "$HOME/.local/bin/taurhaus-daemon", "--version"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match verify {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let version = raw.trim();
            Ok(format!("Daemon installed successfully: {version}"))
        }
        Ok(_) => {
            Err("Daemon was copied but --version check failed. The binary may be corrupted.".to_string())
        }
        Err(e) => {
            Err(format!("Daemon was copied but verification failed: {e}"))
        }
    }
}

/// Translate a Windows path to a WSL-accessible `/mnt/...` path.
///
/// On actual Windows: `C:\Users\foo\bar` → `/mnt/c/Users/foo/bar`
/// On Linux (dev mode): returns the path as-is since it's already accessible.
fn windows_to_wsl_path(path: &std::path::Path) -> Result<String, String> {
    let path_str = path.to_string_lossy();

    // Check if it looks like a Windows path (drive letter)
    if path_str.len() >= 3 && path_str.as_bytes()[1] == b':' {
        let drive = (path_str.as_bytes()[0] as char).to_ascii_lowercase();
        let rest = &path_str[2..].replace('\\', "/");
        Ok(format!("/mnt/{drive}{rest}"))
    } else {
        // Already a Unix path (dev mode on Linux)
        Ok(path_str.to_string())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn windows_path_translation() {
        let win = std::path::PathBuf::from("C:\\Users\\mstie\\AppData\\Local\\com.taurhaus.dev\\resources\\taurhaus-daemon");
        // On Linux this is still a valid PathBuf, just treated as a single component
        // The function checks for drive letter pattern
        let result = windows_to_wsl_path(&win);
        assert!(result.is_ok());
    }

    #[test]
    fn unix_path_passthrough() {
        let unix = std::path::PathBuf::from("/home/mstie/.local/bin/taurhaus-daemon");
        let result = windows_to_wsl_path(&unix).unwrap();
        assert_eq!(result, "/home/mstie/.local/bin/taurhaus-daemon");
    }
}
