use std::path::{Path, PathBuf};

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::launcher::{is_native_daemon, validate_wsl_distro, wsl_command};
use crate::models::{MeshInstallStatus, OperationResult};
use tauri::Manager;

const MESH_BINARY_NAME: &str = "mesh";
const MESH_VERSION_RESOURCE: &str = "mesh.version";
const WSL_INSTALL_MEMBER_DAEMON_MARKER: &str = "__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=";
const WSL_INSTALL_TEAM_DAEMON_MARKER: &str = "__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=";

#[tauri::command]
pub fn check_mesh_install_status(app: tauri::AppHandle) -> Result<MeshInstallStatus, String> {
    let span = IpcCommandSpan::start("check_mesh_install_status");
    let result = {
        let bundled_version = read_bundled_mesh_version(&app)?;
        if is_native_daemon() {
            check_mesh_install_native(&bundled_version)
        } else {
            check_mesh_install_wsl(&bundled_version)
        }
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn install_mesh(app: tauri::AppHandle) -> Result<OperationResult, String> {
    let span = IpcCommandSpan::start("install_mesh");
    let result = {
        let (bundled_binary, bundled_version) = resolve_bundled_mesh_assets(&app)?;
        if is_native_daemon() {
            install_mesh_native(&bundled_binary, &bundled_version)
        } else {
            install_mesh_wsl(&app, &bundled_binary, &bundled_version)
        }
    };
    span.finish_result(&result);
    result
}

fn resolve_bundled_mesh_assets(app: &tauri::AppHandle) -> Result<(PathBuf, String), String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource directory: {e}"))?;
    let bundled_binary = resource_dir.join("resources").join(MESH_BINARY_NAME);
    if !bundled_binary.exists() {
        return Err(format!(
            "Bundled mesh binary not found at {}",
            bundled_binary.display()
        ));
    }
    let bundled_version = read_mesh_version_resource(&resource_dir.join("resources"))?;
    Ok((bundled_binary, bundled_version))
}

fn read_bundled_mesh_version(app: &tauri::AppHandle) -> Result<String, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource directory: {e}"))?;
    read_mesh_version_resource(&resource_dir.join("resources"))
}

fn read_mesh_version_resource(resources_dir: &Path) -> Result<String, String> {
    let version_path = resources_dir.join(MESH_VERSION_RESOURCE);
    if !version_path.exists() {
        return Err(format!(
            "Bundled mesh version file not found at {}",
            version_path.display()
        ));
    }

    let raw = std::fs::read_to_string(&version_path)
        .map_err(|e| format!("Failed to read bundled mesh version: {e}"))?;
    let version = raw.trim();
    if version.is_empty() {
        return Err(format!(
            "Bundled mesh version file is empty: {}",
            version_path.display()
        ));
    }
    Ok(version.to_string())
}

fn parse_distro_from_wsl_output(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .map(|line| line.replace('\0', "").trim().to_string())
        .find(|line| !line.is_empty())
}

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

fn parse_mesh_version(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines().find_map(|line| {
        let line = line.trim();
        let version = line.strip_prefix("mesh ")?;
        version
            .split_whitespace()
            .next()
            .map(std::string::ToString::to_string)
    })
}

fn check_mesh_install_native(bundled_version: &str) -> Result<MeshInstallStatus, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let binary = home.join(".local/bin/mesh");

    if !binary.exists() {
        return Ok(MeshInstallStatus {
            installed: false,
            version: None,
            bundled_version: bundled_version.to_string(),
            needs_update: false,
            environment_available: true,
            error: None,
        });
    }

    let version_output = std::process::Command::new(&binary)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    let version = match version_output {
        Ok(output) if output.status.success() => parse_mesh_version(&output.stdout),
        _ => None,
    };

    let needs_update = match &version {
        Some(v) => v != bundled_version,
        None => true,
    };

    Ok(MeshInstallStatus {
        installed: true,
        version,
        bundled_version: bundled_version.to_string(),
        needs_update,
        environment_available: true,
        error: None,
    })
}

fn check_mesh_install_wsl(bundled_version: &str) -> Result<MeshInstallStatus, String> {
    let wsl_check = wsl_command()
        .arg("--status")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match wsl_check {
        Err(_) => {
            return Ok(MeshInstallStatus {
                installed: false,
                version: None,
                bundled_version: bundled_version.to_string(),
                needs_update: false,
                environment_available: false,
                error: Some("WSL is not installed".to_string()),
            });
        }
        Ok(output) if !output.status.success() => {
            return Ok(MeshInstallStatus {
                installed: false,
                version: None,
                bundled_version: bundled_version.to_string(),
                needs_update: false,
                environment_available: false,
                error: Some("WSL is not available".to_string()),
            });
        }
        _ => {}
    }

    let distro = match detect_default_distro()? {
        Some(d) => d,
        None => {
            return Ok(MeshInstallStatus {
                installed: false,
                version: None,
                bundled_version: bundled_version.to_string(),
                needs_update: false,
                environment_available: true,
                error: Some("No WSL distro configured".to_string()),
            });
        }
    };

    if let Err(e) = validate_wsl_distro(&distro) {
        return Ok(MeshInstallStatus {
            installed: false,
            version: None,
            bundled_version: bundled_version.to_string(),
            needs_update: false,
            environment_available: true,
            error: Some(format!("Invalid WSL distro name: {e}")),
        });
    }

    let exists = wsl_command()
        .args(["-d", &distro, "--", "test", "-f", "$HOME/.local/bin/mesh"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);

    if !exists {
        return Ok(MeshInstallStatus {
            installed: false,
            version: None,
            bundled_version: bundled_version.to_string(),
            needs_update: false,
            environment_available: true,
            error: None,
        });
    }

    let version_output = wsl_command()
        .args(["-d", &distro, "--", "$HOME/.local/bin/mesh", "--version"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    let version = match version_output {
        Ok(output) if output.status.success() => parse_mesh_version(&output.stdout),
        _ => None,
    };

    let needs_update = match &version {
        Some(v) => v != bundled_version,
        None => true,
    };

    Ok(MeshInstallStatus {
        installed: true,
        version,
        bundled_version: bundled_version.to_string(),
        needs_update,
        environment_available: true,
        error: None,
    })
}

fn install_mesh_native(
    bundled_binary: &Path,
    bundled_version: &str,
) -> Result<OperationResult, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let target_dir = home.join(".local/bin");
    let target_path = target_dir.join("mesh");
    let temp_path = target_dir.join(".mesh.new");

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create ~/.local/bin: {e}"))?;
    std::fs::copy(bundled_binary, &temp_path).map_err(|e| format!("Failed to copy mesh: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set executable permission: {e}"))?;
    }

    std::fs::rename(&temp_path, &target_path)
        .map_err(|e| format!("Failed to install mesh binary: {e}"))?;

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
                return Err(format!("Failed to sign mesh binary: {stderr}"));
            }
            Err(e) => {
                return Err(format!("Failed to run codesign: {e}"));
            }
        }
    }

    let verify = std::process::Command::new(&target_path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    match verify {
        Ok(output) if output.status.success() => {
            let installed_version = parse_mesh_version(&output.stdout)
                .ok_or("Mesh was installed but version output was invalid")?;
            if installed_version != bundled_version {
                return Err(format!(
                    "Installed mesh version {installed_version} does not match bundled version {bundled_version}"
                ));
            }
            Ok(OperationResult::success(format!(
                "Mesh installed successfully: mesh {installed_version}"
            )))
        }
        Ok(_) => Err("Mesh was copied but --version check failed.".to_string()),
        Err(e) => Err(format!("Mesh was copied but verification failed: {e}")),
    }
}

fn install_mesh_wsl(
    app: &tauri::AppHandle,
    bundled_binary: &Path,
    bundled_version: &str,
) -> Result<OperationResult, String> {
    let distro = detect_default_distro()?.ok_or("No WSL distro configured")?;
    validate_wsl_distro(&distro).map_err(|e| format!("Invalid distro: {e}"))?;

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
            install_mesh_wsl_script(),
            "taurhaus-install",
            &wsl_source_path,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to install mesh in WSL: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to install mesh in WSL: {stderr}"));
    }

    let result = parse_mesh_wsl_install_output(&output.stdout)?;
    if result.version != format!("mesh {bundled_version}") {
        let installed_version =
            parse_mesh_version(result.version.as_bytes()).unwrap_or_else(|| result.version.clone());
        return Err(format!(
            "Installed mesh version {installed_version} does not match bundled version {bundled_version}"
        ));
    }

    let any_daemons_were_running =
        result.member_daemons_were_running || result.team_daemons_were_running;
    let self_heal_summary = if any_daemons_were_running {
        Some(run_mesh_install_self_heal(app)?)
    } else {
        None
    };

    let message = match self_heal_summary {
        Some(summary) => format!(
            "Mesh installed successfully: {} (cycled {} team daemon{}, repaired {} team{})",
            result.version,
            summary.team_daemons_ensured,
            if summary.team_daemons_ensured == 1 {
                ""
            } else {
                "s"
            },
            summary.teams_reconciled,
            if summary.teams_reconciled == 1 {
                ""
            } else {
                "s"
            },
        ),
        None => format!("Mesh installed successfully: {}", result.version),
    };

    Ok(OperationResult::success(message))
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_mesh_install_self_heal(
    app: &tauri::AppHandle,
) -> Result<crate::coordination::state::BackgroundSelfHealPassResult, String> {
    let state = app.state::<crate::coordination::state::CoordinationState>();
    let summary = state
        .run_background_self_heal_pass()
        .map_err(|e| format!("Mesh installed but daemon self-heal failed: {e}"))?;
    if summary.team_errors > 0 {
        return Err(format!(
            "Mesh installed but daemon self-heal reported {} team error{}",
            summary.team_errors,
            if summary.team_errors == 1 { "" } else { "s" }
        ));
    }
    Ok(summary)
}

#[cfg(not(feature = "mesh-bridged-backend"))]
fn run_mesh_install_self_heal(app: &tauri::AppHandle) -> Result<(), String> {
    let _ = app;
    Ok(())
}

#[derive(Debug)]
struct WslMeshInstallResult {
    version: String,
    member_daemons_were_running: bool,
    team_daemons_were_running: bool,
}

fn install_mesh_wsl_script() -> &'static str {
    r#"set -eu
source_path="$1"
target_dir="$HOME/.local/bin"
target_path="$target_dir/mesh"
temp_path="$target_dir/.mesh.new.$$"
member_pattern='[m]esh([[:space:]]|$).*[[:space:]]daemon([[:space:]]|$).*--pane([[:space:]]|$)'
team_pattern='[m]esh([[:space:]]|$).*team-daemon([[:space:]]|$).*start([[:space:]]|$)'
member_daemons_were_running=0
team_daemons_were_running=0

mkdir -p "$target_dir"

if pgrep -f "$member_pattern" >/dev/null 2>&1; then
  member_daemons_were_running=1
  member_pids="$(pgrep -f "$member_pattern" || true)"
  if [ -n "$member_pids" ]; then
    kill -TERM $member_pids || true
    for _ in $(seq 1 50); do
      if ! pgrep -f "$member_pattern" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    if pgrep -f "$member_pattern" >/dev/null 2>&1; then
      kill -KILL $member_pids || true
    fi
  fi
fi

if pgrep -f "$team_pattern" >/dev/null 2>&1; then
  team_daemons_were_running=1
  team_pids="$(pgrep -f "$team_pattern" || true)"
  if [ -n "$team_pids" ]; then
    kill -TERM $team_pids || true
    for _ in $(seq 1 50); do
      if ! pgrep -f "$team_pattern" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    if pgrep -f "$team_pattern" >/dev/null 2>&1; then
      kill -KILL $team_pids || true
    fi
  fi
fi

cp "$source_path" "$temp_path"
chmod +x "$temp_path"
mv -f "$temp_path" "$target_path"
"$target_path" --version
printf '%s%s\n' "${WSL_INSTALL_MEMBER_DAEMON_MARKER:-__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=}" "$member_daemons_were_running"
printf '%s%s\n' "${WSL_INSTALL_TEAM_DAEMON_MARKER:-__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=}" "$team_daemons_were_running"
"#
}

fn parse_mesh_wsl_install_output(stdout: &[u8]) -> Result<WslMeshInstallResult, String> {
    let text = String::from_utf8_lossy(stdout);
    let mut version = None;
    let mut member_daemons_were_running = false;
    let mut team_daemons_were_running = false;

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_MEMBER_DAEMON_MARKER) {
            member_daemons_were_running = raw == "1";
            continue;
        }
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_TEAM_DAEMON_MARKER) {
            team_daemons_were_running = raw == "1";
            continue;
        }
        if version.is_none() {
            version = Some(line.to_string());
        }
    }

    let version = version.ok_or_else(|| {
        "WSL install completed but no mesh version was returned for verification".to_string()
    })?;

    Ok(WslMeshInstallResult {
        version,
        member_daemons_were_running,
        team_daemons_were_running,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[cfg(not(target_os = "windows"))]
    fn write_executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, body).expect("write script");
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod");
    }

    #[test]
    fn parse_mesh_version_returns_token_after_prefix() {
        assert_eq!(
            parse_mesh_version(b"mesh 0.2.0\n"),
            Some("0.2.0".to_string())
        );
        assert_eq!(
            parse_mesh_version(b"mesh 0.2.0-dev+abc\n"),
            Some("0.2.0-dev+abc".to_string())
        );
    }

    #[test]
    fn parse_mesh_version_ignores_unrelated_output() {
        assert_eq!(parse_mesh_version(b"no version here\n"), None);
        assert_eq!(parse_mesh_version(b""), None);
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
    fn parse_mesh_wsl_install_output_reads_version_and_daemon_markers() {
        let raw = b"mesh 0.5.3\n__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=1\n__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=0\n";
        let result = parse_mesh_wsl_install_output(raw).expect("parsed");
        assert_eq!(result.version, "mesh 0.5.3");
        assert!(result.member_daemons_were_running);
        assert!(!result.team_daemons_were_running);
    }

    #[test]
    fn parse_mesh_wsl_install_output_requires_version_line() {
        let err = parse_mesh_wsl_install_output(
            b"__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=0\n__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=0\n",
        )
        .expect_err("missing version should fail");
        assert!(err.contains("no mesh version"));
    }

    #[test]
    fn install_mesh_wsl_script_uses_atomic_swap_and_emits_daemon_cycle_markers() {
        let script = install_mesh_wsl_script();
        assert!(script.contains("temp_path=\"$target_dir/.mesh.new.$$\""));
        assert!(script.contains("mv -f \"$temp_path\" \"$target_path\""));
        assert!(script.contains("pgrep -f \"$member_pattern\""));
        assert!(script.contains("pgrep -f \"$team_pattern\""));
        assert!(script.contains("[[:space:]]daemon([[:space:]]|$).*--pane"));
        assert!(script.contains("kill -TERM $member_pids || true"));
        assert!(script.contains("kill -TERM $team_pids || true"));
        assert!(script.contains(WSL_INSTALL_MEMBER_DAEMON_MARKER));
        assert!(script.contains(WSL_INSTALL_TEAM_DAEMON_MARKER));
        assert!(script.contains("\"$target_path\" --version"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn install_mesh_wsl_script_executes_atomic_swap_with_live_daemon_like_processes() {
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let bin_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let installed_mesh = bin_dir.join("mesh");
        let source_mesh = temp_home.path().join("mesh-new");

        write_executable(
            &installed_mesh,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "mesh 0.1.0"
  exit 0
fi
exit 0
"#,
        );
        write_executable(
            &source_mesh,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "mesh 9.9.9"
  exit 0
fi
exit 0
"#,
        );

        let mut member = Command::new("bash")
            .args([
                "-lc",
                "exec -a 'mesh daemon --pane %9 --team alpha --name dev' sleep 100",
            ])
            .spawn()
            .expect("spawn member daemon");
        let mut team = Command::new("bash")
            .args([
                "-lc",
                "exec -a 'mesh team-daemon start --team alpha --name lead' sleep 100",
            ])
            .spawn()
            .expect("spawn team daemon");

        std::thread::sleep(Duration::from_millis(150));

        let output = Command::new("sh")
            .arg("-lc")
            .arg(install_mesh_wsl_script())
            .arg("taurhaus-install")
            .arg(&source_mesh)
            .env("HOME", temp_home.path())
            .output()
            .expect("run install script");

        assert!(
            output.status.success(),
            "install script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let parsed = parse_mesh_wsl_install_output(&output.stdout).expect("parse install output");
        assert_eq!(parsed.version, "mesh 9.9.9");
        assert_eq!(
            std::fs::read_to_string(&installed_mesh).expect("installed mesh"),
            std::fs::read_to_string(&source_mesh).expect("source mesh"),
            "installed binary should be atomically replaced by the new source"
        );

        let _ = member.kill();
        let _ = member.wait();
        let _ = team.kill();
        let _ = team.wait();
    }
}
