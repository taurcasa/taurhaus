use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::tmux_layout::{
    derive_window_name, parse_pane_records, parse_window_records, resolve_layout_allocation,
    resolve_split_target_pane, wait_for_tmux_session_ready, TmuxLayoutAllocation, TmuxLayoutPolicy,
    DEFAULT_SPLIT_MAX_PANES, LIST_PANES_FORMAT, LIST_WINDOWS_FORMAT,
};

use super::process::run_system_command;
use super::TAURHAUS_TMUX_SESSION_NAME;

fn tmux_command_invocation(args: &[String]) -> CommandInvocation {
    mesh_cli::command_invocation("tmux", args)
}

pub(super) fn run_tmux(args: &[String]) -> Result<String, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "tmux command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

pub(super) fn run_tmux_output(args: &[String]) -> Result<std::process::Output, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    run_system_command(&invocation)
}

fn ensure_taurhaus_tmux_session() -> Result<(), CoordinationError> {
    let check = run_tmux_output(&[
        "has-session".to_string(),
        "-t".to_string(),
        TAURHAUS_TMUX_SESSION_NAME.to_string(),
    ])?;

    if check.status.success() {
        return Ok(());
    }

    run_tmux(&[
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        TAURHAUS_TMUX_SESSION_NAME.to_string(),
    ])?;

    wait_for_tmux_session_ready(TAURHAUS_TMUX_SESSION_NAME, verify_tmux_session_ready)
        .map_err(CoordinationError::Backend)?;

    Ok(())
}

pub(super) fn create_tmux_pane_with_layout(
    project_id: &str,
    tmux_layout: &str,
) -> Result<String, CoordinationError> {
    ensure_taurhaus_tmux_session()?;

    let window_name = derive_window_name(project_id, "agent");
    let policy = TmuxLayoutPolicy::from_setting(tmux_layout, DEFAULT_SPLIT_MAX_PANES);
    let windows = match policy {
        TmuxLayoutPolicy::NewWindow => Vec::new(),
        _ => list_tmux_windows(TAURHAUS_TMUX_SESSION_NAME)?,
    };

    match resolve_layout_allocation(&policy, TAURHAUS_TMUX_SESSION_NAME, &window_name, &windows) {
        TmuxLayoutAllocation::NewWindow { window_name } => {
            create_tmux_new_window_pane(project_id, &window_name)
        }
        TmuxLayoutAllocation::SplitExisting { window_index, .. } => {
            let target_pane =
                resolve_split_target_pane_for_window(TAURHAUS_TMUX_SESSION_NAME, &window_index)?;
            create_tmux_split_pane(project_id, &target_pane)
        }
    }
}

fn create_tmux_new_window_pane(
    project_id: &str,
    window_name: &str,
) -> Result<String, CoordinationError> {
    let pane_id = run_tmux(&[
        "new-window".to_string(),
        "-n".to_string(),
        window_name.to_string(),
        "-t".to_string(),
        format!("{TAURHAUS_TMUX_SESSION_NAME}:"),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-c".to_string(),
        project_id.to_string(),
    ])?;

    parse_tmux_created_pane_id(&pane_id).ok_or_else(|| {
        CoordinationError::Backend(
            "tmux new-window returned empty output; expected pane identifier".to_string(),
        )
    })
}

fn create_tmux_split_pane(project_id: &str, target: &str) -> Result<String, CoordinationError> {
    let pane_id = run_tmux(&[
        "split-window".to_string(),
        "-h".to_string(),
        "-t".to_string(),
        target.to_string(),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-c".to_string(),
        project_id.to_string(),
    ])?;

    parse_tmux_created_pane_id(&pane_id).ok_or_else(|| {
        CoordinationError::Backend(
            "tmux split-window returned empty output; expected pane identifier".to_string(),
        )
    })
}

fn parse_tmux_created_pane_id(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .find(|token| !token.trim().is_empty())
        .map(str::to_string)
}

fn list_tmux_windows(
    tmux_session: &str,
) -> Result<Vec<crate::tmux_layout::TmuxWindowRecord>, CoordinationError> {
    let out = run_tmux(&[
        "list-windows".to_string(),
        "-t".to_string(),
        tmux_session.to_string(),
        "-F".to_string(),
        LIST_WINDOWS_FORMAT.to_string(),
    ])?;

    Ok(parse_window_records(&out))
}

fn list_tmux_window_panes(
    tmux_session: &str,
    window_index: &str,
) -> Result<Vec<crate::tmux_layout::TmuxPaneRecord>, CoordinationError> {
    let target = format!("{tmux_session}:{window_index}");
    let out = run_tmux(&[
        "list-panes".to_string(),
        "-t".to_string(),
        target,
        "-F".to_string(),
        LIST_PANES_FORMAT.to_string(),
    ])?;

    Ok(parse_pane_records(&out))
}

fn resolve_split_target_pane_for_window(
    tmux_session: &str,
    window_index: &str,
) -> Result<String, CoordinationError> {
    let panes = list_tmux_window_panes(tmux_session, window_index)?;
    let target = resolve_split_target_pane(tmux_session, window_index, &panes)
        .map_err(CoordinationError::Backend)?;

    let validation = run_tmux_output(&[
        "display-message".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        target.clone(),
        "#{pane_id}".to_string(),
    ])?;
    if validation.status.success() {
        return Ok(target);
    }

    let pane_ids = panes
        .iter()
        .map(|pane| pane.pane_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(CoordinationError::Backend(format!(
        "tmux window '{tmux_session}:{window_index}' resolved pane target '{target}' is not addressable; pane_ids=[{pane_ids}]"
    )))
}

fn verify_tmux_session_ready(tmux_session: &str) -> Result<(), String> {
    let windows = list_tmux_windows(tmux_session).map_err(|err| err.to_string())?;
    let first_window = windows
        .first()
        .ok_or_else(|| format!("tmux session '{tmux_session}' has no windows yet"))?;
    let _ = resolve_split_target_pane_for_window(tmux_session, &first_window.index)
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(super) fn tmux_target_for_pane(pane_id: &str) -> String {
    if pane_id.starts_with('%') {
        pane_id.to_string()
    } else {
        format!(":.{pane_id}")
    }
}

pub(super) fn is_shell_command(raw: &str) -> bool {
    let command = raw
        .trim()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches('-')
        .to_ascii_lowercase();
    matches!(command.as_str(), "bash" | "zsh" | "sh" | "fish")
}
