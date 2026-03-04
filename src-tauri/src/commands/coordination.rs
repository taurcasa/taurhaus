//! Coordination IPC commands for team management (M0 surface).

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

pub use crate::commands::coordination_types::*;
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
use crate::coordination::requests::{DeliveryRequest, DeliveryResult, OperatorNoticeDelivery};
use crate::coordination::state::CoordinationState;
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

#[tauri::command]
pub fn coordination_initialize_team(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: InitializeTeamRequest,
) -> Result<InitializeReport, String> {
    let request = normalize_initialize_request_paths(&db, request)?;
    let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
    coordination_initialize_team_with_emitter_and_layout(
        state.inner(),
        request,
        &cli_commands,
        &tmux_layout,
        |event| {
            let _ = app.emit("coordination-step-progress", event);
        },
    )
}

#[tauri::command]
pub fn coordination_add_agent(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: AddAgentRequest,
) -> Result<AddAgentReport, String> {
    let request = normalize_add_agent_request_path(&db, request)?;
    let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
    coordination_add_agent_with_emitter_and_layout(
        state.inner(),
        request,
        &cli_commands,
        &tmux_layout,
        |event| {
            let _ = app.emit("coordination-step-progress", event);
        },
    )
}

#[tauri::command]
pub fn coordination_resume_member(
    app: AppHandle,
    db: State<'_, DbState>,
    state: State<'_, CoordinationState>,
    request: ResumeMemberRequest,
) -> Result<ResumeAgentReport, String> {
    let (cli_commands, tmux_layout) = load_cli_commands_and_layout(&db);
    coordination_resume_member_with_emitter_and_layout(
        state.inner(),
        request,
        &cli_commands,
        &tmux_layout,
        |event| {
            let _ = app.emit("coordination-step-progress", event);
        },
    )
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
) -> Result<RemoveAgentReport, String> {
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

fn coordination_initialize_team_impl_with_cli_commands_and_layout(
    state: &CoordinationState,
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<InitializeReport, String> {
    state
        .with_orchestrator(|orchestrator| {
            orchestrator.initialize_team_with_cli_commands_and_layout(
                &request,
                cli_commands,
                tmux_layout,
            )
        })
        .map_err(map_coordination_error)
}

#[cfg(test)]
fn coordination_initialize_team_with_emitter<E>(
    state: &CoordinationState,
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    emit: E,
) -> Result<InitializeReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    coordination_initialize_team_with_emitter_and_layout(
        state,
        request,
        cli_commands,
        "new_window",
        emit,
    )
}

fn coordination_initialize_team_with_emitter_and_layout<E>(
    state: &CoordinationState,
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: E,
) -> Result<InitializeReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    validate_initialize_request_fields(&request)?;
    let report = coordination_initialize_team_impl_with_cli_commands_and_layout(
        state,
        request,
        cli_commands,
        tmux_layout,
    )?;
    for event in initialize_progress_events(&report) {
        emit(&event);
    }
    Ok(report)
}

#[cfg(test)]
fn coordination_add_agent_impl(
    state: &CoordinationState,
    request: AddAgentRequest,
) -> Result<AddAgentReport, String> {
    coordination_add_agent_impl_with_cli_commands(state, request, &CliCommandSettings::default())
}

#[cfg(test)]
fn coordination_add_agent_impl_with_cli_commands(
    state: &CoordinationState,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
) -> Result<AddAgentReport, String> {
    coordination_add_agent_impl_with_cli_commands_and_layout(
        state,
        request,
        cli_commands,
        "new_window",
    )
}

fn coordination_add_agent_impl_with_cli_commands_and_layout(
    state: &CoordinationState,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<AddAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("agent.name", &request.agent.name)?;
    validate_non_empty("agent.project_id", &request.agent.project_id)?;
    validate_non_empty("agent.cli_tool", &request.agent.cli_tool)?;
    state
        .with_orchestrator(|orchestrator| {
            orchestrator.add_agent_to_team_with_cli_commands_and_layout(
                &request,
                cli_commands,
                tmux_layout,
            )
        })
        .map_err(map_coordination_error)
}

#[cfg(test)]
fn coordination_add_agent_with_emitter<E>(
    state: &CoordinationState,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    emit: E,
) -> Result<AddAgentReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    coordination_add_agent_with_emitter_and_layout(state, request, cli_commands, "new_window", emit)
}

fn coordination_add_agent_with_emitter_and_layout<E>(
    state: &CoordinationState,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: E,
) -> Result<AddAgentReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    let report = coordination_add_agent_impl_with_cli_commands_and_layout(
        state,
        request,
        cli_commands,
        tmux_layout,
    )?;
    for event in add_agent_progress_events(&report) {
        emit(&event);
    }
    Ok(report)
}

#[cfg(test)]
fn coordination_resume_member_impl(
    state: &CoordinationState,
    request: ResumeMemberRequest,
) -> Result<ResumeAgentReport, String> {
    coordination_resume_member_impl_with_cli_commands_and_layout(
        state,
        request,
        &CliCommandSettings::default(),
        "new_window",
    )
}

fn coordination_resume_member_impl_with_cli_commands_and_layout(
    state: &CoordinationState,
    request: ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<ResumeAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;
    state
        .with_orchestrator(|orchestrator| {
            orchestrator.resume_member_with_cli_commands_and_layout(
                &request,
                cli_commands,
                tmux_layout,
            )
        })
        .map_err(map_coordination_error)
}

fn coordination_resume_member_with_emitter_and_layout<E>(
    state: &CoordinationState,
    request: ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: E,
) -> Result<ResumeAgentReport, String>
where
    E: FnMut(&StepProgressEvent),
{
    let report = coordination_resume_member_impl_with_cli_commands_and_layout(
        state,
        request,
        cli_commands,
        tmux_layout,
    )?;
    for event in resume_member_progress_events(&report) {
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
            orchestrator.get_team_status(&team_name)
        })
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
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    validate_non_empty("backend_kind", &backend_kind)?;

    let cli_tool = cli_tool_from_backend_kind(&backend_kind).map_err(map_coordination_error)?;
    let member = Member {
        name: member_name,
        role: MemberRole::Agent,
        role_id: None,
        instructions: None,
        behavioral_contract: None,
        capabilities: None,
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

#[cfg(not(test))]
fn resolve_project_reference(db: &DbState, project_ref: &str) -> Result<String, String> {
    validate_non_empty("project_id", project_ref)?;
    let trimmed = project_ref.trim();

    let project_path = {
        let conn = db.0.lock().map_err(|err| err.to_string())?;
        match crate::db::queries::get_project(&conn, trimmed).map_err(|err| err.to_string())? {
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
