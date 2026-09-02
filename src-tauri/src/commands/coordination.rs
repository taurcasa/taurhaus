//! Coordination IPC commands for team management (M0 surface).

#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager, State};

#[path = "coordination/live_status.rs"]
mod live_status;
#[path = "coordination/mapping.rs"]
mod mapping;
#[path = "coordination/progress.rs"]
mod progress;
#[path = "coordination/request_normalization.rs"]
mod request_normalization;
#[cfg(test)]
#[path = "coordination/state_sync.rs"]
mod state_sync;

#[cfg(test)]
pub(crate) use progress::add_agent_progress_events;
#[allow(unused_imports)]
pub(crate) use progress::{
    emit_progress_log_event, emit_resume_team_progress_log_event, progress_events_for_steps,
    resume_member_progress_event_for_stage, resume_team_progress_event,
};

pub use crate::commands::coordination_types::*;
use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::coordination::backend::bridged::{availability_check, preflight_check};
#[cfg(test)]
use crate::coordination::backend::bridged::{
    availability_check_with_lookup, preflight_check_with_lookup, BinaryLookup,
};
use crate::coordination::compact_hook::{
    ensure_compact_hook_installed, team_has_managed_claude_member,
};
use crate::coordination::errors::CoordinationError;
#[cfg(test)]
use crate::coordination::requests::DeliveryRequest;
use crate::coordination::requests::DeliveryResult;
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::TeamConfigStore;
use crate::errors::{sanitize_error, CommandResultExt, IpcError, IpcResult};
use crate::models::CliCommandSettings;
#[cfg(test)]
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::TMUX_SESSION_NAME;
#[cfg(not(test))]
use crate::ProviderState;
use live_status::coordination_get_live_team_status_impl;
use mapping::*;
use progress::*;
use request_normalization::{
    hydrate_add_agent_request_role_metadata, hydrate_initialize_request_role_metadata,
    normalize_add_agent_request_path, normalize_initialize_request_paths,
};
#[cfg(test)]
use state_sync::*;
#[cfg(test)]
use taurhaus_lib::ProviderState;

/// The launch settings a background pass relaunches a member with.
///
/// The same resolution an operator-driven resume performs, minus the hook
/// write: a pass that runs on a timer reads whether the managed Codex hook is
/// installed rather than reconciling it. Without this the effort relaunch used
/// stock defaults and moved the member off the account it was launched on.
pub(crate) fn background_launch_settings(
    db: &DbState,
    teams_dir: &std::path::Path,
) -> (CliCommandSettings, String) {
    let (settings, discovery_error) = task_effort_launch_settings(db, teams_dir);
    if let Some(err) = discovery_error {
        tracing::warn!(
            error = %err,
            "managed-Codex discovery failed; background pass proceeds with managed inputs"
        );
    }
    settings
}

/// Strict launch settings for the task-arrival effort pass.
///
/// Unlike the shared background helper, this boundary reports a roster scan
/// failure to the caller so the typed effort pass cannot look successful when
/// it never established the launch inputs its target requires.
fn task_effort_launch_settings(
    db: &DbState,
    teams_dir: &std::path::Path,
) -> ((CliCommandSettings, String), Option<CoordinationError>) {
    let (has_managed_codex, discovery_error) =
        match crate::coordination::compact_hook::any_managed_codex_member(teams_dir) {
            Ok(has_managed_codex) => (has_managed_codex, None),
            Err(err) => (true, Some(err)),
        };
    (
        launch_settings_for_managed_codex(db, has_managed_codex),
        discovery_error,
    )
}

fn launch_settings_for_managed_codex(
    db: &DbState,
    has_managed_codex: bool,
) -> (CliCommandSettings, String) {
    let (mut cli_commands, tmux_layout) = load_cli_commands_and_layout(db);
    crate::commands::terminal_settings::apply_managed_codex_launch_inputs(
        &mut cli_commands,
        has_managed_codex,
        has_managed_codex && crate::coordination::compact_hook::codex_compact_hook_is_installed(),
    );
    (cli_commands, tmux_layout)
}

pub(crate) fn run_background_self_heal_pass(
    db: &DbState,
    provider: &ProviderState,
    state: &CoordinationState,
) -> Result<crate::coordination::state::BackgroundSelfHealPassResult, CoordinationError> {
    let (mut cli_commands, tmux_layout) = background_launch_settings(db, state.teams_dir());
    state.run_background_self_heal_pass_with_launch_resolution(
        &mut cli_commands,
        &tmux_layout,
        &mut |tool, commands| {
            crate::commands::accounts::apply_team_resume_launch_base_resolution(
                provider, commands, tool,
            );
        },
    )
}

/// Put a pending assignment effort into force after a project's tasks changed.
///
/// The task scan is the moment an assignment mesh wrote becomes visible to
/// taurhaus: the task record carries the level, and the operational snapshots
/// have just been rewritten from it. mesh applies the level itself before it
/// delivers the notice wherever the harness takes `/effort` in its own prompt;
/// this is the one harness it cannot reach, so acting here rather than on the
/// self-heal timer is the difference between seconds and a whole interval at
/// the wrong level.
///
/// Best-effort and quiet: nothing about a task scan depends on it.
pub(crate) fn apply_task_effort_after_task_change(app: &tauri::AppHandle, project_path: &str) {
    use tauri::Manager;

    let state = app.state::<crate::coordination::state::CoordinationState>();
    let db = app.state::<crate::commands::projects::DbState>();
    let provider = app.state::<ProviderState>();
    let ((mut cli_commands, tmux_layout), discovery_error) =
        task_effort_launch_settings(&db, state.teams_dir());

    match state.apply_task_effort_for_project_with_launch_resolution(
        project_path,
        &mut cli_commands,
        &tmux_layout,
        &mut |tool, commands| {
            crate::commands::accounts::apply_team_resume_launch_base_resolution(
                provider.inner(),
                commands,
                tool,
            );
        },
    ) {
        Ok(outcome) if !outcome.failed.is_empty() || !outcome.skipped_teams.is_empty() => {
            tracing::warn!(
                project_path = %project_path,
                members_failed = outcome.failed.len(),
                teams_skipped = outcome.skipped_teams.len(),
                "task-arrival effort pass completed with errors"
            );
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!(
                project_path = %project_path,
                error = %err,
                "task-arrival effort pass failed"
            );
        }
    }
    if let Some(err) = discovery_error {
        tracing::warn!(
            project_path = %project_path,
            error = %err,
            "task-arrival effort pass used conservative settings after managed-Codex discovery failed"
        );
    }
}

fn load_cli_commands_and_layout(db: &DbState) -> (CliCommandSettings, String) {
    #[cfg(test)]
    {
        (
            crate::commands::terminal_settings::load_cli_commands(db),
            "new_window".to_string(),
        )
    }
    #[cfg(not(test))]
    {
        let terminal_settings = crate::commands::terminal_settings::load_terminal_settings(db);
        (
            terminal_settings.cli_commands,
            terminal_settings.tmux_layout,
        )
    }
}

fn prepare_member_operation_snapshot(
    state: &CoordinationState,
    db: &DbState,
    team_name: &str,
    member_name: &str,
    project_path: &str,
) -> Result<
    (
        crate::coordination::stores::OperationalContextSnapshot,
        Option<chrono::DateTime<chrono::Utc>>,
    ),
    String,
> {
    let conn = db.0.lock().map_err(|_| "db mutex poisoned".to_string())?;
    crate::coordination::operational_context::prepare_member_snapshot_with_task_timestamp(
        state.teams_dir(),
        &conn,
        team_name,
        member_name,
        project_path,
    )
    .map_err(map_coordination_error)
}

#[cfg(test)]
const DEFAULT_TMUX_LAYOUT: &str = "new_window";

fn ensure_team_terminal_visible(db: &DbState, wsl_distro: Option<String>) {
    let terminal_settings = crate::commands::terminal_settings::load_terminal_settings(db);
    let _ = crate::terminal::handle_terminal(crate::terminal::TerminalIntent::EnsureOpen {
        distro: wsl_distro,
        tmux_session: TMUX_SESSION_NAME.to_string(),
        emulator: terminal_settings.emulator,
        custom_command: terminal_settings.custom_command,
    });
}

fn maybe_surface_terminal_after_initialize(
    db: &DbState,
    wsl_distro: Option<String>,
    report: &InitializeReport,
) {
    if report.failed_step.is_none() {
        ensure_team_terminal_visible(db, wsl_distro);
    }
}

fn maybe_surface_terminal_after_resume_team(
    db: &DbState,
    wsl_distro: Option<String>,
    report: &ResumeTeamReport,
) {
    if report.resumed {
        ensure_team_terminal_visible(db, wsl_distro);
    }
}

#[tauri::command]
pub async fn coordination_initialize_team(
    app: AppHandle,
    request: InitializeTeamRequest,
) -> IpcResult<InitializeReport> {
    let span = IpcCommandSpan::start("coordination_initialize_team");
    let requested_team_name = request.team_name.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let db = app_for_task.state::<DbState>();
        let state = app_for_task.state::<CoordinationState>();
        let request = normalize_initialize_request_paths(&db, request)?;
        let request = hydrate_initialize_request_role_metadata(state.inner(), request)?;
        validate_initialize_request_fields(&request)?;
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let contract_request = map_initialize_request_to_contract(&request);
        let operational_snapshots = {
            let conn = db.0.lock().map_err(|_| "db mutex poisoned".to_string())?;
            crate::coordination::operational_context::prepare_initialize_snapshots(
                &conn,
                &contract_request,
            )
            .map_err(map_coordination_error)?
        };
        let params = crate::daemon::protocol::CoordinationInitializeParams {
            request: contract_request,
            cli_commands,
            tmux_layout,
            operational_snapshots,
        };
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "team initialization requires the taurhaus daemon".to_string())?;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app_for_task.emit("coordination-step-progress", event);
        };
        let report = initialize_team_through_daemon(daemon, params, Some(&mut emit))?;
        Ok::<_, String>(map_initialize_report_from_contract(report)).ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join initialize team task: {err}"
        )))
    });
    // Only install compact hook when the pipeline succeeded — cleanup may have
    // deleted the team config, so loading it after failure throws "not found"
    // and masks the actual pipeline error.
    let result = match &result {
        Ok(report) if report.failed_step.is_none() => {
            maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result)
        }
        _ => result,
    };
    if let Ok(report) = &result {
        let provider = app.state::<ProviderState>();
        maybe_surface_terminal_after_initialize(
            &app.state::<DbState>(),
            provider.wsl_distro.clone(),
            report,
        );
    }
    emit_initialize_pipeline_result(&requested_team_name, &result);
    span.finish_result(&result);
    result
}

const COORDINATION_DAEMON_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const COORDINATION_DAEMON_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(500);
const COORDINATION_STOP_DAEMON_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(25);
/// Transient poll failures are tolerated for longer than the daemon
/// client's reconnect cooldown (5s), so a hiccup mid-initialize gets at
/// least one un-throttled reconnect attempt before the app gives up on a
/// run the daemon is still completing.
const COORDINATION_DAEMON_POLL_ERROR_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(8);

#[derive(Debug)]
enum CoordinationDaemonCallError {
    Transport(String),
    Remote(String),
}

impl CoordinationDaemonCallError {
    fn into_message(self) -> String {
        match self {
            Self::Transport(message) | Self::Remote(message) => message,
        }
    }
}

fn initialize_team_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationInitializeParams,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<crate::coordination::requests::InitializeReport, String> {
    initialize_team_through_daemon_with(
        params,
        emit,
        COORDINATION_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn initialize_team_through_daemon_with(
    params: crate::daemon::protocol::CoordinationInitializeParams,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::InitializeReport, String> {
    let accepted: crate::daemon::protocol::CoordinationInitializeAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_INITIALIZE_TEAM,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let adapter = InitializeBatchStageProgressAdapter::new(&params.request.team_name);
    let mut emitted_steps = 0;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_INITIALIZE_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationInitializeStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        for progress in status.steps.iter().skip(emitted_steps) {
            if let Some(emit) = emit.as_deref_mut() {
                emit(&adapter.event(&progress.step, progress.status, progress.message.clone()));
            }
        }
        emitted_steps = status.steps.len();
        match status.outcome {
            crate::daemon::protocol::CoordinationInitializeOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationInitializeOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationInitializeOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn add_agent_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationAddAgentParams,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<crate::coordination::requests::AddAgentReport, String> {
    add_agent_through_daemon_with(
        params,
        emit,
        COORDINATION_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn add_agent_through_daemon_with(
    params: crate::daemon::protocol::CoordinationAddAgentParams,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::AddAgentReport, String> {
    let accepted: crate::daemon::protocol::CoordinationAddAgentAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_ADD_AGENT,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut emitted_steps = 0;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_ADD_AGENT_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationAddAgentStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        emit_member_run_steps(
            &params.request.team_name,
            "add_agent",
            &status.steps,
            &mut emitted_steps,
            &mut emit,
        );
        match status.outcome {
            crate::daemon::protocol::CoordinationAddAgentOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationAddAgentOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationAddAgentOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn resume_member_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationResumeMemberParams,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<crate::coordination::requests::ResumeAgentReport, String> {
    resume_member_through_daemon_with(
        params,
        emit,
        COORDINATION_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn resume_member_through_daemon_with(
    params: crate::daemon::protocol::CoordinationResumeMemberParams,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::ResumeAgentReport, String> {
    let accepted: crate::daemon::protocol::CoordinationResumeMemberAccepted =
        serde_json::from_value(
            call(
                crate::daemon::protocol::method::COORDINATION_RESUME_MEMBER,
                serde_json::to_value(&params).map_err(|error| error.to_string())?,
            )
            .map_err(CoordinationDaemonCallError::into_message)?,
        )
        .map_err(|error| error.to_string())?;
    let mut emitted_steps = 0;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_RESUME_MEMBER_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationResumeMemberStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        emit_member_run_steps(
            &params.request.team_name,
            "resume_member",
            &status.steps,
            &mut emitted_steps,
            &mut emit,
        );
        match status.outcome {
            crate::daemon::protocol::CoordinationResumeMemberOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationResumeMemberOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationResumeMemberOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
fn stop_member_through_daemon_with(
    params: crate::daemon::protocol::CoordinationStopMemberParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::StopMemberReport, String> {
    let accepted: crate::daemon::protocol::CoordinationStopMemberAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_STOP_MEMBER,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_STOP_MEMBER_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationStopMemberStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationStopMemberOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationStopMemberOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationStopMemberOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn resume_team_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationResumeTeamParams,
    emit: Option<&mut dyn FnMut(&ResumeTeamProgressEvent)>,
) -> Result<crate::coordination::requests::ResumeTeamReport, String> {
    resume_team_through_daemon_with(
        params,
        emit,
        COORDINATION_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn resume_team_through_daemon_with(
    params: crate::daemon::protocol::CoordinationResumeTeamParams,
    mut emit: Option<&mut dyn FnMut(&ResumeTeamProgressEvent)>,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::ResumeTeamReport, String> {
    let accepted: crate::daemon::protocol::CoordinationResumeTeamAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_RESUME_TEAM,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut emitted_steps = 0;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_RESUME_TEAM_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationResumeTeamStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        for progress in status.steps.iter().skip(emitted_steps) {
            if let Some(emit) = emit.as_deref_mut() {
                emit(&resume_team_progress_event(
                    &params.request.team_name,
                    progress,
                ));
            }
        }
        emitted_steps = status.steps.len();
        match status.outcome {
            crate::daemon::protocol::CoordinationResumeTeamOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationResumeTeamOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationResumeTeamOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn reonboard_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationReonboardParams,
) -> Result<DeliveryResult, String> {
    reonboard_through_daemon_with(
        params,
        // Delivery-only interaction: keep the stop-class snappy interval so
        // a UI affordance wired to it keeps its inline feel.
        COORDINATION_STOP_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn reonboard_through_daemon_with(
    params: crate::daemon::protocol::CoordinationReonboardParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<DeliveryResult, String> {
    let accepted: crate::daemon::protocol::CoordinationReonboardAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_REONBOARD,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut first_poll_error_at: Option<std::time::Instant> = None;
    loop {
        let status_value = poll_coordination_status(
            &mut call,
            crate::daemon::protocol::method::COORDINATION_REONBOARD_STATUS,
            &accepted.run_id,
            poll_interval,
            &mut first_poll_error_at,
        )?;
        let status: crate::daemon::protocol::CoordinationReonboardStatus =
            serde_json::from_value(status_value).map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationReonboardOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationReonboardOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationReonboardOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn create_team_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationCreateTeamParams,
) -> Result<(), String> {
    create_team_through_daemon_with(
        params,
        COORDINATION_STOP_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn create_team_through_daemon_with(
    params: crate::daemon::protocol::CoordinationCreateTeamParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<(), String> {
    let accepted: crate::daemon::protocol::CoordinationCreateTeamAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_CREATE_TEAM,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut first_poll_error_at = None;
    loop {
        let status: crate::daemon::protocol::CoordinationCreateTeamStatus =
            serde_json::from_value(poll_coordination_status(
                &mut call,
                crate::daemon::protocol::method::COORDINATION_CREATE_TEAM_STATUS,
                &accepted.run_id,
                poll_interval,
                &mut first_poll_error_at,
            )?)
            .map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationCreateTeamOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationCreateTeamOutcome::Completed => return Ok(()),
            crate::daemon::protocol::CoordinationCreateTeamOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn disband_team_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationDisbandTeamParams,
) -> Result<crate::coordination::requests::DisbandTeamReport, String> {
    disband_team_through_daemon_with(
        params,
        COORDINATION_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn disband_team_through_daemon_with(
    params: crate::daemon::protocol::CoordinationDisbandTeamParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::DisbandTeamReport, String> {
    let accepted: crate::daemon::protocol::CoordinationDisbandTeamAccepted =
        serde_json::from_value(
            call(
                crate::daemon::protocol::method::COORDINATION_DISBAND_TEAM,
                serde_json::to_value(&params).map_err(|error| error.to_string())?,
            )
            .map_err(CoordinationDaemonCallError::into_message)?,
        )
        .map_err(|error| error.to_string())?;
    let mut first_poll_error_at = None;
    loop {
        let status: crate::daemon::protocol::CoordinationDisbandTeamStatus =
            serde_json::from_value(poll_coordination_status(
                &mut call,
                crate::daemon::protocol::method::COORDINATION_DISBAND_TEAM_STATUS,
                &accepted.run_id,
                poll_interval,
                &mut first_poll_error_at,
            )?)
            .map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationDisbandTeamOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationDisbandTeamOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationDisbandTeamOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn add_member_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationAddMemberParams,
) -> Result<(), String> {
    add_member_through_daemon_with(
        params,
        COORDINATION_STOP_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn add_member_through_daemon_with(
    params: crate::daemon::protocol::CoordinationAddMemberParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<(), String> {
    let accepted: crate::daemon::protocol::CoordinationAddMemberAccepted = serde_json::from_value(
        call(
            crate::daemon::protocol::method::COORDINATION_ADD_MEMBER,
            serde_json::to_value(&params).map_err(|error| error.to_string())?,
        )
        .map_err(CoordinationDaemonCallError::into_message)?,
    )
    .map_err(|error| error.to_string())?;
    let mut first_poll_error_at = None;
    loop {
        let status: crate::daemon::protocol::CoordinationAddMemberStatus =
            serde_json::from_value(poll_coordination_status(
                &mut call,
                crate::daemon::protocol::method::COORDINATION_ADD_MEMBER_STATUS,
                &accepted.run_id,
                poll_interval,
                &mut first_poll_error_at,
            )?)
            .map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationAddMemberOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationAddMemberOutcome::Completed => return Ok(()),
            crate::daemon::protocol::CoordinationAddMemberOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn remove_member_through_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    params: crate::daemon::protocol::CoordinationRemoveMemberParams,
) -> Result<crate::coordination::requests::StopMemberReport, String> {
    remove_member_through_daemon_with(
        params,
        COORDINATION_STOP_DAEMON_POLL_INTERVAL,
        |method, params| call_coordination_daemon(daemon, method, params),
    )
}

fn remove_member_through_daemon_with(
    params: crate::daemon::protocol::CoordinationRemoveMemberParams,
    poll_interval: std::time::Duration,
    mut call: impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
) -> Result<crate::coordination::requests::StopMemberReport, String> {
    let accepted: crate::daemon::protocol::CoordinationRemoveMemberAccepted =
        serde_json::from_value(
            call(
                crate::daemon::protocol::method::COORDINATION_REMOVE_MEMBER,
                serde_json::to_value(&params).map_err(|error| error.to_string())?,
            )
            .map_err(CoordinationDaemonCallError::into_message)?,
        )
        .map_err(|error| error.to_string())?;
    let mut first_poll_error_at = None;
    loop {
        let status: crate::daemon::protocol::CoordinationRemoveMemberStatus =
            serde_json::from_value(poll_coordination_status(
                &mut call,
                crate::daemon::protocol::method::COORDINATION_REMOVE_MEMBER_STATUS,
                &accepted.run_id,
                poll_interval,
                &mut first_poll_error_at,
            )?)
            .map_err(|error| error.to_string())?;
        match status.outcome {
            crate::daemon::protocol::CoordinationRemoveMemberOutcome::Running => {
                std::thread::sleep(poll_interval);
            }
            crate::daemon::protocol::CoordinationRemoveMemberOutcome::Completed { report } => {
                return Ok(report);
            }
            crate::daemon::protocol::CoordinationRemoveMemberOutcome::Failed { error } => {
                return Err(error);
            }
        }
    }
}

fn poll_coordination_status(
    call: &mut impl FnMut(
        &str,
        serde_json::Value,
    ) -> Result<serde_json::Value, CoordinationDaemonCallError>,
    method: &str,
    run_id: &str,
    poll_interval: std::time::Duration,
    first_poll_error_at: &mut Option<std::time::Instant>,
) -> Result<serde_json::Value, String> {
    loop {
        match call(method, serde_json::json!({ "run_id": run_id })) {
            Ok(value) => {
                *first_poll_error_at = None;
                return Ok(value);
            }
            Err(CoordinationDaemonCallError::Remote(message)) => return Err(message),
            Err(CoordinationDaemonCallError::Transport(message)) => {
                let since = *first_poll_error_at.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= COORDINATION_DAEMON_POLL_ERROR_BUDGET {
                    return Err(message);
                }
                std::thread::sleep(poll_interval);
            }
        }
    }
}

fn emit_member_run_steps(
    team_name: &str,
    operation: &str,
    steps: &[StepProgress],
    emitted_steps: &mut usize,
    emit: &mut Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    for progress in steps.iter().skip(*emitted_steps) {
        if let Some(emit) = emit.as_deref_mut() {
            emit(&StepProgressEvent {
                team_name: team_name.to_string(),
                operation: operation.to_string(),
                progress: progress.clone(),
                canonical_stages: canonical_stages_for_daemon_member_step(
                    operation,
                    &progress.step,
                ),
            });
        }
    }
    *emitted_steps = steps.len();
}

fn call_coordination_daemon(
    daemon: &crate::provider::daemon_client::DaemonProvider,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, CoordinationDaemonCallError> {
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return Err(CoordinationDaemonCallError::Transport(
            "daemon is not connected".to_string(),
        ));
    }
    let request = crate::daemon::protocol::DaemonRequest::new(
        format!("coord-run-{}", uuid::Uuid::new_v4().simple()),
        method,
        params,
    );
    let response = daemon
        .send_status_request_within(&request, COORDINATION_DAEMON_REQUEST_TIMEOUT)
        .map_err(|error| CoordinationDaemonCallError::Transport(error.to_string()))?;
    if let Some(error) = response.error {
        return Err(CoordinationDaemonCallError::Remote(error.message));
    }
    response.result.ok_or_else(|| {
        CoordinationDaemonCallError::Remote(format!("daemon method '{method}' returned no result"))
    })
}

#[tauri::command]
pub async fn coordination_add_agent(
    app: AppHandle,
    request: AddAgentRequest,
) -> IpcResult<AddAgentReport> {
    let span = IpcCommandSpan::start("coordination_add_agent");
    let requested_team_name = request.team_name.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let db = app_for_task.state::<DbState>();
        let state = app_for_task.state::<CoordinationState>();
        let request = normalize_add_agent_request_path(&db, request)?;
        let request = hydrate_add_agent_request_role_metadata(state.inner(), request)?;
        validate_add_agent_request_fields(&request)?;
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let contract_request = map_add_agent_request_to_contract(&request);
        let (operational_snapshot, task_state_changed_at) = prepare_member_operation_snapshot(
            state.inner(),
            &db,
            &contract_request.team_name,
            &contract_request.agent.name,
            &contract_request.agent.project_id,
        )?;
        let params = crate::daemon::protocol::CoordinationAddAgentParams {
            request: contract_request,
            cli_commands,
            tmux_layout,
            operational_snapshot: Some(operational_snapshot),
            task_state_changed_at,
        };
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "adding a team member requires the taurhaus daemon".to_string())?;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app_for_task.emit("coordination-step-progress", event);
        };
        add_agent_through_daemon(daemon, params, Some(&mut emit))
            .map(map_add_agent_report_from_contract)
            .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join add-agent task: {err}"
        )))
    });
    let result = match &result {
        Ok(report) if report.failed_step.is_none() => {
            maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result)
        }
        _ => result,
    };
    emit_add_agent_pipeline_result(&requested_team_name, &result);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_resume_member(
    app: AppHandle,
    request: ResumeMemberRequest,
) -> IpcResult<ResumeAgentReport> {
    let span = IpcCommandSpan::start("coordination_resume_member");
    let requested_team_name = request.team_name.clone();
    let requested_member_name = request.member_name.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &request.team_name)?;
        validate_non_empty("member_name", &request.member_name)?;
        let db = app_for_task.state::<DbState>();
        let state = app_for_task.state::<CoordinationState>();
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let contract_request = map_resume_member_request_to_contract(&request);
        let config = TeamConfigStore::load(state.teams_dir(), &contract_request.team_name)
            .map_err(map_coordination_error)?;
        let project_path = config
            .members
            .iter()
            .find(|member| member.name == contract_request.member_name)
            .map(|member| member.project_path.display().to_string())
            .ok_or_else(|| {
                format!(
                    "member '{}' not found in team '{}'",
                    contract_request.member_name, contract_request.team_name
                )
            })?;
        let (operational_snapshot, task_state_changed_at) = prepare_member_operation_snapshot(
            state.inner(),
            &db,
            &contract_request.team_name,
            &contract_request.member_name,
            &project_path,
        )?;
        let params = crate::daemon::protocol::CoordinationResumeMemberParams {
            request: contract_request,
            cli_commands,
            tmux_layout,
            operational_snapshot: Some(operational_snapshot),
            task_state_changed_at,
        };
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "resuming a team member requires the taurhaus daemon".to_string())?;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app_for_task.emit("coordination-step-progress", event);
        };
        resume_member_through_daemon(daemon, params, Some(&mut emit))
            .map(map_resume_agent_report_from_contract)
            .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join resume-member task: {err}"
        )))
    });
    let result = maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result);
    emit_resume_member_pipeline_result(&requested_team_name, &requested_member_name, &result);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_resume_team(
    app: AppHandle,
    request: ResumeTeamRequest,
) -> IpcResult<ResumeTeamReport> {
    let span = IpcCommandSpan::start("coordination_resume_team");
    let requested_team_name = request.team_name.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &request.team_name)?;
        let db = app_for_task.state::<DbState>();
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let params = crate::daemon::protocol::CoordinationResumeTeamParams {
            request: map_resume_team_request_to_contract(&request),
            cli_commands,
            tmux_layout,
        };
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "resuming a team requires the taurhaus daemon".to_string())?;
        let mut emit = |event: &ResumeTeamProgressEvent| {
            let _ = app_for_task.emit("coordination-resume-team-progress", event);
        };
        resume_team_through_daemon(daemon, params, Some(&mut emit))
            .map(map_resume_team_report_from_contract)
            .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join resume-team task: {err}"
        )))
    });
    let result = maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result);
    if let Ok(report) = &result {
        let provider = app.state::<ProviderState>();
        maybe_surface_terminal_after_resume_team(
            &app.state::<DbState>(),
            provider.wsl_distro.clone(),
            report,
        );
    }
    emit_resume_team_pipeline_result(&requested_team_name, &result);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_reonboard(
    app: AppHandle,
    request: ReonboardRequest,
) -> IpcResult<DeliveryResult> {
    let span = IpcCommandSpan::start("coordination_reonboard");
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &request.team_name)?;
        validate_non_empty("member_name", &request.member_name)?;
        let db = app_for_task.state::<DbState>();
        let state = app_for_task.state::<CoordinationState>();
        let config = TeamConfigStore::load(state.teams_dir(), &request.team_name)
            .map_err(map_coordination_error)?;
        let project_path = config
            .members
            .iter()
            .find(|member| member.name == request.member_name)
            .map(|member| member.project_path.display().to_string())
            .ok_or_else(|| {
                map_coordination_error(CoordinationError::NotFound(format!(
                    "member '{}' not found in team '{}'",
                    request.member_name, request.team_name
                )))
            })?;
        let (operational_snapshot, task_state_changed_at) = prepare_member_operation_snapshot(
            state.inner(),
            &db,
            &request.team_name,
            &request.member_name,
            &project_path,
        )?;
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let params = crate::daemon::protocol::CoordinationReonboardParams {
            request: map_reonboard_request_to_contract(&request),
            cli_commands,
            tmux_layout,
            operational_snapshot: Some(operational_snapshot),
            task_state_changed_at,
        };
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider.daemon.as_ref().ok_or_else(|| {
            "re-onboarding a team member requires the taurhaus daemon".to_string()
        })?;
        reonboard_through_daemon(daemon, params).ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join reonboard task: {err}"
        )))
    });
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_get_live_team_status(
    app: AppHandle,
    team_name: String,
) -> IpcResult<LiveTeamStatus> {
    let span = IpcCommandSpan::start("coordination_get_live_team_status");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<CoordinationState>();
        let provider = app.state::<ProviderState>();
        coordination_get_live_team_status_impl(state.inner(), Some(provider.inner()), team_name)
            .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join live team status task: {err}"
        )))
    });
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_create_team(app: AppHandle, team_name: String) -> IpcResult<()> {
    let span = IpcCommandSpan::start("coordination_create_team");
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &team_name)?;
        let provider = app.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "creating a team requires the taurhaus daemon".to_string())?;
        create_team_through_daemon(
            daemon,
            crate::daemon::protocol::CoordinationCreateTeamParams {
                request: crate::coordination::requests::CreateTeamRequest { team_name },
            },
        )
        .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join create-team task: {err}"
        )))
    });
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_disband_team(
    app: AppHandle,
    team_name: String,
) -> IpcResult<DisbandTeamResponse> {
    let span = IpcCommandSpan::start("coordination_disband_team");
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &team_name)?;
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "disbanding a team requires the taurhaus daemon".to_string())?;
        disband_team_through_daemon(
            daemon,
            crate::daemon::protocol::CoordinationDisbandTeamParams {
                request: crate::coordination::requests::DisbandTeamRequest { team_name },
            },
        )
        .map(map_disband_team_report_from_contract)
        .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join disband-team task: {err}"
        )))
    });
    if result.is_ok() {
        reconcile_global_harness_hooks(&app);
    }
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_add_member(
    app: AppHandle,
    team_name: String,
    member_name: String,
    backend_kind: String,
    project_path: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("coordination_add_member");
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &team_name)?;
        validate_non_empty("member_name", &member_name)?;
        validate_non_empty("backend_kind", &backend_kind)?;
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "adding a team member requires the taurhaus daemon".to_string())?;
        add_member_through_daemon(
            daemon,
            crate::daemon::protocol::CoordinationAddMemberParams {
                request: crate::coordination::requests::AddMemberRequest {
                    team_name,
                    member_name,
                    backend_kind,
                    project_path,
                },
            },
        )
        .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join add-member task: {err}"
        )))
    });
    // The member this persists can be the first grok one on the host, and
    // grok's hook lives in its home rather than in the team.
    if result.is_ok() {
        reconcile_global_harness_hooks(&app);
    }
    span.finish_result(&result);
    result
}

#[tauri::command]
pub async fn coordination_remove_member(
    app: AppHandle,
    team_name: String,
    member_name: String,
) -> IpcResult<RemoveAgentReport> {
    let span = IpcCommandSpan::start("coordination_remove_member");
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_non_empty("team_name", &team_name)?;
        validate_non_empty("member_name", &member_name)?;
        let provider = app_for_task.state::<ProviderState>();
        let daemon = provider
            .daemon
            .as_ref()
            .ok_or_else(|| "removing a team member requires the taurhaus daemon".to_string())?;
        remove_member_through_daemon(
            daemon,
            crate::daemon::protocol::CoordinationRemoveMemberParams {
                request: crate::coordination::requests::RemoveMemberRequest {
                    team_name,
                    member_name,
                },
            },
        )
        .map(map_stop_member_report_from_contract)
        .ipc()
    })
    .await
    .unwrap_or_else(|err| {
        Err(IpcError::internal(format!(
            "failed to join remove-member task: {err}"
        )))
    });
    if result.is_ok() {
        reconcile_global_harness_hooks(&app);
    }
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn coordination_list_teams(
    state: State<'_, CoordinationState>,
) -> IpcResult<TeamDiscoveryResponse> {
    let span = IpcCommandSpan::start("coordination_list_teams");
    let result = coordination_list_teams_impl(state.inner()).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn coordination_get_team_status(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> IpcResult<TeamStatus> {
    let span = IpcCommandSpan::start("coordination_get_team_status");
    let result = coordination_get_team_status_impl(state.inner(), team_name).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn coordination_preflight_check(request: InitializeTeamRequest) -> IpcResult<PreflightReport> {
    let span = IpcCommandSpan::start("coordination_preflight_check");
    let result = coordination_preflight_check_impl(request).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn coordination_get_feature_availability() -> IpcResult<FeatureAvailabilityReport> {
    let span = IpcCommandSpan::start("coordination_get_feature_availability");
    let result = Ok(coordination_get_feature_availability_impl());
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn coordination_get_project_mesh_snapshot(
    state: State<'_, CoordinationState>,
    project_path: String,
) -> IpcResult<ProjectMeshSnapshotResponse> {
    let span = IpcCommandSpan::start("coordination_get_project_mesh_snapshot");
    let result = coordination_get_project_mesh_snapshot_impl(state.inner(), project_path).ipc();
    span.finish_result(&result);
    result
}

fn maybe_ensure_compact_hooks_for_team<T>(
    app: &AppHandle,
    team_name: &str,
    result: IpcResult<T>,
) -> IpcResult<T> {
    result.as_ref().map_err(Clone::clone)?;

    let state = app.state::<CoordinationState>();
    let teams_dir = state.teams_dir();
    let has_claude = team_has_managed_claude_member(teams_dir, team_name)
        .map_err(|err| IpcError::internal(sanitize_error(&err.to_string())))?;
    if has_claude {
        let current_exe = std::env::current_exe().map_err(|err| {
            IpcError::internal(format!("failed to resolve taurhaus executable: {err}"))
        })?;
        let _ = ensure_compact_hook_installed(teams_dir, &current_exe)
            .map_err(|err| IpcError::internal(sanitize_error(&err.to_string())))?;
    }

    reconcile_global_harness_hooks(app);

    result
}

/// Keep the harness hooks that live outside a team in step with the roster.
///
/// grok's hook is one file in its home, so the roster — not this team — decides
/// whether it belongs there. Every mutation that can add the first grok member
/// or remove the last one calls this; a failure is logged and the current
/// installation is left alone rather than failing the mutation the user asked
/// for.
fn reconcile_global_harness_hooks(app: &AppHandle) {
    let (Some(state), Some(db)) = (
        app.try_state::<CoordinationState>(),
        app.try_state::<DbState>(),
    ) else {
        return;
    };
    let terminal = crate::commands::terminal_settings::load_terminal_settings(&db);
    if let Err(error) = crate::commands::terminal_settings::reconcile_grok_hooks_for_roster(
        state.teams_dir(),
        terminal.harness.grok_hooks,
    ) {
        tracing::warn!(error = %error, "Grok compaction hook reconciliation failed after a roster change");
        let mut fields = Map::new();
        fields.insert(
            "error.message".to_string(),
            Value::String(sanitize_error(&error.to_string())),
        );
        taurhaus_lib::logging::emit_global(
            "warn",
            "coordination",
            "compaction.grok_hook.reconcile_failed",
            Some("Grok compaction hooks remain unreconciled".to_string()),
            fields,
        );
    }
}

fn emit_initialize_pipeline_result(team_name: &str, result: &IpcResult<InitializeReport>) {
    match result {
        Ok(report) => {
            let mut fields = base_pipeline_fields("initialize_team", &report.team_name, None);
            fields.insert(
                "succeeded_step_count".to_string(),
                Value::Number(serde_json::Number::from(report.succeeded_steps.len() as u64)),
            );
            fields.insert("retryable".to_string(), Value::Bool(report.retryable));
            if let Some(failed_step) = report.failed_step.as_ref() {
                fields.insert(
                    "failed_step".to_string(),
                    Value::String(failed_step.clone()),
                );
                fields.insert("message".to_string(), Value::String(report.message.clone()));
                emit_coordination_pipeline_event("warn", "coordination.pipeline.failed", fields);
            } else {
                fields.insert("message".to_string(), Value::String(report.message.clone()));
                emit_coordination_pipeline_event("info", "coordination.pipeline.completed", fields);
            }
        }
        Err(error) => emit_coordination_pipeline_failure("initialize_team", team_name, None, error),
    }
}

fn emit_add_agent_pipeline_result(team_name: &str, result: &IpcResult<AddAgentReport>) {
    match result {
        Ok(report) => {
            let mut fields = base_pipeline_fields(
                "add_agent",
                &report.team_name,
                Some(report.member_name.as_str()),
            );
            fields.insert(
                "succeeded_step_count".to_string(),
                Value::Number(serde_json::Number::from(report.succeeded_steps.len() as u64)),
            );
            fields.insert("retryable".to_string(), Value::Bool(report.retryable));
            fields.insert("message".to_string(), Value::String(report.message.clone()));
            if let Some(failed_step) = report.failed_step.as_ref() {
                fields.insert(
                    "failed_step".to_string(),
                    Value::String(failed_step.clone()),
                );
                emit_coordination_pipeline_event("warn", "coordination.pipeline.failed", fields);
            } else {
                emit_coordination_pipeline_event("info", "coordination.pipeline.completed", fields);
            }
        }
        Err(error) => emit_coordination_pipeline_failure("add_agent", team_name, None, error),
    }
}

fn emit_resume_member_pipeline_result(
    team_name: &str,
    member_name: &str,
    result: &IpcResult<ResumeAgentReport>,
) {
    match result {
        Ok(report) => {
            let mut fields = base_pipeline_fields(
                "resume_member",
                &report.team_name,
                Some(report.member_name.as_str()),
            );
            fields.insert("resumed".to_string(), Value::Bool(report.resumed));
            fields.insert(
                "succeeded_step_count".to_string(),
                Value::Number(serde_json::Number::from(report.succeeded_steps.len() as u64)),
            );
            fields.insert(
                "warning_count".to_string(),
                Value::Number(serde_json::Number::from(report.warnings.len() as u64)),
            );
            fields.insert("retryable".to_string(), Value::Bool(report.retryable));
            fields.insert("reused_pane".to_string(), Value::Bool(report.reused_pane));
            if let Some(pane_id) = report.pane_id.as_ref() {
                fields.insert("pane_id".to_string(), Value::String(pane_id.clone()));
            }
            fields.insert("message".to_string(), Value::String(report.message.clone()));
            if let Some(failed_step) = report.failed_step.as_ref() {
                fields.insert(
                    "failed_step".to_string(),
                    Value::String(failed_step.clone()),
                );
                emit_coordination_pipeline_event("warn", "coordination.pipeline.failed", fields);
            } else {
                emit_coordination_pipeline_event("info", "coordination.pipeline.completed", fields);
            }
        }
        Err(error) => {
            emit_coordination_pipeline_failure("resume_member", team_name, Some(member_name), error)
        }
    }
}

fn emit_resume_team_pipeline_result(team_name: &str, result: &IpcResult<ResumeTeamReport>) {
    match result {
        Ok(report) => {
            let mut fields = base_pipeline_fields("resume_team", &report.team_name, None);
            fields.insert("resumed".to_string(), Value::Bool(report.resumed));
            fields.insert(
                "total_members".to_string(),
                Value::Number(serde_json::Number::from(report.total_members as u64)),
            );
            fields.insert(
                "resumed_member_count".to_string(),
                Value::Number(serde_json::Number::from(report.resumed_members.len() as u64)),
            );
            fields.insert(
                "failed_member_count".to_string(),
                Value::Number(serde_json::Number::from(report.failed_members.len() as u64)),
            );
            fields.insert(
                "warning_count".to_string(),
                Value::Number(serde_json::Number::from(report.warnings.len() as u64)),
            );
            fields.insert(
                "started_team_daemon".to_string(),
                Value::Bool(report.started_team_daemon),
            );
            if let Some(warning) = report.team_daemon_warning.as_ref() {
                fields.insert(
                    "team_daemon_warning".to_string(),
                    Value::String(warning.clone()),
                );
            }
            let event_name = if report.failed_members.is_empty() {
                "coordination.pipeline.completed"
            } else {
                "coordination.pipeline.failed"
            };
            let level = if report.failed_members.is_empty() {
                "info"
            } else {
                "warn"
            };
            emit_coordination_pipeline_event(level, event_name, fields);
        }
        Err(error) => emit_coordination_pipeline_failure("resume_team", team_name, None, error),
    }
}

fn base_pipeline_fields(
    operation: &str,
    team_name: &str,
    member_name: Option<&str>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    fields.insert(
        "team_name".to_string(),
        Value::String(team_name.to_string()),
    );
    if let Some(member_name) = member_name {
        fields.insert(
            "member_name".to_string(),
            Value::String(member_name.to_string()),
        );
    }
    fields
}

fn emit_coordination_pipeline_failure(
    operation: &str,
    team_name: &str,
    member_name: Option<&str>,
    error: &IpcError,
) {
    let mut fields = base_pipeline_fields(operation, team_name, member_name);
    fields.insert(
        "error.code".to_string(),
        Value::String(ipc_error_code_name(error).to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error.message.clone()),
    );
    fields.insert("retryable".to_string(), Value::Bool(error.retryable));
    if let Some(command) = error.command.as_ref() {
        fields.insert("command".to_string(), Value::String(command.clone()));
    }
    emit_coordination_pipeline_event("warn", "coordination.pipeline.failed", fields);
}

fn emit_coordination_pipeline_event(level: &str, event: &str, fields: Map<String, Value>) {
    taurhaus_lib::logging::emit_global(
        level,
        "backend",
        event,
        Some("Coordination pipeline lifecycle event".to_string()),
        fields,
    );
}

fn ipc_error_code_name(error: &IpcError) -> &'static str {
    match error.code {
        crate::errors::IpcErrorCode::ValidationError => "VALIDATION_ERROR",
        crate::errors::IpcErrorCode::NotFound => "NOT_FOUND",
        crate::errors::IpcErrorCode::Conflict => "CONFLICT",
        crate::errors::IpcErrorCode::Unavailable => "UNAVAILABLE",
        crate::errors::IpcErrorCode::InternalError => "INTERNAL_ERROR",
    }
}

fn coordination_list_teams_impl(
    state: &CoordinationState,
) -> Result<TeamDiscoveryResponse, String> {
    state
        .with_orchestrator(|orchestrator| orchestrator.discover_teams())
        .map_err(map_coordination_error)
        .map(|discovery| TeamDiscoveryResponse {
            teams: discovery
                .teams
                .into_iter()
                .map(|team| TeamSummary {
                    team_name: team.team_name,
                    lead_project_path: team
                        .lead_project_path
                        .map(|path| path.display().to_string()),
                })
                .collect(),
            warnings: discovery.warnings,
        })
}

fn coordination_get_team_status_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<TeamStatus, String> {
    validate_non_empty("team_name", &team_name)?;
    state
        .with_orchestrator(|orchestrator| orchestrator.get_team_status(&team_name))
        .map_err(map_coordination_error)
        .map(|status| TeamStatus {
            team_name: status.config.name,
            members: status
                .config
                .members
                .into_iter()
                .map(|member| member.name)
                .collect(),
        })
}

fn coordination_get_project_mesh_snapshot_impl(
    state: &CoordinationState,
    project_path: String,
) -> Result<ProjectMeshSnapshotResponse, String> {
    live_status::coordination_get_project_mesh_snapshot_impl(state, project_path)
}

fn coordination_preflight_check_impl(
    request: InitializeTeamRequest,
) -> Result<PreflightReport, String> {
    let preflight_agents = validate_and_collect_preflight_agents(request)?;
    let report = preflight_check(&preflight_agents);
    Ok(map_preflight_report(report))
}

fn coordination_get_feature_availability_impl() -> FeatureAvailabilityReport {
    map_feature_availability_report(availability_check())
}

#[cfg(test)]
fn coordination_preflight_check_with_lookup<L: BinaryLookup + ?Sized>(
    request: InitializeTeamRequest,
    lookup: &L,
) -> Result<PreflightReport, String> {
    let preflight_agents = validate_and_collect_preflight_agents(request)?;
    let report = preflight_check_with_lookup(&preflight_agents, lookup);
    Ok(map_preflight_report(report))
}

#[cfg(test)]
fn coordination_get_feature_availability_with_lookup<L: BinaryLookup + ?Sized>(
    lookup: &L,
) -> FeatureAvailabilityReport {
    map_feature_availability_report(availability_check_with_lookup(lookup))
}

#[cfg(test)]
pub(crate) fn coordination_get_live_team_status_for_tests(
    state: &CoordinationState,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    live_status::coordination_get_live_team_status_for_tests(state, team_name)
}

#[cfg(test)]
pub(crate) fn coordination_get_project_mesh_snapshot_with_lookup<L: BinaryLookup + ?Sized>(
    state: &CoordinationState,
    project_path: String,
    lookup: &L,
) -> Result<ProjectMeshSnapshotResponse, String> {
    live_status::coordination_get_project_mesh_snapshot_with_lookup(state, project_path, lookup)
}

#[cfg(test)]
fn derive_cross_project_status(
    lead_project_path: &Path,
    member_project_path: &Path,
) -> live_status::CrossProjectStatus {
    live_status::derive_cross_project_status(lead_project_path, member_project_path)
}

#[cfg(test)]
#[path = "coordination/tests.rs"]
mod tests;
