use super::*;

use std::path::PathBuf;

use chrono::Utc;

use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentReport, AddAgentRequest, AgentSetupConfig, ResumeAgentReport, ResumeContextMode,
    ResumeMemberRequest,
};
use crate::coordination::runtime::{resolve_or_create_pane_for_member, PaneResolution};
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::coordination::validation::{
    validate_member_name, validate_non_empty, validate_team_name,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

impl CoordinationOrchestrator {
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

        self.ensure_team_daemon_running_best_effort(&request.team_name);

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
        runtime_record.cli_tool.get_or_insert(member.cli_tool);
        if runtime_record.project_path.is_none() {
            runtime_record.project_path = Some(member.project_path.clone());
        }
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

        self.ensure_team_daemon_running_best_effort(&request.team_name);

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

    pub(super) fn load_resume_member_state(
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
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
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

        if matches!(member.cli_tool, CliTool::Claude | CliTool::Codex) {
            runtime_state.session_id = self.runtime.detect_session_id(pane_id, member.cli_tool)?;
        } else {
            runtime_state.session_id = None;
        }

        Ok(())
    }

    fn create_pane_for_agent(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
        tmux_layout: &str,
    ) -> Result<(), CoordinationError> {
        let member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        self.add_member(&request.team_name, member)?;
        runtime_state.member_added = true;

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
        if matches!(cli_tool, CliTool::Claude | CliTool::Codex) {
            runtime_state.session_id = self.runtime.detect_session_id(pane_id, cli_tool)?;
        }
        Ok(())
    }

    pub(super) fn launch_agent_in_pane(
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

    pub(super) fn capture_session_id_for_member(
        &self,
        team_name: &str,
        member_name: &str,
        pane_id: &str,
        agent: &AgentSetupConfig,
    ) -> Result<(), CoordinationError> {
        let cli_tool = parse_cli_tool(&agent.cli_tool)?;
        if !matches!(cli_tool, CliTool::Claude | CliTool::Codex) {
            return Ok(());
        }

        let session_id = self.runtime.detect_session_id(pane_id, cli_tool)?;
        let Some(session_id) = session_id else {
            return Ok(());
        };

        let mut runtime = MemberRuntimeStore::load(&self.teams_dir, team_name, member_name)?;
        runtime.cli_tool.get_or_insert(cli_tool);
        if runtime.project_path.is_none() {
            runtime.project_path = Some(PathBuf::from(&agent.project_id));
        }
        runtime.session_id = Some(session_id);
        MemberRuntimeStore::save(&self.teams_dir, team_name, member_name, &runtime)?;
        Ok(())
    }
}
