use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::MemberActivationContext;
use crate::coordination::requests::{
    AddAgentReport, AgentSetupConfig, InitializeReport, MemberActivationStage, ResumeAgentReport,
    StepProgress, StepStatus,
};
use crate::coordination::runtime::{CoordinationRuntime, DetectedRuntimeSession};
use crate::coordination::stores::MemberRuntimeRecord;
use crate::coordination::validation::{validate_member_name, validate_non_empty};
use crate::daemon::protocol::LaunchMode;
use crate::models::CliCommandSettings;
use crate::session_scanner::claude_accounts::{configured_root_to_name, to_launch_namespace};
use crate::session_scanner::cli_tool::{spec, CliTool};
use crate::session_scanner::control::validate_command_override;
use crate::session_scanner::launch::{
    base_command, redact_command_for_logging, LaunchNote, LaunchSpec, ModelSpec, TeamContext,
};

const TMUX_SEND_RETRY_DELAYS: [Duration; 2] =
    [Duration::from_millis(150), Duration::from_millis(350)];

#[derive(Debug, Default, Clone)]
pub(super) struct MemberActivationRuntimeState {
    pub(super) pane_id: Option<String>,
    pub(super) pane_pid: Option<u32>,
    pub(super) pane_start_time: Option<u64>,
    pub(super) session_id: Option<String>,
    pub(super) jsonl_path: Option<PathBuf>,
    pub(super) daemon_pid: Option<u32>,
    pub(super) new_daemon_pid: Option<u32>,
    pub(super) created_pane_id: Option<String>,
    pub(super) reused_pane: bool,
    pub(super) foreign_daemon_stopped: bool,
    pub(super) attached_at: Option<chrono::DateTime<Utc>>,
    pub(super) health: Option<HealthState>,
    pub(super) mesh_joined: bool,
    pub(super) member_added: bool,
}

pub(super) type PendingRuntimeState = MemberActivationRuntimeState;
pub(super) type PendingResumeState = MemberActivationRuntimeState;
pub(crate) type InitializeProgressEmitter<'a> = &'a mut dyn FnMut(&str, StepStatus, Option<String>);
pub(crate) type ResumeProgressEmitter<'a> =
    &'a mut dyn FnMut(&str, usize, usize, MemberActivationStage, StepStatus, Option<String>);

#[derive(Debug, Default, Clone)]
pub(super) struct RuntimeCommitPatch {
    pub(super) pane_id: Option<Option<String>>,
    pub(super) pane_pid: Option<Option<u32>>,
    pub(super) pane_start_time: Option<Option<u64>>,
    pub(super) session_id: Option<Option<String>>,
    pub(super) jsonl_path: Option<Option<PathBuf>>,
    pub(super) daemon_pid: Option<Option<u32>>,
    pub(super) attached_at: Option<Option<chrono::DateTime<Utc>>>,
    pub(super) health: Option<HealthState>,
}

impl RuntimeCommitPatch {
    pub(super) fn from_pending_runtime_state(state: &PendingRuntimeState) -> Self {
        Self {
            pane_id: Some(state.pane_id.clone()),
            pane_pid: Some(state.pane_pid),
            pane_start_time: Some(state.pane_start_time),
            session_id: Some(state.session_id.clone()),
            jsonl_path: Some(state.jsonl_path.clone()),
            daemon_pid: Some(state.daemon_pid),
            attached_at: Some(state.attached_at),
            health: state.health,
        }
    }

    pub(super) fn from_pending_resume_state(
        state: &PendingResumeState,
        attached_at: chrono::DateTime<Utc>,
        health: HealthState,
    ) -> Self {
        Self {
            pane_id: Some(state.pane_id.clone()),
            pane_pid: Some(state.pane_pid),
            pane_start_time: Some(state.pane_start_time),
            session_id: Some(state.session_id.clone()),
            jsonl_path: Some(state.jsonl_path.clone()),
            daemon_pid: Some(state.daemon_pid),
            attached_at: Some(Some(attached_at)),
            health: Some(health),
        }
    }
}

pub(super) enum MemberSessionPhase<'a> {
    LaunchOnly(&'a CliCommandSettings),
    CaptureOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemberDaemonStartPolicy {
    StartFresh,
    ReplaceStalePid { previous_daemon_pid: Option<u32> },
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
        schema_version: 3,
        member_name: member_name.to_string(),
        cli_tool: None,
        project_path: None,
        pane_id: None,
        pane_pid: None,
        pane_start_time: None,
        session_id: None,
        jsonl_path: None,
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
    render_team_launch_command(
        cli_commands,
        cli_tool,
        &agent.model,
        agent.reasoning_effort.as_deref(),
        team_name,
        &agent.name,
        role,
        cli_commands.codex_bypass_hook_trust,
    )
}

/// Build the CLI launch command for a resumed member session.
///
/// Always starts a fresh session — never uses `--continue` or `resume --last`.
/// Multiple agents share the same project, so checkpoint-based resume would
/// pick up another agent's checkpoint and cause confusion.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn build_resume_cli_launch_command(
    agent: &AgentSetupConfig,
    team_name: &str,
    role: MemberRole,
    cli_commands: &CliCommandSettings,
) -> Result<String, CoordinationError> {
    build_cli_launch_command(agent, team_name, role, cli_commands)
}

pub(super) fn run_member_session_phase(
    runtime: &dyn CoordinationRuntime,
    context: &MemberActivationContext,
    pane_id: &str,
    phase: MemberSessionPhase<'_>,
) -> Result<DetectedRuntimeSession, CoordinationError> {
    match phase {
        MemberSessionPhase::LaunchOnly(cli_commands) => {
            let launch_cmd = build_member_activation_launch_command(context, cli_commands)?;
            send_launch_command_with_retry(runtime, pane_id, launch_cmd.as_str())?;
            Ok(DetectedRuntimeSession::default())
        }
        MemberSessionPhase::CaptureOnly => {
            detect_member_session_identity(runtime, context, pane_id)
        }
    }
}

pub(super) fn capture_member_pane_identity(
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
    runtime_state: &mut MemberActivationRuntimeState,
) -> Result<(), CoordinationError> {
    runtime_state.pane_pid = None;
    runtime_state.pane_start_time = None;
    let live_pane = match runtime.live_pane(pane_id) {
        Ok(Some(live_pane)) if !live_pane.is_dead => live_pane,
        Ok(Some(_)) => {
            tracing::warn!(
                pane_id = %pane_id,
                "launched tmux pane was dead before optional identity capture"
            );
            return Ok(());
        }
        Ok(None) => {
            tracing::warn!(
                pane_id = %pane_id,
                "launched tmux pane disappeared before optional identity capture"
            );
            return Ok(());
        }
        Err(error) => {
            tracing::warn!(
                pane_id = %pane_id,
                error = %error,
                "failed to capture optional tmux pane identity"
            );
            return Ok(());
        }
    };
    if live_pane.pane_pid.is_none() {
        tracing::warn!(
            pane_id = %pane_id,
            "tmux pane identity capture returned no pane pid"
        );
    }
    runtime_state.pane_pid = live_pane.pane_pid;
    runtime_state.pane_start_time = live_pane.pane_start_time;
    Ok(())
}

pub(super) fn should_use_mesh_sidecar_for_cli_tool(cli_tool: CliTool) -> bool {
    !spec(cli_tool).capabilities.native_inbox_poller
}

pub(super) fn should_use_mesh_sidecar(agent: &AgentSetupConfig) -> Result<bool, CoordinationError> {
    Ok(should_use_mesh_sidecar_for_cli_tool(parse_cli_tool(
        &agent.cli_tool,
    )?))
}

pub(super) fn join_mesh_if_required(
    runtime: &dyn CoordinationRuntime,
    team_name: &str,
    member_name: &str,
    project_id: &str,
    role: MemberRole,
    cli_tool: CliTool,
    model: &str,
) -> Result<bool, CoordinationError> {
    if spec(cli_tool).capabilities.native_inbox_poller && role != MemberRole::Lead {
        return Ok(false);
    }

    let member_type = if role == MemberRole::Lead {
        "lead"
    } else {
        "general-purpose"
    };
    let claude_dir = crate::coordination::runtime::resolve_mesh_cli_claude_dir_arg()
        .ok_or_else(|| CoordinationError::Backend("Claude config directory unavailable".into()))?;
    runtime.join_mesh(
        team_name,
        member_name,
        project_id,
        member_type,
        model,
        claude_dir.as_str(),
    )?;
    Ok(true)
}

pub(super) fn start_member_daemon_if_required(
    runtime: &dyn CoordinationRuntime,
    team_name: &str,
    member_name: &str,
    pane_id: &str,
    cli_tool: CliTool,
    policy: MemberDaemonStartPolicy,
    warnings: Option<&mut Vec<String>>,
) -> Result<Option<u32>, CoordinationError> {
    if !should_use_mesh_sidecar_for_cli_tool(cli_tool) {
        return Ok(None);
    }

    if let MemberDaemonStartPolicy::ReplaceStalePid {
        previous_daemon_pid: Some(pid),
    } = policy
    {
        if let Err(err) = runtime.terminate_process_by_pid(pid) {
            if let Some(warnings) = warnings {
                warnings.push(format!("failed to terminate stale daemon pid {pid}: {err}"));
            }
        }
    }

    let pid = runtime.spawn_mesh_daemon(pane_id, team_name, member_name)?;
    tracing::info!(
        team = %team_name,
        member = %member_name,
        pane_id = %pane_id,
        pid = pid,
        "mesh daemon started"
    );
    Ok(Some(pid))
}

pub(super) fn send_launch_command_with_retry(
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
    launch_cmd: &str,
) -> Result<(), CoordinationError> {
    let mut last_err = None;

    for (attempt, retry_delay) in TMUX_SEND_RETRY_DELAYS
        .iter()
        .copied()
        .map(Some)
        .chain(std::iter::once(None))
        .enumerate()
    {
        match runtime.send_tmux_keys_with_enter(pane_id, launch_cmd) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if let Some(delay) = retry_delay {
                    tracing::warn!(
                        pane_id = %pane_id,
                        attempt = attempt + 1,
                        retry_ms = delay.as_millis() as u64,
                        "tmux send-keys failed during launch; retrying"
                    );
                    thread::sleep(delay);
                }
            }
        }
    }

    let err = last_err.unwrap_or_else(|| {
        CoordinationError::Backend("tmux send-keys failed without error detail".to_string())
    });
    Err(CoordinationError::Backend(format!(
        "{err}; pane diagnostics: {}",
        pane_launch_diagnostics(runtime, pane_id)
    )))
}

pub(super) fn build_member_activation_launch_command(
    context: &MemberActivationContext,
    cli_commands: &CliCommandSettings,
) -> Result<String, CoordinationError> {
    render_team_launch_command(
        cli_commands,
        context.member.cli_tool,
        &context.member.model,
        context.member.reasoning_effort.as_deref(),
        &context.team_name,
        &context.member.name,
        context.member.role,
        cli_commands.codex_bypass_hook_trust,
    )
}

// Runtime-only Codex inputs on `CliCommandSettings` keep command rendering
// independent from ambient CODEX_HOME state; grouping the stable team/member
// fields would add a second launch context type for this single renderer.
#[allow(clippy::too_many_arguments)]
fn render_team_launch_command(
    cli_commands: &CliCommandSettings,
    cli_tool: CliTool,
    model: &str,
    reasoning_effort: Option<&str>,
    team_name: &str,
    agent_name: &str,
    role: MemberRole,
    codex_bypass_hook_trust: bool,
) -> Result<String, CoordinationError> {
    let base = base_command(cli_commands, cli_tool, LaunchMode::Fresh);
    if base.trim().is_empty() {
        return Err(CoordinationError::Validation(format!(
            "configured launch command is empty for '{}'",
            cli_tool
        )));
    }

    let mut model = ModelSpec::parse_legacy(model);
    if reasoning_effort.is_some() {
        model.reasoning_effort = reasoning_effort.map(str::to_string);
    }
    // Team members stay on the default config dir: agent inboxes live under
    // `PlatformPaths::teams_dir()`, and v1 does not move a member to another
    // subscription. That root is only implicit while it *is* the dir Claude
    // Code reads on its own — `TAURHAUS_CLAUDE_DIR` moves it, and a member
    // launched without the assignment writes its inbox where no team reads.
    let capabilities = spec(cli_tool).capabilities;
    let team_config_dir = capabilities
        .config_dir_env
        .is_some()
        .then(configured_root_to_name)
        .flatten()
        .map(|dir| to_launch_namespace(&dir));
    let rendered = LaunchSpec {
        tool: cli_tool,
        mode: LaunchMode::Fresh,
        base,
        model: model.clone(),
        codex_bypass_hook_trust: capabilities.hook_trust && codex_bypass_hook_trust,
        codex_notify_executable: if capabilities.notify_sink {
            cli_commands.codex_notify_executable.as_deref()
        } else {
            None
        },
        claude_config_dir: team_config_dir.as_deref(),
        team: capabilities.team_flags.then_some(TeamContext {
            team_name,
            agent_name,
            role,
        }),
    }
    .render();
    validate_command_override(&rendered.command).map_err(CoordinationError::Validation)?;

    let mut fields = Map::new();
    fields.insert("team".to_string(), Value::String(team_name.to_string()));
    fields.insert("member".to_string(), Value::String(agent_name.to_string()));
    fields.insert("tool".to_string(), Value::String(cli_tool.to_string()));
    fields.insert("mode".to_string(), Value::String("fresh".to_string()));
    fields.insert(
        "model".to_string(),
        model.model.map(Value::String).unwrap_or(Value::Null),
    );
    fields.insert(
        "reasoning_effort".to_string(),
        model
            .reasoning_effort
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    fields.insert(
        "command".to_string(),
        Value::String(redact_command_for_logging(&rendered.command)),
    );
    emit_global(
        "info",
        "coordination",
        "launch.command.rendered",
        Some("Rendered team member launch command".to_string()),
        fields,
    );

    for note in rendered.notes {
        let event = note.event_name();
        let mut fields = Map::new();
        fields.insert("team".to_string(), Value::String(team_name.to_string()));
        fields.insert("member".to_string(), Value::String(agent_name.to_string()));
        fields.insert("tool".to_string(), Value::String(cli_tool.to_string()));
        let message = match note {
            LaunchNote::DeprecatedFlag { flag } => {
                fields.insert("flag".to_string(), Value::String(flag));
                "Configured launch base contains a deprecated flag"
            }
            LaunchNote::ModelIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides the role model"
            }
            LaunchNote::NotifyIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides the managed Codex notifier"
            }
            LaunchNote::ModelDeprecated { found, replacement } => {
                fields.insert("found".to_string(), Value::String(found));
                fields.insert(
                    "replacement".to_string(),
                    replacement.map(Value::String).unwrap_or(Value::Null),
                );
                "Role model is deprecated"
            }
            LaunchNote::EffortIgnored { found, .. } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides or cannot use the role reasoning effort"
            }
            LaunchNote::ConfigDirIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base selects its own Claude config dir"
            }
        };
        emit_global(
            "warn",
            "coordination",
            event,
            Some(message.to_string()),
            fields,
        );
    }

    Ok(rendered.command)
}

fn detect_member_session_identity(
    runtime: &dyn CoordinationRuntime,
    context: &MemberActivationContext,
    pane_id: &str,
) -> Result<DetectedRuntimeSession, CoordinationError> {
    if !spec(context.member.cli_tool).capabilities.session_source {
        return Ok(DetectedRuntimeSession::default());
    }

    runtime.detect_runtime_session(pane_id, context.member.cli_tool)
}

pub(super) fn has_non_empty_capabilities(capabilities: Option<&[String]>) -> bool {
    capabilities
        .map(|items| items.iter().any(|item| !item.trim().is_empty()))
        .unwrap_or(false)
}

fn has_non_empty_list(items: Option<&[String]>) -> bool {
    items
        .map(|values| values.iter().any(|value| !value.trim().is_empty()))
        .unwrap_or(false)
}

fn pane_launch_diagnostics(runtime: &dyn CoordinationRuntime, pane_id: &str) -> String {
    let exists = bool_diagnostic(runtime.pane_exists(pane_id));
    let dead = bool_diagnostic(runtime.pane_is_dead(pane_id));
    let shell = bool_diagnostic(runtime.pane_is_shell(pane_id));
    let command = option_diagnostic(runtime.pane_current_command(pane_id));
    format!("pane={pane_id} exists={exists} dead={dead} shell={shell} command={command}")
}

fn bool_diagnostic(result: Result<bool, CoordinationError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(err) => format!("error({err})"),
    }
}

fn option_diagnostic(result: Result<Option<String>, CoordinationError>) -> String {
    match result {
        Ok(Some(value)) => value,
        Ok(None) => "none".to_string(),
        Err(err) => format!("error({err})"),
    }
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
            .communication_style
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        || agent.runtime_compact_summary.is_some()
        || agent
            .behavioral_contract
            .as_ref()
            .map(|contract| {
                !contract.communication.is_empty()
                    || !contract.execution.is_empty()
                    || !contract.escalation.is_empty()
            })
            .unwrap_or(false)
        || has_non_empty_list(agent.quality_gates.as_deref())
        || has_non_empty_list(agent.handoff_expectations.as_deref())
        || has_non_empty_list(agent.definition_of_done.as_deref())
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
            .communication_style
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        || member.runtime_compact_summary.is_some()
        || member
            .behavioral_contract
            .as_ref()
            .map(|contract| {
                !contract.communication.is_empty()
                    || !contract.execution.is_empty()
                    || !contract.escalation.is_empty()
            })
            .unwrap_or(false)
        || has_non_empty_list(member.quality_gates.as_deref())
        || has_non_empty_list(member.handoff_expectations.as_deref())
        || has_non_empty_list(member.definition_of_done.as_deref())
        || has_non_empty_capabilities(member.capabilities.as_deref())
}

pub(super) fn member_from_agent_setup(
    setup: &AgentSetupConfig,
    role: MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    let mut declared_model = ModelSpec::parse_legacy(&setup.model);
    if setup.reasoning_effort.is_some() {
        declared_model.reasoning_effort = setup.reasoning_effort.clone();
    }
    Ok(Member {
        name: setup.name.clone(),
        role,
        role_id: setup.role_id.clone(),
        role_name: setup.role_name.clone(),
        focus_area: setup.focus_area.clone(),
        context_summary: setup.context_summary.clone(),
        behavior_summary: setup.behavior_summary.clone(),
        communication_style: setup.communication_style.clone(),
        runtime_compact_summary: setup.runtime_compact_summary.clone(),
        instructions: setup
            .instructions
            .clone()
            .or_else(|| setup.description.clone()),
        behavioral_contract: setup.behavioral_contract.clone(),
        quality_gates: setup.quality_gates.clone(),
        handoff_expectations: setup.handoff_expectations.clone(),
        definition_of_done: setup.definition_of_done.clone(),
        phase_scope: setup.phase_scope.clone(),
        mode: setup.mode.clone(),
        inherits_from: setup.inherits_from.clone(),
        required_artifacts: setup.required_artifacts.clone(),
        capabilities: setup.capabilities.clone(),
        model: declared_model.model,
        reasoning_effort: declared_model.reasoning_effort,
        project_path: PathBuf::from(&setup.project_id),
        cli_tool: parse_cli_tool(&setup.cli_tool)?,
        extra: Default::default(),
    })
}
