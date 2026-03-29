use super::*;

use chrono::Utc;

use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::MemberActivationContext;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentReport, AddAgentRequest, MemberActivationStage, ResumeAgentReport, ResumeMemberRequest,
    StepStatus,
};
use crate::coordination::runtime::{resolve_or_create_pane_for_member, PaneResolution};
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::coordination::validation::{
    validate_member_name, validate_non_empty, validate_team_name,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeTeamDaemonOwnership {
    Wrapper,
    Caller,
}

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
        let lead_name = self.load_team_lead_name(&request.team_name)?;
        let activation_context =
            MemberActivationContext::for_add_agent(&request.team_name, &lead_name, &request.agent)?;

        if let Err(err) =
            self.create_pane_for_agent(request, &mut runtime_state, cli_commands, tmux_layout)
        {
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
            "pane opened and session started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) =
            self.launch_session_for_agent(&activation_context, request, &mut runtime_state)
        {
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
            "launched session verified",
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

        self.ensure_team_daemon_after_add_agent(request);

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
    ) -> Result<ResumeAgentReport, CoordinationError> {
        let request = ResumeMemberRequest {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
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
        self.resume_member_with_cli_commands_and_layout_and_progress(
            request,
            cli_commands,
            tmux_layout,
            1,
            1,
            None,
        )
    }

    /// Resume a member session in an existing team.
    pub fn resume_member_with_cli_commands_and_layout_and_progress(
        &mut self,
        request: &ResumeMemberRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        member_index: usize,
        member_count: usize,
        emit_progress: Option<
            &mut dyn FnMut(&str, usize, usize, MemberActivationStage, StepStatus, Option<String>),
        >,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        self.resume_member_with_cli_commands_and_layout_and_progress_owned(
            request,
            cli_commands,
            tmux_layout,
            member_index,
            member_count,
            emit_progress,
            ResumeTeamDaemonOwnership::Wrapper,
        )
    }

    pub(crate) fn resume_member_with_cli_commands_and_layout_and_progress_owned(
        &mut self,
        request: &ResumeMemberRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        member_index: usize,
        member_count: usize,
        mut emit_progress: Option<
            &mut dyn FnMut(&str, usize, usize, MemberActivationStage, StepStatus, Option<String>),
        >,
        team_daemon_ownership: ResumeTeamDaemonOwnership,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();
        let mut warnings = Vec::new();
        let mut runtime_state = PendingResumeState::default();
        let member_name = request.member_name.as_str();

        let mut emit_stage =
            |stage: MemberActivationStage, status: StepStatus, message: Option<String>| {
                if let Some(emit) = emit_progress.as_deref_mut() {
                    emit(
                        member_name,
                        member_index,
                        member_count,
                        stage,
                        status,
                        message,
                    );
                }
            };

        emit_stage(
            MemberActivationStage::PrepareMember,
            StepStatus::Running,
            None,
        );
        if let Err(err) = self.validate_resume_request(request) {
            emit_stage(
                MemberActivationStage::PrepareMember,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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

        let (member, runtime_record, lead_name) = match self.load_resume_member_state(request) {
            Ok(value) => value,
            Err(err) => {
                emit_stage(
                    MemberActivationStage::PrepareMember,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
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
                ));
            }
        };
        mark_step_succeeded(
            "load_member",
            "member and runtime state loaded",
            &mut succeeded_steps,
            &mut steps,
        );
        emit_stage(
            MemberActivationStage::PrepareMember,
            StepStatus::Succeeded,
            Some("member request and runtime state prepared".to_string()),
        );
        let activation_context =
            MemberActivationContext::for_resume_member(&request.team_name, &lead_name, &member);

        emit_stage(
            MemberActivationStage::AcquirePane,
            StepStatus::Running,
            None,
        );
        let pane_resolution =
            match self.resolve_resume_pane(&member, Some(&runtime_record), tmux_layout) {
                Ok(resolution) => resolution,
                Err(err) => {
                    self.cleanup_resume_failure(request, &runtime_state);
                    emit_stage(
                        MemberActivationStage::AcquirePane,
                        StepStatus::Failed,
                        Some(err.to_string()),
                    );
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
        emit_stage(
            MemberActivationStage::AcquirePane,
            StepStatus::Succeeded,
            Some(resolve_message.clone()),
        );

        let pane_id = pane_resolution.pane_id;
        emit_stage(
            MemberActivationStage::LaunchSession,
            StepStatus::Running,
            None,
        );
        if let Err(err) = self.launch_resume_session(&activation_context, &pane_id, cli_commands) {
            self.cleanup_resume_failure(request, &runtime_state);
            emit_stage(
                MemberActivationStage::LaunchSession,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
        emit_stage(
            MemberActivationStage::LaunchSession,
            StepStatus::Succeeded,
            Some("cli session launched".to_string()),
        );

        emit_stage(
            MemberActivationStage::CaptureSessionIdentity,
            StepStatus::Running,
            None,
        );
        if let Err(err) =
            self.capture_resume_session_identity(&activation_context, &pane_id, &mut runtime_state)
        {
            self.cleanup_resume_failure(request, &runtime_state);
            emit_stage(
                MemberActivationStage::CaptureSessionIdentity,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
        emit_stage(
            MemberActivationStage::CaptureSessionIdentity,
            StepStatus::Succeeded,
            Some(capture_session_identity_message(&member, &runtime_state)),
        );

        let is_claude = member.cli_tool == CliTool::Claude;
        emit_stage(MemberActivationStage::JoinMesh, StepStatus::Running, None);
        if is_claude {
            mark_step_succeeded(
                "join_mesh",
                "not required for claude",
                &mut succeeded_steps,
                &mut steps,
            );
            emit_stage(
                MemberActivationStage::JoinMesh,
                StepStatus::Succeeded,
                Some("not required for claude".to_string()),
            );
        } else if let Err(err) = self.resume_join_mesh(request, &member, &mut runtime_state) {
            self.cleanup_resume_failure(request, &runtime_state);
            emit_stage(
                MemberActivationStage::JoinMesh,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
            emit_stage(
                MemberActivationStage::JoinMesh,
                StepStatus::Succeeded,
                Some("mesh joined".to_string()),
            );
        }

        emit_stage(
            MemberActivationStage::StartMemberDaemon,
            StepStatus::Running,
            None,
        );
        if is_claude {
            mark_step_succeeded(
                "start_daemon",
                "not required for claude",
                &mut succeeded_steps,
                &mut steps,
            );
            emit_stage(
                MemberActivationStage::StartMemberDaemon,
                StepStatus::Succeeded,
                Some("not required for claude".to_string()),
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
            emit_stage(
                MemberActivationStage::StartMemberDaemon,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
            emit_stage(
                MemberActivationStage::StartMemberDaemon,
                StepStatus::Succeeded,
                Some("mesh daemon started".to_string()),
            );
        }

        emit_stage(
            MemberActivationStage::DeliverOnboarding,
            StepStatus::Running,
            None,
        );
        if let Err(err) = self.resume_send_onboarding(request, &member, &lead_name) {
            self.cleanup_resume_failure(request, &runtime_state);
            emit_stage(
                MemberActivationStage::DeliverOnboarding,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
            emit_stage(
                MemberActivationStage::DeliverOnboarding,
                StepStatus::Succeeded,
                Some("onboarding delivered".to_string()),
            );
        }

        let context =
            MemberActivationContext::for_resume_member(&request.team_name, &lead_name, &member);
        emit_stage(
            MemberActivationStage::CommitRuntime,
            StepStatus::Running,
            None,
        );
        if let Err(err) = self.commit_member_runtime(
            &context,
            RuntimeCommitPatch::from_pending_resume_state(
                &runtime_state,
                Utc::now(),
                HealthState::Healthy,
            ),
        ) {
            self.cleanup_resume_failure(request, &runtime_state);
            emit_stage(
                MemberActivationStage::CommitRuntime,
                StepStatus::Failed,
                Some(err.to_string()),
            );
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
        emit_stage(
            MemberActivationStage::CommitRuntime,
            StepStatus::Succeeded,
            Some("runtime state updated".to_string()),
        );

        if team_daemon_ownership == ResumeTeamDaemonOwnership::Wrapper {
            self.ensure_team_daemon_after_resume_member(request);
        }

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

    fn load_team_lead_name(&self, team_name: &str) -> Result<String, CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        Ok(config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string()))
    }

    pub(super) fn load_resume_member_state(
        &self,
        request: &ResumeMemberRequest,
    ) -> Result<(Member, MemberRuntimeRecord, String), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = self.load_team_lead_name(&request.team_name)?;
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
        activation_context: &MemberActivationContext,
        pane_id: &str,
        cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        run_member_session_phase(
            self.runtime.as_ref(),
            activation_context,
            pane_id,
            MemberSessionPhase::LaunchOnly(cli_commands),
        )?;
        Ok(())
    }

    fn capture_resume_session_identity(
        &self,
        activation_context: &MemberActivationContext,
        pane_id: &str,
        runtime_state: &mut PendingResumeState,
    ) -> Result<(), CoordinationError> {
        let detected = run_member_session_phase(
            self.runtime.as_ref(),
            activation_context,
            pane_id,
            MemberSessionPhase::CaptureOnly,
        )?;
        runtime_state.session_id = detected.session_id;
        runtime_state.jsonl_path = detected.jsonl_path;
        Ok(())
    }

    fn create_pane_for_agent(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<(), CoordinationError> {
        let member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        self.add_member(&request.team_name, member)?;
        runtime_state.member_added = true;

        let launch_cmd = build_cli_launch_command(
            &request.agent,
            &request.team_name,
            MemberRole::Agent,
            cli_commands,
        )?;
        let pane_id = self.runtime.create_aitx_pane_and_launch(
            &request.agent.project_id,
            tmux_layout,
            &launch_cmd,
        )?;
        runtime_state.pane_id = Some(pane_id);
        runtime_state.session_id = None;
        runtime_state.jsonl_path = None;
        runtime_state.attached_at = Some(Utc::now());
        runtime_state.health = Some(HealthState::Healthy);
        Ok(())
    }

    fn launch_session_for_agent(
        &self,
        activation_context: &MemberActivationContext,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        let pane_id = runtime_state.pane_id.as_deref().ok_or_else(|| {
            CoordinationError::Backend(format!(
                "missing pane id for member '{}' in team '{}'",
                request.agent.name, request.team_name
            ))
        })?;
        let detected = run_member_session_phase(
            self.runtime.as_ref(),
            activation_context,
            pane_id,
            MemberSessionPhase::CaptureOnly,
        )?;
        runtime_state.session_id = detected.session_id;
        runtime_state.jsonl_path = detected.jsonl_path;
        Ok(())
    }
}

fn capture_session_identity_message(member: &Member, runtime_state: &PendingResumeState) -> String {
    if !matches!(member.cli_tool, CliTool::Claude | CliTool::Codex) {
        return "session identity not required".to_string();
    }
    if runtime_state.session_id.is_some() || runtime_state.jsonl_path.is_some() {
        return "session identity captured".to_string();
    }
    "session identity unavailable".to_string()
}
