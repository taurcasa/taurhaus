use std::path::{Path, PathBuf};

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::launcher::{is_native_daemon, validate_wsl_distro, wsl_command};
use crate::models::{MeshInstallStatus, OperationResult};
use tauri::Manager;

const MESH_BINARY_NAME: &str = "mesh";
const MESH_VERSION_RESOURCE: &str = "mesh.version";

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
            install_mesh_wsl(&bundled_binary, &bundled_version)
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
    bundled_binary: &Path,
    bundled_version: &str,
) -> Result<OperationResult, String> {
    let distro = detect_default_distro()?.ok_or("No WSL distro configured")?;
    validate_wsl_distro(&distro).map_err(|e| format!("Invalid distro: {e}"))?;

    let bundled_binary_str = bundled_binary.to_string_lossy();
    let wsl_source_path = crate::provider::path::to_linux(&bundled_binary_str)
        .unwrap_or_else(|| bundled_binary_str.to_string());

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

    let cp = wsl_command()
        .args([
            "-d",
            &distro,
            "--",
            "cp",
            &wsl_source_path,
            "$HOME/.local/bin/mesh",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to copy mesh binary: {e}"))?;
    if !cp.status.success() {
        let stderr = String::from_utf8_lossy(&cp.stderr);
        return Err(format!("Failed to copy mesh binary: {stderr}"));
    }

    let chmod = wsl_command()
        .args(["-d", &distro, "--", "chmod", "+x", "$HOME/.local/bin/mesh"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to set permissions: {e}"))?;
    if !chmod.status.success() {
        let stderr = String::from_utf8_lossy(&chmod.stderr);
        return Err(format!("Failed to set executable permission: {stderr}"));
    }

    let verify = wsl_command()
        .args(["-d", &distro, "--", "$HOME/.local/bin/mesh", "--version"])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mesh_version_returns_token_after_prefix() {
        assert_eq!(
            parse_mesh_version(b"mesh 0.1.0\n"),
            Some("0.1.0".to_string())
        );
        assert_eq!(
            parse_mesh_version(b"mesh 0.1.0-dev+abc\n"),
            Some("0.1.0-dev+abc".to_string())
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
}
