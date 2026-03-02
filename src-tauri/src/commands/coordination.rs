//! Coordination IPC commands for team management (M0 surface).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

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

/// Lightweight team list entry returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub team_name: String,
    pub lead_project_path: Option<String>,
}

/// Discovery response with valid teams plus skipped-folder warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamDiscoveryResponse {
    pub teams: Vec<TeamSummary>,
    pub warnings: Vec<String>,
}

/// Team status payload returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatus {
    pub team_name: String,
    pub members: Vec<String>,
}

/// Disband response describing whether state was removed or already absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisbandTeamResponse {
    pub team_name: String,
    pub disbanded: bool,
    pub already_disbanded: bool,
    pub message: String,
}

/// Team-lead startup mode selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadMode {
    AttachExisting,
    LaunchNew,
}

/// Role descriptor shown in the live team roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Lead,
    Member,
}

/// Session runtime status for one roster member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Active,
    Idle,
    Offline,
}

/// Step status used by initialize/hot-add progress models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

/// Agent setup card payload from the frontend setup form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetupConfig {
    pub name: String,
    pub cli_tool: String,
    pub model: String,
    pub project_id: String,
    pub description: Option<String>,
}

/// IPC request for one-click team initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeTeamRequest {
    pub team_name: String,
    pub team_description: Option<String>,
    pub lead_mode: LeadMode,
    pub lead: AgentSetupConfig,
    pub agents: Vec<AgentSetupConfig>,
}

/// Per-step progress shape shared by reports and streamed events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepProgress {
    pub step: String,
    pub status: StepStatus,
    pub message: Option<String>,
}

/// IPC response for initialize operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeReport {
    pub team_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// IPC request for hot-adding one agent to a running team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAgentRequest {
    pub team_name: String,
    pub agent: AgentSetupConfig,
}

/// IPC response for hot-add operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAgentReport {
    pub team_name: String,
    pub member_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// IPC request for re-sending onboarding to an existing team member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReonboardRequest {
    pub team_name: String,
    pub member_name: String,
}

/// Live-team row rendered in runtime roster mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAgentStatus {
    pub name: String,
    pub role: AgentRole,
    pub cli_tool: String,
    pub model: String,
    pub project_id: String,
    pub description: Option<String>,
    pub session_status: SessionStatus,
    pub pane_id: Option<String>,
}

/// Live-team payload for the frontend mesh roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTeamStatus {
    pub team_name: String,
    pub lead_name: String,
    pub members: Vec<LiveAgentStatus>,
}

/// Streamed progress event payload emitted during long operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct StepProgressEvent {
    pub team_name: String,
    pub operation: String,
    pub progress: StepProgress,
}

/// Agent-scoped warning from the environment preflight check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPreflightWarning {
    pub agent_name: String,
    pub cli_tool: String,
    pub message: String,
}

/// Initialization preflight report with blockers and per-agent warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub can_initialize: bool,
    pub blocking_errors: Vec<String>,
    pub agent_warnings: Vec<AgentPreflightWarning>,
}

/// Baseline feature availability for Mesh tab gating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureAvailabilityReport {
    pub can_initialize: bool,
    pub mesh_available: bool,
    pub tmux_available: bool,
    pub blocking_errors: Vec<String>,
}

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

            orchestrator.deliver_message(DeliveryRequest::OperatorNotice(
                OperatorNoticeDelivery {
                    member_name: request.member_name.clone(),
                    team_name: request.team_name.clone(),
                    message,
                },
            ))
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
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::state::CoordinationState;

    #[derive(Debug, Default)]
    struct MockBinaryLookup {
        available: HashSet<String>,
    }

    impl MockBinaryLookup {
        fn with_available(names: &[&str]) -> Self {
            Self {
                available: names.iter().map(|name| (*name).to_string()).collect(),
            }
        }
    }

    impl BinaryLookup for MockBinaryLookup {
        fn is_available(&self, binary_name: &str) -> bool {
            self.available.contains(binary_name)
        }
    }

    fn test_state(teams_dir: PathBuf) -> CoordinationState {
        CoordinationState::with_components(
            teams_dir,
            BackendSelector::m0(),
            Arc::new(|_kind| Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)),
        )
    }

    fn sample_preflight_request() -> InitializeTeamRequest {
        InitializeTeamRequest {
            team_name: "architecture-final".to_string(),
            team_description: Some("Cross-project implementation team".to_string()),
            lead_mode: LeadMode::LaunchNew,
            lead: AgentSetupConfig {
                name: "team-lead".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                project_id: "proj-core".to_string(),
                description: Some("Own orchestration".to_string()),
            },
            agents: vec![
                AgentSetupConfig {
                    name: "frontend-dev".to_string(),
                    cli_tool: "codex".to_string(),
                    model: "gpt-5.3".to_string(),
                    project_id: "proj-web".to_string(),
                    description: Some("UI implementation".to_string()),
                },
                AgentSetupConfig {
                    name: "reviewer".to_string(),
                    cli_tool: "gemini".to_string(),
                    model: "pro".to_string(),
                    project_id: "proj-api".to_string(),
                    description: None,
                },
            ],
        }
    }

    fn sample_add_agent_request(team_name: &str, member_name: &str) -> AddAgentRequest {
        AddAgentRequest {
            team_name: team_name.to_string(),
            agent: AgentSetupConfig {
                name: member_name.to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-api".to_string(),
                description: Some("API ownership".to_string()),
            },
        }
    }

    #[test]
    fn team_summary_serialization_round_trip() {
        let value = TeamSummary {
            team_name: "architecture-final".to_string(),
            lead_project_path: Some("/tmp/taurhaus".to_string()),
        };
        let json = serde_json::to_string(&value).expect("serialize team summary");
        let decoded: TeamSummary = serde_json::from_str(&json).expect("deserialize team summary");
        assert_eq!(decoded, value);
    }

    #[test]
    fn team_discovery_response_serialization_round_trip() {
        let value = TeamDiscoveryResponse {
            teams: vec![TeamSummary {
                team_name: "architecture-final".to_string(),
                lead_project_path: Some("/tmp/taurhaus".to_string()),
            }],
            warnings: vec!["skipped team folder 'broken-team'".to_string()],
        };
        let json = serde_json::to_string(&value).expect("serialize team discovery response");
        let decoded: TeamDiscoveryResponse =
            serde_json::from_str(&json).expect("deserialize team discovery response");
        assert_eq!(decoded, value);
    }

    #[test]
    fn team_status_serialization_round_trip() {
        let value = TeamStatus {
            team_name: "architecture-final".to_string(),
            members: vec!["team-lead".to_string(), "codex-reviewer".to_string()],
        };
        let json = serde_json::to_string(&value).expect("serialize team status");
        let decoded: TeamStatus = serde_json::from_str(&json).expect("deserialize team status");
        assert_eq!(decoded, value);
    }

    #[test]
    fn disband_team_response_serialization_round_trip() {
        let value = DisbandTeamResponse {
            team_name: "architecture-final".to_string(),
            disbanded: false,
            already_disbanded: true,
            message: "team already disbanded".to_string(),
        };
        let json = serde_json::to_string(&value).expect("serialize disband response");
        let decoded: DisbandTeamResponse =
            serde_json::from_str(&json).expect("deserialize disband response");
        assert_eq!(decoded, value);
    }

    #[test]
    fn create_team_rejects_empty_or_whitespace_name() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let err =
            coordination_create_team_impl(&state, "".to_string()).expect_err("empty should fail");
        assert!(err.contains("team_name"));

        let err = coordination_create_team_impl(&state, "   \n\t  ".to_string())
            .expect_err("whitespace should fail");
        assert!(err.contains("team_name"));
    }

    #[test]
    fn member_commands_validate_all_required_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let err = coordination_disband_team_impl(&state, "   ".to_string())
            .expect_err("blank team should fail");
        assert!(err.contains("team_name"));

        let err = coordination_add_member_impl(
            &state,
            "team".to_string(),
            "".to_string(),
            "mesh".to_string(),
        )
        .expect_err("empty member should fail");
        assert!(err.contains("member_name"));

        let err = coordination_add_member_impl(
            &state,
            "team".to_string(),
            "alice".to_string(),
            "".to_string(),
        )
        .expect_err("empty backend should fail");
        assert!(err.contains("backend_kind"));

        let err = coordination_remove_member_impl(&state, "".to_string(), "alice".to_string())
            .expect_err("empty team should fail");
        assert!(err.contains("team_name"));

        let err = coordination_remove_member_impl(&state, "team".to_string(), "  ".to_string())
            .expect_err("whitespace member should fail");
        assert!(err.contains("member_name"));
    }

    #[test]
    fn get_team_status_validates_non_empty_team_name() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let err = coordination_get_team_status_impl(&state, " ".to_string())
            .expect_err("whitespace invalid");
        assert!(err.contains("team_name"));
    }

    #[test]
    fn preflight_all_tools_present_returns_clean_report() {
        let lookup =
            MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex", "gemini"]);
        let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
            .expect("preflight should succeed");
        assert!(report.can_initialize);
        assert!(report.blocking_errors.is_empty());
        assert!(report.agent_warnings.is_empty());
    }

    #[test]
    fn preflight_mesh_missing_returns_blocking_error() {
        let lookup = MockBinaryLookup::with_available(&["tmux", "claude", "codex", "gemini"]);
        let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
            .expect("preflight should succeed");
        assert!(!report.can_initialize);
        assert!(report.blocking_errors.contains(
            &"Mesh CLI not found. Install it to enable multi-agent collaboration.".to_string()
        ));
    }

    #[test]
    fn preflight_tmux_missing_returns_blocking_error() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "claude", "codex", "gemini"]);
        let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
            .expect("preflight should succeed");
        assert!(!report.can_initialize);
        assert!(report
            .blocking_errors
            .contains(&"tmux is required for multi-agent sessions.".to_string()));
    }

    #[test]
    fn preflight_agent_tool_missing_returns_warning_without_blocking() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "gemini"]);
        let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
            .expect("preflight should succeed");
        assert!(report.can_initialize);
        assert!(report.blocking_errors.is_empty());
        assert_eq!(report.agent_warnings.len(), 1);
        assert_eq!(
            report.agent_warnings[0].message,
            "Codex CLI not found - agent 'frontend-dev' cannot be launched."
        );
    }

    #[test]
    fn preflight_multiple_issues_reports_all() {
        let lookup = MockBinaryLookup::with_available(&["codex"]);
        let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
            .expect("preflight should succeed");
        assert!(!report.can_initialize);
        assert_eq!(report.blocking_errors.len(), 2);
        assert_eq!(report.agent_warnings.len(), 2);
        assert!(report.agent_warnings.iter().any(|warning| {
            warning.message == "Claude CLI not found - agent 'team-lead' cannot be launched."
        }));
        assert!(report.agent_warnings.iter().any(|warning| {
            warning.message == "Gemini CLI not found - agent 'reviewer' cannot be launched."
        }));
    }

    #[test]
    fn feature_availability_reports_missing_mesh_and_tmux() {
        let lookup = MockBinaryLookup::with_available(&["claude"]);
        let report = coordination_get_feature_availability_with_lookup(&lookup);
        assert!(!report.can_initialize);
        assert!(!report.mesh_available);
        assert!(!report.tmux_available);
        assert_eq!(report.blocking_errors.len(), 2);
    }

    #[test]
    fn feature_availability_reports_ready_when_required_tools_exist() {
        let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);
        let report = coordination_get_feature_availability_with_lookup(&lookup);
        assert!(report.can_initialize);
        assert!(report.mesh_available);
        assert!(report.tmux_available);
        assert!(report.blocking_errors.is_empty());
    }

    #[test]
    fn initialize_ipc_delegates_to_orchestrator_and_returns_report_shape() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let request = sample_preflight_request();

        let report = coordination_initialize_team_with_emitter(&state, request, |_| {})
            .expect("initialize should return a report");

        assert_eq!(report.team_name, "architecture-final");
        assert!(report.failed_step.is_none());
        assert!(!report.steps.is_empty());
        assert_eq!(report.steps[0].step, "validate_configuration");
    }

    #[test]
    fn initialize_progress_events_are_emitted_in_step_order() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let request = sample_preflight_request();

        let mut emitted = Vec::new();
        let report = coordination_initialize_team_with_emitter(&state, request, |event| {
            emitted.push(event.clone());
        })
        .expect("initialize should return a report");

        assert_eq!(emitted.len(), report.steps.len() * 2);
        for (idx, step) in report.steps.iter().enumerate() {
            let running = &emitted[idx * 2];
            let completed = &emitted[idx * 2 + 1];
            assert_eq!(running.operation, "initialize_team");
            assert_eq!(running.progress.step, step.step);
            assert_eq!(running.progress.status, StepStatus::Running);
            assert_eq!(completed.progress.step, step.step);
            assert_eq!(completed.progress.status, step.status);
        }
    }

    #[test]
    fn initialize_error_case_returns_structured_failed_step_report() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let mut request = sample_preflight_request();
        request.agents[0].name = request.lead.name.clone(); // duplicate name -> validation step failure

        let report = coordination_initialize_team_with_emitter(&state, request, |_| {})
            .expect("initialize should return structured failure report");
        assert_eq!(
            report.failed_step.as_deref(),
            Some("validate_configuration")
        );
        assert!(report.retryable);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].status, StepStatus::Failed);
    }

    #[test]
    fn add_agent_ipc_returns_add_agent_report_shape() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        coordination_create_team_impl(&state, "arch".to_string()).expect("create");

        let report = coordination_add_agent_impl(&state, sample_add_agent_request("arch", "bob"))
            .expect("add-agent should return report");

        assert_eq!(report.team_name, "arch");
        assert_eq!(report.member_name, "bob");
        assert!(report.failed_step.is_none());
        assert!(report
            .succeeded_steps
            .contains(&"update_roster".to_string()));
        assert!(!report.steps.is_empty());
    }

    #[test]
    fn add_agent_progress_events_are_emitted_in_step_order() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        coordination_create_team_impl(&state, "arch".to_string()).expect("create");

        let mut emitted = Vec::new();
        let report = coordination_add_agent_with_emitter(
            &state,
            sample_add_agent_request("arch", "bob"),
            |event| emitted.push(event.clone()),
        )
        .expect("add-agent should return report");

        assert_eq!(emitted.len(), report.steps.len() * 2);
        for (idx, step) in report.steps.iter().enumerate() {
            let running = &emitted[idx * 2];
            let completed = &emitted[idx * 2 + 1];
            assert_eq!(running.operation, "add_agent");
            assert_eq!(running.progress.step, step.step);
            assert_eq!(running.progress.status, StepStatus::Running);
            assert_eq!(completed.progress.step, step.step);
            assert_eq!(completed.progress.status, step.status);
        }
    }

    #[test]
    fn reonboard_succeeds_for_existing_member() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        coordination_initialize_team_with_emitter(&state, sample_preflight_request(), |_| {})
            .expect("initialize");

        let result = coordination_reonboard_impl(
            &state,
            ReonboardRequest {
                team_name: "architecture-final".to_string(),
                member_name: "frontend-dev".to_string(),
            },
        )
        .expect("reonboard should succeed");

        assert!(result.delivered);
    }

    #[test]
    fn reonboard_fails_for_nonexistent_team_or_member() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        let missing_team = coordination_reonboard_impl(
            &state,
            ReonboardRequest {
                team_name: "missing".to_string(),
                member_name: "bob".to_string(),
            },
        )
        .expect_err("missing team should fail");
        assert!(missing_team.contains("Not found"));

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        let missing_member = coordination_reonboard_impl(
            &state,
            ReonboardRequest {
                team_name: "arch".to_string(),
                member_name: "bob".to_string(),
            },
        )
        .expect_err("missing member should fail");
        assert!(missing_member.contains("Not found"));
    }

    #[test]
    fn add_agent_and_reonboard_validate_empty_strings() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        coordination_create_team_impl(&state, "arch".to_string()).expect("create");

        let add_agent_err = coordination_add_agent_impl(
            &state,
            AddAgentRequest {
                team_name: " ".to_string(),
                agent: AgentSetupConfig {
                    name: "".to_string(),
                    cli_tool: "".to_string(),
                    model: "gpt-5.3".to_string(),
                    project_id: "".to_string(),
                    description: None,
                },
            },
        )
        .expect_err("empty add-agent fields should fail");
        assert!(add_agent_err.contains("team_name"));

        let reonboard_team_err = coordination_reonboard_impl(
            &state,
            ReonboardRequest {
                team_name: "".to_string(),
                member_name: "bob".to_string(),
            },
        )
        .expect_err("empty team_name should fail");
        assert!(reonboard_team_err.contains("team_name"));

        let reonboard_member_err = coordination_reonboard_impl(
            &state,
            ReonboardRequest {
                team_name: "arch".to_string(),
                member_name: "  ".to_string(),
            },
        )
        .expect_err("empty member_name should fail");
        assert!(reonboard_member_err.contains("member_name"));
    }

    #[test]
    fn create_team_happy_path_persists_team() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create should succeed");
        let discovery = coordination_list_teams_impl(&state).expect("list should succeed");
        assert_eq!(
            discovery.teams,
            vec![TeamSummary {
                team_name: "arch".to_string(),
                lead_project_path: None,
            }]
        );
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn create_team_error_mapping_conflict() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("first create");
        let err = coordination_create_team_impl(&state, "arch".to_string())
            .expect_err("duplicate should fail");
        assert!(err.contains("Conflict"));
    }

    #[test]
    fn disband_team_happy_path_removes_team() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        let result = coordination_disband_team_impl(&state, "arch".to_string()).expect("disband");
        assert!(result.disbanded);
        assert!(!result.already_disbanded);
        assert_eq!(result.message, "team disbanded");

        let discovery = coordination_list_teams_impl(&state).expect("list");
        assert!(discovery.teams.is_empty());
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn disband_team_is_idempotent_and_reports_already_disbanded() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        let result = coordination_disband_team_impl(&state, "missing".to_string())
            .expect("idempotent disband should succeed");
        assert!(!result.disbanded);
        assert!(result.already_disbanded);
        assert_eq!(result.message, "team already disbanded");
    }

    #[test]
    fn add_member_happy_path_persists_member() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        coordination_add_member_impl(
            &state,
            "arch".to_string(),
            "alice".to_string(),
            "mesh".to_string(),
        )
        .expect("add member");
        let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
        assert_eq!(status.members, vec!["alice".to_string()]);
    }

    #[test]
    fn add_member_error_mapping_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        let err = coordination_add_member_impl(
            &state,
            "missing".to_string(),
            "alice".to_string(),
            "mesh".to_string(),
        )
        .expect_err("missing team");
        assert!(err.contains("Not found"));
    }

    #[test]
    fn remove_member_happy_path_removes_member() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        coordination_add_member_impl(
            &state,
            "arch".to_string(),
            "alice".to_string(),
            "mesh".to_string(),
        )
        .expect("add");
        coordination_remove_member_impl(&state, "arch".to_string(), "alice".to_string())
            .expect("remove");
        let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
        assert!(status.members.is_empty());
    }

    #[test]
    fn remove_member_error_mapping_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        let err =
            coordination_remove_member_impl(&state, "arch".to_string(), "missing".to_string())
                .expect_err("missing member");
        assert!(err.contains("Not found"));
    }

    #[test]
    fn list_teams_happy_path_returns_sorted_summaries() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "zeta".to_string()).expect("create zeta");
        coordination_create_team_impl(&state, "alpha".to_string()).expect("create alpha");

        let discovery = coordination_list_teams_impl(&state).expect("list");
        assert_eq!(
            discovery.teams,
            vec![
                TeamSummary {
                    team_name: "alpha".to_string(),
                    lead_project_path: None,
                },
                TeamSummary {
                    team_name: "zeta".to_string(),
                    lead_project_path: None,
                }
            ]
        );
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn list_teams_error_mapping_io() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("teams-file");
        std::fs::write(&file_path, "not a directory").expect("write marker");
        let state = test_state(file_path);

        let err = coordination_list_teams_impl(&state).expect_err("list should fail");
        assert!(err.contains("IO error"));
    }

    #[test]
    fn list_teams_includes_lead_project_anchor() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        let request = sample_preflight_request();
        coordination_initialize_team_with_emitter(&state, request, |_| {})
            .expect("initialize should succeed");

        let discovery = coordination_list_teams_impl(&state).expect("list");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, "architecture-final");
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some("proj-core")
        );
    }

    #[test]
    fn list_teams_skips_corrupt_folders_with_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "good".to_string()).expect("create");
        let broken_dir = tmp.path().join("broken-team");
        std::fs::create_dir_all(&broken_dir).expect("create broken dir");
        std::fs::write(broken_dir.join("config.json"), "{ invalid").expect("write broken");

        let discovery = coordination_list_teams_impl(&state).expect("list");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, "good");
        assert_eq!(discovery.warnings.len(), 1);
        assert!(discovery.warnings[0].contains("broken-team"));
    }

    #[test]
    fn get_team_status_happy_path_returns_team_and_members() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        coordination_create_team_impl(&state, "arch".to_string()).expect("create");
        coordination_add_member_impl(
            &state,
            "arch".to_string(),
            "alice".to_string(),
            "mesh".to_string(),
        )
        .expect("add");
        let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
        assert_eq!(status.team_name, "arch");
        assert_eq!(status.members, vec!["alice".to_string()]);
    }

    #[test]
    fn get_team_status_error_mapping_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());

        let err = coordination_get_team_status_impl(&state, "missing".to_string())
            .expect_err("missing team");
        assert!(err.contains("Not found"));
    }

    #[test]
    fn initialize_team_request_round_trip() {
        let value = InitializeTeamRequest {
            team_name: "architecture-final".to_string(),
            team_description: Some("Cross-project implementation team".to_string()),
            lead_mode: LeadMode::LaunchNew,
            lead: AgentSetupConfig {
                name: "team-lead".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                project_id: "proj-core".to_string(),
                description: Some("Own orchestration".to_string()),
            },
            agents: vec![
                AgentSetupConfig {
                    name: "frontend-dev".to_string(),
                    cli_tool: "codex".to_string(),
                    model: "gpt-5.3".to_string(),
                    project_id: "proj-web".to_string(),
                    description: Some("UI implementation".to_string()),
                },
                AgentSetupConfig {
                    name: "reviewer".to_string(),
                    cli_tool: "gemini".to_string(),
                    model: "pro".to_string(),
                    project_id: "proj-api".to_string(),
                    description: None,
                },
            ],
        };

        let json = serde_json::to_string(&value).expect("serialize init request");
        let decoded: InitializeTeamRequest =
            serde_json::from_str(&json).expect("deserialize init request");
        assert_eq!(decoded, value);
    }

    #[test]
    fn initialize_report_round_trip_includes_required_fields() {
        let value = InitializeReport {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec![
                "validate_configuration".to_string(),
                "create_team".to_string(),
            ],
            failed_step: Some("launch_sessions".to_string()),
            retryable: true,
            message: "launch failed for one member".to_string(),
            steps: vec![
                StepProgress {
                    step: "validate_configuration".to_string(),
                    status: StepStatus::Succeeded,
                    message: Some("ok".to_string()),
                },
                StepProgress {
                    step: "launch_sessions".to_string(),
                    status: StepStatus::Failed,
                    message: Some("codex binary missing".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&value).expect("serialize init report");
        let decoded: InitializeReport =
            serde_json::from_str(&json).expect("deserialize init report");
        assert_eq!(decoded, value);
        assert_eq!(decoded.failed_step, Some("launch_sessions".to_string()));
        assert!(decoded.retryable);
    }

    #[test]
    fn add_agent_request_and_report_round_trip() {
        let request = AddAgentRequest {
            team_name: "architecture-final".to_string(),
            agent: AgentSetupConfig {
                name: "backend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-api".to_string(),
                description: Some("API ownership".to_string()),
            },
        };
        let req_json = serde_json::to_string(&request).expect("serialize add-agent request");
        let req_decoded: AddAgentRequest =
            serde_json::from_str(&req_json).expect("deserialize add-agent request");
        assert_eq!(req_decoded, request);

        let report = AddAgentReport {
            team_name: "architecture-final".to_string(),
            member_name: "backend-dev".to_string(),
            succeeded_steps: vec![
                "create_pane".to_string(),
                "launch_session".to_string(),
                "join_mesh".to_string(),
            ],
            failed_step: None,
            retryable: false,
            message: "agent added".to_string(),
            steps: vec![StepProgress {
                step: "join_mesh".to_string(),
                status: StepStatus::Succeeded,
                message: Some("joined".to_string()),
            }],
        };
        let report_json = serde_json::to_string(&report).expect("serialize add-agent report");
        let report_decoded: AddAgentReport =
            serde_json::from_str(&report_json).expect("deserialize add-agent report");
        assert_eq!(report_decoded, report);
    }

    #[test]
    fn live_team_status_round_trip() {
        let value = LiveTeamStatus {
            team_name: "architecture-final".to_string(),
            lead_name: "team-lead".to_string(),
            members: vec![
                LiveAgentStatus {
                    name: "team-lead".to_string(),
                    role: AgentRole::Lead,
                    cli_tool: "claude".to_string(),
                    model: "opus".to_string(),
                    project_id: "proj-core".to_string(),
                    description: Some("orchestrates work".to_string()),
                    session_status: SessionStatus::Active,
                    pane_id: Some("%1".to_string()),
                },
                LiveAgentStatus {
                    name: "frontend-dev".to_string(),
                    role: AgentRole::Member,
                    cli_tool: "codex".to_string(),
                    model: "gpt-5.3".to_string(),
                    project_id: "proj-web".to_string(),
                    description: None,
                    session_status: SessionStatus::Idle,
                    pane_id: Some("%2".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&value).expect("serialize live team status");
        let decoded: LiveTeamStatus =
            serde_json::from_str(&json).expect("deserialize live team status");
        assert_eq!(decoded, value);
    }

    #[test]
    fn step_progress_event_round_trip() {
        let value = StepProgressEvent {
            team_name: "architecture-final".to_string(),
            operation: "initialize_team".to_string(),
            progress: StepProgress {
                step: "send_onboarding".to_string(),
                status: StepStatus::Running,
                message: Some("sending to frontend-dev".to_string()),
            },
        };

        let json = serde_json::to_string(&value).expect("serialize step progress event");
        let decoded: StepProgressEvent =
            serde_json::from_str(&json).expect("deserialize step progress event");
        assert_eq!(decoded, value);
    }
}
