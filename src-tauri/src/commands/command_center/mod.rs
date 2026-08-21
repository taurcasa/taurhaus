mod activity_tracking;
mod launching;
mod navigation;
mod session_listing;

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::coordination::requests::ResumeMemberRequest;
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::TeamConfigStore;
use crate::daemon::protocol::{self, LaunchMode};
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::platform::apply_background_command_settings;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::TMUX_SESSION_NAME;
use crate::session_scanner::tmux::TmuxFocusState;
use crate::session_scanner::DisplaySession;
use crate::ProviderState;

#[cfg(test)]
use self::activity_tracking::promote_activity_from_sessions_impl;
use self::activity_tracking::{
    get_project_activity_impl, promote_activity_from_sessions, record_session_activity_impl,
};
#[cfg(test)]
use self::launching::decode_daemon_launch_result;
use self::launching::launch_cli_session_impl;
use self::navigation::{navigate_to_session_impl, stop_cli_session_impl};
use self::session_listing::list_cli_sessions_impl;
#[cfg(test)]
use self::session_listing::{daemon_display_sessions, decode_daemon_session_list};
use crate::commands::runtime_snapshot::daemon_runtime_session_snapshot;

static SESSION_ACTIVITY_RECONCILE_QUEUED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn list_cli_sessions(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> IpcResult<Vec<DisplaySession>> {
    let span = IpcCommandSpan::start("list_cli_sessions");
    let result =
        list_cli_sessions_impl(&app, db.inner(), provider.inner()).ipc_cmd("list_cli_sessions");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn launch_cli_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, crate::commands::logging::LogFileState>,
    coordination_state: State<'_, CoordinationState>,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
) -> IpcResult<protocol::LaunchSessionResult> {
    let span = IpcCommandSpan::start("launch_cli_session");
    let result = launch_cli_session_impl(
        db.inner(),
        provider.inner(),
        log_file.inner(),
        Some(coordination_state.inner()),
        project_id,
        mode,
        cli_tool,
    )
    .ipc_cmd("launch_cli_session");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn stop_cli_session(
    log_file: State<'_, crate::commands::logging::LogFileState>,
    provider: State<'_, ProviderState>,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("stop_cli_session");
    let result = stop_cli_session_impl(log_file.inner(), provider.inner(), tmux_pane, cli_tool)
        .ipc_cmd("stop_cli_session");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn navigate_to_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, crate::commands::logging::LogFileState>,
    tmux_session: String,
    tmux_window: String,
    tmux_pane: String,
    open_terminal: Option<bool>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("navigate_to_session");
    let result = navigate_to_session_impl(
        db.inner(),
        provider.inner(),
        log_file.inner(),
        tmux_session,
        tmux_window,
        tmux_pane,
        open_terminal,
    )
    .ipc_cmd("navigate_to_session");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn record_session_activity(
    db: State<'_, DbState>,
    project_id: String,
    cli_tool: CliTool,
    started_at: String,
    ended_at: String,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("record_session_activity");
    let result = record_session_activity_impl(
        db.inner(),
        project_id,
        cli_tool,
        started_at,
        ended_at,
        active_duration_ms,
        total_duration_ms,
    )
    .ipc_cmd("record_session_activity");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_project_activity(
    db: State<'_, DbState>,
    project_id: String,
) -> IpcResult<crate::db::activity_queries::ProjectActivityStats> {
    let span = IpcCommandSpan::start("get_project_activity");
    let result = get_project_activity_impl(db.inner(), &project_id).ipc_cmd("get_project_activity");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_foreground_project(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> IpcResult<Option<String>> {
    let span = IpcCommandSpan::start("get_foreground_project");
    let result = get_foreground_project_impl(&app, db.inner(), provider.inner())
        .ipc_cmd("get_foreground_project");
    span.finish_result(&result);
    result
}

pub(crate) fn get_foreground_project_impl(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
) -> Result<Option<String>, String> {
    if let Some(snapshot) = daemon_runtime_session_snapshot(provider)?.snapshot {
        if let Some(mut project_path) = snapshot.foreground_project_path.clone() {
            if !crate::daemon::launcher::is_native_daemon() && project_path.starts_with('/') {
                if let Some(ref distro) = provider.wsl_distro {
                    project_path = crate::provider::path::to_windows(&project_path, distro);
                }
            }
            return resolve_project_id_from_path(db, &project_path);
        }
        if snapshot.focus.is_some() {
            return Ok(None);
        }

        let data_dir = crate::startup::resolve_app_data_dir(app.clone()).map_err(|error| {
            format!("Failed to resolve app data dir for tmux focus lookup: {error}")
        })?;
        return resolve_foreground_project_from_daemon_snapshot(&data_dir, db, provider, snapshot);
    }

    if provider.daemon.is_some() {
        return Ok(None);
    }

    let data_dir = crate::startup::resolve_app_data_dir(app.clone()).map_err(|error| {
        format!("Failed to resolve app data dir for tmux focus lookup: {error}")
    })?;
    let Some(focus) = crate::session_scanner::tmux::read_focus_state(&data_dir) else {
        return Ok(None);
    };

    let sessions = list_cli_sessions_impl(app, db, provider)?;
    resolve_foreground_project_id_from_sessions(db, &focus, &sessions)
}

fn resolve_foreground_project_from_daemon_snapshot(
    data_dir: &Path,
    db: &DbState,
    provider: &ProviderState,
    mut snapshot: protocol::RuntimeSessionSnapshotResult,
) -> Result<Option<String>, String> {
    let Some(focus) = crate::session_scanner::tmux::read_focus_state(data_dir) else {
        return Ok(None);
    };
    if !crate::daemon::launcher::is_native_daemon() {
        if let Some(ref distro) = provider.wsl_distro {
            for session in &mut snapshot.display_sessions {
                if session.project_path.starts_with('/') {
                    session.project_path =
                        crate::provider::path::to_windows(&session.project_path, distro);
                }
            }
        }
    }

    resolve_foreground_project_id_from_sessions(db, &focus, &snapshot.display_sessions)
}

fn resolve_project_id_from_path(
    db: &DbState,
    project_path: &str,
) -> Result<Option<String>, String> {
    let project_key = crate::provider::path::normalize_project_path(project_path);
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let projects = crate::db::queries::list_projects(&conn).sanitize_err()?;
    Ok(projects
        .into_iter()
        .find(|project| crate::provider::path::normalize_project_path(&project.path) == project_key)
        .map(|project| project.id))
}

fn resolve_foreground_project_id_from_sessions(
    db: &DbState,
    focus: &TmuxFocusState,
    sessions: &[DisplaySession],
) -> Result<Option<String>, String> {
    let Some(project_path) =
        crate::session_scanner::tmux::resolve_focus_project_path(focus, sessions)
    else {
        return Ok(None);
    };

    resolve_project_id_from_path(db, &project_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TeamMemberMatch {
    team_name: String,
    member_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TeamMemberMatchResult {
    None,
    Unique(TeamMemberMatch),
    Ambiguous,
}

fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = crate::db::queries::get_project(&conn, project_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
}

fn find_unique_team_member_match(
    teams_dir: &Path,
    project_path: &str,
    cli_tool: CliTool,
) -> TeamMemberMatchResult {
    let team_names = match TeamConfigStore::list(teams_dir) {
        Ok(team_names) => team_names,
        Err(error) => {
            tracing::warn!(
                teams_dir = %teams_dir.display(),
                error = %error,
                "Failed to list team configs while resolving generic resume delegation"
            );
            return TeamMemberMatchResult::None;
        }
    };

    let project_key = crate::provider::path::normalize_project_path(project_path);
    let mut matches = Vec::new();

    for team_name in team_names {
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    team_name = %team_name,
                    error = %error,
                    "Failed to load team config while resolving generic resume delegation"
                );
                continue;
            }
        };

        for member in config.members {
            if member.cli_tool != cli_tool {
                continue;
            }

            if crate::provider::path::normalize_project_path(
                &member.project_path.display().to_string(),
            ) != project_key
            {
                continue;
            }

            matches.push(TeamMemberMatch {
                team_name: team_name.clone(),
                member_name: member.name,
            });

            if matches.len() > 1 {
                return TeamMemberMatchResult::Ambiguous;
            }
        }
    }

    matches
        .into_iter()
        .next()
        .map(TeamMemberMatchResult::Unique)
        .unwrap_or(TeamMemberMatchResult::None)
}

fn tmux_launch_result_for_pane(pane_id: &str) -> protocol::LaunchSessionResult {
    let mut tmux_session = TMUX_SESSION_NAME.to_string();
    let mut tmux_window = "0".to_string();

    let mut cmd = Command::new("tmux");
    apply_background_command_settings(&mut cmd);

    if let Ok(output) = cmd
        .args([
            "display-message",
            "-p",
            "-t",
            pane_id,
            "#{session_name}\t#{window_index}",
        ])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            let mut parts = raw.trim().splitn(2, '\t');
            if let Some(session_name) = parts.next().filter(|value| !value.is_empty()) {
                tmux_session = session_name.to_string();
            }
            if let Some(window_index) = parts.next().filter(|value| !value.is_empty()) {
                tmux_window = window_index.to_string();
            }
        }
    }

    protocol::LaunchSessionResult {
        tmux_session: Some(tmux_session),
        tmux_window,
        tmux_pane: pane_id.to_string(),
    }
}

fn delegate_launch_to_coordination_resume(
    db: &DbState,
    coordination_state: &CoordinationState,
    target: &TeamMemberMatch,
) -> Result<protocol::LaunchSessionResult, String> {
    let terminal_settings = crate::commands::terminal_settings::load_terminal_settings(db);
    let request = ResumeMemberRequest {
        team_name: target.team_name.clone(),
        member_name: target.member_name.clone(),
    };

    let report = coordination_state
        .with_orchestrator(|orchestrator| {
            orchestrator.resume_member_with_cli_commands_and_layout(
                &request,
                &terminal_settings.cli_commands,
                &terminal_settings.tmux_layout,
            )
        })
        .map_err(|error| error.to_string())?;

    if !report.resumed {
        return Err(format!(
            "Failed to resume team member '{}': {}",
            target.member_name, report.message
        ));
    }

    let pane_id = report.pane_id.ok_or_else(|| {
        format!(
            "Coordination resume did not return a pane id for '{}'",
            target.member_name
        )
    })?;

    Ok(tmux_launch_result_for_pane(&pane_id))
}

fn enqueue_activity_watch_reconcile(app: tauri::AppHandle, reason: &'static str) {
    if SESSION_ACTIVITY_RECONCILE_QUEUED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    #[cfg(test)]
    {
        crate::startup::watchers::reconcile_activity_watches(&app, reason);
        SESSION_ACTIVITY_RECONCILE_QUEUED.store(false, Ordering::Release);
    }

    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            struct ResetQueuedFlag;
            impl Drop for ResetQueuedFlag {
                fn drop(&mut self) {
                    SESSION_ACTIVITY_RECONCILE_QUEUED.store(false, Ordering::Release);
                }
            }

            let _reset_queued_flag = ResetQueuedFlag;
            crate::startup::watchers::reconcile_activity_watches(&app, reason);
        });
    }
}

#[cfg(test)]
mod tests;
