use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use chrono::Utc;

use crate::commands::coordination_types::{
    AddAgentReport, AddAgentRequest, AgentSetupConfig, InitializeReport, InitializeTeamRequest,
    StepProgress, StepStatus,
};
use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    DeliveryRequest, OperatorNoticeDelivery, TeardownMode, TeardownRequest,
};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
use crate::coordination::validation::{
    validate_member_name, validate_non_empty, validate_team_name,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::{build_team_launch_command, validate_command_override};

impl CoordinationOrchestrator {
    /// Initialize a team via the high-level multi-step pipeline.
    ///
    /// Pipeline steps:
    /// 1. validate_configuration
    /// 2. create_team
    /// 3. add_lead
    /// 4. create_panes
    /// 5. launch_sessions
    /// 6. join_mesh
    /// 7. start_daemons
    /// 8. send_onboarding (render + deliver)
    pub fn initialize_team(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<InitializeReport, CoordinationError> {
        self.initialize_team_with_cli_commands(request, &CliCommandSettings::default())
    }

    pub fn initialize_team_with_cli_commands(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<InitializeReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

        if let Err(err) = self.validate_initialize_configuration(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "validate_configuration",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "validate_configuration",
            "configuration validated",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.create_team(&request.team_name, request.team_description.clone()) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_team",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_team",
            "team created",
            &mut succeeded_steps,
            &mut steps,
        );

        let lead_member = match member_from_agent_setup(&request.lead, MemberRole::Lead) {
            Ok(member) => member,
            Err(err) => {
                self.cleanup_initialize_failure(&request.team_name);
                return Ok(failed_initialize_report(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                ));
            }
        };
        if let Err(err) = self.add_member(&request.team_name, lead_member) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "add_lead",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("add_lead", "lead added", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.create_panes(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_panes",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_panes",
            "agent panes created",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_sessions(request, cli_commands) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "launch_sessions",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "launch_sessions",
            "cli sessions launched",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemons(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "start_daemons",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "start_daemons",
            "mesh daemons started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.send_onboarding_messages(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "send_onboarding",
            "onboarding messages sent",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(InitializeReport {
            team_name: request.team_name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps,
        })
    }

    fn cleanup_initialize_failure(&mut self, team_name: &str) {
        let _ = self.disband_team(
            team_name,
            Some("initialization failed — cleaning up".to_string()),
        );
    }

    fn cleanup_add_agent_failure(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &PendingRuntimeState,
    ) {
        if let Some(pid) = runtime_state.daemon_pid {
            if let Err(err) = terminate_process_by_pid(pid) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.agent.name,
                    pid = pid,
                    error = %err,
                    "hot-add rollback: failed to stop daemon process"
                );
            }
        }

        if runtime_state.mesh_joined {
            if let Err(err) = self.backend.teardown(TeardownRequest {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                mode: TeardownMode::Graceful,
            }) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.agent.name,
                    error = %err,
                    "hot-add rollback: failed to leave mesh"
                );
            }
        }

        if let Some(pane_id) = runtime_state.pane_id.as_deref() {
            if let Err(err) = kill_aitx_pane(pane_id) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.agent.name,
                    pane_id = %pane_id,
                    error = %err,
                    "hot-add rollback: failed to kill pane"
                );
            }
        }

        if runtime_state.member_added {
            if let Err(err) = self.remove_member(
                &request.team_name,
                &request.agent.name,
                Some("hot-add rollback after pipeline failure".to_string()),
            ) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.agent.name,
                    error = %err,
                    "hot-add rollback: failed to remove member from roster"
                );
            }
        }
    }

    /// Hot-add a single agent to an already running team.
    ///
    /// Pipeline steps:
    /// 1. validate
    /// 2. create_pane (stubbed)
    /// 3. launch_session (stubbed)
    /// 4. join_mesh (stubbed)
    /// 5. start_daemon (stubbed)
    /// 6. send_onboarding (render + backend delivery)
    /// 7. update_roster
    pub fn add_agent_to_team(
        &mut self,
        request: &AddAgentRequest,
    ) -> Result<AddAgentReport, CoordinationError> {
        self.add_agent_to_team_with_cli_commands(request, &CliCommandSettings::default())
    }

    pub fn add_agent_to_team_with_cli_commands(
        &mut self,
        request: &AddAgentRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<AddAgentReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();
        let mut runtime_state = PendingRuntimeState::default();

        if let Err(err) = self.validate_add_agent_request(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "validate",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "validate",
            "request validated",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.create_pane_for_agent(request, &mut runtime_state) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "create_pane",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_pane",
            "agent pane created",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_session_for_agent(request, &runtime_state, cli_commands) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "launch_session",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "launch_session",
            "cli session launched",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh_for_agent(request, &mut runtime_state) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemon_for_agent(request, &mut runtime_state) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "start_daemon",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "start_daemon",
            "mesh daemon started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.send_onboarding_for_agent(request) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "send_onboarding",
            "onboarding delivered",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.update_roster_with_agent(request, &mut runtime_state) {
            self.cleanup_add_agent_failure(request, &runtime_state);
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "update_roster",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "update_roster",
            "team roster updated",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(AddAgentReport {
            team_name: request.team_name.clone(),
            member_name: request.agent.name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "agent added".to_string(),
            steps,
        })
    }

    fn validate_initialize_configuration(
        &self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_non_empty("lead name", &request.lead.name)?;
        validate_non_empty("lead cli tool", &request.lead.cli_tool)?;

        let mut seen = std::collections::HashSet::new();
        seen.insert(request.lead.name.trim().to_string());
        for agent in &request.agents {
            validate_non_empty("agent name", &agent.name)?;
            validate_non_empty("agent cli tool", &agent.cli_tool)?;
            let inserted = seen.insert(agent.name.trim().to_string());
            if !inserted {
                return Err(CoordinationError::Validation(format!(
                    "duplicate member name '{}' in initialize request",
                    agent.name
                )));
            }
        }

        Ok(())
    }

    fn create_panes(&mut self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if cfg!(test) {
            return self.create_panes_test_stub(request);
        }

        for agent in &request.agents {
            let member = member_from_agent_setup(agent, MemberRole::Agent)?;
            self.add_member(&request.team_name, member.clone())?;
            let pane_id = self.create_aitx_pane(&agent.project_id)?;

            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)?;
            runtime.pane_id = Some(pane_id);
            runtime.daemon_pid = None;
            runtime.attached_at = Some(Utc::now());
            runtime.health = HealthState::Healthy;
            MemberRuntimeStore::save(&self.teams_dir, &request.team_name, &member.name, &runtime)?;
        }
        Ok(())
    }

    fn create_panes_test_stub(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for (idx, agent) in request.agents.iter().enumerate() {
            let member = member_from_agent_setup(agent, MemberRole::Agent)?;
            self.add_member(&request.team_name, member.clone())?;
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)?;
            runtime.pane_id = Some(format!("%{}", idx + 1));
            runtime.daemon_pid = None;
            runtime.attached_at = Some(Utc::now());
            runtime.health = HealthState::Healthy;
            MemberRuntimeStore::save(&self.teams_dir, &request.team_name, &member.name, &runtime)?;
        }
        Ok(())
    }

    fn launch_sessions(
        &self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        if cfg!(test) {
            return Ok(());
        }
        for agent in &request.agents {
            let runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &agent.name)?;
            let pane_id = runtime.pane_id.ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    agent.name, request.team_name
                ))
            })?;
            self.launch_agent_in_pane(&pane_id, agent, cli_commands)?;
        }
        Ok(())
    }

    fn join_mesh(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if cfg!(test) {
            return Ok(());
        }
        for agent in &request.agents {
            run_mesh(&[
                "join",
                "--team",
                &request.team_name,
                "--name",
                &agent.name,
            ])?;
        }
        Ok(())
    }

    fn start_daemons(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if cfg!(test) {
            return Ok(());
        }
        for agent in &request.agents {
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &agent.name)?;
            let pane_id = runtime.pane_id.clone().ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    agent.name, request.team_name
                ))
            })?;
            let pid = spawn_mesh_daemon(&pane_id, &request.team_name, &agent.name)?;
            runtime.daemon_pid = Some(pid);
            MemberRuntimeStore::save(&self.teams_dir, &request.team_name, &agent.name, &runtime)?;
            tracing::info!(
                team = %request.team_name,
                member = %agent.name,
                pane_id = %pane_id,
                pid = pid,
                "mesh daemon started"
            );
        }
        Ok(())
    }

    fn send_onboarding_messages(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for agent in &request.agents {
            let cli_tool = parse_cli_tool(&agent.cli_tool)?;
            if cli_tool == CliTool::Claude {
                continue;
            }

            let onboarding = DeliveryRenderer::render_onboarding(
                &request.team_name,
                &agent.name,
                &request.lead.name,
            );
            self.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        }
        Ok(())
    }

    fn validate_add_agent_request(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_member_name(&request.agent.name)?;
        validate_non_empty("agent project id", &request.agent.project_id)?;
        validate_non_empty("agent cli tool", &request.agent.cli_tool)?;
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        if config
            .members
            .iter()
            .any(|member| member.name == request.agent.name)
        {
            return Err(CoordinationError::Conflict(format!(
                "member '{}' already exists in team '{}'",
                request.agent.name, request.team_name
            )));
        }
        Ok(())
    }

    fn create_pane_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        if cfg!(test) {
            runtime_state.pane_id = Some("%hot-add-1".to_string());
            runtime_state.attached_at = Some(Utc::now());
            runtime_state.health = Some(HealthState::Healthy);
            return Ok(());
        }

        let pane_id = self.create_aitx_pane(&request.agent.project_id)?;
        runtime_state.pane_id = Some(pane_id);
        runtime_state.attached_at = Some(Utc::now());
        runtime_state.health = Some(HealthState::Healthy);
        Ok(())
    }

    fn launch_session_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &PendingRuntimeState,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        if cfg!(test) {
            return Ok(());
        }

        let pane_id = runtime_state.pane_id.as_deref().ok_or_else(|| {
            CoordinationError::Backend(format!(
                "missing pane id for member '{}' in team '{}'",
                request.agent.name, request.team_name
            ))
        })?;
        self.launch_agent_in_pane(pane_id, &request.agent, cli_commands)?;
        Ok(())
    }

    fn launch_agent_in_pane(
        &self,
        pane_id: &str,
        agent: &AgentSetupConfig,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        let launch_cmd = build_cli_launch_command(agent, cli_commands)?;
        send_tmux_keys_with_enter(pane_id, launch_cmd.as_str())?;
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }

    fn join_mesh_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        if cfg!(test) {
            runtime_state.mesh_joined = true;
            return Ok(());
        }

        run_mesh(&[
            "join",
            "--team",
            &request.team_name,
            "--name",
            &request.agent.name,
        ])?;
        runtime_state.mesh_joined = true;
        Ok(())
    }

    fn start_daemon_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        if cfg!(test) {
            runtime_state.daemon_pid = None;
            return Ok(());
        }

        let pane_id = runtime_state.pane_id.as_deref().ok_or_else(|| {
            CoordinationError::Backend(format!(
                "missing pane id for member '{}' in team '{}'",
                request.agent.name, request.team_name
            ))
        })?;
        let pid = spawn_mesh_daemon(pane_id, &request.team_name, &request.agent.name)?;
        runtime_state.daemon_pid = Some(pid);
        tracing::info!(
            team = %request.team_name,
            member = %request.agent.name,
            pane_id = %pane_id,
            pid = pid,
            "mesh daemon started"
        );
        Ok(())
    }

    fn send_onboarding_for_agent(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let cli_tool = parse_cli_tool(&request.agent.cli_tool)?;
        if cli_tool == CliTool::Claude {
            return Ok(());
        }
        let team = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = team
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        let onboarding = DeliveryRenderer::render_onboarding(
            &request.team_name,
            &request.agent.name,
            &lead_name,
        );
        self.backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        Ok(())
    }

    fn update_roster_with_agent(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        let member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        self.add_member(&request.team_name, member)?;
        runtime_state.member_added = true;

        let mut runtime =
            MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.agent.name)?;
        runtime.pane_id = runtime_state.pane_id.clone();
        runtime.daemon_pid = runtime_state.daemon_pid;
        runtime.attached_at = runtime_state.attached_at;
        runtime.health = runtime_state.health.unwrap_or(HealthState::SessionDead);
        MemberRuntimeStore::save(
            &self.teams_dir,
            &request.team_name,
            &request.agent.name,
            &runtime,
        )?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
struct PendingRuntimeState {
    pane_id: Option<String>,
    daemon_pid: Option<u32>,
    attached_at: Option<chrono::DateTime<Utc>>,
    health: Option<HealthState>,
    mesh_joined: bool,
    member_added: bool,
}

fn mark_step_succeeded(
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

fn failed_initialize_report(
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

fn failed_add_agent_report(
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

fn parse_cli_tool(raw: &str) -> Result<CliTool, CoordinationError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude_native" => Ok(CliTool::Claude),
        "codex" | "mesh" | "mesh_bridged" => Ok(CliTool::Codex),
        "gemini" => Ok(CliTool::Gemini),
        other => Err(CoordinationError::Validation(format!(
            "unsupported cli tool '{other}'"
        ))),
    }
}

fn build_cli_launch_command(
    agent: &AgentSetupConfig,
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
    Ok(command)
}

#[derive(Debug, Clone)]
struct CommandInvocation {
    program: String,
    args: Vec<String>,
}

fn resolve_wsl_home_for_coordination() -> Option<String> {
    let output = wsl_command_for_coordination()
        .args(["--", "sh", "-c", "echo $HOME"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wsl_unix_path_from_stdout(&output.stdout)
}

fn resolve_wsl_binary_path(binary_name: &str) -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    if !binary_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    // Prefer the known install location under ~/.local/bin when available.
    if let Some(home) = resolve_wsl_home_for_coordination() {
        let candidate = format!("{home}/.local/bin/{binary_name}");
        let check = wsl_command_for_coordination()
            .args(["--", "test", "-x", &candidate])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if check.status.success() {
            return Some(candidate);
        }
    }

    let cmd = format!("command -v {binary_name}");
    let output = wsl_command_for_coordination()
        .args(["--", "sh", "-c", &cmd])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wsl_unix_path_from_stdout(&output.stdout)
}

fn parse_wsl_unix_path_from_stdout(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    text.lines()
        .map(str::trim)
        .rev()
        .find(|line| !line.is_empty() && line.starts_with('/'))
        .map(ToString::to_string)
}

fn mesh_binary_path() -> Option<String> {
    if cfg!(target_os = "windows") {
        resolve_wsl_home_for_coordination().map(|home| format!("{home}/.local/bin/mesh"))
    } else {
        dirs::home_dir().map(|home| home.join(".local/bin/mesh").to_string_lossy().to_string())
    }
}

fn aitx_binary_path() -> Option<String> {
    if cfg!(target_os = "windows") {
        resolve_wsl_binary_path("aitx")
    } else {
        None
    }
}

fn command_invocation(program: &str, args: &[String]) -> CommandInvocation {
    if cfg!(target_os = "windows") {
        let mut invocation_args = vec!["-e".to_string(), program.to_string()];
        invocation_args.extend(args.iter().cloned());
        CommandInvocation {
            program: "wsl".to_string(),
            args: invocation_args,
        }
    } else {
        CommandInvocation {
            program: program.to_string(),
            args: args.to_vec(),
        }
    }
}

fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    let mesh_path = mesh_binary_path().unwrap_or_else(|| "mesh".to_string());
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    command_invocation(&mesh_path, &args)
}

fn aitx_command_invocation(args: &[&str]) -> CommandInvocation {
    let aitx_path = aitx_binary_path().unwrap_or_else(|| "aitx".to_string());
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    command_invocation(&aitx_path, &args)
}

fn tmux_command_invocation(args: &[String]) -> CommandInvocation {
    command_invocation("tmux", args)
}

fn wsl_command_for_coordination() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("wsl");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Output, CoordinationError> {
    let output = if invocation.program == "wsl" {
        let mut cmd = wsl_command_for_coordination();
        cmd.args(&invocation.args).output()
    } else {
        Command::new(&invocation.program)
            .args(&invocation.args)
            .output()
    };
    output.map_err(CoordinationError::Io)
}

fn spawn_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Child, CoordinationError> {
    let child = if invocation.program == "wsl" {
        let mut cmd = wsl_command_for_coordination();
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    } else {
        Command::new(&invocation.program)
            .args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    child.map_err(CoordinationError::Io)
}

fn run_mesh(args: &[&str]) -> Result<String, CoordinationError> {
    let invocation = mesh_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "mesh command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

fn run_aitx(args: &[&str]) -> Result<String, CoordinationError> {
    let invocation = aitx_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "aitx command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

fn run_tmux(args: &[String]) -> Result<String, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "tmux command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

fn tmux_target_for_pane(pane_id: &str) -> String {
    if pane_id.starts_with('%') {
        pane_id.to_string()
    } else {
        format!(":.{pane_id}")
    }
}

fn send_tmux_keys_with_enter(pane_id: &str, keys: &str) -> Result<(), CoordinationError> {
    let target = tmux_target_for_pane(pane_id);
    run_tmux(&[
        "send-keys".to_string(),
        "-t".to_string(),
        target.clone(),
        keys.to_string(),
    ])?;
    thread::sleep(Duration::from_millis(200));
    run_tmux(&[
        "send-keys".to_string(),
        "-t".to_string(),
        target,
        "Enter".to_string(),
    ])?;
    Ok(())
}

pub(crate) fn kill_aitx_pane(pane_id: &str) -> Result<(), CoordinationError> {
    run_tmux(&[
        "kill-pane".to_string(),
        "-t".to_string(),
        tmux_target_for_pane(pane_id),
    ])
    .map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn validate_unix_pid(pid: u32) -> Result<String, CoordinationError> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(CoordinationError::Validation(format!(
            "pid out of Unix kill range: {pid}"
        )));
    }
    Ok(pid.to_string())
}

pub(crate) fn terminate_process_by_pid(pid: u32) -> Result<(), CoordinationError> {
    #[cfg(target_os = "windows")]
    let pid_arg = pid.to_string();
    #[cfg(not(target_os = "windows"))]
    let pid_arg = validate_unix_pid(pid)?;

    #[cfg(target_os = "windows")]
    let invocation = CommandInvocation {
        program: "taskkill".to_string(),
        args: vec!["/PID".to_string(), pid_arg, "/F".to_string()],
    };
    #[cfg(not(target_os = "windows"))]
    let invocation = CommandInvocation {
        program: "kill".to_string(),
        args: vec!["-TERM".to_string(), pid_arg],
    };

    let output = run_system_command(&invocation)?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(CoordinationError::Backend(format!(
            "process kill failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

pub(crate) fn is_process_running_by_pid(pid: u32) -> Result<bool, CoordinationError> {
    #[cfg(target_os = "windows")]
    let pid_arg = pid.to_string();
    #[cfg(not(target_os = "windows"))]
    if pid == 0 || pid > i32::MAX as u32 {
        return Ok(false);
    }
    #[cfg(not(target_os = "windows"))]
    let pid_arg = pid.to_string();

    #[cfg(target_os = "windows")]
    let invocation = CommandInvocation {
        program: "tasklist".to_string(),
        args: vec!["/FI".to_string(), format!("PID eq {pid_arg}")],
    };
    #[cfg(not(target_os = "windows"))]
    let invocation = CommandInvocation {
        program: "kill".to_string(),
        args: vec!["-0".to_string(), pid_arg.clone()],
    };

    let output = run_system_command(&invocation)?;
    #[cfg(target_os = "windows")]
    {
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(CoordinationError::Backend(format!(
                "pid check failed ({} {}): {}",
                invocation.program,
                invocation.args.join(" "),
                stderr
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(&pid_arg))
    }
    #[cfg(not(target_os = "windows"))]
    {
        if output.status.success() {
            return Ok(true);
        }
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("operation not permitted") {
            return Ok(true);
        }
        Ok(false)
    }
}

fn spawn_mesh_daemon(
    pane_id: &str,
    team_name: &str,
    agent_name: &str,
) -> Result<u32, CoordinationError> {
    let invocation = mesh_command_invocation(&[
        "daemon", "--pane", pane_id, "--team", team_name, "--name", agent_name,
    ]);
    let child = spawn_system_command(&invocation)?;
    Ok(child.id())
}

impl CoordinationOrchestrator {
    fn create_aitx_pane(&self, project_id: &str) -> Result<String, CoordinationError> {
        let stdout = run_aitx(&["new", "--path", project_id])?;
        let pane = stdout
            .split_whitespace()
            .find(|token| !token.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                CoordinationError::Backend(
                    "aitx new returned empty output; expected pane identifier".to_string(),
                )
            })?;
        Ok(pane)
    }
}

fn member_from_agent_setup(
    setup: &AgentSetupConfig,
    role: MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    Ok(Member {
        name: setup.name.clone(),
        role,
        instructions: setup.description.clone(),
        project_path: PathBuf::from(&setup.project_id),
        cli_tool: parse_cli_tool(&setup.cli_tool)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_target_uses_pane_id_when_present() {
        assert_eq!(tmux_target_for_pane("%12"), "%12");
    }

    #[test]
    fn tmux_target_wraps_numeric_index() {
        assert_eq!(tmux_target_for_pane("3"), ":.3");
    }

    #[test]
    fn build_cli_launch_command_uses_configured_fresh_command() {
        let mut cmds = crate::models::CliCommandSettings::default();
        cmds.gemini.fresh = "gemini --yolo --sandbox read-only".to_string();
        let agent = AgentSetupConfig {
            name: "reviewer".to_string(),
            cli_tool: "gemini".to_string(),
            model: "gemini-2.5-pro".to_string(),
            project_id: "/tmp/project".to_string(),
            description: None,
        };
        assert_eq!(
            build_cli_launch_command(&agent, &cmds).expect("command"),
            "gemini --yolo --sandbox read-only"
        );
    }

    #[test]
    fn build_cli_launch_command_for_codex_appends_model_when_missing() {
        let cmds = crate::models::CliCommandSettings::default();
        let agent = AgentSetupConfig {
            name: "builder".to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.3".to_string(),
            project_id: "/tmp/project".to_string(),
            description: None,
        };
        assert_eq!(
            build_cli_launch_command(&agent, &cmds).expect("command"),
            "codex --yolo -m 'gpt-5.3'"
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_handles_clean_output() {
        let stdout = b"/home/mstie\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/mstie".to_string())
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_ignores_banner_noise() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nThis message is shown once a day.\n/home/mstie/.local/bin/aitx\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/mstie/.local/bin/aitx".to_string())
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_returns_none_without_path() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nNo path here\n";
        assert_eq!(parse_wsl_unix_path_from_stdout(stdout), None);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_accepts_normal_pid() {
        assert_eq!(validate_unix_pid(12345).unwrap(), "12345");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_zero() {
        let err = validate_unix_pid(0).expect_err("pid 0 should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_values_above_i32_max() {
        let err = validate_unix_pid(u32::MAX).expect_err("out-of-range pid should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn is_process_running_returns_false_for_out_of_range_pid() {
        assert!(!is_process_running_by_pid(u32::MAX).unwrap());
        assert!(!is_process_running_by_pid(0).unwrap());
    }
}
