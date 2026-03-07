//! Coordination IPC commands for team management (M0 surface).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, State};

pub use crate::commands::coordination_types::*;
use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::coordination::backend::bridged::{
    availability_check, preflight_check, AvailabilityReport as BackendAvailabilityReport,
    PreflightAgent, PreflightReport as BackendPreflightReport,
};
#[cfg(test)]
use crate::coordination::backend::bridged::{
    availability_check_with_lookup, preflight_check_with_lookup, BinaryLookup,
};
use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    self as contracts, DeliveryRequest, DeliveryResult, OperatorNoticeDelivery,
};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::TeamConfigStore;
use crate::errors::{CommandResultExt, IpcResult};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

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

#[tauri::command]
pub fn coordination_initialize_team(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: InitializeTeamRequest,
) -> IpcResult<InitializeReport> {
    let span = IpcCommandSpan::start("coordination_initialize_team");
    let result = {
        let request = normalize_initialize_request_paths(&db, request)?;
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let mut emit = |event: &StepProgressEvent| {
            let _ = app.emit("coordination-step-progress", event);
        };
        coordination_initialize_team_internal(
            state.inner(),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
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
    let result = {
        let request = normalize_add_agent_request_path(&db, request)?;
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let mut emit = |event: &StepProgressEvent| {
            let _ = app.emit("coordination-step-progress", event);
        };
        coordination_add_agent_internal(
            state.inner(),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
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
    let result = {
        let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
        let mut emit = |event: &StepProgressEvent| {
            let _ = app.emit("coordination-step-progress", event);
        };
        coordination_resume_member_internal(
            state.inner(),
            request,
            &cli_commands,
            &tmux_layout,
            Some(&mut emit),
        )
        .ipc()
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_reonboard(
    state: State<'_, CoordinationState>,
    request: ReonboardRequest,
) -> IpcResult<DeliveryResult> {
    let span = IpcCommandSpan::start("coordination_reonboard");
    let result = coordination_reonboard_impl(state.inner(), request).ipc();
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn coordination_get_live_team_status(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> IpcResult<LiveTeamStatus> {
    let span = IpcCommandSpan::start("coordination_get_live_team_status");
    let result = coordination_get_live_team_status_impl(state.inner(), team_name).ipc();
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
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<InitializeReport, String> {
    validate_initialize_request_fields(&request)?;
    let contract_request = map_initialize_request_to_contract(&request);
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.initialize_team_with_cli_commands_and_layout(
                &contract_request,
                cli_commands,
                tmux_layout,
            )
        })
        .map(map_initialize_report_from_contract)
        .map_err(map_coordination_error)?;
    emit_progress_events(initialize_progress_events(&report), emit);
    Ok(report)
}

fn coordination_add_agent_internal(
    state: &CoordinationState,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<AddAgentReport, String> {
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
    emit_progress_events(add_agent_progress_events(&report), emit);
    Ok(report)
}

fn coordination_resume_member_internal(
    state: &CoordinationState,
    request: ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<ResumeAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;
    let contract_request = map_resume_member_request_to_contract(&request);
    let report = state
        .with_orchestrator(|orchestrator| {
            orchestrator.resume_member_with_cli_commands_and_layout(
                &contract_request,
                cli_commands,
                tmux_layout,
            )
        })
        .map(map_resume_agent_report_from_contract)
        .map_err(map_coordination_error)?;
    emit_progress_events(resume_member_progress_events(&report), emit);
    Ok(report)
}

fn emit_progress_events(
    events: Vec<StepProgressEvent>,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    let Some(emit) = emit.as_mut() else {
        return;
    };
    for event in events {
        emit(&event);
    }
}

fn coordination_reonboard_impl(
    state: &CoordinationState,
    request: ReonboardRequest,
) -> Result<DeliveryResult, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;

    state
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
                member.role_id.as_deref(),
                member.instructions.as_deref(),
                member.behavioral_contract.as_ref(),
                member.capabilities.as_deref(),
            );

            orchestrator.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.member_name.clone(),
                team_name: request.team_name.clone(),
                message,
                sender_name: Some(lead_name),
            }))
        })
        .map_err(map_coordination_error)
}

fn coordination_get_live_team_status_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    let status = state
        .with_orchestrator(|orchestrator| {
            orchestrator.reconcile_team_liveness(&team_name)?;
            orchestrator.get_team_status_fast(&team_name)
        })
        .map_err(map_coordination_error)?;

    let runtime_by_member = status
        .members_runtime
        .into_iter()
        .collect::<HashMap<_, _>>();

    let lead_member = status
        .config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| status.config.members.first());
    let lead_name = lead_member
        .map(|member| member.name.clone())
        .unwrap_or_default();
    let lead_project_path = lead_member.map(|member| member.project_path.clone());

    let members = status
        .config
        .members
        .into_iter()
        .map(|member| {
            let runtime = runtime_by_member.get(&member.name);
            let cross_project = member_cross_project_status(
                lead_project_path.as_deref(),
                member.project_path.as_path(),
            );
            LiveAgentStatus {
                name: member.name,
                role: match member.role {
                    MemberRole::Lead => AgentRole::Lead,
                    MemberRole::Agent => AgentRole::Member,
                },
                cli_tool: member.cli_tool.to_string(),
                model: String::new(),
                role_id: member.role_id,
                role_name: member.role_name,
                focus_area: member.focus_area,
                context_summary: member.context_summary,
                behavior_summary: member.behavior_summary,
                project_id: member.project_path.display().to_string(),
                is_cross_project: cross_project.is_cross_project,
                project_label: cross_project.project_label,
                description: member.instructions,
                session_status: runtime
                    .map(|entry| session_status_from_health(entry.health))
                    .unwrap_or(SessionStatus::Offline),
                pane_id: runtime.and_then(|entry| entry.pane_id.clone()),
            }
        })
        .collect();

    Ok(LiveTeamStatus {
        team_name: status.config.name,
        lead_name,
        members,
    })
}

#[cfg(test)]
pub(crate) fn coordination_get_live_team_status_for_tests(
    state: &CoordinationState,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    coordination_get_live_team_status_impl(state, team_name)
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
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
                project_path,
                cli_tool,
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
    let availability = availability_check();
    coordination_get_project_mesh_snapshot_with_availability(state, project_path, availability)
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
fn coordination_get_project_mesh_snapshot_with_lookup<L: BinaryLookup + ?Sized>(
    state: &CoordinationState,
    project_path: String,
    lookup: &L,
) -> Result<ProjectMeshSnapshotResponse, String> {
    let availability = availability_check_with_lookup(lookup);
    coordination_get_project_mesh_snapshot_with_availability(state, project_path, availability)
}

fn coordination_get_project_mesh_snapshot_with_availability(
    state: &CoordinationState,
    project_path: String,
    availability: BackendAvailabilityReport,
) -> Result<ProjectMeshSnapshotResponse, String> {
    validate_non_empty("project_path", &project_path)?;
    let project_path = crate::provider::path::normalize_project_path(project_path.trim());
    let discovery = discover_team_for_project_path(state.teams_dir(), &project_path)
        .map_err(map_coordination_error)?;

    let team_status = if let Some(team_name) = discovery.team_name.as_deref() {
        Some(
            state
                .with_orchestrator(|orchestrator| orchestrator.get_team_status_fast(team_name))
                .map(map_fast_team_snapshot)
                .map_err(map_coordination_error)?,
        )
    } else {
        None
    };

    Ok(ProjectMeshSnapshotResponse {
        mesh_available: availability.mesh_available,
        tmux_available: availability.tmux_available,
        team_name: discovery.team_name,
        team_status,
        warnings: discovery.warnings,
    })
}

fn validate_and_collect_preflight_agents(
    request: InitializeTeamRequest,
) -> Result<Vec<PreflightAgent>, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("lead.name", &request.lead.name)?;
    validate_non_empty("lead.cli_tool", &request.lead.cli_tool)?;
    for (idx, agent) in request.agents.iter().enumerate() {
        validate_non_empty(&format!("agents[{idx}].name"), &agent.name)?;
        validate_non_empty(&format!("agents[{idx}].cli_tool"), &agent.cli_tool)?;
    }

    let mut preflight_agents = Vec::with_capacity(1 + request.agents.len());
    preflight_agents.push(PreflightAgent {
        agent_name: request.lead.name,
        cli_tool: request.lead.cli_tool,
    });
    for agent in request.agents {
        preflight_agents.push(PreflightAgent {
            agent_name: agent.name,
            cli_tool: agent.cli_tool,
        });
    }
    Ok(preflight_agents)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

fn validate_initialize_request_fields(request: &InitializeTeamRequest) -> Result<(), String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("lead.name", &request.lead.name)?;
    validate_non_empty("lead.project_id", &request.lead.project_id)?;
    validate_non_empty("lead.cli_tool", &request.lead.cli_tool)?;
    for (idx, agent) in request.agents.iter().enumerate() {
        validate_non_empty(&format!("agents[{idx}].name"), &agent.name)?;
        validate_non_empty(&format!("agents[{idx}].project_id"), &agent.project_id)?;
        validate_non_empty(&format!("agents[{idx}].cli_tool"), &agent.cli_tool)?;
    }
    Ok(())
}

fn normalize_initialize_request_paths(
    db: &DbState,
    mut request: InitializeTeamRequest,
) -> Result<InitializeTeamRequest, String> {
    request.lead.project_id = resolve_project_reference(db, &request.lead.project_id)?;
    for agent in &mut request.agents {
        agent.project_id = resolve_project_reference(db, &agent.project_id)?;
    }
    Ok(request)
}

fn normalize_add_agent_request_path(
    db: &DbState,
    mut request: AddAgentRequest,
) -> Result<AddAgentRequest, String> {
    request.agent.project_id = resolve_project_reference(db, &request.agent.project_id)?;
    Ok(request)
}

fn map_lead_mode_to_contract(mode: LeadMode) -> contracts::LeadMode {
    match mode {
        LeadMode::AttachExisting => contracts::LeadMode::AttachExisting,
        LeadMode::LaunchNew => contracts::LeadMode::LaunchNew,
    }
}

fn map_step_status_from_contract(status: contracts::StepStatus) -> StepStatus {
    match status {
        contracts::StepStatus::Pending => StepStatus::Pending,
        contracts::StepStatus::Running => StepStatus::Running,
        contracts::StepStatus::Succeeded => StepStatus::Succeeded,
        contracts::StepStatus::Failed => StepStatus::Failed,
    }
}

fn map_resume_context_mode_to_contract(mode: ResumeContextMode) -> contracts::ResumeContextMode {
    match mode {
        ResumeContextMode::Continue => contracts::ResumeContextMode::Continue,
        ResumeContextMode::Fresh => contracts::ResumeContextMode::Fresh,
    }
}

fn map_agent_setup_to_contract(agent: &AgentSetupConfig) -> contracts::AgentSetupConfig {
    contracts::AgentSetupConfig {
        name: agent.name.clone(),
        cli_tool: agent.cli_tool.clone(),
        model: agent.model.clone(),
        project_id: agent.project_id.clone(),
        description: agent.description.clone(),
        role_id: agent.role_id.clone(),
        role_name: agent.role_name.clone(),
        focus_area: agent.focus_area.clone(),
        context_summary: agent.context_summary.clone(),
        behavior_summary: agent.behavior_summary.clone(),
        instructions: agent.instructions.clone(),
        behavioral_contract: agent.behavioral_contract.clone(),
        capabilities: agent.capabilities.clone(),
    }
}

fn map_step_progress_from_contract(progress: contracts::StepProgress) -> StepProgress {
    StepProgress {
        step: progress.step,
        status: map_step_status_from_contract(progress.status),
        message: progress.message,
    }
}

fn map_initialize_request_to_contract(
    request: &InitializeTeamRequest,
) -> contracts::InitializeTeamRequest {
    contracts::InitializeTeamRequest {
        team_name: request.team_name.clone(),
        team_description: request.team_description.clone(),
        lead_mode: map_lead_mode_to_contract(request.lead_mode),
        lead: map_agent_setup_to_contract(&request.lead),
        agents: request
            .agents
            .iter()
            .map(map_agent_setup_to_contract)
            .collect(),
    }
}

fn map_add_agent_request_to_contract(request: &AddAgentRequest) -> contracts::AddAgentRequest {
    contracts::AddAgentRequest {
        team_name: request.team_name.clone(),
        agent: map_agent_setup_to_contract(&request.agent),
    }
}

fn map_resume_member_request_to_contract(
    request: &ResumeMemberRequest,
) -> contracts::ResumeMemberRequest {
    contracts::ResumeMemberRequest {
        team_name: request.team_name.clone(),
        member_name: request.member_name.clone(),
        context_mode: map_resume_context_mode_to_contract(request.context_mode),
    }
}

fn map_initialize_report_from_contract(report: contracts::InitializeReport) -> InitializeReport {
    InitializeReport {
        team_name: report.team_name,
        succeeded_steps: report.succeeded_steps,
        failed_step: report.failed_step,
        retryable: report.retryable,
        message: report.message,
        steps: report
            .steps
            .into_iter()
            .map(map_step_progress_from_contract)
            .collect(),
    }
}

fn map_add_agent_report_from_contract(report: contracts::AddAgentReport) -> AddAgentReport {
    AddAgentReport {
        team_name: report.team_name,
        member_name: report.member_name,
        succeeded_steps: report.succeeded_steps,
        failed_step: report.failed_step,
        retryable: report.retryable,
        message: report.message,
        steps: report
            .steps
            .into_iter()
            .map(map_step_progress_from_contract)
            .collect(),
    }
}

fn map_resume_agent_report_from_contract(
    report: contracts::ResumeAgentReport,
) -> ResumeAgentReport {
    ResumeAgentReport {
        team_name: report.team_name,
        member_name: report.member_name,
        resumed: report.resumed,
        succeeded_steps: report.succeeded_steps,
        failed_step: report.failed_step,
        retryable: report.retryable,
        message: report.message,
        steps: report
            .steps
            .into_iter()
            .map(map_step_progress_from_contract)
            .collect(),
        warnings: report.warnings,
        pane_id: report.pane_id,
        reused_pane: report.reused_pane,
    }
}

#[cfg(not(test))]
fn resolve_project_reference(db: &DbState, project_ref: &str) -> Result<String, String> {
    validate_non_empty("project_id", project_ref)?;
    let trimmed = project_ref.trim();

    let project_path = {
        let conn = db.0.lock().map_err(|err| format!("{err}"))?;
        match crate::db::queries::get_project(&conn, trimmed).map_err(|err| format!("{err}"))? {
            Some(project) => project.path,
            None => trimmed.to_string(),
        }
    };

    Ok(crate::provider::path::to_linux(&project_path).unwrap_or(project_path))
}

#[cfg(test)]
fn resolve_project_reference(_db: &DbState, project_ref: &str) -> Result<String, String> {
    validate_non_empty("project_id", project_ref)?;
    Ok(project_ref.trim().to_string())
}

fn cli_tool_from_backend_kind(backend_kind: &str) -> Result<CliTool, CoordinationError> {
    CliTool::from_alias(backend_kind).map_err(|_| {
        CoordinationError::Validation(format!(
            "unsupported backend_kind '{}'",
            backend_kind.trim()
        ))
    })
}

fn session_status_from_health(health: HealthState) -> SessionStatus {
    match health {
        HealthState::Healthy => SessionStatus::Active,
        HealthState::AwaitingRead
        | HealthState::SuspectedStuck
        | HealthState::Rebriefed
        | HealthState::Suppressed => SessionStatus::Idle,
        HealthState::SessionDead => SessionStatus::Offline,
    }
}

#[derive(Debug, Default)]
struct ProjectPathDiscovery {
    team_name: Option<String>,
    warnings: Vec<String>,
}

fn discover_team_for_project_path(
    teams_dir: &Path,
    project_path: &str,
) -> Result<ProjectPathDiscovery, CoordinationError> {
    if !teams_dir.exists() {
        return Ok(ProjectPathDiscovery::default());
    }

    let mut team_name = None;
    let mut warnings = Vec::new();
    for listed_team in TeamConfigStore::list(teams_dir)? {
        match TeamConfigStore::load(teams_dir, &listed_team) {
            Ok(config) => {
                if team_name.is_none()
                    && config.members.iter().any(|member| {
                        crate::provider::path::normalize_project_path(
                            &member.project_path.display().to_string(),
                        ) == project_path
                    })
                {
                    team_name = Some(config.name);
                }
            }
            Err(CoordinationError::NotFound(_)) => {}
            Err(CoordinationError::StoreError(_)) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' because config is missing or invalid"
                ));
            }
            Err(CoordinationError::Io(err)) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' due to IO error: {err}"
                ));
            }
            Err(other) => {
                warnings.push(format!(
                    "skipped team folder '{listed_team}' due to discovery error: {other}"
                ));
            }
        }
    }
    warnings.sort();

    Ok(ProjectPathDiscovery {
        team_name,
        warnings,
    })
}

fn map_fast_team_snapshot(
    status: crate::coordination::orchestrator::TeamStatus,
) -> FastTeamSnapshot {
    let runtime_by_member = status
        .members_runtime
        .into_iter()
        .collect::<HashMap<_, _>>();

    let lead_member = status
        .config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| status.config.members.first());
    let lead_name = lead_member
        .map(|member| member.name.clone())
        .unwrap_or_default();
    let lead_project_path = lead_member.map(|member| member.project_path.clone());

    let members = status
        .config
        .members
        .into_iter()
        .map(|member| {
            let runtime = runtime_by_member.get(&member.name);
            let cross_project = member_cross_project_status(
                lead_project_path.as_deref(),
                member.project_path.as_path(),
            );
            FastAgentSnapshot {
                name: member.name,
                role: match member.role {
                    MemberRole::Lead => AgentRole::Lead,
                    MemberRole::Agent => AgentRole::Member,
                },
                cli_tool: member.cli_tool.to_string(),
                role_id: member.role_id,
                role_name: member.role_name,
                focus_area: member.focus_area,
                context_summary: member.context_summary,
                behavior_summary: member.behavior_summary,
                project_id: member.project_path.display().to_string(),
                is_cross_project: cross_project.is_cross_project,
                project_label: cross_project.project_label,
                description: member.instructions,
                session_status: runtime
                    .map(|entry| session_status_from_health(entry.health))
                    .unwrap_or(SessionStatus::Offline),
                pane_id: runtime.and_then(|entry| entry.pane_id.clone()),
            }
        })
        .collect();

    FastTeamSnapshot { lead_name, members }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CrossProjectStatus {
    is_cross_project: bool,
    project_label: String,
}

fn member_cross_project_status(
    lead_project_path: Option<&Path>,
    member_project_path: &Path,
) -> CrossProjectStatus {
    lead_project_path
        .map(|lead_project_path| {
            derive_cross_project_status(lead_project_path, member_project_path)
        })
        .unwrap_or_default()
}

fn derive_cross_project_status(
    lead_project_path: &Path,
    member_project_path: &Path,
) -> CrossProjectStatus {
    let lead_project_path = canonical_project_identity(&lead_project_path.display().to_string());
    let member_project_path =
        canonical_project_identity(&member_project_path.display().to_string());
    let is_cross_project = lead_project_path != member_project_path;
    let project_label = if is_cross_project {
        project_label_from_path(&member_project_path)
    } else {
        String::new()
    };

    CrossProjectStatus {
        is_cross_project,
        project_label,
    }
}

fn canonical_project_identity(project_path: &str) -> String {
    let normalized = crate::provider::path::normalize_project_path(project_path);
    if is_windows_mount_path(&normalized) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn is_windows_mount_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 7
        && path.starts_with("/mnt/")
        && bytes[5].is_ascii_alphabetic()
        && (bytes[6] == b'/' || bytes[6] == b'\\')
}

fn project_label_from_path(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            project_path
                .rsplit('/')
                .find(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}

fn resolve_legacy_member_project_path(
    existing_members: &[Member],
    project_path_override: Option<&str>,
) -> Result<PathBuf, CoordinationError> {
    if let Some(project_path) = project_path_override {
        let project_path = project_path.trim();
        if project_path.is_empty() {
            return Err(CoordinationError::Validation(
                "project_path must not be empty".to_string(),
            ));
        }
        return Ok(PathBuf::from(project_path));
    }

    existing_members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| existing_members.first())
        .map(|member| member.project_path.clone())
        .ok_or_else(|| {
            CoordinationError::Validation(
                "project_path must be provided for legacy add-member when team has no members"
                    .to_string(),
            )
        })
}

fn map_coordination_error(err: CoordinationError) -> String {
    err.to_string()
}

fn initialize_progress_events(report: &InitializeReport) -> Vec<StepProgressEvent> {
    let mut events = Vec::new();
    for progress in &report.steps {
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "initialize_team".to_string(),
            progress: StepProgress {
                step: progress.step.clone(),
                status: StepStatus::Running,
                message: None,
            },
        });
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "initialize_team".to_string(),
            progress: progress.clone(),
        });
    }
    events
}

fn add_agent_progress_events(report: &AddAgentReport) -> Vec<StepProgressEvent> {
    let mut events = Vec::new();
    for progress in &report.steps {
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "add_agent".to_string(),
            progress: StepProgress {
                step: progress.step.clone(),
                status: StepStatus::Running,
                message: None,
            },
        });
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "add_agent".to_string(),
            progress: progress.clone(),
        });
    }
    events
}

fn resume_member_progress_events(report: &ResumeAgentReport) -> Vec<StepProgressEvent> {
    let mut events = Vec::new();
    for progress in &report.steps {
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "resume_member".to_string(),
            progress: StepProgress {
                step: progress.step.clone(),
                status: StepStatus::Running,
                message: None,
            },
        });
        events.push(StepProgressEvent {
            team_name: report.team_name.clone(),
            operation: "resume_member".to_string(),
            progress: progress.clone(),
        });
    }
    events
}

fn map_preflight_report(report: BackendPreflightReport) -> PreflightReport {
    PreflightReport {
        can_initialize: report.can_initialize(),
        blocking_errors: report.blocking_errors,
        agent_warnings: report
            .agent_warnings
            .into_iter()
            .map(|warning| AgentPreflightWarning {
                agent_name: warning.agent_name,
                cli_tool: warning.cli_tool,
                message: warning.message,
            })
            .collect(),
    }
}

fn map_feature_availability_report(report: BackendAvailabilityReport) -> FeatureAvailabilityReport {
    FeatureAvailabilityReport {
        can_initialize: report.can_initialize(),
        mesh_available: report.mesh_available,
        tmux_available: report.tmux_available,
        blocking_errors: report.blocking_errors,
    }
}

#[cfg(test)]
#[path = "coordination/tests.rs"]
mod tests;
