use std::path::PathBuf;

use chrono::Utc;

use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddAgentReport, AgentSetupConfig, InitializeReport, ResumeAgentReport, ResumeContextMode,
    StepProgress, StepStatus,
};
use crate::coordination::stores::MemberRuntimeRecord;
use crate::coordination::validation::{validate_member_name, validate_non_empty};
use crate::daemon::protocol::LaunchMode as DaemonLaunchMode;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::{
    build_team_launch_command, resolve_configured_tool_command, validate_command_override,
};

#[derive(Debug, Default, Clone)]
pub(super) struct PendingRuntimeState {
    pub(super) pane_id: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) daemon_pid: Option<u32>,
    pub(super) attached_at: Option<chrono::DateTime<Utc>>,
    pub(super) health: Option<HealthState>,
    pub(super) mesh_joined: bool,
    pub(super) member_added: bool,
}

#[derive(Debug, Default, Clone)]
pub(super) struct PendingResumeState {
    pub(super) pane_id: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) daemon_pid: Option<u32>,
    pub(super) new_daemon_pid: Option<u32>,
    pub(super) created_pane_id: Option<String>,
    pub(super) reused_pane: bool,
    pub(super) mesh_joined: bool,
}

pub(super) fn mark_step_succeeded(
    step: &str,
    message: &str,
    succeeded_steps: &mut Vec<String>,
    steps: &mut Vec<StepProgress>,
) {
    succeeded_steps.push(step.to_string());
    steps.push(StepProgress {
        step: step.to_string(),
        status: StepStatus::Succeeded,
        message: Some(message.to_string()),
    });
}

pub(super) fn failed_initialize_report(
    team_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> InitializeReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    InitializeReport {
        team_name: team_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

pub(super) fn failed_add_agent_report(
    team_name: &str,
    member_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> AddAgentReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    AddAgentReport {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn failed_resume_report(
    team_name: &str,
    member_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
    warnings: Vec<String>,
    pane_id: Option<String>,
    reused_pane: bool,
) -> ResumeAgentReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    ResumeAgentReport {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        resumed: false,
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
        warnings,
        pane_id,
        reused_pane,
    }
}

pub(super) fn parse_cli_tool(raw: &str) -> Result<CliTool, CoordinationError> {
    CliTool::from_alias(raw).map_err(|err| CoordinationError::Validation(err.to_string()))
}

pub(super) fn default_runtime_record(member_name: &str) -> MemberRuntimeRecord {
    MemberRuntimeRecord {
        schema_version: 1,
        member_name: member_name.to_string(),
        pane_id: None,
        session_id: None,
        daemon_pid: None,
        health: HealthState::SessionDead,
        delivery_lease: None,
        attached_at: None,
        last_seen_at: None,
    }
}

pub(super) fn build_cli_launch_command(
    agent: &AgentSetupConfig,
    team_name: &str,
    role: MemberRole,
    cli_commands: &CliCommandSettings,
) -> Result<String, CoordinationError> {
    let cli_tool = parse_cli_tool(&agent.cli_tool)?;
    let command = build_team_launch_command(cli_commands, cli_tool, &agent.model);
    if command.trim().is_empty() {
        return Err(CoordinationError::Validation(format!(
            "configured launch command is empty for '{}'",
            agent.cli_tool
        )));
    }
    validate_command_override(&command).map_err(CoordinationError::Validation)?;

    if cli_tool != CliTool::Claude {
        return Ok(command);
    }

    Ok(with_claude_team_context(
        command,
        team_name,
        &agent.name,
        role,
    ))
}

pub(super) fn build_resume_cli_launch_command(
    agent: &AgentSetupConfig,
    team_name: &str,
    role: MemberRole,
    context_mode: ResumeContextMode,
    cli_commands: &CliCommandSettings,
) -> Result<String, CoordinationError> {
    let cli_tool = parse_cli_tool(&agent.cli_tool)?;
    let command = match context_mode {
        ResumeContextMode::Fresh => build_team_launch_command(cli_commands, cli_tool, &agent.model),
        ResumeContextMode::Continue => {
            let mode = if cli_tool == CliTool::Claude {
                DaemonLaunchMode::Continue
            } else {
                DaemonLaunchMode::Resume
            };
            resolve_configured_tool_command(cli_commands, cli_tool, mode)
        }
    };
    if command.trim().is_empty() {
        return Err(CoordinationError::Validation(format!(
            "configured resume command is empty for '{}'",
            agent.cli_tool
        )));
    }
    validate_command_override(&command).map_err(CoordinationError::Validation)?;

    if cli_tool != CliTool::Claude {
        return Ok(command);
    }

    Ok(with_claude_team_context(
        command,
        team_name,
        &agent.name,
        role,
    ))
}

pub(super) fn should_use_mesh_sidecar(agent: &AgentSetupConfig) -> Result<bool, CoordinationError> {
    Ok(parse_cli_tool(&agent.cli_tool)? != CliTool::Claude)
}

pub(super) fn with_claude_team_context(
    mut command: String,
    team_name: &str,
    agent_name: &str,
    role: MemberRole,
) -> String {
    if !command.contains("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=") {
        command = format!("CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 {command}");
    }

    if !command_contains_flag(&command, "--team-name") {
        command.push_str(" --team-name ");
        command.push_str(&shell_escape_for_cmd(team_name));
    }
    if !command_contains_flag(&command, "--agent-name") {
        command.push_str(" --agent-name ");
        command.push_str(&shell_escape_for_cmd(agent_name));
    }
    if !command_contains_flag(&command, "--agent-id") {
        command.push_str(" --agent-id ");
        command.push_str(&shell_escape_for_cmd(&format!("{agent_name}@{team_name}")));
    }
    if !command_contains_flag(&command, "--agent-type") {
        let agent_type = if role == MemberRole::Lead {
            "orchestrator"
        } else {
            "general-purpose"
        };
        command.push_str(" --agent-type ");
        command.push_str(&shell_escape_for_cmd(agent_type));
    }

    command
}

pub(super) fn command_contains_flag(command: &str, flag: &str) -> bool {
    command.split_whitespace().any(|token| {
        token == flag
            || token
                .strip_prefix(flag)
                .is_some_and(|suffix| suffix.starts_with('='))
    })
}

pub(super) fn shell_escape_for_cmd(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '@'))
    {
        return value.to_string();
    }

    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(super) fn has_non_empty_capabilities(capabilities: Option<&[String]>) -> bool {
    capabilities
        .map(|items| items.iter().any(|item| !item.trim().is_empty()))
        .unwrap_or(false)
}

pub(super) fn agent_instructions(agent: &AgentSetupConfig) -> Option<&str> {
    agent
        .instructions
        .as_deref()
        .or(agent.description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn agent_has_role_context(agent: &AgentSetupConfig) -> bool {
    agent
        .role_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || agent_instructions(agent).is_some()
        || agent
            .behavioral_contract
            .as_ref()
            .map(|contract| {
                !contract.communication.is_empty()
                    || !contract.execution.is_empty()
                    || !contract.escalation.is_empty()
            })
            .unwrap_or(false)
        || has_non_empty_capabilities(agent.capabilities.as_deref())
}

pub(super) fn member_has_role_context(member: &Member) -> bool {
    member
        .role_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || member
            .instructions
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        || member
            .behavioral_contract
            .as_ref()
            .map(|contract| {
                !contract.communication.is_empty()
                    || !contract.execution.is_empty()
                    || !contract.escalation.is_empty()
            })
            .unwrap_or(false)
        || has_non_empty_capabilities(member.capabilities.as_deref())
}

pub(super) fn member_from_agent_setup(
    setup: &AgentSetupConfig,
    role: MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    Ok(Member {
        name: setup.name.clone(),
        role,
        role_id: setup.role_id.clone(),
        role_name: setup.role_name.clone(),
        focus_area: setup.focus_area.clone(),
        context_summary: setup.context_summary.clone(),
        behavior_summary: setup.behavior_summary.clone(),
        instructions: setup
            .instructions
            .clone()
            .or_else(|| setup.description.clone()),
        behavioral_contract: setup.behavioral_contract.clone(),
        capabilities: setup.capabilities.clone(),
        project_path: PathBuf::from(&setup.project_id),
        cli_tool: parse_cli_tool(&setup.cli_tool)?,
    })
}
