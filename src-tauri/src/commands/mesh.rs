use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::launcher::{is_native_daemon, validate_wsl_distro, wsl_command};
use crate::models::{
    MeshCompatibilityContract, MeshCompatibilityIssue, MeshInstallStatus, OperationResult,
};
use serde::Deserialize;
use tauri::Manager;

const MESH_BINARY_NAME: &str = "mesh";
const MESH_MANIFEST_RESOURCE: &str = "mesh.manifest.json";
const WSL_MESH_BINARY_PATH: &str = "$HOME/.local/bin/mesh";
const WSL_INSTALL_VERSION_JSON_MARKER: &str = "__TAURHAUS_MESH_VERSION_JSON__=";
const WSL_INSTALL_MEMBER_DAEMON_MARKER: &str = "__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=";
const WSL_INSTALL_TEAM_DAEMON_MARKER: &str = "__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=";
const INSTALL_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const INSTALL_ACTION_TIMEOUT: Duration = Duration::from_secs(12);

#[tauri::command]
pub fn check_mesh_install_status(app: tauri::AppHandle) -> Result<MeshInstallStatus, String> {
    let span = IpcCommandSpan::start("check_mesh_install_status");
    let result = read_mesh_install_status(&app);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn install_mesh(app: tauri::AppHandle) -> Result<OperationResult, String> {
    let span = IpcCommandSpan::start("install_mesh");
    let result = install_bundled_mesh(&app);
    span.finish_result(&result);
    result
}

pub(crate) fn ensure_bundled_mesh_installed(
    app: &tauri::AppHandle,
) -> Result<Option<OperationResult>, String> {
    let status = read_mesh_install_status(app)?;
    let install_required = mesh_install_required(&status);
    if !install_required {
        return Ok(None);
    }

    install_bundled_mesh(app).map(Some)
}

fn mesh_install_required(status: &MeshInstallStatus) -> bool {
    status.environment_available && (!status.installed || !status.compatibility_issues.is_empty())
}

fn read_mesh_install_status(app: &tauri::AppHandle) -> Result<MeshInstallStatus, String> {
    let bundled_contract = read_bundled_mesh_contract(app)?;
    if is_native_daemon() {
        check_mesh_install_native(&bundled_contract)
    } else {
        check_mesh_install_wsl(&bundled_contract)
    }
}

fn install_bundled_mesh(app: &tauri::AppHandle) -> Result<OperationResult, String> {
    let (bundled_binary, bundled_contract) = resolve_bundled_mesh_assets(app)?;
    if is_native_daemon() {
        install_mesh_native(app, &bundled_binary, &bundled_contract)
    } else {
        install_mesh_wsl(app, &bundled_binary, &bundled_contract)
    }
}

fn resolve_bundled_mesh_assets(
    app: &tauri::AppHandle,
) -> Result<(PathBuf, MeshCompatibilityContract), String> {
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
    let bundled_contract = read_mesh_manifest_resource(&resource_dir.join("resources"))?;
    Ok((bundled_binary, bundled_contract))
}

fn read_bundled_mesh_contract(app: &tauri::AppHandle) -> Result<MeshCompatibilityContract, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource directory: {e}"))?;
    read_mesh_manifest_resource(&resource_dir.join("resources"))
}

fn read_mesh_manifest_resource(resources_dir: &Path) -> Result<MeshCompatibilityContract, String> {
    let manifest_path = resources_dir.join(MESH_MANIFEST_RESOURCE);
    if !manifest_path.exists() {
        return Err(format!(
            "Bundled mesh manifest file not found at {}",
            manifest_path.display()
        ));
    }

    let raw = std::fs::read(&manifest_path)
        .map_err(|e| format!("Failed to read bundled mesh manifest: {e}"))?;
    parse_mesh_contract_json(&raw).map_err(|e| {
        format!(
            "Bundled mesh manifest is invalid at {}: {e}",
            manifest_path.display()
        )
    })
}

fn parse_distro_from_wsl_output(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .map(|line| line.replace('\0', "").trim().to_string())
        .find(|line| !line.is_empty())
}

fn detect_default_distro() -> Result<Option<String>, String> {
    let output = crate::process_utils::run_command_with_timeout(
        wsl_command().args(["--list", "--quiet"]),
        INSTALL_STATUS_TIMEOUT,
        "wsl --list --quiet",
    )
    .map_err(|e| format!("Failed to run wsl.exe: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(parse_distro_from_wsl_output(&output.stdout))
}

#[derive(Debug, Deserialize)]
struct WireMeshCompatibilityContract {
    version: String,
    protocol_version: u32,
    schema_version: u32,
    #[serde(default)]
    git_commit: Option<String>,
}

fn normalize_optional_git_commit(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_mesh_contract_json(raw: &[u8]) -> Result<MeshCompatibilityContract, String> {
    let parsed: WireMeshCompatibilityContract =
        serde_json::from_slice(raw).map_err(|e| format!("failed to parse JSON: {e}"))?;
    let version = parsed.version.trim();
    if version.is_empty() {
        return Err("missing required non-empty \"version\" field".to_string());
    }

    Ok(MeshCompatibilityContract {
        version: version.to_string(),
        protocol_version: parsed.protocol_version,
        schema_version: parsed.schema_version,
        git_commit: normalize_optional_git_commit(parsed.git_commit),
    })
}

fn format_mesh_command_error(context: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("{context} failed with status {}", output.status)
    } else {
        format!("{context} failed: {stderr}")
    }
}

fn read_mesh_contract_from_output(
    context: &str,
    output: std::process::Output,
) -> Result<MeshCompatibilityContract, String> {
    if !output.status.success() {
        return Err(format_mesh_command_error(context, &output));
    }
    parse_mesh_contract_json(&output.stdout)
        .map_err(|e| format!("{context} returned invalid JSON: {e}"))
}

fn read_mesh_contract_native(binary: &Path) -> Result<MeshCompatibilityContract, String> {
    let mut command = std::process::Command::new(binary);
    command
        .args(["version", "--json"])
        .stdin(std::process::Stdio::null());
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        INSTALL_STATUS_TIMEOUT,
        "mesh version --json",
    )
    .map_err(|e| format!("Failed to run mesh version --json: {e}"))?;
    read_mesh_contract_from_output("mesh version --json", output)
}

fn read_mesh_contract_wsl(distro: &str, binary: &str) -> Result<MeshCompatibilityContract, String> {
    let version_script = format!("\"{binary}\" version --json");
    let mut command = wsl_command();
    command
        .args(crate::daemon::launcher::wsl_shell_args(
            distro,
            "-lc",
            &version_script,
        ))
        .stdin(std::process::Stdio::null());
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        INSTALL_STATUS_TIMEOUT,
        "wsl mesh version --json",
    )
    .map_err(|e| format!("Failed to run mesh version --json in WSL: {e}"))?;
    read_mesh_contract_from_output("mesh version --json", output)
}

fn compatibility_issue(
    code: &str,
    message: String,
    expected: Option<String>,
    actual: Option<String>,
) -> MeshCompatibilityIssue {
    MeshCompatibilityIssue {
        code: code.to_string(),
        message,
        expected,
        actual,
    }
}

fn mesh_contract_read_issue(read_error: String) -> MeshCompatibilityIssue {
    compatibility_issue(
        "json_contract_unavailable",
        "Installed Mesh CLI could not be verified with `mesh version --json`. Install bundled Mesh to continue.".to_string(),
        Some("mesh version --json".to_string()),
        Some(read_error),
    )
}

fn compare_mesh_contracts(
    bundled: &MeshCompatibilityContract,
    installed: &MeshCompatibilityContract,
) -> Vec<MeshCompatibilityIssue> {
    let mut issues = Vec::new();

    if installed.version != bundled.version {
        issues.push(compatibility_issue(
            "version_mismatch",
            format!(
                "Installed Mesh CLI version {} does not match taurhaus bundled Mesh version {}. Install bundled Mesh to continue.",
                installed.version, bundled.version
            ),
            Some(bundled.version.clone()),
            Some(installed.version.clone()),
        ));
    }

    if installed.protocol_version != bundled.protocol_version {
        issues.push(compatibility_issue(
            "protocol_version_mismatch",
            format!(
                "Installed Mesh CLI protocol version {} does not match taurhaus required protocol version {}. Install bundled Mesh to continue.",
                installed.protocol_version, bundled.protocol_version
            ),
            Some(bundled.protocol_version.to_string()),
            Some(installed.protocol_version.to_string()),
        ));
    }

    if installed.schema_version != bundled.schema_version {
        issues.push(compatibility_issue(
            "schema_version_mismatch",
            format!(
                "Installed Mesh CLI schema version {} does not match taurhaus required schema version {}. Install bundled Mesh to continue.",
                installed.schema_version, bundled.schema_version
            ),
            Some(bundled.schema_version.to_string()),
            Some(installed.schema_version.to_string()),
        ));
    }

    if let Some(expected_commit) = bundled.git_commit.as_ref() {
        let actual_commit = installed
            .git_commit
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        if &actual_commit != expected_commit {
            issues.push(compatibility_issue(
                "git_commit_mismatch",
                format!(
                    "Installed Mesh CLI git commit {} does not match taurhaus pinned Mesh commit {}. Install bundled Mesh to continue.",
                    actual_commit, expected_commit
                ),
                Some(expected_commit.clone()),
                Some(actual_commit),
            ));
        }
    }

    issues
}

fn mesh_status_not_installed(
    bundled_contract: &MeshCompatibilityContract,
    environment_available: bool,
    error: Option<String>,
) -> MeshInstallStatus {
    MeshInstallStatus {
        installed: false,
        version: None,
        bundled_version: bundled_contract.version.clone(),
        needs_update: false,
        bundled_contract: bundled_contract.clone(),
        installed_contract: None,
        compatibility_issues: Vec::new(),
        environment_available,
        error,
    }
}

fn mesh_status_from_contract(
    bundled_contract: &MeshCompatibilityContract,
    installed_contract: Option<MeshCompatibilityContract>,
    compatibility_issues: Vec<MeshCompatibilityIssue>,
    environment_available: bool,
    error: Option<String>,
) -> MeshInstallStatus {
    MeshInstallStatus {
        installed: true,
        version: installed_contract
            .as_ref()
            .map(|contract| contract.version.clone()),
        bundled_version: bundled_contract.version.clone(),
        needs_update: !compatibility_issues.is_empty(),
        bundled_contract: bundled_contract.clone(),
        installed_contract,
        compatibility_issues,
        environment_available,
        error,
    }
}

fn check_mesh_install_native(
    bundled_contract: &MeshCompatibilityContract,
) -> Result<MeshInstallStatus, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let binary = home.join(".local/bin/mesh");

    if !binary.exists() {
        return Ok(mesh_status_not_installed(bundled_contract, true, None));
    }

    match read_mesh_contract_native(&binary) {
        Ok(installed_contract) => {
            let issues = compare_mesh_contracts(bundled_contract, &installed_contract);
            Ok(mesh_status_from_contract(
                bundled_contract,
                Some(installed_contract),
                issues,
                true,
                None,
            ))
        }
        Err(read_error) => Ok(mesh_status_from_contract(
            bundled_contract,
            None,
            vec![mesh_contract_read_issue(read_error)],
            true,
            None,
        )),
    }
}

fn check_mesh_install_wsl(
    bundled_contract: &MeshCompatibilityContract,
) -> Result<MeshInstallStatus, String> {
    let mut wsl_check = wsl_command();
    wsl_check.arg("--status").stdin(std::process::Stdio::null());
    let wsl_check = crate::process_utils::run_command_with_timeout(
        &mut wsl_check,
        INSTALL_STATUS_TIMEOUT,
        "wsl --status",
    );

    match wsl_check {
        Err(_) => {
            return Ok(mesh_status_not_installed(
                bundled_contract,
                false,
                Some("WSL is not installed".to_string()),
            ));
        }
        Ok(output) if !output.status.success() => {
            return Ok(mesh_status_not_installed(
                bundled_contract,
                false,
                Some("WSL is not available".to_string()),
            ));
        }
        _ => {}
    }

    let distro = match detect_default_distro()? {
        Some(d) => d,
        None => {
            return Ok(mesh_status_not_installed(
                bundled_contract,
                true,
                Some("No WSL distro configured".to_string()),
            ));
        }
    };

    if let Err(e) = validate_wsl_distro(&distro) {
        return Ok(mesh_status_not_installed(
            bundled_contract,
            true,
            Some(format!("Invalid WSL distro name: {e}")),
        ));
    }

    let mut exists = wsl_command();
    exists
        .args(crate::daemon::launcher::wsl_shell_args(
            &distro,
            "-lc",
            "test -f \"$HOME/.local/bin/mesh\"",
        ))
        .stdin(std::process::Stdio::null());
    let exists = crate::process_utils::run_command_with_timeout(
        &mut exists,
        INSTALL_STATUS_TIMEOUT,
        "wsl test -f ~/.local/bin/mesh",
    )
    .map(|out| out.status.success())
    .unwrap_or(false);

    if !exists {
        return Ok(mesh_status_not_installed(bundled_contract, true, None));
    }

    match read_mesh_contract_wsl(&distro, WSL_MESH_BINARY_PATH) {
        Ok(installed_contract) => {
            let issues = compare_mesh_contracts(bundled_contract, &installed_contract);
            Ok(mesh_status_from_contract(
                bundled_contract,
                Some(installed_contract),
                issues,
                true,
                None,
            ))
        }
        Err(read_error) => Ok(mesh_status_from_contract(
            bundled_contract,
            None,
            vec![mesh_contract_read_issue(read_error)],
            true,
            None,
        )),
    }
}

fn install_mesh_native(
    app: &tauri::AppHandle,
    bundled_binary: &Path,
    bundled_contract: &MeshCompatibilityContract,
) -> Result<OperationResult, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let target_dir = home.join(".local/bin");
    install_mesh_native_at(&target_dir, bundled_binary, bundled_contract, || {
        run_mesh_install_self_heal(app).map(Some)
    })
}

fn install_mesh_native_at<F>(
    target_dir: &Path,
    bundled_binary: &Path,
    bundled_contract: &MeshCompatibilityContract,
    run_self_heal: F,
) -> Result<OperationResult, String>
where
    F: FnOnce() -> Result<Option<MeshInstallSelfHealSummary>, String>,
{
    let target_path = target_dir.join("mesh");
    let temp_path = target_dir.join(".mesh.new");

    std::fs::create_dir_all(target_dir)
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

    let mut verify = std::process::Command::new(&target_path);
    verify
        .args(["version", "--json"])
        .stdin(std::process::Stdio::null());
    let verify = crate::process_utils::run_command_with_timeout(
        &mut verify,
        INSTALL_STATUS_TIMEOUT,
        "mesh version --json",
    );

    match verify {
        Ok(output) => {
            let installed_contract = read_mesh_contract_from_output("mesh version --json", output)
                .map_err(|e| format!("Mesh was copied but verification failed: {e}"))?;
            let issues = compare_mesh_contracts(bundled_contract, &installed_contract);
            if !issues.is_empty() {
                let summary = issues
                    .into_iter()
                    .map(|issue| issue.message)
                    .collect::<Vec<_>>()
                    .join(" ");
                return Err(format!(
                    "Mesh was copied but compatibility verification failed: {summary}"
                ));
            }
            let self_heal_summary = run_self_heal()?;
            Ok(OperationResult::success(
                format_mesh_install_success_message(&installed_contract.version, self_heal_summary),
            ))
        }
        Err(e) => Err(format!("Mesh was copied but verification failed: {e}")),
    }
}

fn install_mesh_wsl(
    app: &tauri::AppHandle,
    bundled_binary: &Path,
    bundled_contract: &MeshCompatibilityContract,
) -> Result<OperationResult, String> {
    let distro = detect_default_distro()?.ok_or("No WSL distro configured")?;
    validate_wsl_distro(&distro).map_err(|e| format!("Invalid distro: {e}"))?;

    let bundled_binary_str = bundled_binary.to_string_lossy();
    let wsl_source_path = crate::provider::path::to_linux(&bundled_binary_str)
        .unwrap_or_else(|| bundled_binary_str.to_string());

    let mut command = wsl_command();
    command
        .args(crate::daemon::launcher::wsl_shell_args(
            &distro,
            "-lc",
            install_mesh_wsl_script(),
        ))
        .arg("taurhaus-install")
        .arg(&wsl_source_path)
        .stdin(std::process::Stdio::null());
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        INSTALL_ACTION_TIMEOUT,
        "wsl mesh install script",
    )
    .map_err(|e| format!("Failed to install mesh in WSL: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to install mesh in WSL: {stderr}"));
    }

    let result = parse_mesh_wsl_install_output(&output.stdout)?;
    let issues = compare_mesh_contracts(bundled_contract, &result.contract);
    if !issues.is_empty() {
        let summary = issues
            .into_iter()
            .map(|issue| issue.message)
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "Installed mesh compatibility contract does not match bundled Mesh: {summary}"
        ));
    }

    let any_daemons_were_running =
        result.member_daemons_were_running || result.team_daemons_were_running;
    let self_heal_summary = if any_daemons_were_running {
        Some(run_mesh_install_self_heal(app)?)
    } else {
        None
    };

    let message = format_mesh_install_success_message(&result.contract.version, self_heal_summary);

    Ok(OperationResult::success(message))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MeshInstallSelfHealSummary {
    teams_reconciled: usize,
    team_daemons_ensured: usize,
}

fn format_mesh_install_success_message(
    version: &str,
    self_heal_summary: Option<MeshInstallSelfHealSummary>,
) -> String {
    match self_heal_summary {
        Some(summary) => {
            format!(
            "Mesh installed successfully: mesh {} (cycled {} team daemon{}, repaired {} team{})",
            version,
            summary.team_daemons_ensured,
            if summary.team_daemons_ensured == 1 {
                ""
            } else {
                "s"
            },
            summary.teams_reconciled,
            if summary.teams_reconciled == 1 { "" } else { "s" },
        )
        }
        None => format!("Mesh installed successfully: mesh {version}"),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_mesh_install_self_heal(
    app: &tauri::AppHandle,
) -> Result<MeshInstallSelfHealSummary, String> {
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
    Ok(MeshInstallSelfHealSummary {
        teams_reconciled: summary.teams_reconciled,
        team_daemons_ensured: summary.team_daemons_ensured,
    })
}

#[cfg(not(feature = "mesh-bridged-backend"))]
fn run_mesh_install_self_heal(
    app: &tauri::AppHandle,
) -> Result<MeshInstallSelfHealSummary, String> {
    let _ = app;
    Ok(MeshInstallSelfHealSummary::default())
}

#[derive(Debug)]
struct WslMeshInstallResult {
    contract: MeshCompatibilityContract,
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
version_json="$("$target_path" version --json | tr -d '\r\n')"
printf '%s%s\n' "${WSL_INSTALL_VERSION_JSON_MARKER:-__TAURHAUS_MESH_VERSION_JSON__=}" "$version_json"
printf '%s%s\n' "${WSL_INSTALL_MEMBER_DAEMON_MARKER:-__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=}" "$member_daemons_were_running"
printf '%s%s\n' "${WSL_INSTALL_TEAM_DAEMON_MARKER:-__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=}" "$team_daemons_were_running"
"#
}

fn parse_mesh_wsl_install_output(stdout: &[u8]) -> Result<WslMeshInstallResult, String> {
    let text = String::from_utf8_lossy(stdout);
    let mut version_json = None;
    let mut member_daemons_were_running = false;
    let mut team_daemons_were_running = false;

    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_VERSION_JSON_MARKER) {
            version_json = Some(raw.to_string());
            continue;
        }
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_MEMBER_DAEMON_MARKER) {
            member_daemons_were_running = raw == "1";
            continue;
        }
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_TEAM_DAEMON_MARKER) {
            team_daemons_were_running = raw == "1";
            continue;
        }
    }

    let version_json = version_json.ok_or_else(|| {
        "WSL install completed but no mesh compatibility JSON was returned for verification"
            .to_string()
    })?;
    let contract = parse_mesh_contract_json(version_json.as_bytes())
        .map_err(|e| format!("WSL install completed but mesh version JSON was invalid: {e}"))?;

    Ok(WslMeshInstallResult {
        contract,
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
    fn parse_mesh_contract_json_reads_required_fields() {
        let contract = parse_mesh_contract_json(
            br#"{"version":"0.2.0","protocol_version":1,"schema_version":2,"git_commit":"abc123"}"#,
        )
        .expect("parsed");
        assert_eq!(contract.version, "0.2.0");
        assert_eq!(contract.protocol_version, 1);
        assert_eq!(contract.schema_version, 2);
        assert_eq!(contract.git_commit.as_deref(), Some("abc123"));
    }

    #[test]
    fn parse_mesh_contract_json_rejects_missing_version() {
        let err = parse_mesh_contract_json(
            br#"{"version":"  ","protocol_version":1,"schema_version":1}"#,
        )
        .expect_err("missing version should fail");
        assert!(err.contains("version"));
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
    fn compare_mesh_contracts_reports_all_contract_mismatches() {
        let bundled = MeshCompatibilityContract {
            version: "0.5.4".to_string(),
            protocol_version: 2,
            schema_version: 3,
            git_commit: Some("expected-commit".to_string()),
        };
        let installed = MeshCompatibilityContract {
            version: "0.5.3".to_string(),
            protocol_version: 1,
            schema_version: 4,
            git_commit: Some("actual-commit".to_string()),
        };

        let issues = compare_mesh_contracts(&bundled, &installed);
        let codes = issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            vec![
                "version_mismatch",
                "protocol_version_mismatch",
                "schema_version_mismatch",
                "git_commit_mismatch"
            ]
        );
    }

    #[test]
    fn parse_mesh_wsl_install_output_reads_contract_and_daemon_markers() {
        let raw = b"__TAURHAUS_MESH_VERSION_JSON__={\"version\":\"0.5.3\",\"protocol_version\":1,\"schema_version\":1,\"git_commit\":\"abc123\"}\n__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=1\n__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=0\n";
        let result = parse_mesh_wsl_install_output(raw).expect("parsed");
        assert_eq!(result.contract.version, "0.5.3");
        assert!(result.member_daemons_were_running);
        assert!(!result.team_daemons_were_running);
    }

    #[test]
    fn parse_mesh_wsl_install_output_requires_version_json_line() {
        let err = parse_mesh_wsl_install_output(
            b"__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=0\n__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=0\n",
        )
        .expect_err("missing version JSON should fail");
        assert!(err.contains("no mesh compatibility JSON"));
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
        assert!(script.contains(WSL_INSTALL_VERSION_JSON_MARKER));
        assert!(script.contains(WSL_INSTALL_MEMBER_DAEMON_MARKER));
        assert!(script.contains(WSL_INSTALL_TEAM_DAEMON_MARKER));
        assert!(script.contains("\"$target_path\" version --json"));
    }

    #[test]
    fn wsl_mesh_binary_path_is_shell_quoted_for_home_expansion() {
        let command_line = format!("\"{WSL_MESH_BINARY_PATH}\" version --json");
        assert_eq!(command_line, "\"$HOME/.local/bin/mesh\" version --json");
    }

    #[test]
    fn tauri_bundle_resources_include_mesh_manifest() {
        let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
            .expect("read tauri config");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri config");
        let resources = json["bundle"]["resources"]
            .as_object()
            .expect("bundle.resources object");
        assert_eq!(
            resources
                .get("resources/mesh.manifest.json")
                .and_then(serde_json::Value::as_str),
            Some("resources/mesh.manifest.json")
        );
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
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  echo '{"version":"0.1.0","protocol_version":1,"schema_version":1,"git_commit":"old"}'
  exit 0
fi
exit 0
"#,
        );
        write_executable(
            &source_mesh,
            r#"#!/bin/sh
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  echo '{"version":"9.9.9","protocol_version":1,"schema_version":1,"git_commit":"new"}'
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
        assert_eq!(parsed.contract.version, "9.9.9");
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn install_mesh_native_triggers_self_heal_after_successful_install() {
        use std::cell::Cell;

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(
            &source_mesh,
            r#"#!/bin/sh
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  echo '{"version":"9.9.9","protocol_version":1,"schema_version":1,"git_commit":"new"}'
  exit 0
fi
exit 0
"#,
        );

        let bundled_contract = MeshCompatibilityContract {
            version: "9.9.9".to_string(),
            protocol_version: 1,
            schema_version: 1,
            git_commit: Some("new".to_string()),
        };
        let self_heal_called = Cell::new(false);
        let result = install_mesh_native_at(
            &temp_home.path().join(".local").join("bin"),
            &source_mesh,
            &bundled_contract,
            || {
                self_heal_called.set(true);
                Ok(Some(MeshInstallSelfHealSummary {
                    teams_reconciled: 2,
                    team_daemons_ensured: 1,
                }))
            },
        )
        .expect("install should succeed");

        assert!(
            self_heal_called.get(),
            "native install should trigger self-heal"
        );
        assert_eq!(
            result.message,
            "Mesh installed successfully: mesh 9.9.9 (cycled 1 team daemon, repaired 2 teams)"
        );
        assert!(
            temp_home
                .path()
                .join(".local")
                .join("bin")
                .join("mesh")
                .exists(),
            "native install should replace the target mesh binary"
        );
    }

    #[test]
    fn mesh_install_required_when_binary_missing() {
        let status = MeshInstallStatus {
            installed: false,
            version: None,
            bundled_version: "0.2.13".to_string(),
            needs_update: false,
            bundled_contract: MeshCompatibilityContract {
                version: "0.2.13".to_string(),
                protocol_version: 1,
                schema_version: 1,
                git_commit: Some("abc".to_string()),
            },
            installed_contract: None,
            compatibility_issues: Vec::new(),
            environment_available: true,
            error: None,
        };

        assert!(mesh_install_required(&status));
    }

    #[test]
    fn mesh_install_required_when_contract_drifts() {
        let status = MeshInstallStatus {
            installed: true,
            version: Some("0.2.12".to_string()),
            bundled_version: "0.2.13".to_string(),
            needs_update: true,
            bundled_contract: MeshCompatibilityContract {
                version: "0.2.13".to_string(),
                protocol_version: 1,
                schema_version: 1,
                git_commit: Some("abc".to_string()),
            },
            installed_contract: Some(MeshCompatibilityContract {
                version: "0.2.12".to_string(),
                protocol_version: 1,
                schema_version: 1,
                git_commit: Some("def".to_string()),
            }),
            compatibility_issues: vec![MeshCompatibilityIssue {
                code: "version_mismatch".to_string(),
                message: "mismatch".to_string(),
                expected: Some("0.2.13".to_string()),
                actual: Some("0.2.12".to_string()),
            }],
            environment_available: true,
            error: None,
        };

        assert!(mesh_install_required(&status));
    }

    #[test]
    fn mesh_install_required_skips_when_environment_unavailable() {
        let status = MeshInstallStatus {
            installed: false,
            version: None,
            bundled_version: "0.2.13".to_string(),
            needs_update: false,
            bundled_contract: MeshCompatibilityContract {
                version: "0.2.13".to_string(),
                protocol_version: 1,
                schema_version: 1,
                git_commit: Some("abc".to_string()),
            },
            installed_contract: None,
            compatibility_issues: Vec::new(),
            environment_available: false,
            error: Some("WSL is not available".to_string()),
        };

        assert!(!mesh_install_required(&status));
    }
}
