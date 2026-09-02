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
use crate::session_scanner::DisplaySession;
use crate::ProviderState;

#[cfg(test)]
use self::activity_tracking::promote_activity_from_sessions_impl;
use self::activity_tracking::{
    get_project_activity_impl, promote_activity_from_sessions, record_session_activity_impl,
};
pub use self::launching::LaunchAccountPreview;
#[cfg(test)]
use self::launching::{decode_daemon_launch_result, launch_cli_session_impl};
use self::launching::{
    launch_cli_session_through_daemon_impl, resolve_launch_account_preview_impl,
};
use self::navigation::{navigate_to_session_impl, stop_cli_session_impl};
use self::session_listing::list_cli_sessions_impl;
pub use self::session_listing::CliSessionSnapshot;
#[cfg(test)]
use self::session_listing::{daemon_display_sessions, decode_daemon_session_list};
use crate::commands::runtime_snapshot::daemon_runtime_session_snapshot;

static SESSION_ACTIVITY_RECONCILE_QUEUED: AtomicBool = AtomicBool::new(false);

#[tauri::command(async)]
pub fn list_cli_sessions(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> IpcResult<Vec<DisplaySession>> {
    let span = IpcCommandSpan::start("list_cli_sessions");
    let result = list_cli_sessions_impl(&app, db.inner(), provider.inner())
        .map(|snapshot| snapshot.sessions)
        .ipc_cmd("list_cli_sessions");
    span.finish_result(&result);
    result
}

/// The same sessions as `list_cli_sessions`, plus how they were obtained.
///
/// The store that polls when the daemon bridge is down measures time against
/// the interval between two observations; a replayed or cached list is not one,
/// and reading it as one credits the outage to the last state seen.
#[tauri::command(async)]
pub fn list_cli_session_snapshot(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> IpcResult<CliSessionSnapshot> {
    let span = IpcCommandSpan::start("list_cli_session_snapshot");
    let result = list_cli_sessions_impl(&app, db.inner(), provider.inner())
        .ipc_cmd("list_cli_session_snapshot");
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub fn launch_cli_session(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, crate::commands::logging::LogFileState>,
    coordination_state: State<'_, CoordinationState>,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
    account_id: Option<String>,
) -> IpcResult<protocol::LaunchSessionResult> {
    let span = IpcCommandSpan::start("launch_cli_session");
    let result = launch_cli_session_through_daemon_impl(
        &app,
        db.inner(),
        provider.inner(),
        log_file.inner(),
        Some(coordination_state.inner()),
        project_id,
        mode,
        cli_tool,
        account_id,
    )
    .ipc_cmd("launch_cli_session");
    span.finish_result(&result);
    result
}

/// Which tool account a launch would run on, before it runs.
///
/// The chooser asks the user exactly once, and only when it has to. Whether it
/// has to is a backend question: the transcript of the project's last session
/// decides every resume, and a stored choice that logged out decides nothing.
#[tauri::command(async)]
pub fn resolve_launch_account(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    project_id: String,
    tool: CliTool,
    mode: LaunchMode,
    session_id: Option<String>,
) -> IpcResult<LaunchAccountPreview> {
    let span = IpcCommandSpan::start("resolve_launch_account");
    let result = resolve_launch_account_preview_impl(
        db.inner(),
        provider.inner(),
        project_id,
        tool,
        mode,
        session_id.as_deref(),
    )
    .ipc_cmd("resolve_launch_account");
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn get_project_activity(
    db: State<'_, DbState>,
    project_id: String,
) -> IpcResult<crate::db::activity_queries::ProjectActivityStats> {
    let span = IpcCommandSpan::start("get_project_activity");
    let result = get_project_activity_impl(db.inner(), &project_id).ipc_cmd("get_project_activity");
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn get_foreground_project(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
) -> IpcResult<Option<String>> {
    let span = IpcCommandSpan::start("get_foreground_project");
    let result =
        get_foreground_project_impl(db.inner(), provider.inner()).ipc_cmd("get_foreground_project");
    span.finish_result(&result);
    result
}

/// The foreground project, as the daemon hub last observed it.
///
/// The hub owns tmux focus (`tmux list-clients` once per scanner cycle) and
/// resolves it to a project path; this IPC is the app's startup read of that
/// snapshot. Live updates arrive on the `tmux-focus-changed` event instead.
pub(crate) fn get_foreground_project_impl(
    db: &DbState,
    provider: &ProviderState,
) -> Result<Option<String>, String> {
    let Some(snapshot) = daemon_runtime_session_snapshot(provider)?.snapshot else {
        return Ok(None);
    };
    let Some(project_path) = snapshot.foreground_project_path else {
        return Ok(None);
    };

    resolve_project_id_from_path(db, &localize_daemon_project_path(provider, project_path))
}

/// Translate a daemon-side (Linux) project path into the app's path form.
pub(crate) fn localize_daemon_project_path(
    provider: &ProviderState,
    project_path: String,
) -> String {
    if crate::daemon::launcher::is_native_daemon() || !project_path.starts_with('/') {
        return project_path;
    }
    match provider.wsl_distro {
        Some(ref distro) => crate::provider::path::to_windows(&project_path, distro),
        None => project_path,
    }
}

pub(crate) fn resolve_project_id_from_path(
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
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    crate::db::queries::get_project(&conn, project_id)
        .sanitize_err()?
        .map(|project| project.path)
        .ok_or_else(|| format!("Project not found: {project_id}"))
}

/// A project's path and memory for one tool, in one read.
fn resolve_project_launch_target(
    db: &DbState,
    project_id: &str,
    tool: CliTool,
) -> Result<(String, Option<crate::models::AccountMemory>), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = crate::db::queries::get_project(&conn, project_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    let account_memory = project.account_memory.get(&tool.to_string()).cloned();
    Ok((project.path, account_memory))
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
        ..Default::default()
    }
}

fn delegate_launch_to_coordination_resume(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
    target: &TeamMemberMatch,
    tool: CliTool,
) -> Result<protocol::LaunchSessionResult, String> {
    delegate_launch_to_coordination_resume_with(
        db,
        provider,
        target,
        tool,
        |request, cli_commands, tmux_layout| {
            let daemon = provider
                .daemon
                .as_ref()
                .ok_or_else(|| "resuming a team member requires the taurhaus daemon".to_string())?;
            crate::commands::coordination::resume_member_through_daemon(
                app,
                daemon,
                crate::daemon::protocol::CoordinationResumeMemberParams {
                    request,
                    cli_commands,
                    tmux_layout,
                    operational_snapshot: None,
                    task_state_changed_at: None,
                },
                None,
            )
        },
    )
}

fn delegate_launch_to_coordination_resume_with(
    db: &DbState,
    provider: &ProviderState,
    target: &TeamMemberMatch,
    tool: CliTool,
    resume: impl FnOnce(
        ResumeMemberRequest,
        crate::models::CliCommandSettings,
        String,
    ) -> Result<crate::coordination::requests::ResumeAgentReport, String>,
) -> Result<protocol::LaunchSessionResult, String> {
    let mut terminal_settings = crate::commands::terminal_settings::load_terminal_settings(db);
    crate::commands::terminal_settings::apply_managed_codex_launch_inputs(
        &mut terminal_settings.cli_commands,
        crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .hook_trust,
        false,
    );
    crate::commands::accounts::apply_team_launch_base_resolutions(
        provider,
        &mut terminal_settings.cli_commands,
        [tool],
    );
    let opaque_head = terminal_settings
        .cli_commands
        .resolved_bases
        // This delegated request has no resume session id, so coordination's
        // renderer selects the Fresh base even though the UI action is Resume.
        .get(&(tool, protocol::LaunchMode::Fresh))
        .and_then(|base| base.opaque_head.clone());
    let request = ResumeMemberRequest {
        team_name: target.team_name.clone(),
        member_name: target.member_name.clone(),
        reasoning_effort_override: None,
    };

    let report = resume(
        request,
        terminal_settings.cli_commands,
        terminal_settings.tmux_layout,
    )?;

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

    Ok(launching::note_opaque_base(
        tmux_launch_result_for_pane(&pane_id),
        opaque_head.as_deref(),
    ))
}

#[cfg(test)]
fn delegate_launch_to_coordination_resume_in_process_for_test(
    db: &DbState,
    provider: &ProviderState,
    coordination_state: &CoordinationState,
    target: &TeamMemberMatch,
    tool: CliTool,
) -> Result<protocol::LaunchSessionResult, String> {
    delegate_launch_to_coordination_resume_with(
        db,
        provider,
        target,
        tool,
        |request, cli_commands, tmux_layout| {
            coordination_state
                .with_orchestrator(|orchestrator| {
                    orchestrator.resume_member_with_cli_commands_and_layout(
                        &request,
                        &cli_commands,
                        &tmux_layout,
                    )
                })
                .map_err(|error| error.to_string())
        },
    )
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
