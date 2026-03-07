use super::*;

use chrono::Utc;

use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    DeliveryRequest, InitializeReport, InitializeTeamRequest, LeadMode, OperatorNoticeDelivery,
};
use crate::coordination::stores::MemberRuntimeStore;
use crate::coordination::validation::{validate_non_empty, validate_team_name};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

impl CoordinationOrchestrator {
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

        self.ensure_team_daemon_running_best_effort(&request.team_name);

        Ok(InitializeReport {
            team_name: request.team_name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
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
}
