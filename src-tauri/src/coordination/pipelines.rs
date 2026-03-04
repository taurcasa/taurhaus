use std::path::PathBuf;

use chrono::Utc;

use crate::commands::coordination_types::{
    AddAgentReport, AddAgentRequest, AgentSetupConfig, InitializeReport, InitializeTeamRequest,
    LeadMode, ResumeAgentReport, ResumeContextMode, ResumeMemberRequest, StepProgress, StepStatus,
};
use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    DeliveryRequest, OperatorNoticeDelivery, TeardownMode, TeardownRequest,
};
use crate::coordination::runtime::{resolve_or_create_pane_for_member, PaneResolution};
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::coordination::validation::{
    validate_member_name, validate_non_empty, validate_team_name,
};
use crate::daemon::protocol::LaunchMode as DaemonLaunchMode;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::{
    build_team_launch_command, resolve_configured_tool_command, validate_command_override,
};

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
        self.initialize_team_with_cli_commands_and_layout(
            request,
            &CliCommandSettings::default(),
            "new_window",
        )
    }

    pub fn initialize_team_with_cli_commands(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<InitializeReport, CoordinationError> {
        self.initialize_team_with_cli_commands_and_layout(request, cli_commands, "new_window")
    }

    pub fn initialize_team_with_cli_commands_and_layout(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
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

        if let Err(err) = self.create_panes(request, tmux_layout) {
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
        if let Err(err) = self.sync_team_config_metadata(&request.team_name) {
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
            if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
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
            if let Err(err) = self.runtime.kill_aitx_pane(pane_id) {
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
        self.add_agent_to_team_with_cli_commands_and_layout(
            request,
            &CliCommandSettings::default(),
            "new_window",
        )
    }

    pub fn add_agent_to_team_with_cli_commands(
        &mut self,
        request: &AddAgentRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<AddAgentReport, CoordinationError> {
        self.add_agent_to_team_with_cli_commands_and_layout(request, cli_commands, "new_window")
    }

    pub fn add_agent_to_team_with_cli_commands_and_layout(
        &mut self,
        request: &AddAgentRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
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

        if let Err(err) = self.create_pane_for_agent(request, &mut runtime_state, tmux_layout) {
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

        if let Err(err) = self.launch_session_for_agent(request, &mut runtime_state, cli_commands) {
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

    /// Resume a member session with default command settings.
    pub fn resume_member(
        &mut self,
        team_name: &str,
        member_name: &str,
        context_mode: ResumeContextMode,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        let request = ResumeMemberRequest {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            context_mode,
        };
        self.resume_member_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
    }

    pub fn resume_member_with_cli_commands(
        &mut self,
        request: &ResumeMemberRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        self.resume_member_with_cli_commands_and_layout(request, cli_commands, "new_window")
    }

    /// Resume a member session in an existing team.
    pub fn resume_member_with_cli_commands_and_layout(
        &mut self,
        request: &ResumeMemberRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();
        let mut warnings = Vec::new();
        let mut runtime_state = PendingResumeState::default();

        if let Err(err) = self.validate_resume_request(request) {
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "validate",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                runtime_state.pane_id.clone(),
                runtime_state.reused_pane,
            ));
        }
        mark_step_succeeded(
            "validate",
            "request validated",
            &mut succeeded_steps,
            &mut steps,
        );

        let (member, mut runtime_record, lead_name) = match self.load_resume_member_state(request) {
            Ok(value) => value,
            Err(err) => {
                return Ok(failed_resume_report(
                    &request.team_name,
                    &request.member_name,
                    "load_member",
                    err,
                    succeeded_steps,
                    &mut steps,
                    warnings,
                    runtime_state.pane_id.clone(),
                    runtime_state.reused_pane,
                ))
            }
        };
        mark_step_succeeded(
            "load_member",
            "member and runtime state loaded",
            &mut succeeded_steps,
            &mut steps,
        );

        let pane_resolution =
            match self.resolve_resume_pane(&member, Some(&runtime_record), tmux_layout) {
                Ok(resolution) => resolution,
                Err(err) => {
                    self.cleanup_resume_failure(request, &runtime_state);
                    return Ok(failed_resume_report(
                        &request.team_name,
                        &request.member_name,
                        "resolve_pane",
                        err,
                        succeeded_steps,
                        &mut steps,
                        warnings,
                        runtime_state.pane_id.clone(),
                        runtime_state.reused_pane,
                    ));
                }
            };
        runtime_state.pane_id = Some(pane_resolution.pane_id.clone());
        runtime_state.reused_pane = pane_resolution.reused_pane;
        if pane_resolution.created_new_pane {
            runtime_state.created_pane_id = Some(pane_resolution.pane_id.clone());
            if runtime_record.pane_id.is_some() && !pane_resolution.reused_pane {
                warnings.push(format!(
                    "existing pane was not reusable for '{}'; created a new pane",
                    request.member_name
                ));
            }
        }
        let resolve_message = if pane_resolution.reused_pane {
            format!("reused pane {}", pane_resolution.pane_id)
        } else {
            format!("created pane {}", pane_resolution.pane_id)
        };
        mark_step_succeeded(
            "resolve_pane",
            resolve_message.as_str(),
            &mut succeeded_steps,
            &mut steps,
        );

        let pane_id = pane_resolution.pane_id;
        if let Err(err) =
            self.launch_resume_session(request, &member, &pane_id, cli_commands, &mut runtime_state)
        {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "launch_session",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        }
        mark_step_succeeded(
            "launch_session",
            "cli session launched",
            &mut succeeded_steps,
            &mut steps,
        );

        let is_claude = member.cli_tool == CliTool::Claude;
        let is_lead = member.role == MemberRole::Lead;
        if is_claude {
            mark_step_succeeded(
                "join_mesh",
                "not required for claude",
                &mut succeeded_steps,
                &mut steps,
            );
        } else if let Err(err) = self.resume_join_mesh(request, &member, &mut runtime_state) {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        } else {
            mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);
        }

        if is_claude {
            mark_step_succeeded(
                "start_daemon",
                "not required for claude",
                &mut succeeded_steps,
                &mut steps,
            );
        } else if let Err(err) = self.resume_start_daemon(
            request,
            &member,
            &pane_id,
            runtime_record.daemon_pid,
            &mut runtime_state,
            &mut warnings,
        ) {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "start_daemon",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        } else {
            mark_step_succeeded(
                "start_daemon",
                "mesh daemon started",
                &mut succeeded_steps,
                &mut steps,
            );
        }

        if is_claude && is_lead {
            mark_step_succeeded(
                "send_onboarding",
                "not required for claude lead",
                &mut succeeded_steps,
                &mut steps,
            );
        } else if let Err(err) = self.resume_send_onboarding(request, &member, &lead_name) {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        } else {
            mark_step_succeeded(
                "send_onboarding",
                "onboarding delivered",
                &mut succeeded_steps,
                &mut steps,
            );
        }

        runtime_record.pane_id = Some(pane_id.clone());
        runtime_record.session_id = runtime_state.session_id.clone();
        runtime_record.daemon_pid = runtime_state.daemon_pid;
        runtime_record.attached_at = Some(Utc::now());
        runtime_record.health = HealthState::Healthy;
        if let Err(err) = MemberRuntimeStore::save(
            &self.teams_dir,
            &request.team_name,
            &request.member_name,
            &runtime_record,
        ) {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "update_runtime",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        }
        if let Err(err) = self.sync_team_config_metadata(&request.team_name) {
            self.cleanup_resume_failure(request, &runtime_state);
            return Ok(failed_resume_report(
                &request.team_name,
                &request.member_name,
                "update_runtime",
                err,
                succeeded_steps,
                &mut steps,
                warnings,
                Some(pane_id.clone()),
                runtime_state.reused_pane,
            ));
        }
        mark_step_succeeded(
            "update_runtime",
            "runtime state updated",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(ResumeAgentReport {
            team_name: request.team_name.clone(),
            member_name: request.member_name.clone(),
            resumed: true,
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "member resumed".to_string(),
            steps,
            warnings,
            pane_id: Some(pane_id),
            reused_pane: runtime_state.reused_pane,
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

    fn create_panes(
        &mut self,
        request: &InitializeTeamRequest,
        tmux_layout: &str,
    ) -> Result<(), CoordinationError> {
        if request.lead_mode == LeadMode::LaunchNew {
            let pane_id = self
                .runtime
                .create_aitx_pane(&request.lead.project_id, tmux_layout)?;
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            runtime.pane_id = Some(pane_id);
            runtime.session_id = None;
            runtime.daemon_pid = None;
            runtime.attached_at = Some(Utc::now());
            runtime.health = HealthState::Healthy;
            MemberRuntimeStore::save(
                &self.teams_dir,
                &request.team_name,
                &request.lead.name,
                &runtime,
            )?;
        }

        for agent in &request.agents {
            let member = member_from_agent_setup(agent, MemberRole::Agent)?;
            self.add_member(&request.team_name, member.clone())?;
            let pane_id = self
                .runtime
                .create_aitx_pane(&agent.project_id, tmux_layout)?;

            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)?;
            runtime.pane_id = Some(pane_id);
            runtime.session_id = None;
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
        if request.lead_mode == LeadMode::LaunchNew {
            let runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            let pane_id = runtime.pane_id.ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    request.lead.name, request.team_name
                ))
            })?;
            self.launch_agent_in_pane(
                &pane_id,
                &request.team_name,
                &request.lead,
                MemberRole::Lead,
                cli_commands,
            )?;
            self.capture_session_id_for_member(
                &request.team_name,
                &request.lead.name,
                &pane_id,
                &request.lead,
            )?;
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
            self.launch_agent_in_pane(
                &pane_id,
                &request.team_name,
                agent,
                MemberRole::Agent,
                cli_commands,
            )?;
            self.capture_session_id_for_member(&request.team_name, &agent.name, &pane_id, agent)?;
        }
        Ok(())
    }

    fn join_mesh(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if should_use_mesh_sidecar(&request.lead)? {
            self.runtime.join_mesh(
                &request.team_name,
                &request.lead.name,
                &request.lead.project_id,
            )?;
        }
        for agent in &request.agents {
            if should_use_mesh_sidecar(agent)? {
                self.runtime
                    .join_mesh(&request.team_name, &agent.name, &agent.project_id)?;
            }
        }
        Ok(())
    }

    fn start_daemons(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if request.lead_mode == LeadMode::LaunchNew && should_use_mesh_sidecar(&request.lead)? {
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            let pane_id = runtime.pane_id.clone().ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    request.lead.name, request.team_name
                ))
            })?;
            let pid =
                self.runtime
                    .spawn_mesh_daemon(&pane_id, &request.team_name, &request.lead.name)?;
            runtime.daemon_pid = Some(pid);
            MemberRuntimeStore::save(
                &self.teams_dir,
                &request.team_name,
                &request.lead.name,
                &runtime,
            )?;
            tracing::info!(
                team = %request.team_name,
                member = %request.lead.name,
                pane_id = %pane_id,
                pid = pid,
                "mesh daemon started"
            );
        }

        for agent in &request.agents {
            if !should_use_mesh_sidecar(agent)? {
                continue;
            }
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &agent.name)?;
            let pane_id = runtime.pane_id.clone().ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    agent.name, request.team_name
                ))
            })?;
            let pid = self
                .runtime
                .spawn_mesh_daemon(&pane_id, &request.team_name, &agent.name)?;
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
            let onboarding = if cli_tool == CliTool::Claude {
                if !agent_has_role_context(agent) {
                    continue;
                }
                DeliveryRenderer::render_claude_role_context(
                    &request.team_name,
                    &agent.name,
                    &request.lead.name,
                    agent.role_id.as_deref(),
                    agent_instructions(agent),
                    agent.behavioral_contract.as_ref(),
                    agent.capabilities.as_deref(),
                )
            } else {
                DeliveryRenderer::render_onboarding(
                    &request.team_name,
                    &agent.name,
                    &request.lead.name,
                    agent.role_id.as_deref(),
                    agent_instructions(agent),
                    agent.behavioral_contract.as_ref(),
                    agent.capabilities.as_deref(),
                )
            };
            self.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
                sender_name: Some(request.lead.name.clone()),
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

    fn validate_resume_request(
        &self,
        request: &ResumeMemberRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_member_name(&request.member_name)?;
        Ok(())
    }

    fn load_resume_member_state(
        &self,
        request: &ResumeMemberRequest,
    ) -> Result<(Member, MemberRuntimeRecord, String), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());
        let member = config
            .members
            .iter()
            .find(|member| member.name == request.member_name)
            .cloned()
            .ok_or_else(|| {
                CoordinationError::NotFound(format!(
                    "member '{}' not found in team '{}'",
                    request.member_name, request.team_name
                ))
            })?;

        let runtime = match MemberRuntimeStore::load(
            &self.teams_dir,
            &request.team_name,
            &request.member_name,
        ) {
            Ok(runtime) => runtime,
            Err(CoordinationError::NotFound(_)) => default_runtime_record(&request.member_name),
            Err(err) => return Err(err),
        };
        if runtime.health != HealthState::SessionDead {
            return Err(CoordinationError::Conflict(format!(
                "member '{}' in team '{}' is not offline",
                request.member_name, request.team_name
            )));
        }

        Ok((member, runtime, lead_name))
    }

    fn resolve_resume_pane(
        &self,
        member: &Member,
        runtime_record: Option<&MemberRuntimeRecord>,
        tmux_layout: &str,
    ) -> Result<PaneResolution, CoordinationError> {
        resolve_or_create_pane_for_member(
            self.runtime.as_ref(),
            member,
            runtime_record,
            tmux_layout,
        )
    }

    fn launch_resume_session(
        &self,
        request: &ResumeMemberRequest,
        member: &Member,
        pane_id: &str,
        cli_commands: &CliCommandSettings,
        runtime_state: &mut PendingResumeState,
    ) -> Result<(), CoordinationError> {
        let agent = AgentSetupConfig {
            name: member.name.clone(),
            cli_tool: member.cli_tool.to_string(),
            model: String::new(),
            project_id: member.project_path.display().to_string(),
            description: member.instructions.clone(),
            role_id: member.role_id.clone(),
            instructions: member.instructions.clone(),
            behavioral_contract: member.behavioral_contract.clone(),
            capabilities: member.capabilities.clone(),
        };
        let launch_cmd = build_resume_cli_launch_command(
            &agent,
            &request.team_name,
            member.role,
            request.context_mode,
            cli_commands,
        )?;
        self.runtime
            .send_tmux_keys_with_enter(pane_id, launch_cmd.as_str())?;

        if member.cli_tool == CliTool::Claude {
            runtime_state.session_id = self.runtime.detect_session_id(pane_id, CliTool::Claude)?;
        } else {
            runtime_state.session_id = None;
        }

        Ok(())
    }

    fn resume_join_mesh(
        &self,
        request: &ResumeMemberRequest,
        member: &Member,
        runtime_state: &mut PendingResumeState,
    ) -> Result<(), CoordinationError> {
        let project_id = member.project_path.display().to_string();
        self.runtime.join_mesh(
            &request.team_name,
            &request.member_name,
            project_id.as_str(),
        )?;
        runtime_state.mesh_joined = true;
        Ok(())
    }

    fn resume_start_daemon(
        &self,
        request: &ResumeMemberRequest,
        member: &Member,
        pane_id: &str,
        previous_daemon_pid: Option<u32>,
        runtime_state: &mut PendingResumeState,
        warnings: &mut Vec<String>,
    ) -> Result<(), CoordinationError> {
        if let Some(pid) = previous_daemon_pid {
            if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                warnings.push(format!("failed to terminate stale daemon pid {pid}: {err}"));
            }
        }
        let new_pid = self
            .runtime
            .spawn_mesh_daemon(pane_id, &request.team_name, &member.name)?;
        runtime_state.daemon_pid = Some(new_pid);
        runtime_state.new_daemon_pid = Some(new_pid);
        Ok(())
    }

    fn resume_send_onboarding(
        &mut self,
        request: &ResumeMemberRequest,
        member: &Member,
        lead_name: &str,
    ) -> Result<(), CoordinationError> {
        let onboarding = if member.cli_tool == CliTool::Claude && member_has_role_context(member) {
            DeliveryRenderer::render_claude_role_context(
                &request.team_name,
                &member.name,
                lead_name,
                member.role_id.as_deref(),
                member.instructions.as_deref(),
                member.behavioral_contract.as_ref(),
                member.capabilities.as_deref(),
            )
        } else {
            DeliveryRenderer::render_onboarding(
                &request.team_name,
                &member.name,
                lead_name,
                member.role_id.as_deref(),
                member.instructions.as_deref(),
                member.behavioral_contract.as_ref(),
                member.capabilities.as_deref(),
            )
        };
        self.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: member.name.clone(),
            team_name: request.team_name.clone(),
            message: onboarding,
            sender_name: Some(lead_name.to_string()),
        }))?;
        Ok(())
    }

    fn cleanup_resume_failure(
        &mut self,
        request: &ResumeMemberRequest,
        runtime_state: &PendingResumeState,
    ) {
        if let Some(pid) = runtime_state.new_daemon_pid {
            if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.member_name,
                    pid = pid,
                    error = %err,
                    "resume rollback: failed to stop daemon process"
                );
            }
        }

        if runtime_state.mesh_joined {
            if let Err(err) = self.backend.teardown(TeardownRequest {
                member_name: request.member_name.clone(),
                team_name: request.team_name.clone(),
                mode: TeardownMode::Graceful,
            }) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.member_name,
                    error = %err,
                    "resume rollback: failed to leave mesh"
                );
            }
        }

        if let Some(pane_id) = runtime_state.created_pane_id.as_deref() {
            if let Err(err) = self.runtime.kill_aitx_pane(pane_id) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.member_name,
                    pane_id = %pane_id,
                    error = %err,
                    "resume rollback: failed to kill newly created pane"
                );
            }
        }
    }

    fn create_pane_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
        tmux_layout: &str,
    ) -> Result<(), CoordinationError> {
        let pane_id = self
            .runtime
            .create_aitx_pane(&request.agent.project_id, tmux_layout)?;
        runtime_state.pane_id = Some(pane_id);
        runtime_state.session_id = None;
        runtime_state.attached_at = Some(Utc::now());
        runtime_state.health = Some(HealthState::Healthy);
        Ok(())
    }

    fn launch_session_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        let pane_id = runtime_state.pane_id.as_deref().ok_or_else(|| {
            CoordinationError::Backend(format!(
                "missing pane id for member '{}' in team '{}'",
                request.agent.name, request.team_name
            ))
        })?;
        self.launch_agent_in_pane(
            pane_id,
            &request.team_name,
            &request.agent,
            MemberRole::Agent,
            cli_commands,
        )?;
        let cli_tool = parse_cli_tool(&request.agent.cli_tool)?;
        if cli_tool == CliTool::Claude {
            runtime_state.session_id = self.runtime.detect_session_id(pane_id, cli_tool)?;
        }
        Ok(())
    }

    fn launch_agent_in_pane(
        &self,
        pane_id: &str,
        team_name: &str,
        agent: &AgentSetupConfig,
        role: MemberRole,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        let launch_cmd = build_cli_launch_command(agent, team_name, role, cli_commands)?;
        self.runtime
            .send_tmux_keys_with_enter(pane_id, launch_cmd.as_str())
    }

    fn join_mesh_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        if !should_use_mesh_sidecar(&request.agent)? {
            return Ok(());
        }
        self.runtime.join_mesh(
            &request.team_name,
            &request.agent.name,
            &request.agent.project_id,
        )?;
        runtime_state.mesh_joined = true;
        Ok(())
    }

    fn start_daemon_for_agent(
        &self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        if !should_use_mesh_sidecar(&request.agent)? {
            return Ok(());
        }
        let pane_id = runtime_state.pane_id.as_deref().ok_or_else(|| {
            CoordinationError::Backend(format!(
                "missing pane id for member '{}' in team '{}'",
                request.agent.name, request.team_name
            ))
        })?;
        let pid =
            self.runtime
                .spawn_mesh_daemon(pane_id, &request.team_name, &request.agent.name)?;
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
        let team = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = team
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        let onboarding = if cli_tool == CliTool::Claude {
            if !agent_has_role_context(&request.agent) {
                return Ok(());
            }
            DeliveryRenderer::render_claude_role_context(
                &request.team_name,
                &request.agent.name,
                &lead_name,
                request.agent.role_id.as_deref(),
                agent_instructions(&request.agent),
                request.agent.behavioral_contract.as_ref(),
                request.agent.capabilities.as_deref(),
            )
        } else {
            DeliveryRenderer::render_onboarding(
                &request.team_name,
                &request.agent.name,
                &lead_name,
                request.agent.role_id.as_deref(),
                agent_instructions(&request.agent),
                request.agent.behavioral_contract.as_ref(),
                request.agent.capabilities.as_deref(),
            )
        };
        self.backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
                sender_name: Some(lead_name),
            }))?;
        Ok(())
    }

    fn update_roster_with_agent(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        let desired_member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        let mut config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;

        if let Some(existing) = config
            .members
            .iter_mut()
            .find(|member| member.name == desired_member.name)
        {
            *existing = desired_member;
            TeamConfigStore::save(&self.teams_dir, &request.team_name, &config)?;
            runtime_state.member_added = false;
        } else {
            self.add_member(&request.team_name, desired_member)?;
            runtime_state.member_added = true;
        }

        let mut runtime = match MemberRuntimeStore::load(
            &self.teams_dir,
            &request.team_name,
            &request.agent.name,
        ) {
            Ok(runtime) => runtime,
            Err(CoordinationError::NotFound(_)) => default_runtime_record(&request.agent.name),
            Err(err) => return Err(err),
        };
        runtime.pane_id = runtime_state.pane_id.clone();
        runtime.session_id = runtime_state.session_id.clone();
        runtime.daemon_pid = runtime_state.daemon_pid;
        runtime.attached_at = runtime_state.attached_at;
        runtime.health = runtime_state.health.unwrap_or(HealthState::SessionDead);
        MemberRuntimeStore::save(
            &self.teams_dir,
            &request.team_name,
            &request.agent.name,
            &runtime,
        )?;
        self.sync_team_config_metadata(&request.team_name)?;
        Ok(())
    }

    fn capture_session_id_for_member(
        &self,
        team_name: &str,
        member_name: &str,
        pane_id: &str,
        agent: &AgentSetupConfig,
    ) -> Result<(), CoordinationError> {
        let cli_tool = parse_cli_tool(&agent.cli_tool)?;
        if cli_tool != CliTool::Claude {
            return Ok(());
        }

        let session_id = self.runtime.detect_session_id(pane_id, cli_tool)?;
        let Some(session_id) = session_id else {
            return Ok(());
        };

        let mut runtime = MemberRuntimeStore::load(&self.teams_dir, team_name, member_name)?;
        runtime.session_id = Some(session_id);
        MemberRuntimeStore::save(&self.teams_dir, team_name, member_name, &runtime)?;
        Ok(())
    }

    fn sync_team_config_metadata(&self, team_name: &str) -> Result<(), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        TeamConfigStore::save(&self.teams_dir, team_name, &config)
    }
}

#[derive(Debug, Default, Clone)]
struct PendingRuntimeState {
    pane_id: Option<String>,
    session_id: Option<String>,
    daemon_pid: Option<u32>,
    attached_at: Option<chrono::DateTime<Utc>>,
    health: Option<HealthState>,
    mesh_joined: bool,
    member_added: bool,
}

#[derive(Debug, Default, Clone)]
struct PendingResumeState {
    pane_id: Option<String>,
    session_id: Option<String>,
    daemon_pid: Option<u32>,
    new_daemon_pid: Option<u32>,
    created_pane_id: Option<String>,
    reused_pane: bool,
    mesh_joined: bool,
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

#[allow(clippy::too_many_arguments)]
fn failed_resume_report(
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

fn default_runtime_record(member_name: &str) -> MemberRuntimeRecord {
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

fn build_cli_launch_command(
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

fn build_resume_cli_launch_command(
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

fn should_use_mesh_sidecar(agent: &AgentSetupConfig) -> Result<bool, CoordinationError> {
    Ok(parse_cli_tool(&agent.cli_tool)? != CliTool::Claude)
}

fn with_claude_team_context(
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

fn command_contains_flag(command: &str, flag: &str) -> bool {
    command.split_whitespace().any(|token| {
        token == flag
            || token
                .strip_prefix(flag)
                .is_some_and(|suffix| suffix.starts_with('='))
    })
}

fn shell_escape_for_cmd(value: &str) -> String {
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

fn has_non_empty_capabilities(capabilities: Option<&[String]>) -> bool {
    capabilities
        .map(|items| items.iter().any(|item| !item.trim().is_empty()))
        .unwrap_or(false)
}

fn agent_instructions(agent: &AgentSetupConfig) -> Option<&str> {
    agent
        .instructions
        .as_deref()
        .or(agent.description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn agent_has_role_context(agent: &AgentSetupConfig) -> bool {
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

fn member_has_role_context(member: &Member) -> bool {
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

fn member_from_agent_setup(
    setup: &AgentSetupConfig,
    role: MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    Ok(Member {
        name: setup.name.clone(),
        role,
        role_id: setup.role_id.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
    use crate::coordination::stores::MemberRuntimeStore;
    use crate::templates::types::BehavioralContract;

    fn member(name: &str, role: MemberRole, cli_tool: CliTool, project: &str) -> Member {
        Member {
            name: name.to_string(),
            role,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from(project),
            cli_tool,
        }
    }

    fn setup_config(name: &str, cli_tool: &str, model: &str, project_id: &str) -> AgentSetupConfig {
        AgentSetupConfig {
            name: name.to_string(),
            cli_tool: cli_tool.to_string(),
            model: model.to_string(),
            project_id: project_id.to_string(),
            description: None,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        }
    }

    fn new_orchestrator(
        tmp: &TempDir,
        backend: Arc<FakeBackend>,
        runtime: Arc<RecordingCoordinationRuntime>,
    ) -> CoordinationOrchestrator {
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime)
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
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        };
        assert_eq!(
            build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
                .expect("command"),
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
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        };
        assert_eq!(
            build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
                .expect("command"),
            "codex --yolo -m 'gpt-5.3-codex'"
        );
    }

    #[test]
    fn build_cli_launch_command_for_claude_appends_team_context() {
        let cmds = crate::models::CliCommandSettings::default();
        let agent = AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "claude-opus-4-6".to_string(),
            project_id: "/tmp/project".to_string(),
            description: None,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        };
        let command = build_cli_launch_command(&agent, "ledger-team", MemberRole::Lead, &cmds)
            .expect("command");
        assert!(command.contains("CLAUDECODE=1"));
        assert!(command.contains("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1"));
        assert!(command.contains("--team-name ledger-team"));
        assert!(command.contains("--agent-name team-lead"));
        assert!(command.contains("--agent-id team-lead@ledger-team"));
        assert!(command.contains("--agent-type orchestrator"));
    }

    #[test]
    fn build_resume_cli_launch_command_continue_uses_resume_for_codex() {
        let cmds = crate::models::CliCommandSettings::default();
        let agent = AgentSetupConfig {
            name: "builder".to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.3".to_string(),
            project_id: "/tmp/project".to_string(),
            description: None,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        };

        let command = build_resume_cli_launch_command(
            &agent,
            "architecture-final",
            MemberRole::Agent,
            ResumeContextMode::Continue,
            &cmds,
        )
        .expect("command");
        assert_eq!(command, "codex resume --last --yolo");
    }

    #[test]
    fn build_resume_cli_launch_command_continue_uses_claude_continue_with_team_context() {
        let cmds = crate::models::CliCommandSettings::default();
        let agent = AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: "/tmp/project".to_string(),
            description: None,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        };

        let command = build_resume_cli_launch_command(
            &agent,
            "architecture-final",
            MemberRole::Lead,
            ResumeContextMode::Continue,
            &cmds,
        )
        .expect("command");
        assert!(command.contains("--continue"));
        assert!(command.contains("--agent-type orchestrator"));
        assert!(command.contains("--team-name architecture-final"));
    }

    #[test]
    fn member_from_agent_setup_maps_role_template_context() {
        let mut setup = setup_config("codex-dev", "codex", "gpt-5.3", "/tmp/project");
        setup.description = Some("fallback instructions".to_string());
        setup.role_id = Some("codex-developer".to_string());
        setup.instructions = Some("template instructions".to_string());
        setup.behavioral_contract = Some(BehavioralContract {
            communication: vec!["post updates".to_string()],
            execution: vec!["ship patches".to_string()],
            escalation: vec!["raise blockers".to_string()],
        });
        setup.capabilities = Some(vec!["implementation".to_string()]);

        let member =
            member_from_agent_setup(&setup, MemberRole::Agent).expect("member mapping should work");

        assert_eq!(member.role_id.as_deref(), Some("codex-developer"));
        assert_eq!(
            member.instructions.as_deref(),
            Some("template instructions")
        );
        assert_eq!(
            member
                .behavioral_contract
                .as_ref()
                .map(|contract| contract.execution.clone())
                .unwrap_or_default(),
            vec!["ship patches".to_string()]
        );
        assert_eq!(
            member.capabilities.as_ref().cloned().unwrap_or_default(),
            vec!["implementation".to_string()]
        );
    }

    #[test]
    fn initialize_pipeline_claude_template_agent_receives_role_context_message() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

        let mut claude_agent =
            setup_config("researcher", "claude", "claude-opus-4-6", "/tmp/research");
        claude_agent.role_id = Some("claude-researcher".to_string());
        claude_agent.instructions = Some("Investigate architecture tradeoffs.".to_string());
        claude_agent.behavioral_contract = Some(BehavioralContract {
            communication: vec!["post concise findings".to_string()],
            execution: vec!["run focused experiments".to_string()],
            escalation: vec!["escalate ambiguous requirements".to_string()],
        });
        claude_agent.capabilities = Some(vec!["analysis".to_string(), "research".to_string()]);

        let request = InitializeTeamRequest {
            team_name: "architecture-final".to_string(),
            team_description: None,
            lead_mode: LeadMode::LaunchNew,
            lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
            agents: vec![claude_agent],
        };

        let report = orchestrator
            .initialize_team_with_cli_commands_and_layout(
                &request,
                &CliCommandSettings::default(),
                "new_window",
            )
            .expect("initialize report");
        assert_eq!(report.failed_step, None);

        let delivered = backend.delivered_requests();
        assert_eq!(
            delivered.len(),
            1,
            "claude template agent should receive role context"
        );
        match &delivered[0] {
            DeliveryRequest::OperatorNotice(payload) => {
                assert_eq!(payload.member_name, "researcher");
                assert!(payload.message.contains("[taurhaus] role_context"));
                assert!(payload.message.contains("Role: claude-researcher"));
                assert!(payload.message.contains("Capabilities:"));
                assert!(payload.message.contains("- analysis"));
                assert!(payload.message.contains("- research"));
                assert!(!payload.message.contains("mesh read --unread"));
            }
            other => panic!("unexpected delivery payload: {other:?}"),
        }
    }

    #[test]
    fn initialize_pipeline_claude_agent_without_role_context_stays_skipped() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

        let request = InitializeTeamRequest {
            team_name: "architecture-final".to_string(),
            team_description: None,
            lead_mode: LeadMode::LaunchNew,
            lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
            agents: vec![setup_config(
                "researcher",
                "claude",
                "claude-opus-4-6",
                "/tmp/research",
            )],
        };

        let report = orchestrator
            .initialize_team_with_cli_commands_and_layout(
                &request,
                &CliCommandSettings::default(),
                "new_window",
            )
            .expect("initialize report");
        assert_eq!(report.failed_step, None);
        assert!(
            backend.delivered_requests().is_empty(),
            "claude agent without template context should keep legacy skip behavior"
        );
    }

    #[test]
    fn load_resume_member_state_preserves_role_template_context() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");
        orchestrator
            .add_member(
                "architecture-final",
                Member {
                    name: "builder".to_string(),
                    role: MemberRole::Agent,
                    role_id: Some("codex-developer".to_string()),
                    instructions: Some("Implement safely".to_string()),
                    behavioral_contract: Some(BehavioralContract {
                        communication: vec!["post updates".to_string()],
                        execution: vec!["ship patches".to_string()],
                        escalation: vec!["raise blockers".to_string()],
                    }),
                    capabilities: Some(vec!["implementation".to_string(), "testing".to_string()]),
                    project_path: PathBuf::from("/tmp/builder"),
                    cli_tool: CliTool::Codex,
                },
            )
            .expect("add member");

        let request = ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "builder".to_string(),
            context_mode: ResumeContextMode::Continue,
        };

        let (loaded_member, _runtime_record, lead_name) = orchestrator
            .load_resume_member_state(&request)
            .expect("resume state should load");

        assert_eq!(lead_name, "team-lead");
        assert_eq!(loaded_member.role_id.as_deref(), Some("codex-developer"));
        assert_eq!(
            loaded_member.instructions.as_deref(),
            Some("Implement safely")
        );
        assert_eq!(
            loaded_member
                .behavioral_contract
                .as_ref()
                .map(|contract| contract.execution.clone())
                .unwrap_or_default(),
            vec!["ship patches".to_string()]
        );
        assert_eq!(
            loaded_member
                .capabilities
                .as_ref()
                .cloned()
                .unwrap_or_default(),
            vec!["implementation".to_string(), "testing".to_string()]
        );
    }

    #[test]
    fn resume_pipeline_claude_lead_skips_mesh_daemon_and_onboarding() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");

        let mut lead_runtime =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead")
                .expect("runtime");
        lead_runtime.pane_id = Some("%9".to_string());
        lead_runtime.health = HealthState::SessionDead;
        MemberRuntimeStore::save(tmp.path(), "architecture-final", "team-lead", &lead_runtime)
            .expect("save runtime");

        let report = orchestrator
            .resume_member(
                "architecture-final",
                "team-lead",
                ResumeContextMode::Continue,
            )
            .expect("resume report");

        assert!(report.resumed);
        assert!(report.reused_pane);
        assert_eq!(report.failed_step, None);
        let join_step = report
            .steps
            .iter()
            .find(|step| step.step == "join_mesh")
            .expect("join step");
        assert_eq!(join_step.status, StepStatus::Succeeded);
        assert!(join_step
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("not required"));
        let daemon_step = report
            .steps
            .iter()
            .find(|step| step.step == "start_daemon")
            .expect("daemon step");
        assert!(daemon_step
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("not required"));
        let onboarding_step = report
            .steps
            .iter()
            .find(|step| step.step == "send_onboarding")
            .expect("onboarding step");
        assert!(onboarding_step
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("not required"));

        let calls = runtime.calls();
        let launch = calls
            .iter()
            .find_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
                _ => None,
            })
            .expect("launch command");
        assert!(launch.contains("--continue"));
        assert!(launch.contains("--agent-type orchestrator"));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
        assert_eq!(
            backend.call_counts().1,
            0,
            "onboarding should be skipped for lead"
        );
    }

    #[test]
    fn resume_pipeline_claude_member_sends_onboarding_and_skips_mesh_daemon() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "researcher",
                    MemberRole::Agent,
                    CliTool::Claude,
                    "/tmp/research",
                ),
            )
            .expect("add member");

        let mut member_runtime =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "researcher")
                .expect("runtime");
        member_runtime.pane_id = Some("%10".to_string());
        member_runtime.health = HealthState::SessionDead;
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "researcher",
            &member_runtime,
        )
        .expect("save runtime");

        let report = orchestrator
            .resume_member(
                "architecture-final",
                "researcher",
                ResumeContextMode::Continue,
            )
            .expect("resume report");

        assert!(report.resumed);
        let calls = runtime.calls();
        let launch = calls
            .iter()
            .find_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
                _ => None,
            })
            .expect("launch command");
        assert!(launch.contains("--agent-type general-purpose"));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
        assert_eq!(backend.call_counts().1, 1, "onboarding should be delivered");
    }

    #[test]
    fn resume_pipeline_claude_member_with_role_context_sends_role_context_message() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");
        orchestrator
            .add_member(
                "architecture-final",
                Member {
                    name: "researcher".to_string(),
                    role: MemberRole::Agent,
                    role_id: Some("claude-researcher".to_string()),
                    instructions: Some("Investigate tradeoffs and summarize findings.".to_string()),
                    behavioral_contract: Some(BehavioralContract {
                        communication: vec!["post concise updates".to_string()],
                        execution: vec!["run experiments".to_string()],
                        escalation: vec!["escalate blockers immediately".to_string()],
                    }),
                    capabilities: Some(vec!["analysis".to_string()]),
                    project_path: PathBuf::from("/tmp/research"),
                    cli_tool: CliTool::Claude,
                },
            )
            .expect("add member");

        let mut member_runtime =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "researcher")
                .expect("runtime");
        member_runtime.pane_id = Some("%10".to_string());
        member_runtime.health = HealthState::SessionDead;
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            "researcher",
            &member_runtime,
        )
        .expect("save runtime");

        let report = orchestrator
            .resume_member(
                "architecture-final",
                "researcher",
                ResumeContextMode::Continue,
            )
            .expect("resume report");

        assert!(report.resumed);
        let delivered = backend.delivered_requests();
        assert_eq!(delivered.len(), 1);
        match &delivered[0] {
            DeliveryRequest::OperatorNotice(payload) => {
                assert!(payload.message.contains("[taurhaus] role_context"));
                assert!(payload.message.contains("Role: claude-researcher"));
                assert!(payload.message.contains("Capabilities:"));
                assert!(payload.message.contains("- analysis"));
            }
            other => panic!("unexpected delivery payload: {other:?}"),
        }
    }

    #[test]
    fn resume_pipeline_non_claude_continue_uses_resume_command_and_updates_runtime() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");
        orchestrator
            .add_member(
                "architecture-final",
                member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
            )
            .expect("add member");

        let mut member_runtime =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
        member_runtime.pane_id = Some("%11".to_string());
        member_runtime.daemon_pid = Some(55);
        member_runtime.health = HealthState::SessionDead;
        MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
            .expect("save runtime");

        let report = orchestrator
            .resume_member("architecture-final", "builder", ResumeContextMode::Continue)
            .expect("resume report");
        assert!(report.resumed);
        assert!(report.reused_pane);

        let calls = runtime.calls();
        let launch = calls
            .iter()
            .find_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
                _ => None,
            })
            .expect("launch command");
        assert_eq!(launch, "codex resume --last --yolo");
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 55)));
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
        assert_eq!(backend.call_counts().1, 1, "onboarding should be delivered");

        let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder")
            .expect("updated runtime");
        assert_eq!(updated.pane_id.as_deref(), Some("%11"));
        assert_eq!(updated.health, HealthState::Healthy);
        assert_eq!(updated.daemon_pid, Some(10000));
        assert!(updated.attached_at.is_some());
    }

    #[test]
    fn resume_failure_cleans_created_resources_and_keeps_member_config() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = Arc::new(FakeBackend::default());
        backend.set_deliver_error(CoordinationError::Backend("delivery failed".to_string()));
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

        orchestrator
            .create_team("architecture-final", None)
            .expect("create team");
        orchestrator
            .add_member(
                "architecture-final",
                member(
                    "team-lead",
                    MemberRole::Lead,
                    CliTool::Claude,
                    "/tmp/lead-project",
                ),
            )
            .expect("add lead");
        orchestrator
            .add_member(
                "architecture-final",
                member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
            )
            .expect("add member");

        // Existing pane should be reused; rollback must not kill it.
        let mut member_runtime =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
        member_runtime.pane_id = Some("%77".to_string());
        member_runtime.health = HealthState::SessionDead;
        MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
            .expect("save runtime");

        let report = orchestrator
            .resume_member("architecture-final", "builder", ResumeContextMode::Continue)
            .expect("resume report");
        assert!(!report.resumed);
        assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));

        let config = TeamConfigStore::load(tmp.path(), "architecture-final").expect("team config");
        assert!(config.members.iter().any(|entry| entry.name == "builder"));

        let calls = runtime.calls();
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 10000)));
        assert!(
            !calls
                .iter()
                .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%77")),
            "reused pane must not be killed during rollback"
        );
    }
}
