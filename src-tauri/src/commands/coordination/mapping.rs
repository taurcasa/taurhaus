use std::path::PathBuf;

use super::request_normalization;
use crate::commands::coordination_types::*;
use crate::coordination::backend::bridged::{
    AvailabilityReport as BackendAvailabilityReport, PreflightAgent,
    PreflightReport as BackendPreflightReport,
};
use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{self as contracts};
use crate::session_scanner::cli_tool::CliTool;

pub(super) fn validate_and_collect_preflight_agents(
    request: InitializeTeamRequest,
) -> Result<Vec<PreflightAgent>, String> {
    request_normalization::validate_and_collect_preflight_agents(request)
}

pub(super) fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

pub(super) fn validate_initialize_request_fields(
    request: &InitializeTeamRequest,
) -> Result<(), String> {
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

pub(super) fn map_lead_mode_to_contract(mode: LeadMode) -> contracts::LeadMode {
    match mode {
        LeadMode::AttachExisting => contracts::LeadMode::AttachExisting,
        LeadMode::LaunchNew => contracts::LeadMode::LaunchNew,
    }
}

pub(super) fn map_step_status_from_contract(status: contracts::StepStatus) -> StepStatus {
    match status {
        contracts::StepStatus::Pending => StepStatus::Pending,
        contracts::StepStatus::Running => StepStatus::Running,
        contracts::StepStatus::Succeeded => StepStatus::Succeeded,
        contracts::StepStatus::Failed => StepStatus::Failed,
    }
}

pub(super) fn map_agent_setup_to_contract(agent: &AgentSetupConfig) -> contracts::AgentSetupConfig {
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
        runtime_compact_summary: agent.runtime_compact_summary.clone(),
        instructions: agent.instructions.clone(),
        behavioral_contract: agent.behavioral_contract.clone(),
        capabilities: agent.capabilities.clone(),
    }
}

pub(super) fn map_step_progress_from_contract(progress: contracts::StepProgress) -> StepProgress {
    StepProgress {
        step: progress.step,
        status: map_step_status_from_contract(progress.status),
        message: progress.message,
    }
}

pub(super) fn map_initialize_request_to_contract(
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

pub(super) fn map_add_agent_request_to_contract(
    request: &AddAgentRequest,
) -> contracts::AddAgentRequest {
    contracts::AddAgentRequest {
        team_name: request.team_name.clone(),
        agent: map_agent_setup_to_contract(&request.agent),
    }
}

pub(super) fn map_resume_member_request_to_contract(
    request: &ResumeMemberRequest,
) -> contracts::ResumeMemberRequest {
    contracts::ResumeMemberRequest {
        team_name: request.team_name.clone(),
        member_name: request.member_name.clone(),
    }
}

pub(super) fn map_resume_team_request_to_contract(
    request: &ResumeTeamRequest,
) -> contracts::ResumeTeamRequest {
    contracts::ResumeTeamRequest {
        team_name: request.team_name.clone(),
    }
}

pub(super) fn map_initialize_report_from_contract(
    report: contracts::InitializeReport,
) -> InitializeReport {
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

pub(super) fn map_add_agent_report_from_contract(
    report: contracts::AddAgentReport,
) -> AddAgentReport {
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

pub(super) fn map_resume_agent_report_from_contract(
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

pub(super) fn map_resume_team_report_from_contract(
    report: contracts::ResumeTeamReport,
) -> ResumeTeamReport {
    ResumeTeamReport {
        team_name: report.team_name,
        resumed: report.resumed,
        total_members: report.total_members,
        resumed_members: report.resumed_members,
        failed_members: report
            .failed_members
            .into_iter()
            .map(|failure| ResumeTeamMemberFailure {
                member_name: failure.member_name,
                message: failure.message,
                retryable: failure.retryable,
            })
            .collect(),
        warnings: report.warnings,
        started_team_daemon: report.started_team_daemon,
        team_daemon_warning: report.team_daemon_warning,
    }
}

pub(super) fn map_preflight_report(report: BackendPreflightReport) -> PreflightReport {
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

pub(super) fn map_feature_availability_report(
    report: BackendAvailabilityReport,
) -> FeatureAvailabilityReport {
    FeatureAvailabilityReport {
        can_initialize: report.can_initialize(),
        mesh_available: report.mesh_available,
        tmux_available: report.tmux_available,
        blocking_errors: report.blocking_errors,
    }
}

pub(super) fn cli_tool_from_backend_kind(backend_kind: &str) -> Result<CliTool, CoordinationError> {
    CliTool::from_alias(backend_kind).map_err(|_| {
        CoordinationError::Validation(format!(
            "unsupported backend_kind '{}'",
            backend_kind.trim()
        ))
    })
}

pub(super) fn resolve_legacy_member_project_path(
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
        .find(|member| member.role == crate::coordination::domain::MemberRole::Lead)
        .or_else(|| existing_members.first())
        .map(|member| member.project_path.clone())
        .ok_or_else(|| {
            CoordinationError::Validation(
                "project_path must be provided for legacy add-member when team has no members"
                    .to_string(),
            )
        })
}

pub(super) fn map_coordination_error(err: CoordinationError) -> String {
    err.to_string()
}
