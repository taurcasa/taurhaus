use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use super::{DAEMON_START_ATTEMPTS, DAEMON_START_INTERVAL, MESH_CONTROL_TOKEN_ENV};
use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::provider::platform_paths::PlatformPaths;

pub(crate) fn apply_background_command_settings(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    mesh_cli::mesh_command_invocation(args)
}

pub(crate) fn mesh_command_invocation_for_member(
    args: &[&str],
    team_name: &str,
    member_name: &str,
) -> CommandInvocation {
    mesh_cli::mesh_command_invocation_with_env(
        args,
        &mesh_member_control_env(team_name, member_name),
    )
}

fn mesh_member_control_env(team_name: &str, member_name: &str) -> Vec<(String, String)> {
    resolve_mesh_control_token(team_name, member_name)
        .map(|token| vec![(MESH_CONTROL_TOKEN_ENV.to_string(), token)])
        .unwrap_or_default()
}

pub(super) fn spawn_mesh_daemon_command_and_resolve_pid(
    invocation: &CommandInvocation,
    daemon_pid_path: Option<&Path>,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<u32, CoordinationError> {
    let mut child = spawn_system_command(invocation)?;
    let Some(daemon_pid_path) = daemon_pid_path else {
        return Ok(child.id());
    };

    match wait_for_mesh_daemon_pid_file(daemon_pid_path, pane_id, team_name, member_name) {
        Ok(pid) => Ok(pid),
        Err(err) => {
            let launcher_status = child
                .try_wait()
                .map_err(CoordinationError::Io)?
                .map(|status| format!("launcher exited with status {status}"))
                .unwrap_or_else(|| format!("launcher pid {} still alive", child.id()));
            Err(CoordinationError::Backend(format!(
                "daemon startup verification failed for {} {}: {err}; {launcher_status}",
                invocation.program,
                invocation.args.join(" ")
            )))
        }
    }
}

pub(super) fn wait_for_mesh_daemon_pid_file(
    daemon_pid_path: &Path,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<u32, CoordinationError> {
    wait_for_mesh_daemon_pid_file_with_retries(
        daemon_pid_path,
        pane_id,
        team_name,
        member_name,
        DAEMON_START_ATTEMPTS,
        DAEMON_START_INTERVAL,
    )
}

pub(super) fn wait_for_mesh_daemon_pid_file_with_retries(
    daemon_pid_path: &Path,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
    attempts: usize,
    interval: Duration,
) -> Result<u32, CoordinationError> {
    for attempt in 0..attempts {
        if let Some(pid) =
            validated_mesh_daemon_pid_file(daemon_pid_path, pane_id, team_name, member_name)?
        {
            return Ok(pid);
        }
        if attempt + 1 < attempts {
            thread::sleep(interval);
        }
    }

    Err(CoordinationError::Backend(format!(
        "timed out waiting for valid mesh daemon pid at {}",
        daemon_pid_path.display()
    )))
}

pub(super) fn wait_for_team_daemon_pid_file(
    daemon_pid_path: &Path,
    team_name: &str,
) -> Result<u32, CoordinationError> {
    wait_for_team_daemon_pid_file_with_retries(
        daemon_pid_path,
        team_name,
        DAEMON_START_ATTEMPTS,
        DAEMON_START_INTERVAL,
    )
}

pub(super) fn wait_for_team_daemon_pid_file_with_retries(
    daemon_pid_path: &Path,
    team_name: &str,
    attempts: usize,
    interval: Duration,
) -> Result<u32, CoordinationError> {
    for attempt in 0..attempts {
        if let Some(pid) = validated_team_daemon_pid_file(daemon_pid_path, team_name, true)? {
            return Ok(pid);
        }
        if attempt + 1 < attempts {
            thread::sleep(interval);
        }
    }

    Err(CoordinationError::Backend(format!(
        "timed out waiting for valid team daemon pid at {}",
        daemon_pid_path.display()
    )))
}

pub(super) fn read_pid_file(pid_path: &Path) -> Option<u32> {
    fs::read_to_string(pid_path)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
}

pub(super) fn validated_team_daemon_pid_file(
    pid_path: &Path,
    team_name: &str,
    require_current_binary: bool,
) -> Result<Option<u32>, CoordinationError> {
    let Some(pid) = read_pid_file(pid_path) else {
        return Ok(None);
    };
    if !is_process_running_by_pid_system(pid)? {
        return Ok(None);
    }
    if !process_matches_team_daemon(pid, team_name)? {
        return Ok(None);
    }
    if require_current_binary && !process_uses_current_mesh_binary(pid)? {
        return Ok(None);
    }
    Ok(Some(pid))
}

fn validated_mesh_daemon_pid_file(
    pid_path: &Path,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<Option<u32>, CoordinationError> {
    let Some(pid) = read_pid_file(pid_path) else {
        return Ok(None);
    };
    if !is_process_running_by_pid_system(pid)? {
        return Ok(None);
    }
    if !process_matches_mesh_daemon(pid, pane_id, team_name, member_name)? {
        return Ok(None);
    }
    Ok(Some(pid))
}

pub(super) fn validated_mesh_daemon_pid_file_by_member(
    pid_path: &Path,
    team_name: &str,
    member_name: &str,
) -> Result<Option<u32>, CoordinationError> {
    let Some(pid) = read_pid_file(pid_path) else {
        return Ok(None);
    };
    if !is_process_running_by_pid_system(pid)? {
        return Ok(None);
    }
    if !process_matches_mesh_daemon_member(pid, team_name, member_name)? {
        return Ok(None);
    }
    Ok(Some(pid))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

pub(super) fn delete_pid_file_if_present(pid_path: Option<&Path>) -> Result<(), CoordinationError> {
    let Some(pid_path) = pid_path else {
        return Ok(());
    };
    match fs::remove_file(pid_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(CoordinationError::Io(err)),
    }
}

pub(super) fn find_existing_mesh_daemon_pids_system(
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<Vec<u32>, CoordinationError> {
    let mut matches = Vec::new();

    if let Some(pid_path) = resolve_mesh_daemon_pid_path(team_name, member_name) {
        if let Some(pid) = read_pid_file(&pid_path) {
            if is_process_running_by_pid_system(pid)?
                && process_matches_mesh_daemon(pid, pane_id, team_name, member_name)?
            {
                matches.push(pid);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        for entry in fs::read_dir("/proc").map_err(CoordinationError::Io)? {
            let entry = entry.map_err(CoordinationError::Io)?;
            let raw_name = entry.file_name();
            let Ok(pid) = raw_name.to_string_lossy().parse::<u32>() else {
                continue;
            };
            if matches.contains(&pid) {
                continue;
            }
            if process_matches_mesh_daemon(pid, pane_id, team_name, member_name)? {
                matches.push(pid);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        for pid in list_process_ids_via_ps()? {
            if matches.contains(&pid) {
                continue;
            }
            if process_matches_mesh_daemon(pid, pane_id, team_name, member_name)? {
                matches.push(pid);
            }
        }
    }

    Ok(matches)
}

pub(super) fn resolve_mesh_daemon_pid_path(team_name: &str, member_name: &str) -> Option<PathBuf> {
    resolve_host_claude_dir().map(|claude_dir| {
        claude_dir
            .join("teams")
            .join(team_name)
            .join("daemons")
            .join(format!("{member_name}.pid"))
    })
}

pub(super) fn resolve_team_daemon_pid_path(team_name: &str) -> Option<PathBuf> {
    resolve_host_claude_dir().map(|claude_dir| {
        claude_dir
            .join("teams")
            .join(team_name)
            .join("daemons")
            .join("team.pid")
    })
}

fn resolve_mesh_control_credential_path(team_name: &str, member_name: &str) -> Option<PathBuf> {
    resolve_host_claude_dir().map(|claude_dir| {
        claude_dir
            .join("teams")
            .join(team_name)
            .join("state")
            .join("control_auth")
            .join(format!("{member_name}.json"))
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeshControlCredential {
    name: String,
    token: String,
}

pub(crate) fn resolve_mesh_control_token(team_name: &str, member_name: &str) -> Option<String> {
    let path = resolve_mesh_control_credential_path(team_name, member_name)?;
    let raw = fs::read_to_string(path).ok()?;
    let credential: MeshControlCredential = serde_json::from_str(&raw).ok()?;
    if credential.name != member_name || credential.token.trim().is_empty() {
        return None;
    }
    Some(credential.token)
}

fn resolve_host_claude_dir() -> Option<PathBuf> {
    Some(PlatformPaths::claude_dir())
}

pub(crate) fn resolve_mesh_cli_claude_dir_arg() -> Option<String> {
    resolve_host_claude_dir().map(|path| mesh_cli_claude_dir_arg_from_path(&path))
}

fn mesh_cli_claude_dir_arg_from_path(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        crate::provider::path::to_linux(&raw).unwrap_or(raw)
    }
    #[cfg(not(target_os = "windows"))]
    {
        raw
    }
}

fn process_matches_mesh_daemon(
    pid: u32,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<bool, CoordinationError> {
    match process_uses_current_mesh_binary(pid) {
        Ok(false) => return Ok(false),
        Ok(true) => {}
        Err(err) => {
            tracing::debug!(
                pid,
                error = %err,
                "mesh daemon binary identity check failed, assuming current"
            );
        }
    }
    let Some(args) = read_process_cmdline_args(pid)? else {
        return Ok(false);
    };
    Ok(command_matches_mesh_daemon(
        &args,
        pane_id,
        team_name,
        member_name,
    ))
}

fn process_matches_mesh_daemon_member(
    pid: u32,
    team_name: &str,
    member_name: &str,
) -> Result<bool, CoordinationError> {
    match process_uses_current_mesh_binary(pid) {
        Ok(false) => return Ok(false),
        Ok(true) => {}
        Err(err) => {
            tracing::debug!(
                pid,
                error = %err,
                "mesh daemon binary identity check failed, assuming current"
            );
        }
    }
    let Some(args) = read_process_cmdline_args(pid)? else {
        return Ok(false);
    };
    Ok(command_matches_mesh_daemon_member(
        &args,
        team_name,
        member_name,
    ))
}

fn command_matches_mesh_daemon(
    args: &[String],
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> bool {
    if !command_matches_mesh_binary(args) {
        return false;
    }
    args.iter().any(|arg| arg == "daemon")
        && command_has_flag_value(args, "--pane", pane_id)
        && command_has_flag_value(args, "--team", team_name)
        && command_has_flag_value(args, "--name", member_name)
}

fn command_matches_mesh_daemon_member(args: &[String], team_name: &str, member_name: &str) -> bool {
    command_matches_mesh_binary(args)
        && args.iter().any(|arg| arg == "daemon")
        && command_has_flag_value(args, "--team", team_name)
        && command_has_flag_value(args, "--name", member_name)
}

pub(super) fn process_matches_team_daemon(
    pid: u32,
    team_name: &str,
) -> Result<bool, CoordinationError> {
    let Some(args) = read_process_cmdline_args(pid)? else {
        return Ok(false);
    };
    Ok(command_matches_team_daemon(&args, team_name))
}

pub(super) fn command_matches_team_daemon(args: &[String], team_name: &str) -> bool {
    if !command_matches_mesh_binary(args) {
        return false;
    }
    args.iter().any(|arg| arg == "team-daemon")
        && args.iter().any(|arg| arg == "start")
        && command_has_flag_value(args, "--team", team_name)
}

fn command_matches_mesh_binary(args: &[String]) -> bool {
    let Some(program) = args.first() else {
        return false;
    };
    let binary = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    binary == "mesh" || binary == "mesh.exe"
}

fn command_has_flag_value(args: &[String], flag: &str, expected: &str) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == expected)
}

pub(super) fn is_process_running_by_pid_system(pid: u32) -> Result<bool, CoordinationError> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Ok(false);
    }
    let pid_arg = validate_coordination_pid(pid)?;

    #[cfg(target_os = "windows")]
    let invocation = wsl_kill_invocation("-0", &pid_arg);
    #[cfg(not(target_os = "windows"))]
    let invocation = CommandInvocation {
        program: "kill".to_string(),
        args: vec!["-0".to_string(), pid_arg.clone()],
    };

    let output = run_system_command(&invocation)?;
    if output.status.success() {
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    if stderr.contains("operation not permitted") {
        return Ok(true);
    }
    Ok(false)
}

fn read_process_cmdline_args(pid: u32) -> Result<Option<Vec<String>>, CoordinationError> {
    if !is_process_running_by_pid_system(pid)? {
        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        let output = run_system_command(&wsl_cat_proc_cmdline_invocation(pid))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(CoordinationError::Backend(format!(
                "failed to read process command line for pid {pid}: {stderr}"
            )));
        }
        Ok(parse_process_cmdline_bytes(&output.stdout))
    }
    #[cfg(target_os = "linux")]
    {
        let cmdline_path = PathBuf::from("/proc").join(pid.to_string()).join("cmdline");
        let raw = match fs::read(cmdline_path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(CoordinationError::Io(err)),
        };
        Ok(parse_process_cmdline_bytes(&raw))
    }

    #[cfg(target_os = "macos")]
    {
        let output = run_system_command(&CommandInvocation {
            program: "ps".to_string(),
            args: vec![
                "-ww".to_string(),
                "-o".to_string(),
                "command=".to_string(),
                "-p".to_string(),
                pid.to_string(),
            ],
        })?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(parse_process_command_text(&output.stdout))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn parse_process_cmdline_bytes(raw: &[u8]) -> Option<Vec<String>> {
    let args = raw
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect::<Vec<_>>();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

#[cfg(target_os = "macos")]
fn parse_process_command_text(raw: &[u8]) -> Option<Vec<String>> {
    let text = String::from_utf8_lossy(raw);
    let line = text.lines().find(|line| !line.trim().is_empty())?.trim();
    let args = line
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

pub(super) fn process_uses_current_mesh_binary(pid: u32) -> Result<bool, CoordinationError> {
    let Some(mesh_path) = mesh_cli::mesh_binary_path() else {
        return Ok(true);
    };

    let Some(process_identity) = process_executable_identity(pid)? else {
        return Ok(false);
    };
    let Some(installed_identity) = mesh_binary_identity(&mesh_path)? else {
        return Ok(true);
    };
    Ok(process_identity == installed_identity)
}

fn mesh_binary_identity(mesh_path: &str) -> Result<Option<FileIdentity>, CoordinationError> {
    #[cfg(target_os = "windows")]
    {
        wsl_file_identity(mesh_path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_file_identity(Path::new(mesh_path))
    }
}

fn process_executable_identity(pid: u32) -> Result<Option<FileIdentity>, CoordinationError> {
    #[cfg(target_os = "windows")]
    {
        wsl_file_identity(&format!("/proc/{pid}/exe"))
    }
    #[cfg(target_os = "linux")]
    {
        unix_file_identity(&PathBuf::from("/proc").join(pid.to_string()).join("exe"))
    }
    #[cfg(target_os = "macos")]
    {
        let path = process_executable_path_macos(pid)?;
        match path {
            Some(path) => unix_file_identity(&path),
            None => Ok(None),
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn unix_file_identity(path: &Path) -> Result<Option<FileIdentity>, CoordinationError> {
    use std::os::unix::fs::MetadataExt;

    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(CoordinationError::Io(err)),
    }
}

#[cfg(target_os = "macos")]
fn process_executable_path_macos(pid: u32) -> Result<Option<PathBuf>, CoordinationError> {
    use libproc::libproc::proc_pid::pidpath;

    let pid = validate_coordination_pid(pid)?
        .parse::<i32>()
        .map_err(|_| CoordinationError::Validation(format!("pid out of supported range: {pid}")))?;
    match pidpath(pid) {
        Ok(path) => Ok(Some(PathBuf::from(path))),
        Err(err) if err.contains("No such process") => Ok(None),
        Err(err) => Err(CoordinationError::Backend(format!(
            "failed to resolve executable path for pid {pid}: {err}"
        ))),
    }
}

#[cfg(target_os = "macos")]
fn list_process_ids_via_ps() -> Result<Vec<u32>, CoordinationError> {
    let output = run_system_command(&CommandInvocation {
        program: "ps".to_string(),
        args: vec!["-axo".to_string(), "pid=".to_string()],
    })?;
    if !output.status.success() {
        return Err(CoordinationError::Backend(format!(
            "failed to enumerate process ids via ps: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect())
}

#[cfg(target_os = "windows")]
fn wsl_file_identity(path: &str) -> Result<Option<FileIdentity>, CoordinationError> {
    let invocation = CommandInvocation {
        program: "wsl".to_string(),
        args: mesh_cli::wrap_wsl_args_for_coordination(
            vec![
                "--".to_string(),
                "stat".to_string(),
                "-Lc".to_string(),
                "%d:%i".to_string(),
                path.to_string(),
            ],
            None,
        ),
    };
    let output = run_system_command(&invocation)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("no such file") || stderr.contains("cannot stat") {
            return Ok(None);
        }
        return Err(CoordinationError::Backend(format!(
            "failed to stat WSL path {path}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_file_identity(&output.stdout)
}

#[cfg(target_os = "windows")]
fn parse_file_identity(raw: &[u8]) -> Result<Option<FileIdentity>, CoordinationError> {
    let text = String::from_utf8_lossy(raw);
    let Some(line) = text.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    let Some((device, inode)) = line.trim().split_once(':') else {
        return Err(CoordinationError::Backend(format!(
            "invalid file identity output: {}",
            line.trim()
        )));
    };
    Ok(Some(FileIdentity {
        device: device
            .parse()
            .map_err(|_| CoordinationError::Backend(format!("invalid device id: {device}")))?,
        inode: inode
            .parse()
            .map_err(|_| CoordinationError::Backend(format!("invalid inode id: {inode}")))?,
    }))
}

pub(super) fn terminate_pid_invocation(
    pid: u32,
    force: bool,
) -> Result<CommandInvocation, CoordinationError> {
    #[cfg(target_os = "windows")]
    {
        let pid_arg = validate_coordination_pid(pid)?;
        let signal = if force { "-KILL" } else { "-TERM" };
        Ok(wsl_kill_invocation(signal, &pid_arg))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let pid_arg = validate_unix_pid(pid)?;
        let signal = if force { "-KILL" } else { "-TERM" };
        Ok(CommandInvocation {
            program: "kill".to_string(),
            args: vec![signal.to_string(), pid_arg],
        })
    }
}

fn validate_coordination_pid(pid: u32) -> Result<String, CoordinationError> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(CoordinationError::Validation(format!(
            "pid out of supported range: {pid}"
        )));
    }
    Ok(pid.to_string())
}

#[cfg(target_os = "windows")]
fn wsl_kill_invocation(signal: &str, pid_arg: &str) -> CommandInvocation {
    CommandInvocation {
        program: "wsl".to_string(),
        args: mesh_cli::wrap_wsl_args_for_coordination(
            vec![
                "--".to_string(),
                "kill".to_string(),
                signal.to_string(),
                pid_arg.to_string(),
            ],
            None,
        ),
    }
}

#[cfg(target_os = "windows")]
fn wsl_cat_proc_cmdline_invocation(pid: u32) -> CommandInvocation {
    CommandInvocation {
        program: "wsl".to_string(),
        args: mesh_cli::wrap_wsl_args_for_coordination(
            vec![
                "--".to_string(),
                "cat".to_string(),
                format!("/proc/{pid}/cmdline"),
            ],
            None,
        ),
    }
}

pub(super) fn run_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Output, CoordinationError> {
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args).output()
    } else {
        let mut cmd = Command::new(&invocation.program);
        apply_background_command_settings(&mut cmd);
        cmd.args(&invocation.args).output()
    };
    output.map_err(CoordinationError::Io)
}

pub(super) fn spawn_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Child, CoordinationError> {
    let child = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    } else {
        let mut cmd = Command::new(&invocation.program);
        apply_background_command_settings(&mut cmd);
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    child.map_err(CoordinationError::Io)
}

pub(super) fn run_mesh(args: &[&str], cwd: Option<&str>) -> Result<String, CoordinationError> {
    let invocation = mesh_command_invocation(args);
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        if let Some(project_id) = cwd {
            cmd.args(["--cd", project_id]);
        }
        cmd.args(&invocation.args).output()
    } else {
        let mut cmd = Command::new(&invocation.program);
        apply_background_command_settings(&mut cmd);
        cmd.args(&invocation.args);
        if let Some(project_id) = cwd {
            cmd.current_dir(project_id);
        }
        cmd.output()
    }
    .map_err(CoordinationError::Io)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "mesh command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn validate_unix_pid(pid: u32) -> Result<String, CoordinationError> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(CoordinationError::Validation(format!(
            "pid out of Unix kill range: {pid}"
        )));
    }
    Ok(pid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{LazyLock, Mutex, MutexGuard};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

    struct EnvGuard {
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                _guard: ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner()),
            }
        }
    }

    #[test]
    fn claude_override_drives_runtime_host_paths() {
        let _guard = EnvGuard::new();
        let temp = tempfile::tempdir().expect("tempdir");
        let override_dir = temp.path().join("custom-claude-root");
        std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, &override_dir);

        let host_claude_dir = resolve_host_claude_dir().expect("claude dir should resolve");
        let mesh_pid_path =
            resolve_mesh_daemon_pid_path("taurhaus-team", "dev-3").expect("mesh pid path");
        let team_pid_path = resolve_team_daemon_pid_path("taurhaus-team").expect("team pid path");
        let control_path = resolve_mesh_control_credential_path("taurhaus-team", "dev-3")
            .expect("control credential path");
        let mesh_arg = resolve_mesh_cli_claude_dir_arg().expect("mesh claude dir arg");

        std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV);

        assert_eq!(host_claude_dir, override_dir);
        assert_eq!(
            mesh_pid_path,
            override_dir
                .join("teams")
                .join("taurhaus-team")
                .join("daemons")
                .join("dev-3.pid")
        );
        assert_eq!(
            team_pid_path,
            override_dir
                .join("teams")
                .join("taurhaus-team")
                .join("daemons")
                .join("team.pid")
        );
        assert_eq!(
            control_path,
            override_dir
                .join("teams")
                .join("taurhaus-team")
                .join("state")
                .join("control_auth")
                .join("dev-3.json")
        );
        assert_eq!(mesh_arg, override_dir.to_string_lossy());
    }

    #[test]
    fn mesh_cli_claude_dir_arg_translation_keeps_runtime_accessible_paths() {
        let native = Path::new("/tmp/custom-claude");
        assert_eq!(
            mesh_cli_claude_dir_arg_from_path(native),
            "/tmp/custom-claude"
        );

        #[cfg(target_os = "windows")]
        {
            assert_eq!(
                mesh_cli_claude_dir_arg_from_path(Path::new(r"C:\Users\dev\custom-claude")),
                "/mnt/c/Users/dev/custom-claude"
            );
            assert_eq!(
                mesh_cli_claude_dir_arg_from_path(Path::new(
                    r"\\wsl.localhost\Ubuntu\home\dev\.claude"
                )),
                "/home/dev/.claude"
            );
        }
    }
}
