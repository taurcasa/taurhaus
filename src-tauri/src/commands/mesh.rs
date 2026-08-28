use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::launcher::{is_native_daemon, validate_wsl_distro, wsl_command};
use crate::errors::{CommandResultExt, IpcResult};
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
/// `pgrep -f` patterns for the mesh daemons an install has to cycle. They are
/// passed to the install script as arguments so the destructive `pgrep`/`kill`
/// block is never driven by a pattern the script itself invented — a test can
/// scope them to its own processes instead of every mesh daemon on the host.
const MESH_MEMBER_DAEMON_PATTERN: &str =
    "[m]esh([[:space:]]|$).*[[:space:]]daemon([[:space:]]|$).*--pane([[:space:]]|$)";
const MESH_TEAM_DAEMON_PATTERN: &str =
    "[m]esh([[:space:]]|$).*team-daemon([[:space:]]|$).*start([[:space:]]|$)";
const INSTALL_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const INSTALL_ACTION_TIMEOUT: Duration = Duration::from_secs(12);

#[tauri::command]
pub fn check_mesh_install_status(app: tauri::AppHandle) -> IpcResult<MeshInstallStatus> {
    let span = IpcCommandSpan::start("check_mesh_install_status");
    let result = read_mesh_install_status(&app).ipc_cmd("check_mesh_install_status");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn install_mesh(app: tauri::AppHandle) -> IpcResult<OperationResult> {
    let span = IpcCommandSpan::start("install_mesh");
    let result = install_bundled_mesh(&app).ipc_cmd("install_mesh");
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
    resolve_bundled_mesh_assets_in(&resource_dir.join("resources"))
}

/// Resolve the bundled mesh binary + manifest from a resources directory.
///
/// The binary must be a regular file. A directory at `resources/mesh` (which
/// `just bundle-mesh` could previously produce if a stray directory existed,
/// turning `cp mesh resources/mesh` into `resources/mesh/mesh`) is rejected
/// explicitly instead of being handed to the installer as if it were a binary.
fn resolve_bundled_mesh_assets_in(
    resources_dir: &Path,
) -> Result<(PathBuf, MeshCompatibilityContract), String> {
    let bundled_binary = resources_dir.join(MESH_BINARY_NAME);
    if !bundled_binary.exists() {
        return Err(format!(
            "Bundled mesh binary not found at {}",
            bundled_binary.display()
        ));
    }
    if !bundled_binary.is_file() {
        return Err(format!(
            "Bundled mesh binary at {} is not a regular file (found a directory); \
             the resource bundle is corrupt — rebuild with `just bundle-mesh`",
            bundled_binary.display()
        ));
    }
    let bundled_contract = read_mesh_manifest_resource(resources_dir)?;
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

fn mesh_contract_read_issue(binary_display: &str, read_error: String) -> MeshCompatibilityIssue {
    compatibility_issue(
        "json_contract_unavailable",
        format!(
            "Installed Mesh CLI at {binary_display} could not be verified with `mesh version --json`. Install bundled Mesh to continue."
        ),
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

/// A binary that exists but cannot report a version is not an install.
///
/// The 0-byte `~/.local/bin/mesh` of 2026-08-28 existed and was executable, so
/// reporting it as installed made a silently broken mesh look healthy. Reporting
/// it as *not* installed makes the next start repair it from the bundle — through
/// the guarded installer, which refuses to make things worse.
fn mesh_status_unrunnable(
    bundled_contract: &MeshCompatibilityContract,
    binary_display: &str,
    read_error: String,
) -> MeshInstallStatus {
    MeshInstallStatus {
        installed: false,
        version: None,
        bundled_version: bundled_contract.version.clone(),
        needs_update: true,
        bundled_contract: bundled_contract.clone(),
        installed_contract: None,
        compatibility_issues: vec![mesh_contract_read_issue(binary_display, read_error)],
        environment_available: true,
        error: None,
    }
}

fn mesh_status_for_native_binary(
    bundled_contract: &MeshCompatibilityContract,
    binary: &Path,
) -> MeshInstallStatus {
    if !binary.exists() {
        return mesh_status_not_installed(bundled_contract, true, None);
    }

    match read_mesh_contract_native(binary) {
        Ok(installed_contract) => {
            let issues = compare_mesh_contracts(bundled_contract, &installed_contract);
            mesh_status_from_contract(
                bundled_contract,
                Some(installed_contract),
                issues,
                true,
                None,
            )
        }
        Err(read_error) => {
            mesh_status_unrunnable(bundled_contract, &binary.display().to_string(), read_error)
        }
    }
}

fn check_mesh_install_native(
    bundled_contract: &MeshCompatibilityContract,
) -> Result<MeshInstallStatus, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(mesh_status_for_native_binary(
        bundled_contract,
        &home.join(".local/bin/mesh"),
    ))
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
        Err(read_error) => Ok(mesh_status_unrunnable(
            bundled_contract,
            WSL_MESH_BINARY_PATH,
            read_error,
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

    ensure_bundled_mesh_source_usable(bundled_binary)?;

    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed to create ~/.local/bin: {e}"))?;

    let mut staged = StagedMeshCopy::arm(&temp_path);
    std::fs::copy(bundled_binary, &temp_path).map_err(|e| format!("Failed to copy mesh: {e}"))?;
    let installed_contract = prepare_and_verify_mesh_copy(&temp_path, bundled_contract)?;
    std::fs::rename(&temp_path, &target_path)
        .map_err(|e| format!("Failed to install mesh binary: {e}"))?;
    staged.disarm();

    let self_heal_summary = run_self_heal()?;
    Ok(OperationResult::success(
        format_mesh_install_success_message(&installed_contract.version, self_heal_summary),
    ))
}

/// Removes the staged `.mesh.new` copy unless the install reached the rename.
///
/// Every step between the copy and the swap can fail, and a half-written or
/// verified-but-unswapped copy must never be left sitting next to the live binary
/// where the next run — or a curious operator — could mistake it for an install.
struct StagedMeshCopy {
    path: Option<PathBuf>,
}

impl StagedMeshCopy {
    fn arm(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
        }
    }

    /// The staged copy has become the installed binary; there is nothing to remove.
    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for StagedMeshCopy {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Reject a bundled mesh that cannot possibly be a working binary before the
/// installer touches the live one. A debug `target/debug/resources/mesh` can be
/// mid-copy while another cargo process rebuilds the same checkout, which is how
/// a 0-byte file reached `~/.local/bin/mesh`.
fn ensure_bundled_mesh_source_usable(bundled_binary: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(bundled_binary).map_err(|e| {
        format!(
            "Failed to read bundled mesh at {}: {e}",
            bundled_binary.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "bundled mesh is not a regular file at {}",
            bundled_binary.display()
        ));
    }
    if metadata.len() == 0 {
        return Err(format!(
            "bundled mesh is empty at {}",
            bundled_binary.display()
        ));
    }
    Ok(())
}

/// Make the copy runnable and prove it is the bundled mesh *before* it is allowed
/// to replace the installed binary. Every failure here leaves the live binary alone.
fn prepare_and_verify_mesh_copy(
    temp_path: &Path,
    bundled_contract: &MeshCompatibilityContract,
) -> Result<MeshCompatibilityContract, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(temp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set executable permission: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        let sign = std::process::Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(temp_path)
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

    let copied_contract = read_mesh_contract_native(temp_path)
        .map_err(|e| format!("copied mesh did not report a version: {e}"))?;

    if copied_contract.version != bundled_contract.version
        || copied_contract.protocol_version != bundled_contract.protocol_version
        || copied_contract.schema_version != bundled_contract.schema_version
    {
        return Err(format!(
            "copied mesh reports {}, bundle manifest says {}",
            describe_mesh_contract(&copied_contract),
            describe_mesh_contract(bundled_contract)
        ));
    }

    let issues = compare_mesh_contracts(bundled_contract, &copied_contract);
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

    Ok(copied_contract)
}

fn describe_mesh_contract(contract: &MeshCompatibilityContract) -> String {
    format!(
        "version {} (protocol {}, schema {})",
        contract.version, contract.protocol_version, contract.schema_version
    )
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

    let plan = WslMeshInstallPlan {
        source_path: &wsl_source_path,
        member_pattern: MESH_MEMBER_DAEMON_PATTERN,
        team_pattern: MESH_TEAM_DAEMON_PATTERN,
    };

    install_mesh_wsl_orchestrated(
        &plan,
        bundled_contract,
        |script, args| run_wsl_install_phase(&distro, script, args),
        |any_daemons_were_running| {
            if any_daemons_were_running {
                run_mesh_install_self_heal(app).map(Some)
            } else {
                Ok(None)
            }
        },
    )
}

/// Everything the WSL install script needs from the app side.
struct WslMeshInstallPlan<'a> {
    source_path: &'a str,
    member_pattern: &'a str,
    team_pattern: &'a str,
}

fn run_wsl_install_phase(
    distro: &str,
    script: &str,
    args: &[&str],
) -> Result<std::process::Output, String> {
    let mut command = wsl_command();
    command
        .args(crate::daemon::launcher::wsl_shell_args(
            distro, "-lc", script,
        ))
        .arg("taurhaus-install");
    for arg in args {
        command.arg(arg);
    }
    command.stdin(std::process::Stdio::null());
    crate::process_utils::run_command_with_timeout(
        &mut command,
        INSTALL_ACTION_TIMEOUT,
        "wsl mesh install script",
    )
    .map_err(|e| e.to_string())
}

/// The WSL install, with the shell out of the decisions: the phase runner only
/// executes a script, and every judgement about the copied binary is made here.
fn install_mesh_wsl_orchestrated<R, F>(
    plan: &WslMeshInstallPlan<'_>,
    bundled_contract: &MeshCompatibilityContract,
    mut run_phase: R,
    run_self_heal: F,
) -> Result<OperationResult, String>
where
    R: FnMut(&str, &[&str]) -> Result<std::process::Output, String>,
    F: FnOnce(bool) -> Result<Option<MeshInstallSelfHealSummary>, String>,
{
    let output = run_phase(
        install_mesh_wsl_script(),
        &[plan.source_path, plan.member_pattern, plan.team_pattern],
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
    let self_heal_summary = run_self_heal(any_daemons_were_running)?;

    Ok(OperationResult::success(
        format_mesh_install_success_message(&result.contract.version, self_heal_summary),
    ))
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
member_pattern="$2"
team_pattern="$3"
target_dir="$HOME/.local/bin"
target_path="$target_dir/mesh"
temp_path="$target_dir/.mesh.new.$$"
member_daemons_were_running=0
team_daemons_were_running=0

if [ ! -s "$source_path" ]; then
  echo "bundled mesh is empty: $source_path" >&2
  exit 1
fi

mkdir -p "$target_dir"

cp "$source_path" "$temp_path"
chmod +x "$temp_path"

version_json=""
if raw_version_json="$("$temp_path" version --json 2>/dev/null)"; then
  version_json="$(printf '%s' "$raw_version_json" | tr -d '\r\n')"
fi
case "$version_json" in
  *'"version"'*) ;;
  *)
    rm -f "$temp_path"
    echo "copied mesh did not report a version: $source_path" >&2
    exit 1
    ;;
esac

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

mv -f "$temp_path" "$target_path"
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

    #[cfg(not(target_os = "windows"))]
    fn mesh_version_script(version: &str) -> String {
        format!(
            r#"#!/bin/sh
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  echo '{{"version":"{version}","protocol_version":1,"schema_version":1,"git_commit":"new"}}'
  exit 0
fi
exit 0
"#
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn bundled_test_contract() -> MeshCompatibilityContract {
        MeshCompatibilityContract {
            version: "9.9.9".to_string(),
            protocol_version: 1,
            schema_version: 1,
            git_commit: Some("new".to_string()),
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn bash_is_available() -> bool {
        Command::new("bash")
            .args(["-c", "exit 0"])
            .stdin(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    /// Runs an install phase the way the WSL runner does, but locally under `bash`.
    #[cfg(not(target_os = "windows"))]
    fn run_local_install_phase(
        home: &Path,
        script: &str,
        args: &[&str],
    ) -> Result<std::process::Output, String> {
        let mut command = Command::new("bash");
        command.arg("-c").arg(script).arg("taurhaus-install");
        for arg in args {
            command.arg(arg);
        }
        command
            .env("HOME", home)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| e.to_string())
    }

    /// A token no other process on this machine carries, so a test that drives the
    /// installer's `pgrep`/`kill` block can only ever reach its own fake daemons —
    /// never the operator's live mesh daemons.
    #[cfg(not(target_os = "windows"))]
    fn unique_daemon_token() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        format!("taurhaus-test-{}-{nanos}", std::process::id())
    }

    /// Patterns of the same shape as the shipped ones, narrowed to one token so a
    /// test can drive the installer's `pgrep`/`kill` block without any chance of
    /// reaching a mesh daemon that belongs to the operator.
    #[cfg(not(target_os = "windows"))]
    fn scoped_daemon_patterns(token: &str) -> (String, String) {
        (
            format!("[m]esh([[:space:]]|$).*daemon([[:space:]]|$).*--pane([[:space:]]|$).*{token}"),
            format!(
                "[m]esh([[:space:]]|$).*team-daemon([[:space:]]|$).*start([[:space:]]|$).*{token}"
            ),
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn spawn_fake_daemon(argv0: &str) -> std::process::Child {
        Command::new("bash")
            .args(["-lc", &format!("exec -a '{argv0}' sleep 100")])
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("spawn fake daemon")
    }

    #[cfg(not(target_os = "windows"))]
    fn has_exited(child: &mut std::process::Child) -> bool {
        for _ in 0..50 {
            if child.try_wait().expect("try_wait").is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[cfg(not(target_os = "windows"))]
    fn stop_fake_daemon(mut child: std::process::Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(not(target_os = "windows"))]
    fn leftover_temp_copies(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .expect("read target dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".mesh.new"))
            .count()
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
        assert!(script.contains("member_pattern=\"$2\""));
        assert!(script.contains("team_pattern=\"$3\""));
        assert!(MESH_MEMBER_DAEMON_PATTERN.contains("[[:space:]]daemon([[:space:]]|$).*--pane"));
        assert!(MESH_TEAM_DAEMON_PATTERN.contains("team-daemon([[:space:]]|$).*start"));
        assert!(script.contains("kill -TERM $member_pids || true"));
        assert!(script.contains("kill -TERM $team_pids || true"));
        assert!(script.contains(WSL_INSTALL_VERSION_JSON_MARKER));
        assert!(script.contains(WSL_INSTALL_MEMBER_DAEMON_MARKER));
        assert!(script.contains(WSL_INSTALL_TEAM_DAEMON_MARKER));
        // The copy must prove it runs before it is allowed to replace the live binary.
        assert!(script.contains("\"$temp_path\" version --json"));
        assert!(script.contains("rm -f \"$temp_path\""));
        let verify_at = script
            .find("\"$temp_path\" version --json")
            .expect("verification step");
        let swap_at = script
            .find("mv -f \"$temp_path\" \"$target_path\"")
            .expect("swap step");
        assert!(verify_at < swap_at, "verification must precede the swap");
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
    // Regression: v0.6.4 shipped with `resources/mesh` as a *directory*
    // (`resources/mesh/mesh`) because a stray directory pre-existed and
    // `just bundle-mesh` copied the binary into it. `exists()` was true, so
    // the installer was handed a directory. The resolver must reject that.
    #[test]
    fn resolve_bundled_mesh_assets_rejects_directory_at_binary_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let resources = tmp.path().join("resources");
        std::fs::create_dir_all(resources.join(MESH_BINARY_NAME)).expect("dir at mesh path");
        std::fs::write(
            resources.join(MESH_MANIFEST_RESOURCE),
            r#"{"version":"0.2.17","protocol_version":1,"schema_version":1,"git_commit":"abc","bundled_at_utc":"x"}"#,
        )
        .expect("manifest");

        let err =
            resolve_bundled_mesh_assets_in(&resources).expect_err("directory must be rejected");
        assert!(
            err.contains("not a regular file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_bundled_mesh_assets_accepts_regular_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let resources = tmp.path().join("resources");
        std::fs::create_dir_all(&resources).expect("resources dir");
        std::fs::write(resources.join(MESH_BINARY_NAME), b"#!/bin/sh\n").expect("binary");
        std::fs::write(
            resources.join(MESH_MANIFEST_RESOURCE),
            r#"{"version":"0.2.17","protocol_version":1,"schema_version":1,"git_commit":"abc","bundled_at_utc":"x"}"#,
        )
        .expect("manifest");

        let (binary, contract) =
            resolve_bundled_mesh_assets_in(&resources).expect("regular file resolves");
        assert_eq!(binary, resources.join(MESH_BINARY_NAME));
        assert_eq!(contract.version, "0.2.17");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn install_mesh_wsl_replaces_the_installed_binary_and_cycles_matching_daemons() {
        if !bash_is_available() {
            eprintln!("skipping WSL install orchestration test: bash is unavailable");
            return;
        }

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let bin_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let installed_mesh = bin_dir.join("mesh");
        write_executable(&installed_mesh, &mesh_version_script("0.1.0"));
        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_version_script("9.9.9"));

        let token = unique_daemon_token();
        let (member_pattern, team_pattern) = scoped_daemon_patterns(&token);
        let mut member = spawn_fake_daemon(&format!(
            "mesh daemon --pane %9 --team alpha --name dev --marker {token}"
        ));
        let mut team = spawn_fake_daemon(&format!(
            "mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        std::thread::sleep(Duration::from_millis(150));

        let plan = WslMeshInstallPlan {
            source_path: source_mesh.to_str().expect("source path"),
            member_pattern: &member_pattern,
            team_pattern: &team_pattern,
        };
        let mut daemons_were_running = None;
        let result = install_mesh_wsl_orchestrated(
            &plan,
            &bundled_test_contract(),
            |script, args| run_local_install_phase(temp_home.path(), script, args),
            |any_daemons_were_running| {
                daemons_were_running = Some(any_daemons_were_running);
                Ok(Some(MeshInstallSelfHealSummary {
                    teams_reconciled: 2,
                    team_daemons_ensured: 1,
                }))
            },
        )
        .expect("install should succeed");

        assert_eq!(
            result.message,
            "Mesh installed successfully: mesh 9.9.9 (cycled 1 team daemon, repaired 2 teams)"
        );
        assert_eq!(daemons_were_running, Some(true));
        assert_eq!(
            std::fs::read(&installed_mesh).expect("installed mesh"),
            std::fs::read(&source_mesh).expect("source mesh"),
            "installed binary should be atomically replaced by the verified source"
        );
        assert_eq!(leftover_temp_copies(&bin_dir), 0);
        assert!(
            has_exited(&mut member),
            "the member daemon should be cycled"
        );
        assert!(has_exited(&mut team), "the team daemon should be cycled");

        stop_fake_daemon(member);
        stop_fake_daemon(team);
    }

    #[cfg(not(target_os = "windows"))]
    // The shipped team-daemon pattern matches live `mesh team-daemon start` command
    // lines on the host running the tests, which is exactly why the guard tests below
    // drive the installer with token-scoped patterns instead of the shipped ones.
    #[test]
    fn shipped_team_daemon_pattern_matches_a_real_team_daemon_command_line() {
        let token = unique_daemon_token();
        let team = spawn_fake_daemon(&format!(
            "/home/someone/.local/bin/mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        std::thread::sleep(Duration::from_millis(150));

        // `pgrep` only, never `kill`: this test observes the host, it does not touch it.
        let output = Command::new("pgrep")
            .arg("-f")
            .arg(MESH_TEAM_DAEMON_PATTERN)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run pgrep");
        let matched = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>();

        assert!(
            matched.contains(&team.id().to_string()),
            "the team daemon pattern must match a `mesh team-daemon start` command line"
        );

        stop_fake_daemon(team);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn install_mesh_native_triggers_self_heal_after_successful_install() {
        use std::cell::Cell;

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_version_script("9.9.9"));

        let target_dir = temp_home.path().join(".local").join("bin");
        let self_heal_called = Cell::new(false);
        let result =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                self_heal_called.set(true);
                Ok(Some(MeshInstallSelfHealSummary {
                    teams_reconciled: 2,
                    team_daemons_ensured: 1,
                }))
            })
            .expect("install should succeed");

        assert!(
            self_heal_called.get(),
            "native install should trigger self-heal"
        );
        assert_eq!(
            result.message,
            "Mesh installed successfully: mesh 9.9.9 (cycled 1 team daemon, repaired 2 teams)"
        );
        // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident. The
        // guarded installer must still install the bundle it verified, and must not
        // leave its temp copy behind.
        assert_eq!(
            std::fs::read(target_dir.join("mesh")).expect("read installed mesh"),
            std::fs::read(&source_mesh).expect("read bundled mesh"),
            "native install should replace the target with the verified bundle"
        );
        assert_eq!(leftover_temp_copies(&target_dir), 0);
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

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident. The status
    // check reported an existing-but-unrunnable binary as `installed: true` with no
    // version, which reads as a healthy install everywhere the flag is trusted.
    #[test]
    fn mesh_status_reports_unrunnable_installed_binary_as_not_installed() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let binary = temp.path().join("mesh");
        write_executable(&binary, "");

        let status = mesh_status_for_native_binary(&bundled_test_contract(), &binary);

        assert!(!status.installed, "an unrunnable mesh is not an install");
        assert!(status.version.is_none());
        assert!(status.installed_contract.is_none());
        assert!(
            mesh_install_required(&status),
            "the next start must repair it from the bundle"
        );
        let issue = status
            .compatibility_issues
            .first()
            .expect("an unrunnable mesh must be reported as a compatibility issue");
        assert_eq!(issue.code, "json_contract_unavailable");
        assert!(
            issue.message.contains(&binary.display().to_string()),
            "the issue must name the binary: {}",
            issue.message
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn mesh_status_reports_a_matching_installed_binary_as_installed() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let binary = temp.path().join("mesh");
        write_executable(&binary, &mesh_version_script("9.9.9"));

        let status = mesh_status_for_native_binary(&bundled_test_contract(), &binary);

        assert!(status.installed);
        assert_eq!(status.version.as_deref(), Some("9.9.9"));
        assert!(status.compatibility_issues.is_empty());
        assert!(!mesh_install_required(&status));
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — `~/.local/bin/mesh` became a 0-byte file (mode 0755).
    // An empty file "runs" as an empty shell script, so `mesh join` / `mesh send` /
    // `mesh daemon` exited 0 doing nothing and every managed member silently lost
    // delivery. The installer had copied a mid-rebuild `resources/mesh` over the live
    // binary without ever checking the bundled source was non-empty.
    #[test]
    fn install_mesh_native_rejects_empty_bundled_source_and_keeps_installed_binary() {
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let target_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&target_dir).expect("bin dir");
        let target_path = target_dir.join("mesh");
        write_executable(&target_path, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&target_path).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, "");

        let err =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                panic!("self-heal must not run when the install is rejected")
            })
            .expect_err("an empty bundled mesh must be rejected");

        assert!(
            err.contains("bundled mesh is empty"),
            "unexpected error: {err}"
        );
        assert_eq!(
            std::fs::read(&target_path).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert_eq!(leftover_temp_copies(&target_dir), 0);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident. The native
    // installer renamed the copy over the live binary and only then ran
    // `mesh version --json`, so a copy that cannot report a version had already
    // replaced a working mesh by the time verification failed.
    #[test]
    fn install_mesh_native_keeps_installed_binary_when_copy_reports_no_version() {
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let target_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&target_dir).expect("bin dir");
        let target_path = target_dir.join("mesh");
        write_executable(&target_path, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&target_path).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, "#!/bin/sh\nexit 0\n");

        let err =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                panic!("self-heal must not run when the install is rejected")
            })
            .expect_err("a mesh copy that reports no version must be rejected");

        assert!(
            err.contains("copied mesh did not report a version"),
            "unexpected error: {err}"
        );
        assert_eq!(
            std::fs::read(&target_path).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert_eq!(leftover_temp_copies(&target_dir), 0);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident. A copy whose
    // contract does not match the bundle manifest must not reach the target either;
    // before the guard the mismatch was only reported after the swap.
    #[test]
    fn install_mesh_native_keeps_installed_binary_when_copy_reports_wrong_version() {
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let target_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&target_dir).expect("bin dir");
        let target_path = target_dir.join("mesh");
        write_executable(&target_path, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&target_path).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_version_script("0.0.1"));

        let err =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                panic!("self-heal must not run when the install is rejected")
            })
            .expect_err("a mesh copy with the wrong version must be rejected");

        assert!(
            err.contains("copied mesh reports") && err.contains("bundle manifest says"),
            "unexpected error: {err}"
        );
        assert!(err.contains("0.0.1") && err.contains("9.9.9"), "{err}");
        assert_eq!(
            std::fs::read(&target_path).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert_eq!(leftover_temp_copies(&target_dir), 0);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident. The WSL
    // install script did `cp`, `chmod`, `mv -f` and only THEN `version --json`, so a
    // broken copy replaced a working mesh before anything proved it runs.
    #[test]
    fn install_mesh_wsl_keeps_installed_binary_when_copy_reports_no_version() {
        if !bash_is_available() {
            eprintln!("skipping install_mesh_wsl guard test: bash is unavailable");
            return;
        }

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let bin_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let installed_mesh = bin_dir.join("mesh");
        write_executable(&installed_mesh, &mesh_version_script("0.1.0"));
        let before = std::fs::read(&installed_mesh).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, "#!/bin/sh\nexit 0\n");

        let token = unique_daemon_token();
        let (member_pattern, team_pattern) = scoped_daemon_patterns(&token);
        let plan = WslMeshInstallPlan {
            source_path: source_mesh.to_str().expect("source path"),
            member_pattern: &member_pattern,
            team_pattern: &team_pattern,
        };
        let err = install_mesh_wsl_orchestrated(
            &plan,
            &bundled_test_contract(),
            |script, args| run_local_install_phase(temp_home.path(), script, args),
            |_| panic!("self-heal must not run when the install is rejected"),
        )
        .expect_err("a mesh copy that reports no version must be rejected");

        assert!(
            err.contains("copied mesh did not report a version"),
            "unexpected error: {err}"
        );
        assert_eq!(
            std::fs::read(&installed_mesh).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert_eq!(leftover_temp_copies(&bin_dir), 0);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident, review
    // follow-up. Only `prepare_and_verify_mesh_copy` failures removed the staged
    // `.mesh.new`: a failing `fs::copy` left whatever sat at that path behind,
    // right next to the live binary and named like a half-finished install.
    #[test]
    fn install_mesh_native_removes_the_staged_copy_when_the_copy_fails() {
        use std::os::unix::fs::PermissionsExt;

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let target_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&target_dir).expect("bin dir");
        let target_path = target_dir.join("mesh");
        write_executable(&target_path, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&target_path).expect("read installed mesh");
        std::fs::write(target_dir.join(".mesh.new"), b"stale staged copy")
            .expect("staged copy from an earlier crashed install");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_version_script("9.9.9"));
        std::fs::set_permissions(&source_mesh, std::fs::Permissions::from_mode(0o000))
            .expect("make the bundled source unreadable");
        if std::fs::File::open(&source_mesh).is_ok() {
            eprintln!(
                "skipping install_mesh_native_removes_the_staged_copy_when_the_copy_fails: \
                 this process can read a 0o000 file"
            );
            return;
        }

        let err =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                panic!("self-heal must not run when the install is rejected")
            })
            .expect_err("an unreadable bundled mesh must be rejected");

        assert!(
            err.contains("Failed to copy mesh"),
            "unexpected error: {err}"
        );
        assert_eq!(
            std::fs::read(&target_path).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert_eq!(
            leftover_temp_copies(&target_dir),
            0,
            "a failed install must not leave a staged copy behind"
        );
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident, review
    // follow-up. A rename that cannot replace the target returned the error with the
    // verified copy still staged as an executable `.mesh.new`.
    #[test]
    fn install_mesh_native_removes_the_staged_copy_when_the_swap_fails() {
        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let target_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&target_dir).expect("bin dir");
        // A directory at the install path fails the rename *after* the copy has been
        // staged and verified, which is the only step left that can still fail.
        let target_path = target_dir.join("mesh");
        std::fs::create_dir(&target_path).expect("directory at the install path");
        std::fs::write(target_path.join("occupant"), b"x").expect("occupant");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_version_script("9.9.9"));

        let err =
            install_mesh_native_at(&target_dir, &source_mesh, &bundled_test_contract(), || {
                panic!("self-heal must not run when the swap fails")
            })
            .expect_err("a swap onto a directory must fail");

        assert!(
            err.contains("Failed to install mesh binary"),
            "unexpected error: {err}"
        );
        assert!(target_path.is_dir(), "the install path must be untouched");
        assert_eq!(
            leftover_temp_copies(&target_dir),
            0,
            "a failed swap must not leave a staged copy behind"
        );
    }
}
