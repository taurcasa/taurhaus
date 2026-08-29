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

    let stage_id = new_mesh_stage_id();
    let plan = WslMeshInstallPlan {
        source_path: &wsl_source_path,
        stage_id: &stage_id,
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

/// Everything the WSL install scripts need from the app side.
struct WslMeshInstallPlan<'a> {
    source_path: &'a str,
    /// Names the staged copy, so the swap phase addresses the file the stage phase
    /// wrote and two concurrent installs cannot stage over each other.
    stage_id: &'a str,
    member_pattern: &'a str,
    team_pattern: &'a str,
}

fn new_mesh_stage_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
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

/// The WSL install, with the shell out of the decisions.
///
/// The stage phase only copies and asks the copy for a version; the app parses that
/// answer and compares the whole contract against the bundle manifest; only then
/// does the swap phase stop daemons and replace the installed binary. The script
/// used to swap first and leave every judgement to the app afterwards, so a
/// truncated answer — or a valid answer from the wrong mesh — replaced a working
/// installation before anything could reject it.
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
    let staged = match stage_mesh_copy_in_wsl(plan, bundled_contract, &mut run_phase) {
        Ok(contract) => contract,
        Err(error) => {
            discard_staged_mesh_copy_in_wsl(plan, &mut run_phase);
            return Err(error);
        }
    };

    let output = run_phase(
        install_mesh_wsl_finish_script(),
        &[
            "swap",
            plan.stage_id,
            plan.member_pattern,
            plan.team_pattern,
        ],
    )
    .map_err(|e| format!("Failed to install mesh in WSL: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to install mesh in WSL: {stderr}"));
    }

    let cycle = parse_mesh_wsl_finish_output(&output.stdout);
    let self_heal_summary = run_self_heal(cycle.any_daemons_were_running())?;

    Ok(OperationResult::success(
        format_mesh_install_success_message(&staged.version, self_heal_summary),
    ))
}

/// Copies the bundled mesh next to the installed one and proves the copy is the
/// bundled mesh. Nothing here touches the installed binary or any daemon.
fn stage_mesh_copy_in_wsl<R>(
    plan: &WslMeshInstallPlan<'_>,
    bundled_contract: &MeshCompatibilityContract,
    run_phase: &mut R,
) -> Result<MeshCompatibilityContract, String>
where
    R: FnMut(&str, &[&str]) -> Result<std::process::Output, String>,
{
    let output = run_phase(
        install_mesh_wsl_stage_script(),
        &[plan.source_path, plan.stage_id],
    )
    .map_err(|e| format!("Failed to install mesh in WSL: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to install mesh in WSL: {stderr}"));
    }

    let staged_contract = parse_mesh_wsl_stage_output(&output.stdout)?;
    let issues = compare_mesh_contracts(bundled_contract, &staged_contract);
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

    Ok(staged_contract)
}

/// Best-effort removal of a staged copy the app has decided not to install. The
/// scripts clean up after themselves; this covers the rejections the app makes
/// between the two phases, when the staged copy is deliberately still there.
fn discard_staged_mesh_copy_in_wsl<R>(plan: &WslMeshInstallPlan<'_>, run_phase: &mut R)
where
    R: FnMut(&str, &[&str]) -> Result<std::process::Output, String>,
{
    let _ = run_phase(
        install_mesh_wsl_finish_script(),
        &["abort", plan.stage_id, "", ""],
    );
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
    let db = app.state::<crate::commands::projects::DbState>();
    let (cli_commands, tmux_layout) =
        crate::commands::coordination::background_launch_settings(&db, state.teams_dir());
    let summary = state
        .run_background_self_heal_pass(&cli_commands, &tmux_layout)
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct WslMeshDaemonCycle {
    member_daemons_were_running: bool,
    team_daemons_were_running: bool,
}

impl WslMeshDaemonCycle {
    fn any_daemons_were_running(&self) -> bool {
        self.member_daemons_were_running || self.team_daemons_were_running
    }
}

/// Stage phase: copy the bundled mesh beside the installed one and ask the copy for
/// its version. It never touches the installed binary and never stops a daemon, so
/// a failure here — including a timeout that kills the phase — costs nothing beyond
/// the staged copy, which the exit trap removes.
fn install_mesh_wsl_stage_script() -> &'static str {
    r#"set -eu
source_path="$1"
stage_id="$2"
target_dir="$HOME/.local/bin"
temp_path="$target_dir/.mesh.new.$stage_id"

discard_staged_copy() { rm -f "$temp_path"; }
trap discard_staged_copy EXIT
trap 'discard_staged_copy; exit 1' HUP INT TERM

if [ ! -s "$source_path" ]; then
  echo "bundled mesh is empty: $source_path" >&2
  exit 1
fi

mkdir -p "$target_dir"
rm -f "$temp_path"
cp "$source_path" "$temp_path"
chmod +x "$temp_path"

version_json=""
if raw_version_json="$("$temp_path" version --json 2>/dev/null)"; then
  version_json="$(printf '%s' "$raw_version_json" | tr -d '\r\n')"
fi
case "$version_json" in
  *'"version"'*) ;;
  *)
    echo "copied mesh did not report a version: $source_path" >&2
    exit 1
    ;;
esac

trap - EXIT HUP INT TERM
printf '%s%s\n' "${WSL_INSTALL_VERSION_JSON_MARKER:-__TAURHAUS_MESH_VERSION_JSON__=}" "$version_json"
"#
}

/// Swap phase: cycle the mesh daemons and move the staged copy onto the installed
/// path. It runs only after the app has parsed the staged copy's version JSON and
/// matched the whole contract against the bundle manifest; `abort` discards the
/// staged copy when that verification rejected it.
///
/// The daemon patterns stay command-line shaped instead of being anchored to the
/// binary being replaced. Anchoring was considered and rejected: a daemon still
/// running the *previous* mesh reads `/proc/<pid>/exe -> <target_path> (deleted)`,
/// `mesh_command_invocation_with_env` puts `env` in argv[0], and `mesh_cli` falls back
/// to a bare `mesh` from `PATH` when no home resolves — so an exe/argv0 anchor would
/// skip exactly the drifted daemons this block exists to cycle. Tests that execute
/// this script for real isolate themselves in a private PID namespace instead.
fn install_mesh_wsl_finish_script() -> &'static str {
    r#"set -eu
mode="$1"
stage_id="$2"
member_pattern="${3:-}"
team_pattern="${4:-}"
target_dir="$HOME/.local/bin"
target_path="$target_dir/mesh"
temp_path="$target_dir/.mesh.new.$stage_id"
member_daemons_were_running=0
team_daemons_were_running=0

discard_staged_copy() { rm -f "$temp_path"; }
trap discard_staged_copy EXIT
trap 'discard_staged_copy; exit 1' HUP INT TERM

if [ "$mode" != "swap" ]; then
  exit 0
fi

if [ -z "$member_pattern" ] || [ -z "$team_pattern" ]; then
  echo "mesh install swap called without daemon patterns" >&2
  exit 1
fi

if [ ! -s "$temp_path" ]; then
  echo "verified mesh copy is missing at $temp_path" >&2
  exit 1
fi

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
trap - EXIT HUP INT TERM
printf '%s%s\n' "${WSL_INSTALL_MEMBER_DAEMON_MARKER:-__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=}" "$member_daemons_were_running"
printf '%s%s\n' "${WSL_INSTALL_TEAM_DAEMON_MARKER:-__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=}" "$team_daemons_were_running"
"#
}

fn parse_mesh_wsl_stage_output(stdout: &[u8]) -> Result<MeshCompatibilityContract, String> {
    let text = String::from_utf8_lossy(stdout);
    let version_json = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(WSL_INSTALL_VERSION_JSON_MARKER))
        .ok_or_else(|| {
            "WSL install staged a mesh copy but no mesh compatibility JSON was returned for verification"
                .to_string()
        })?;

    parse_mesh_contract_json(version_json.as_bytes())
        .map_err(|e| format!("the copied mesh returned invalid version JSON: {e}"))
}

fn parse_mesh_wsl_finish_output(stdout: &[u8]) -> WslMeshDaemonCycle {
    let text = String::from_utf8_lossy(stdout);
    let mut cycle = WslMeshDaemonCycle::default();

    for line in text.lines().map(str::trim) {
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_MEMBER_DAEMON_MARKER) {
            cycle.member_daemons_were_running = raw == "1";
            continue;
        }
        if let Some(raw) = line.strip_prefix(WSL_INSTALL_TEAM_DAEMON_MARKER) {
            cycle.team_daemons_were_running = raw == "1";
        }
    }

    cycle
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

    /// A mesh that answers `version --json` with a contract of the caller's choosing.
    #[cfg(not(target_os = "windows"))]
    fn mesh_contract_script(version: &str, protocol: u32, schema: u32) -> String {
        format!(
            r#"#!/bin/sh
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  echo '{{"version":"{version}","protocol_version":{protocol},"schema_version":{schema},"git_commit":"new"}}'
  exit 0
fi
exit 0
"#
        )
    }

    /// A mesh whose version output is truncated: it carries the `"version"` key the
    /// install script looks for, but it is not JSON.
    #[cfg(not(target_os = "windows"))]
    fn mesh_malformed_version_script() -> String {
        r#"#!/bin/sh
if [ "$1" = "version" ] && [ "$2" = "--json" ]; then
  printf '%s' '{"version":"9.9.9","protocol_version":1'
  exit 0
fi
exit 0
"#
        .to_string()
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

    /// Set on a test binary that is already running inside a private PID namespace,
    /// so the nested run executes the test body instead of nesting again.
    #[cfg(not(target_os = "windows"))]
    const PRIVATE_PID_NAMESPACE_ENV: &str = "TAURHAUS_MESH_TEST_PID_NAMESPACE";

    /// Whether this host can give a command its own PID namespace. Unprivileged user
    /// namespaces are a kernel/distro switch, so it is probed once, for real.
    #[cfg(not(target_os = "windows"))]
    fn private_pid_namespace_available() -> bool {
        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            Command::new("unshare")
                .args(["-Urpf", "--mount-proc", "--", "true"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
    }

    /// `program`, prepared to run with its own PID namespace and its own `/proc`:
    /// `pgrep` inside it can only see processes started inside it, and `kill` inside
    /// it can only reach those. Check `private_pid_namespace_available()` first.
    #[cfg(not(target_os = "windows"))]
    fn private_pid_namespace_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new("unshare");
        command.args(["-Urpf", "--mount-proc", "--"]);
        command.arg(program);
        command.stdin(std::process::Stdio::null());
        command
    }

    /// `module_path!()` is prefixed with the crate name; libtest filters are not.
    #[cfg(not(target_os = "windows"))]
    fn libtest_module_path() -> &'static str {
        module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, rest)| rest)
    }

    /// Runs the calling test inside a private PID namespace and reports whether the
    /// caller is done.
    ///
    /// The install script's daemon cycle is `pgrep -f` plus `kill`, which is host-wide:
    /// run from a plain `cargo test`, it reaches the operator's live `mesh team-daemon`
    /// and `mesh daemon` processes. Every test that executes that script for real, or
    /// that puts a mesh-daemon-shaped process on the process table, re-runs itself in
    /// here, where the host's daemons do not exist. A host that cannot isolate skips
    /// the test — the host PID namespace is never an acceptable fallback.
    #[cfg(not(target_os = "windows"))]
    #[must_use]
    fn isolated_from_host_mesh_daemons(test_name: &str) -> bool {
        if std::env::var_os(PRIVATE_PID_NAMESPACE_ENV).is_some() {
            return false;
        }
        if !private_pid_namespace_available() {
            eprintln!(
                "skipping {test_name}: `unshare -Urpf --mount-proc` is unavailable on this host, \
                 and running the mesh installer's pgrep/kill block in the host PID namespace \
                 would reach the operator's live mesh daemons"
            );
            return true;
        }

        let test_binary = std::env::current_exe().expect("test binary path");
        let filter = format!("{}::{test_name}", libtest_module_path());
        let output = private_pid_namespace_command(&test_binary)
            .args(["--exact", &filter, "--nocapture", "--test-threads=1"])
            .env(PRIVATE_PID_NAMESPACE_ENV, "1")
            .output()
            .expect("re-run the test inside a private PID namespace");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{test_name} failed inside its private PID namespace\n\
             --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
        assert!(
            stdout.contains("1 passed"),
            "{test_name} never ran inside its private PID namespace: the filter `{filter}` \
             matched no test\n--- stdout ---\n{stdout}"
        );
        true
    }

    /// The pids the installer's shipped team-daemon pattern can reach from where a
    /// test runs. The installer kills what this matches, so a test that drives that
    /// block must be able to reach nothing but its own fake daemons.
    #[cfg(not(target_os = "windows"))]
    fn team_daemon_pids_visible_to_the_installer() -> Vec<String> {
        pgrep_pids(
            private_pid_namespace_command("pgrep"),
            MESH_TEAM_DAEMON_PATTERN,
        )
    }

    /// `pgrep -f <pattern>`, as a list of pid strings. Never kills anything.
    #[cfg(not(target_os = "windows"))]
    fn pgrep_pids(mut pgrep: Command, pattern: &str) -> Vec<String> {
        let output = pgrep
            .args(["-f", pattern])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run pgrep");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
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
        let child = Command::new("bash")
            .args(["-lc", &format!("exec -a '{argv0}' sleep 100")])
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("spawn fake daemon");
        wait_for_fake_daemon_command_line(child.id(), argv0);
        child
    }

    /// `bash -lc` has not exec'd into the fake daemon's command line when `spawn`
    /// returns, and a `pgrep` that looks before it does sees no daemon at all. Waits
    /// for the process table to show the command line the test asked for.
    #[cfg(not(target_os = "windows"))]
    fn wait_for_fake_daemon_command_line(pid: u32, argv0: &str) {
        for _ in 0..500 {
            let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
            if String::from_utf8_lossy(&cmdline).starts_with(argv0) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("fake daemon {pid} never showed the command line `{argv0}`");
    }

    #[cfg(not(target_os = "windows"))]
    fn is_running(child: &mut std::process::Child) -> bool {
        child.try_wait().expect("try_wait").is_none()
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
    fn parse_mesh_wsl_stage_output_reads_the_staged_contract() {
        let raw = b"__TAURHAUS_MESH_VERSION_JSON__={\"version\":\"0.5.3\",\"protocol_version\":1,\"schema_version\":1,\"git_commit\":\"abc123\"}\n";
        let contract = parse_mesh_wsl_stage_output(raw).expect("parsed");
        assert_eq!(contract.version, "0.5.3");
    }

    #[test]
    fn parse_mesh_wsl_stage_output_requires_version_json_line() {
        let err = parse_mesh_wsl_stage_output(b"nothing to see here\n")
            .expect_err("missing version JSON should fail");
        assert!(err.contains("no mesh compatibility JSON"));
    }

    #[test]
    fn parse_mesh_wsl_stage_output_rejects_malformed_version_json() {
        let err = parse_mesh_wsl_stage_output(
            b"__TAURHAUS_MESH_VERSION_JSON__={\"version\":\"0.5.3\",\"protocol_version\":1\n",
        )
        .expect_err("malformed version JSON should fail");
        assert!(
            err.contains("invalid version JSON"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_mesh_wsl_finish_output_reads_daemon_markers() {
        let cycle = parse_mesh_wsl_finish_output(
            b"__TAURHAUS_MESH_MEMBER_DAEMONS_WERE_RUNNING__=1\n__TAURHAUS_MESH_TEAM_DAEMONS_WERE_RUNNING__=0\n",
        );
        assert!(cycle.member_daemons_were_running);
        assert!(!cycle.team_daemons_were_running);
        assert!(cycle.any_daemons_were_running());
    }

    #[test]
    fn install_mesh_wsl_stage_script_only_stages_and_verifies() {
        let stage = install_mesh_wsl_stage_script();
        assert!(stage.contains("temp_path=\"$target_dir/.mesh.new.$stage_id\""));
        // The copy must prove it runs, and the staged copy must survive for the swap.
        assert!(stage.contains("\"$temp_path\" version --json"));
        assert!(stage.contains(WSL_INSTALL_VERSION_JSON_MARKER));
        // Nothing destructive may live in the stage phase.
        assert!(
            !stage.contains("mv -f"),
            "the stage phase must not replace the installed binary"
        );
        assert!(
            !stage.contains("pgrep"),
            "the stage phase must not touch running daemons"
        );
        // Every exit that is not the successful one removes the staged copy.
        assert!(stage.contains("discard_staged_copy() { rm -f \"$temp_path\"; }"));
        assert!(stage.contains("trap discard_staged_copy EXIT"));
        assert!(stage.contains("trap 'discard_staged_copy; exit 1' HUP INT TERM"));
        assert!(stage.contains("trap - EXIT HUP INT TERM"));
    }

    #[test]
    fn install_mesh_wsl_finish_script_swaps_and_emits_daemon_cycle_markers() {
        let finish = install_mesh_wsl_finish_script();
        assert!(finish.contains("temp_path=\"$target_dir/.mesh.new.$stage_id\""));
        assert!(finish.contains("mv -f \"$temp_path\" \"$target_path\""));
        assert!(finish.contains("pgrep -f \"$member_pattern\""));
        assert!(finish.contains("pgrep -f \"$team_pattern\""));
        assert!(finish.contains("kill -TERM $member_pids || true"));
        assert!(finish.contains("kill -TERM $team_pids || true"));
        assert!(finish.contains(WSL_INSTALL_MEMBER_DAEMON_MARKER));
        assert!(finish.contains(WSL_INSTALL_TEAM_DAEMON_MARKER));
        // An empty pattern would match every process on the host.
        assert!(finish.contains("if [ -z \"$member_pattern\" ] || [ -z \"$team_pattern\" ]; then"));
        assert!(finish.contains("discard_staged_copy() { rm -f \"$temp_path\"; }"));
        assert!(finish.contains("trap discard_staged_copy EXIT"));
        assert!(MESH_MEMBER_DAEMON_PATTERN.contains("[[:space:]]daemon([[:space:]]|$).*--pane"));
        assert!(MESH_TEAM_DAEMON_PATTERN.contains("team-daemon([[:space:]]|$).*start"));
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
        if isolated_from_host_mesh_daemons(
            "install_mesh_wsl_replaces_the_installed_binary_and_cycles_matching_daemons",
        ) {
            return;
        }
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
        let plan = WslMeshInstallPlan {
            source_path: source_mesh.to_str().expect("source path"),
            stage_id: &token,
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
    // lines on the host running the tests, which is exactly why every test that puts a
    // daemon-shaped process on the process table, or that runs the installer's cycle
    // block for real, runs inside its own PID namespace and drives the installer with
    // token-scoped patterns rather than the shipped ones.
    #[test]
    fn shipped_team_daemon_pattern_matches_a_real_team_daemon_command_line() {
        if isolated_from_host_mesh_daemons(
            "shipped_team_daemon_pattern_matches_a_real_team_daemon_command_line",
        ) {
            return;
        }

        let token = unique_daemon_token();
        let team = spawn_fake_daemon(&format!(
            "/home/someone/.local/bin/mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        // `pgrep` only, never `kill`, and only inside this test's own PID namespace.
        let matched = pgrep_pids(Command::new("pgrep"), MESH_TEAM_DAEMON_PATTERN);

        assert!(
            matched.contains(&team.id().to_string()),
            "the team daemon pattern must match a `mesh team-daemon start` command line"
        );

        stop_fake_daemon(team);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — a plain `cargo test --lib -- commands::mesh` stopped
    // the operator's live `mesh team-daemon` processes. The installer's cycle block
    // is driven by `pgrep -f`, which is host-wide, so every test that runs that block
    // has to run somewhere the host's own mesh daemons are invisible.
    #[test]
    fn the_installer_patterns_cannot_reach_host_mesh_daemons_from_a_test() {
        if !private_pid_namespace_available() {
            eprintln!(
                "skipping PID namespace isolation test: `unshare -Urpf --mount-proc` is \
                 unavailable on this host"
            );
            return;
        }

        let token = unique_daemon_token();
        // A host process shaped exactly like a live `mesh team-daemon start`. This
        // test only ever runs `pgrep`; it never kills by pattern.
        let team = spawn_fake_daemon(&format!(
            "/home/someone/.local/bin/mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        let team_pid = team.id().to_string();
        let on_host = pgrep_pids(Command::new("pgrep"), MESH_TEAM_DAEMON_PATTERN);
        let visible = team_daemon_pids_visible_to_the_installer();

        stop_fake_daemon(team);

        assert!(
            on_host.contains(&team_pid),
            "the shipped team pattern must match a live-shaped team daemon on this host, \
             otherwise this test proves nothing"
        );
        assert!(
            visible.is_empty(),
            "a test that drives the installer's pgrep/kill block must not be able to see \
             any mesh daemon on this host, but it can reach pids {visible:?}"
        );
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident, review
    // follow-up. The WSL script only checked that the copy's output contained the
    // literal `"version"`, then stopped every mesh daemon and swapped the binary;
    // the JSON was parsed, and the contract compared, back in the app afterwards. A
    // truncated answer therefore replaced a working mesh before anyone could say so.
    #[test]
    fn install_mesh_wsl_keeps_installed_binary_when_the_copy_reports_malformed_json() {
        if isolated_from_host_mesh_daemons(
            "install_mesh_wsl_keeps_installed_binary_when_the_copy_reports_malformed_json",
        ) {
            return;
        }
        if !bash_is_available() {
            eprintln!("skipping WSL malformed-JSON guard test: bash is unavailable");
            return;
        }

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let bin_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let installed_mesh = bin_dir.join("mesh");
        write_executable(&installed_mesh, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&installed_mesh).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_malformed_version_script());

        let token = unique_daemon_token();
        let (member_pattern, team_pattern) = scoped_daemon_patterns(&token);
        let mut member = spawn_fake_daemon(&format!(
            "mesh daemon --pane %9 --team alpha --name dev --marker {token}"
        ));
        let mut team = spawn_fake_daemon(&format!(
            "mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        let plan = WslMeshInstallPlan {
            source_path: source_mesh.to_str().expect("source path"),
            stage_id: &token,
            member_pattern: &member_pattern,
            team_pattern: &team_pattern,
        };
        let err = install_mesh_wsl_orchestrated(
            &plan,
            &bundled_test_contract(),
            |script, args| run_local_install_phase(temp_home.path(), script, args),
            |_| panic!("self-heal must not run when the install is rejected"),
        )
        .expect_err("a copy whose version output is not JSON must be rejected");

        assert!(err.contains("JSON"), "unexpected error: {err}");
        assert_eq!(
            std::fs::read(&installed_mesh).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert!(
            is_running(&mut member),
            "no daemon may be stopped for an install that is rejected"
        );
        assert!(
            is_running(&mut team),
            "no daemon may be stopped for an install that is rejected"
        );
        assert_eq!(leftover_temp_copies(&bin_dir), 0);

        stop_fake_daemon(member);
        stop_fake_daemon(team);
    }

    #[cfg(not(target_os = "windows"))]
    // Regression: 2026-08-28 — the 0-byte `~/.local/bin/mesh` incident, review
    // follow-up. A copy that answers with a *valid* contract for a different mesh
    // was compared against the bundle manifest only after the swap, so the wrong
    // mesh was already installed by the time the mismatch was reported.
    #[test]
    fn install_mesh_wsl_keeps_installed_binary_when_the_copy_reports_a_mismatched_contract() {
        if isolated_from_host_mesh_daemons(
            "install_mesh_wsl_keeps_installed_binary_when_the_copy_reports_a_mismatched_contract",
        ) {
            return;
        }
        if !bash_is_available() {
            eprintln!("skipping WSL contract-mismatch guard test: bash is unavailable");
            return;
        }

        let temp_home = tempfile::TempDir::new().expect("tempdir");
        let bin_dir = temp_home.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let installed_mesh = bin_dir.join("mesh");
        write_executable(&installed_mesh, &mesh_version_script("9.9.9"));
        let before = std::fs::read(&installed_mesh).expect("read installed mesh");

        let source_mesh = temp_home.path().join("mesh-new");
        write_executable(&source_mesh, &mesh_contract_script("0.0.1", 2, 3));

        let token = unique_daemon_token();
        let (member_pattern, team_pattern) = scoped_daemon_patterns(&token);
        let mut member = spawn_fake_daemon(&format!(
            "mesh daemon --pane %9 --team alpha --name dev --marker {token}"
        ));
        let mut team = spawn_fake_daemon(&format!(
            "mesh team-daemon start --team alpha --name lead --marker {token}"
        ));
        let plan = WslMeshInstallPlan {
            source_path: source_mesh.to_str().expect("source path"),
            stage_id: &token,
            member_pattern: &member_pattern,
            team_pattern: &team_pattern,
        };
        let err = install_mesh_wsl_orchestrated(
            &plan,
            &bundled_test_contract(),
            |script, args| run_local_install_phase(temp_home.path(), script, args),
            |_| panic!("self-heal must not run when the install is rejected"),
        )
        .expect_err("a copy whose contract differs from the bundle must be rejected");

        assert!(err.contains("does not match"), "unexpected error: {err}");
        assert!(err.contains("0.0.1") && err.contains("9.9.9"), "{err}");
        assert_eq!(
            std::fs::read(&installed_mesh).expect("read installed mesh"),
            before,
            "the working installed binary must be left byte-identical"
        );
        assert!(
            is_running(&mut member),
            "no daemon may be stopped for an install that is rejected"
        );
        assert!(
            is_running(&mut team),
            "no daemon may be stopped for an install that is rejected"
        );
        assert_eq!(leftover_temp_copies(&bin_dir), 0);

        stop_fake_daemon(member);
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

        assert!(status.installed, "{status:?}");
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
        if isolated_from_host_mesh_daemons(
            "install_mesh_wsl_keeps_installed_binary_when_copy_reports_no_version",
        ) {
            return;
        }
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
            stage_id: &token,
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
