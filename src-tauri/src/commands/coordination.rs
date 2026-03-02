//! Coordination IPC commands for team management (M0 surface).

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

pub use crate::commands::coordination_types::*;
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
use crate::coordination::requests::{DeliveryRequest, DeliveryResult, OperatorNoticeDelivery};
use crate::coordination::state::CoordinationState;
use crate::session_scanner::cli_tool::CliTool;

#[tauri::command]
pub fn coordination_initialize_team(
    app: AppHandle,
    state: State<'_, CoordinationState>,
    request: InitializeTeamRequest,
) -> Result<InitializeReport, String> {
    coordination_initialize_team_with_emitter(state.inner(), request, |event| {
        let _ = app.emit("coordination-step-progress", event);
    })
}

#[tauri::command]
pub fn coordination_add_agent(
    app: AppHandle,
    state: State<'_, CoordinationState>,
    request: AddAgentRequest,
) -> Result<AddAgentReport, String> {
    coordination_add_agent_with_emitter(state.inner(), request, |event| {
        let _ = app.emit("coordination-step-progress", event);
    })
}

#[tauri::command]
pub fn coordination_reonboard(
    state: State<'_, CoordinationState>,
    request: ReonboardRequest,
) -> Result<DeliveryResult, String> {
    coordination_reonboard_impl(state.inner(), request)
}

#[tauri::command]
pub fn coordination_get_live_team_status(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    coordination_get_live_team_status_impl(state.inner(), team_name)
}

#[tauri::command]
pub fn coordination_create_team(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> Result<(), String> {
    coordination_create_team_impl(state.inner(), team_name)
}

#[tauri::command]
pub fn coordination_disband_team(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> Result<DisbandTeamResponse, String> {
    coordination_disband_team_impl(state.inner(), team_name)
}

#[tauri::command]
pub fn coordination_add_member(
    state: State<'_, CoordinationState>,
    team_name: String,
    member_name: String,
    backend_kind: String,
) -> Result<(), String> {
    coordination_add_member_impl(state.inner(), team_name, member_name, backend_kind)
}

#[tauri::command]
pub fn coordination_remove_member(
    state: State<'_, CoordinationState>,
    team_name: String,
    member_name: String,
) -> Result<(), String> {
    coordination_remove_member_impl(state.inner(), team_name, member_name)
}

#[tauri::command]
pub fn coordination_list_teams(
    state: State<'_, CoordinationState>,
) -> Result<TeamDiscoveryResponse, String> {
    coordination_list_teams_impl(state.inner())
}

#[tauri::command]
pub fn coordination_get_team_status(
    state: State<'_, CoordinationState>,
    team_name: String,
) -> Result<TeamStatus, String> {
    coordination_get_team_status_impl(state.inner(), team_name)
}

#[tauri::command]
pub fn coordination_preflight_check(
    request: InitializeTeamRequest,
) -> Result<PreflightReport, String> {
    coordination_preflight_check_impl(request)
}

#[tauri::command]
pub fn coordination_get_feature_availability() -> Result<FeatureAvailabilityReport, String> {
    Ok(coordination_get_feature_availability_impl())
}

fn coordination_initialize_team_impl(
    state: &CoordinationState,
    request: InitializeTeamRequest,
) -> Result<InitializeReport, String> {
    state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&request))
        .map_err(map_coordination_error)
}

fn coordination_initialize_team_with_emitter<E>(
    state: &CoordinationState,
    request: InitializeTeamRequest,
    mut emit: E,
) -> Result<InitializeReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    validate_initialize_request_fields(&request)?;
    let report = coordination_initialize_team_impl(state, request)?;
    for event in initialize_progress_events(&report) {
        emit(&event);
    }
    Ok(report)
}

fn coordination_add_agent_impl(
    state: &CoordinationState,
    request: AddAgentRequest,
) -> Result<AddAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("agent.name", &request.agent.name)?;
    validate_non_empty("agent.project_id", &request.agent.project_id)?;
    validate_non_empty("agent.cli_tool", &request.agent.cli_tool)?;
    state
        .with_orchestrator(|orchestrator| orchestrator.add_agent_to_team(&request))
        .map_err(map_coordination_error)
}

fn coordination_add_agent_with_emitter<E>(
    state: &CoordinationState,
    request: AddAgentRequest,
    mut emit: E,
) -> Result<AddAgentReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    let report = coordination_add_agent_impl(state, request)?;
    for event in add_agent_progress_events(&report) {
        emit(&event);
    }
    Ok(report)
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
            let message = DeliveryRenderer::render_onboarding(
                &request.team_name,
                &request.member_name,
                &lead_name,
            );

            orchestrator.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.member_name.clone(),
                team_name: request.team_name.clone(),
                message,
            }))
        })
        .map_err(map_coordination_error)
}

fn coordination_get_live_team_status_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<LiveTeamStatus, String> {
    let status = state
        .with_orchestrator(|orchestrator| orchestrator.get_team_status(&team_name))
        .map_err(map_coordination_error)?;

    let runtime_by_member = status
        .members_runtime
        .into_iter()
        .collect::<HashMap<_, _>>();

    let lead_name = status
        .config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .map(|member| member.name.clone())
        .or_else(|| {
            status
                .config
                .members
                .first()
                .map(|member| member.name.clone())
        })
        .unwrap_or_default();

    let members = status
        .config
        .members
        .into_iter()
        .map(|member| {
            let runtime = runtime_by_member.get(&member.name);
            LiveAgentStatus {
                name: member.name,
                role: match member.role {
                    MemberRole::Lead => AgentRole::Lead,
                    MemberRole::Agent => AgentRole::Member,
                },
                cli_tool: member.cli_tool.to_string(),
                model: String::new(),
                project_id: member.project_path.display().to_string(),
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
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    validate_non_empty("backend_kind", &backend_kind)?;

    let cli_tool = cli_tool_from_backend_kind(&backend_kind).map_err(map_coordination_error)?;
    let member = Member {
        name: member_name,
        role: MemberRole::Agent,
        instructions: None,
        project_path: default_project_path(),
        cli_tool,
    };
    state
        .with_orchestrator(|orchestrator| orchestrator.add_member(&team_name, member))
        .map_err(map_coordination_error)
}

fn coordination_remove_member_impl(
    state: &CoordinationState,
    team_name: String,
    member_name: String,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    state
        .with_orchestrator(|orchestrator| {
            orchestrator.remove_member(&team_name, &member_name, None)
        })
        .map_err(map_coordination_error)
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

fn cli_tool_from_backend_kind(backend_kind: &str) -> Result<CliTool, CoordinationError> {
    match backend_kind.trim().to_ascii_lowercase().as_str() {
        "mesh" | "mesh_bridged" | "codex" => Ok(CliTool::Codex),
        "claude" | "claude_native" => Ok(CliTool::Claude),
        "gemini" => Ok(CliTool::Gemini),
        other => Err(CoordinationError::Validation(format!(
            "unsupported backend_kind '{other}'"
        ))),
    }
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

fn default_project_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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
