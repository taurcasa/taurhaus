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
#[path = "coordination/state_sync.rs"]
mod state_sync;

pub use crate::commands::coordination_types::*;
use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::coordination::backend::bridged::{availability_check, preflight_check};
#[cfg(test)]
use crate::coordination::backend::bridged::{
    availability_check_with_lookup, preflight_check_with_lookup, BinaryLookup,
};
use crate::coordination::compact_hook::{
    ensure_compact_hook_installed, team_has_managed_claude_member, team_has_managed_codex_member,
};
use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{DeliveryRequest, DeliveryResult, OperatorNoticeDelivery};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::ActiveProjectTeamStore;
use crate::errors::{sanitize_error, CommandResultExt, IpcError, IpcResult};
use crate::models::CliCommandSettings;
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
use state_sync::*;
#[cfg(test)]
use taurhaus_lib::ProviderState;

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
        let codex_bypass_hook_trust = reconcile_codex_before_managed_launch(
            &app_for_task,
            &db,
            matches!(
                CliTool::from_alias(&request.lead.cli_tool),
                Ok(CliTool::Codex)
            ) || request
                .agents
                .iter()
                .any(|agent| matches!(CliTool::from_alias(&agent.cli_tool), Ok(CliTool::Codex))),
        );
        let (mut cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app_for_task.emit("coordination-step-progress", event);
        };
        coordination_initialize_team_internal(
            state.inner(),
            Some(&db),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
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

#[tauri::command]
pub fn coordination_add_agent(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: AddAgentRequest,
) -> IpcResult<AddAgentReport> {
    let span = IpcCommandSpan::start("coordination_add_agent");
    let requested_team_name = request.team_name.clone();
    let result = {
        let request = normalize_add_agent_request_path(&db, request)?;
        let codex_bypass_hook_trust = reconcile_codex_before_managed_launch(
            &app,
            &db,
            matches!(
                CliTool::from_alias(&request.agent.cli_tool),
                Ok(CliTool::Codex)
            ),
        );
        let (mut cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app.emit("coordination-step-progress", event);
        };
        coordination_add_agent_internal(
            state.inner(),
            Some(&db),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
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
pub fn coordination_resume_member(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: ResumeMemberRequest,
) -> IpcResult<ResumeAgentReport> {
    let span = IpcCommandSpan::start("coordination_resume_member");
    let requested_team_name = request.team_name.clone();
    let requested_member_name = request.member_name.clone();
    let result = {
        let has_codex = team_has_managed_codex_member(state.teams_dir(), &requested_team_name)
            .map_err(|error| IpcError::internal(sanitize_error(&error.to_string())))?;
        let codex_bypass_hook_trust = reconcile_codex_before_managed_launch(&app, &db, has_codex);
        let (mut cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
        let mut emit = |event: &StepProgressEvent| {
            let _ = app.emit("coordination-step-progress", event);
        };
        coordination_resume_member_internal(
            state.inner(),
            Some(&db),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
    let result = maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result);
    emit_resume_member_pipeline_result(&requested_team_name, &requested_member_name, &result);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_resume_team(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: ResumeTeamRequest,
) -> IpcResult<ResumeTeamReport> {
    let span = IpcCommandSpan::start("coordination_resume_team");
    let requested_team_name = request.team_name.clone();
    let result = {
        let has_codex = team_has_managed_codex_member(state.teams_dir(), &requested_team_name)
            .map_err(|error| IpcError::internal(sanitize_error(&error.to_string())))?;
        let codex_bypass_hook_trust = reconcile_codex_before_managed_launch(&app, &db, has_codex);
        let (mut cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
        let mut emit = |event: &ResumeTeamProgressEvent| {
            emit_resume_team_progress_log_event(event);
            let _ = app.emit("coordination-resume-team-progress", event);
        };
        coordination_resume_team_internal(
            state.inner(),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
    let result = maybe_ensure_compact_hooks_for_team(&app, &requested_team_name, result);
    if let Ok(report) = &result {
        let provider = app.state::<ProviderState>();
        maybe_surface_terminal_after_resume_team(&db, provider.wsl_distro.clone(), report);
    }
    emit_resume_team_pipeline_result(&requested_team_name, &result);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_reonboard(
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: ReonboardRequest,
) -> IpcResult<DeliveryResult> {
    let span = IpcCommandSpan::start("coordination_reonboard");
    let result = coordination_reonboard_impl(Some(&db), state.inner(), request).ipc();
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
pub fn coordination_create_team(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("coordination_create_team");
    let result = coordination_create_team_impl(state.inner(), team_name).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_disband_team(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> IpcResult<DisbandTeamResponse> {
    let span = IpcCommandSpan::start("coordination_disband_team");
    let result = coordination_disband_team_impl(state.inner(), team_name).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_add_member(
    state: State<'_, CoordinationState>,
    team_name: String,
    member_name: String,
    backend_kind: String,
    project_path: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("coordination_add_member");
    let result = coordination_add_member_impl(
        state.inner(),
        team_name,
        member_name,
        backend_kind,
        project_path,
    )
    .ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_remove_member(
    state: State<'_, CoordinationState>,
    team_name: String,
    member_name: String,
) -> IpcResult<RemoveAgentReport> {
    let span = IpcCommandSpan::start("coordination_remove_member");
    let result = coordination_remove_member_impl(state.inner(), team_name, member_name).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_list_teams(
    state: State<'_, CoordinationState>,
) -> IpcResult<TeamDiscoveryResponse> {
    let span = IpcCommandSpan::start("coordination_list_teams");
    let result = coordination_list_teams_impl(state.inner()).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_get_team_status(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> IpcResult<TeamStatus> {
    let span = IpcCommandSpan::start("coordination_get_team_status");
    let result = coordination_get_team_status_impl(state.inner(), team_name).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_preflight_check(request: InitializeTeamRequest) -> IpcResult<PreflightReport> {
    let span = IpcCommandSpan::start("coordination_preflight_check");
    let result = coordination_preflight_check_impl(request).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_get_feature_availability() -> IpcResult<FeatureAvailabilityReport> {
    let span = IpcCommandSpan::start("coordination_get_feature_availability");
    let result = Ok(coordination_get_feature_availability_impl());
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_get_project_mesh_snapshot(
    state: State<'_, CoordinationState>,
    project_path: String,
) -> IpcResult<ProjectMeshSnapshotResponse> {
    let span = IpcCommandSpan::start("coordination_get_project_mesh_snapshot");
    let result = coordination_get_project_mesh_snapshot_impl(state.inner(), project_path).ipc();
    span.finish_result(&result);
    result
}

fn coordination_initialize_team_internal(
    state: &CoordinationState,
    db: Option<&DbState>,
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<InitializeReport, String> {
    let request = hydrate_initialize_request_role_metadata(state, request)?;
    validate_initialize_request_fields(&request)?;
    let contract_request = map_initialize_request_to_contract(&request);
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.initialize_team_with_cli_commands_and_layout_and_progress(
                &contract_request,
                cli_commands,
                tmux_layout,
                Some(&mut |step, status, message| {
                    let adapter = InitializeBatchStageProgressAdapter::new(&request.team_name);
                    adapter.emit(step, status, message, &mut emit);
                }),
            )
        })
        .map(map_initialize_report_from_contract)
        .map_err(map_coordination_error)?;
    let team_was_created = report
        .succeeded_steps
        .iter()
        .any(|step| step == "create_team");
    let initialize_succeeded = report.failed_step.is_none();
    if team_was_created && initialize_succeeded {
        if let Some(db) = db {
            sync_team_snapshots_after_change(state, db, &report.team_name)
                .map_err(map_coordination_error)?;
        }
        sync_active_team_projects_after_change(state, &report.team_name)
            .map_err(map_coordination_error)?;
    }
    Ok(report)
}

fn coordination_add_agent_internal(
    state: &CoordinationState,
    db: Option<&DbState>,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<AddAgentReport, String> {
    let request = hydrate_add_agent_request_role_metadata(state, request)?;
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("agent.name", &request.agent.name)?;
    validate_non_empty("agent.project_id", &request.agent.project_id)?;
    validate_non_empty("agent.cli_tool", &request.agent.cli_tool)?;
    let contract_request = map_add_agent_request_to_contract(&request);
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.add_agent_to_team_with_cli_commands_and_layout(
                &contract_request,
                cli_commands,
                tmux_layout,
            )
        })
        .map(map_add_agent_report_from_contract)
        .map_err(map_coordination_error)?;
    // Only sync snapshots on success — cleanup_add_agent_failure may have
    // removed the member, so syncing after failure throws "not found" and
    // masks the actual pipeline error.
    if report.failed_step.is_none() {
        if let Some(db) = db {
            sync_member_snapshot_after_change(state, db, &report.team_name, &report.member_name)
                .map_err(map_coordination_error)?;
        }
        sync_active_team_projects_after_change(state, &report.team_name)
            .map_err(map_coordination_error)?;
    }
    emit_progress_events(add_agent_progress_events(&report), emit);
    Ok(report)
}

fn coordination_resume_member_internal(
    state: &CoordinationState,
    db: Option<&DbState>,
    request: ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<ResumeAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;
    let contract_request = map_resume_member_request_to_contract(&request);
    let mut emit_resume_progress = |_: &str,
                                    _: usize,
                                    _: usize,
                                    stage: MemberActivationStage,
                                    status: StepStatus,
                                    message: Option<String>| {
        let event =
            resume_member_progress_event_for_stage(&request.team_name, stage, status, message);
        emit_progress_event(event, &mut emit);
    };
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.resume_member_with_cli_commands_and_layout_and_progress(
                &contract_request,
                cli_commands,
                tmux_layout,
                1,
                1,
                Some(&mut emit_resume_progress),
            )
        })
        .map(map_resume_agent_report_from_contract)
        .map_err(map_coordination_error)?;
    if let Some(db) = db {
        sync_member_snapshot_after_change(state, db, &report.team_name, &report.member_name)
            .map_err(map_coordination_error)?;
    }
    sync_active_team_projects_after_change(state, &report.team_name)
        .map_err(map_coordination_error)?;
    Ok(report)
}

fn coordination_resume_team_internal(
    state: &CoordinationState,
    request: ResumeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&ResumeTeamProgressEvent)>,
) -> Result<ResumeTeamReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    let contract_request = map_resume_team_request_to_contract(&request);
    let mut emit_resume_progress = |member_name: &str,
                                    member_index: usize,
                                    member_count: usize,
                                    stage: MemberActivationStage,
                                    status: StepStatus,
                                    message: Option<String>| {
        if let Some(emit) = emit.as_deref_mut() {
            emit(&resume_team_progress_event_for_stage(
                &request.team_name,
                member_name,
                member_index,
                member_count,
                stage,
                status,
                message,
            ));
        }
    };
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.resume_team_with_cli_commands_and_layout_and_progress(
                &contract_request,
                cli_commands,
                tmux_layout,
                Some(&mut emit_resume_progress),
            )
        })
        .map(map_resume_team_report_from_contract)
        .map_err(map_coordination_error)?;
    sync_active_team_projects_after_change(state, &report.team_name)
        .map_err(map_coordination_error)?;
    Ok(report)
}

fn coordination_reonboard_impl(
    db: Option<&DbState>,
    state: &CoordinationState,
    request: ReonboardRequest,
) -> Result<DeliveryResult, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;

    let result = state
        .with_orchestrator(|orchestrator| {
            let team = orchestrator.get_team_status(&request.team_name)?;
            if !team
                .config
                .members
                .iter()
                .any(|member| member.name == request.member_name)
            {
                return Err(CoordinationError::NotFound(format!(
                    "member '{}' not found in team '{}'",
                    request.member_name, request.team_name
                )));
            }

            let lead_name = team
                .config
                .members
                .iter()
                .find(|member| member.role == MemberRole::Lead)
                .map(|member| member.name.clone())
                .unwrap_or_else(|| "team-lead".to_string());
            let member = team
                .config
                .members
                .iter()
                .find(|member| member.name == request.member_name)
                .ok_or_else(|| {
                    CoordinationError::NotFound(format!(
                        "member '{}' not found in team '{}'",
                        request.member_name, request.team_name
                    ))
                })?;
            let message = DeliveryRenderer::render_onboarding(
                &request.team_name,
                &request.member_name,
                &lead_name,
                RoleContext {
                    role_id: member.role_id.as_deref(),
                    communication_style: member.communication_style.as_deref(),
                    instructions: member.instructions.as_deref(),
                    behavioral_contract: member.behavioral_contract.as_ref(),
                    quality_gates: member.quality_gates.as_deref(),
                    handoff_expectations: member.handoff_expectations.as_deref(),
                    definition_of_done: member.definition_of_done.as_deref(),
                    capabilities: member.capabilities.as_deref(),
                },
            );

            orchestrator.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: request.member_name.clone(),
                team_name: request.team_name.clone(),
                message,
                sender_name: Some(lead_name),
                operational_context: None,
            }))
        })
        .map_err(map_coordination_error)?;
    if let Some(db) = db {
        sync_member_snapshot_after_change(state, db, &request.team_name, &request.member_name)
            .map_err(map_coordination_error)?;
    }
    Ok(result)
}

fn reconcile_codex_before_managed_launch(
    app: &AppHandle,
    db: &DbState,
    has_managed_codex: bool,
) -> bool {
    let mode = crate::commands::terminal_settings::load_terminal_settings(db)
        .harness
        .codex_compaction;
    let mut hook_ready = match crate::commands::terminal_settings::reconcile_codex_compaction(
        mode,
        has_managed_codex,
    ) {
        Ok(_) => {
            mode == crate::models::CodexCompactionMode::Hooks
                && has_managed_codex
                && crate::coordination::compact_hook::codex_compact_hook_is_installed()
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Codex compact hook reconciliation degraded; continuing managed launch"
            );
            let mut fields = Map::new();
            fields.insert("tool".to_string(), Value::String("codex".to_string()));
            fields.insert(
                "error.message".to_string(),
                Value::String(sanitize_error(&error.to_string())),
            );
            taurhaus_lib::logging::emit_global(
                "warn",
                "coordination",
                "compaction.codex_hook.degraded",
                Some("Managed launch continued without Codex hook trust bypass".to_string()),
                fields,
            );
            false
        }
    };
    if let Err(error) = crate::startup::compaction::reconcile_compaction_runtime(
        app,
        mode,
        "managed_launch_hook_reconciled",
    ) {
        tracing::warn!(
            error = %error,
            "managed launch continued after compaction runtime reconciliation degraded"
        );
        let mut fields = Map::new();
        fields.insert("tool".to_string(), Value::String("codex".to_string()));
        fields.insert(
            "stage".to_string(),
            Value::String("reconcile_runtime_owner".to_string()),
        );
        fields.insert(
            "error.message".to_string(),
            Value::String(sanitize_error(&error.to_string())),
        );
        taurhaus_lib::logging::emit_global(
            "warn",
            "coordination",
            "compaction.codex_hook.degraded",
            Some("Managed launch continued with transcript fallback".to_string()),
            fields,
        );
        hook_ready = false;
    }
    hook_ready
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

    result
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

fn coordination_create_team_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    state
        .with_orchestrator(|orchestrator| orchestrator.create_team(&team_name, None).map(|_| ()))
        .map_err(map_coordination_error)
}

fn coordination_disband_team_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<DisbandTeamResponse, String> {
    validate_non_empty("team_name", &team_name)?;
    let result = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(&team_name, None))
        .map_err(map_coordination_error)?;
    ActiveProjectTeamStore::clear_team(state.teams_dir(), &result.team_name)
        .map_err(map_coordination_error)?;
    let message = if result.already_disbanded {
        "team already disbanded".to_string()
    } else {
        "team disbanded".to_string()
    };
    Ok(DisbandTeamResponse {
        team_name: result.team_name,
        disbanded: result.disbanded,
        already_disbanded: result.already_disbanded,
        message,
    })
}

fn coordination_add_member_impl(
    state: &CoordinationState,
    team_name: String,
    member_name: String,
    backend_kind: String,
    project_path: Option<String>,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    validate_non_empty("backend_kind", &backend_kind)?;
    let cli_tool = cli_tool_from_backend_kind(&backend_kind).map_err(map_coordination_error)?;
    state
        .with_orchestrator(|orchestrator| {
            let team_status = orchestrator.get_team_status(&team_name)?;
            let project_path = resolve_legacy_member_project_path(
                &team_status.config.members,
                project_path.as_deref(),
            )?;
            let member = Member {
                name: member_name,
                role: MemberRole::Agent,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: None,
                reasoning_effort: None,
                project_path,
                cli_tool,
                extra: Default::default(),
            };
            orchestrator.add_member(&team_name, member)
        })
        .map_err(map_coordination_error)
}

fn coordination_remove_member_impl(
    state: &CoordinationState,
    team_name: String,
    member_name: String,
) -> Result<RemoveAgentReport, String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    let result = state
        .with_orchestrator(|orchestrator| {
            orchestrator.remove_member(&team_name, &member_name, None)
        })
        .map_err(map_coordination_error)?;
    if result.removed {
        sync_active_team_projects_after_change(state, &result.team_name)
            .map_err(map_coordination_error)?;
    }

    let steps = result
        .steps
        .into_iter()
        .map(|step| StepProgress {
            step: step.step,
            status: if step.success {
                StepStatus::Succeeded
            } else {
                StepStatus::Failed
            },
            message: step.message,
        })
        .collect::<Vec<_>>();

    let warning_count = result.warnings.len();
    let message = if warning_count == 0 {
        "member removed".to_string()
    } else {
        format!(
            "member removed with {warning_count} warning{}",
            if warning_count == 1 { "" } else { "s" }
        )
    };

    Ok(RemoveAgentReport {
        team_name: result.team_name,
        member_name: result.member_name,
        removed: result.removed,
        message,
        steps,
        warnings: result.warnings,
    })
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
