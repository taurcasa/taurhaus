use super::*;

use chrono::Utc;

use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::{
    hydrate_member_model_fields, load_role_for_member_hydration, MemberActivationContext,
};
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentReport, AddAgentRequest, DeliveryRequest, InitializeTeamRequest, MemberActivationStage,
    OperatorNoticeDelivery, ResumeAgentReport, ResumeMemberRequest, StepProgress, StepStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InitializeMemberActivationStage {
    CreatePanes,
    LaunchSessions,
    JoinMesh,
    StartDaemons,
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
        SharedMemberActivationExecutor::for_add_agent(self, request, cli_commands, tmux_layout)
            .run_add_agent()
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
    pub fn resume_member_with_cli_commands_and_layout_and_progress<'a>(
        &mut self,
        request: &'a ResumeMemberRequest,
        cli_commands: &'a CliCommandSettings,
        tmux_layout: &'a str,
        member_index: usize,
        member_count: usize,
        emit_progress: Option<ResumeProgressEmitter<'a>>,
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resume_member_with_cli_commands_and_layout_and_progress_owned<'a>(
        &mut self,
        request: &'a ResumeMemberRequest,
        cli_commands: &'a CliCommandSettings,
        tmux_layout: &'a str,
        member_index: usize,
        member_count: usize,
        emit_progress: Option<ResumeProgressEmitter<'a>>,
        team_daemon_ownership: ResumeTeamDaemonOwnership,
    ) -> Result<ResumeAgentReport, CoordinationError> {
        SharedMemberActivationExecutor::for_resume(
            self,
            request,
            cli_commands,
            tmux_layout,
            member_index,
            member_count,
            emit_progress,
            team_daemon_ownership,
        )
        .run_resume()
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
        let mut member = config
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

        let role = member.role_id.as_deref().and_then(|role_id| {
            load_role_for_member_hydration(&self.template_root, role_id, &member.name, "resume")
        });
        hydrate_member_model_fields(&mut member, role.as_ref());

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

struct PreparedMemberActivation {
    member: Member,
    activation_context: MemberActivationContext,
    lead_name: String,
    previous_runtime: Option<MemberRuntimeRecord>,
}

enum SharedMemberActivationWrapper<'b> {
    Initialize {
        request: &'b InitializeTeamRequest,
        member: &'b crate::coordination::requests::AgentSetupConfig,
        role: MemberRole,
    },
    AddAgent {
        request: &'b AddAgentRequest,
    },
    Resume {
        request: &'b ResumeMemberRequest,
        team_daemon_ownership: ResumeTeamDaemonOwnership,
        emit_progress: Option<ResumeProgressEmitter<'b>>,
        member_index: usize,
        member_count: usize,
    },
}

pub(super) struct SharedMemberActivationExecutor<'a, 'b> {
    orchestrator: &'a mut CoordinationOrchestrator,
    wrapper: SharedMemberActivationWrapper<'b>,
    cli_commands: &'b CliCommandSettings,
    tmux_layout: &'b str,
    succeeded_steps: Vec<String>,
    steps: Vec<StepProgress>,
    warnings: Vec<String>,
    runtime_state: PendingResumeState,
}

impl<'a, 'b> SharedMemberActivationExecutor<'a, 'b> {
    pub(super) fn for_initialize(
        orchestrator: &'a mut CoordinationOrchestrator,
        request: &'b InitializeTeamRequest,
        member: &'b crate::coordination::requests::AgentSetupConfig,
        role: MemberRole,
        cli_commands: &'b CliCommandSettings,
        tmux_layout: &'b str,
    ) -> Self {
        Self {
            orchestrator,
            wrapper: SharedMemberActivationWrapper::Initialize {
                request,
                member,
                role,
            },
            cli_commands,
            tmux_layout,
            succeeded_steps: Vec::new(),
            steps: Vec::new(),
            warnings: Vec::new(),
            runtime_state: PendingResumeState::default(),
        }
    }

    fn for_add_agent(
        orchestrator: &'a mut CoordinationOrchestrator,
        request: &'b AddAgentRequest,
        cli_commands: &'b CliCommandSettings,
        tmux_layout: &'b str,
    ) -> Self {
        Self {
            orchestrator,
            wrapper: SharedMemberActivationWrapper::AddAgent { request },
            cli_commands,
            tmux_layout,
            succeeded_steps: Vec::new(),
            steps: Vec::new(),
            warnings: Vec::new(),
            runtime_state: PendingResumeState::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn for_resume(
        orchestrator: &'a mut CoordinationOrchestrator,
        request: &'b ResumeMemberRequest,
        cli_commands: &'b CliCommandSettings,
        tmux_layout: &'b str,
        member_index: usize,
        member_count: usize,
        emit_progress: Option<ResumeProgressEmitter<'b>>,
        team_daemon_ownership: ResumeTeamDaemonOwnership,
    ) -> Self {
        Self {
            orchestrator,
            wrapper: SharedMemberActivationWrapper::Resume {
                request,
                team_daemon_ownership,
                emit_progress,
                member_index,
                member_count,
            },
            cli_commands,
            tmux_layout,
            succeeded_steps: Vec::new(),
            steps: Vec::new(),
            warnings: Vec::new(),
            runtime_state: PendingResumeState::default(),
        }
    }

    fn run_add_agent(mut self) -> Result<AddAgentReport, CoordinationError> {
        let request = self.add_agent_request();
        if let Err(err) = self.orchestrator.validate_add_agent_request(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "validate",
                err,
                self.succeeded_steps,
                &mut self.steps,
            ));
        }
        self.record_step_success("validate", "request validated");

        let prepared = self.prepare_add_agent()?;
        if let Err((failed_step, err)) = self.run_shared_activation(&prepared) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                &failed_step,
                err,
                self.succeeded_steps,
                &mut self.steps,
            ));
        }

        self.orchestrator
            .ensure_team_daemon_after_add_agent(request);
        Ok(AddAgentReport {
            team_name: request.team_name.clone(),
            member_name: request.agent.name.clone(),
            succeeded_steps: self.succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "agent added".to_string(),
            steps: self.steps,
        })
    }

    fn run_resume(mut self) -> Result<ResumeAgentReport, CoordinationError> {
        self.emit_stage(
            MemberActivationStage::PrepareMember,
            StepStatus::Running,
            None,
        );
        let request = self.resume_request();
        if let Err(err) = self.orchestrator.validate_resume_request(request) {
            self.emit_stage(
                MemberActivationStage::PrepareMember,
                StepStatus::Failed,
                Some(err.to_string()),
            );
            return Ok(self.failed_resume_report("validate", err));
        }
        self.record_step_success("validate", "request validated");

        let prepared = match self.prepare_resume() {
            Ok(value) => value,
            Err(err) => {
                self.emit_stage(
                    MemberActivationStage::PrepareMember,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
                return Ok(self.failed_resume_report("load_member", err));
            }
        };
        self.record_step_success("load_member", "member and runtime state loaded");
        self.emit_stage(
            MemberActivationStage::PrepareMember,
            StepStatus::Succeeded,
            Some("member request and runtime state prepared".to_string()),
        );
        let pane_id = match self.run_shared_activation(&prepared) {
            Ok(pane_id) => pane_id,
            Err((failed_step, err)) => return Ok(self.failed_resume_report(&failed_step, err)),
        };

        if self.resume_team_daemon_ownership() == ResumeTeamDaemonOwnership::Wrapper {
            self.orchestrator
                .ensure_team_daemon_after_resume_member(request);
        }

        Ok(ResumeAgentReport {
            team_name: request.team_name.clone(),
            member_name: request.member_name.clone(),
            resumed: true,
            succeeded_steps: self.succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "member resumed".to_string(),
            steps: self.steps,
            warnings: self.warnings,
            pane_id: Some(pane_id),
            reused_pane: self.runtime_state.reused_pane,
        })
    }

    pub(super) fn run_initialize_stage(
        mut self,
        stage: InitializeMemberActivationStage,
        per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
    ) -> Result<(), (String, CoordinationError)> {
        let prepared = match self.prepare_initialize() {
            Ok(prepared) => prepared,
            Err(err) => return Err(("add_lead".to_string(), err)),
        };

        match stage {
            InitializeMemberActivationStage::CreatePanes => {
                if self.initialize_member_skips_launch() {
                    return Ok(());
                }
                self.initialize_create_pane_and_launch(&prepared, per_project_anchor_panes)
            }
            InitializeMemberActivationStage::LaunchSessions => {
                if self.initialize_member_skips_launch() {
                    return Ok(());
                }
                self.initialize_capture_session_identity(&prepared)
            }
            InitializeMemberActivationStage::JoinMesh => self.join_mesh(&prepared),
            InitializeMemberActivationStage::StartDaemons => {
                let pane_id = if prepared.member.cli_tool == CliTool::Claude {
                    String::new()
                } else {
                    self.load_initialize_pane_id(&prepared, "start_daemons")?
                };
                self.start_member_daemon(&prepared, pane_id.as_str())
                    .map_err(|(failed_step, err)| {
                        let failed_step = match failed_step.as_str() {
                            "start_daemon" => "start_daemons".to_string(),
                            _ => failed_step,
                        };
                        (failed_step, err)
                    })?;
                if let Some(daemon_pid) = self.runtime_state.daemon_pid {
                    self.orchestrator
                        .commit_member_runtime(
                            &prepared.activation_context,
                            RuntimeCommitPatch {
                                daemon_pid: Some(Some(daemon_pid)),
                                ..Default::default()
                            },
                        )
                        .map_err(|err| ("start_daemons".to_string(), err))?;
                }
                Ok(())
            }
        }
    }

    fn prepare_initialize(&mut self) -> Result<PreparedMemberActivation, CoordinationError> {
        let (request, member, role) = self.initialize_request();
        let prepared_member = member_from_agent_setup(member, role)?;
        let activation_context = MemberActivationContext::for_initialize_member(
            &request.team_name,
            &request.lead.name,
            member,
            role,
        )?;
        Ok(PreparedMemberActivation {
            member: prepared_member,
            activation_context,
            lead_name: request.lead.name.clone(),
            previous_runtime: None,
        })
    }

    fn prepare_add_agent(&mut self) -> Result<PreparedMemberActivation, CoordinationError> {
        let request = self.add_agent_request();
        let lead_name = self.orchestrator.load_team_lead_name(&request.team_name)?;
        let member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        let activation_context =
            MemberActivationContext::for_add_agent(&request.team_name, &lead_name, &request.agent)?;
        Ok(PreparedMemberActivation {
            member,
            activation_context,
            lead_name,
            previous_runtime: None,
        })
    }

    fn prepare_resume(&mut self) -> Result<PreparedMemberActivation, CoordinationError> {
        let request = self.resume_request();
        let (member, runtime_record, lead_name) =
            self.orchestrator.load_resume_member_state(request)?;
        let activation_context =
            MemberActivationContext::for_resume_member(&request.team_name, &lead_name, &member);
        Ok(PreparedMemberActivation {
            member,
            activation_context,
            lead_name,
            previous_runtime: Some(runtime_record),
        })
    }

    fn run_shared_activation(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<String, (String, CoordinationError)> {
        let pane_id = self.acquire_pane(prepared)?;
        self.launch_session(prepared, &pane_id)?;
        self.capture_session_identity(prepared, &pane_id)?;
        self.join_mesh(prepared)?;
        self.start_member_daemon(prepared, &pane_id)?;
        self.deliver_onboarding(prepared)?;
        self.commit_runtime(prepared)?;
        Ok(pane_id)
    }

    fn acquire_pane(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<String, (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::AcquirePane,
            StepStatus::Running,
            None,
        );
        match &self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => {
                unreachable!("initialize should use initialize_create_pane_and_launch")
            }
            SharedMemberActivationWrapper::AddAgent { request } => {
                if let Err(err) = self
                    .orchestrator
                    .add_member(&request.team_name, prepared.member.clone())
                    .and_then(|_| {
                        self.runtime_state.member_added = true;
                        let pane_id = self.orchestrator.runtime.create_aitx_pane(
                            prepared.member.project_path.to_string_lossy().as_ref(),
                            self.tmux_layout,
                        )?;
                        self.runtime_state.pane_id = Some(pane_id.clone());
                        self.runtime_state.session_id = None;
                        self.runtime_state.jsonl_path = None;
                        self.runtime_state.attached_at = Some(Utc::now());
                        self.runtime_state.health = Some(HealthState::Healthy);
                        Ok::<(), CoordinationError>(())
                    })
                {
                    self.cleanup_failure();
                    self.emit_stage(
                        MemberActivationStage::AcquirePane,
                        StepStatus::Failed,
                        Some(err.to_string()),
                    );
                    return Err(("create_pane".to_string(), err));
                }

                self.record_step_success("create_pane", "pane opened and session started");
                let pane_id = self
                    .runtime_state
                    .pane_id
                    .clone()
                    .expect("add-agent pane acquisition should set pane_id");
                self.emit_stage(
                    MemberActivationStage::AcquirePane,
                    StepStatus::Succeeded,
                    Some(format!("created pane {pane_id}")),
                );
                Ok(pane_id)
            }
            SharedMemberActivationWrapper::Resume { .. } => {
                let runtime_record = prepared
                    .previous_runtime
                    .as_ref()
                    .expect("resume activation should have previous runtime");
                let pane_resolution = match self.orchestrator.resolve_resume_pane(
                    &prepared.member,
                    Some(runtime_record),
                    self.tmux_layout,
                ) {
                    Ok(resolution) => resolution,
                    Err(err) => {
                        self.cleanup_failure();
                        self.emit_stage(
                            MemberActivationStage::AcquirePane,
                            StepStatus::Failed,
                            Some(err.to_string()),
                        );
                        return Err(("resolve_pane".to_string(), err));
                    }
                };
                self.runtime_state.pane_id = Some(pane_resolution.pane_id.clone());
                self.runtime_state.reused_pane = pane_resolution.reused_pane;
                if pane_resolution.created_new_pane {
                    self.runtime_state.created_pane_id = Some(pane_resolution.pane_id.clone());
                    if runtime_record.pane_id.is_some() && !pane_resolution.reused_pane {
                        self.warnings.push(format!(
                            "existing pane was not reusable for '{}'; created a new pane",
                            prepared.member.name
                        ));
                    }
                }
                let message = if pane_resolution.reused_pane {
                    format!("reused pane {}", pane_resolution.pane_id)
                } else {
                    format!("created pane {}", pane_resolution.pane_id)
                };
                self.record_step_success("resolve_pane", &message);
                self.emit_stage(
                    MemberActivationStage::AcquirePane,
                    StepStatus::Succeeded,
                    Some(message),
                );
                Ok(pane_resolution.pane_id)
            }
        }
    }

    fn launch_session(
        &mut self,
        prepared: &PreparedMemberActivation,
        pane_id: &str,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::LaunchSession,
            StepStatus::Running,
            None,
        );
        if let Err(err) = run_member_session_phase(
            self.orchestrator.runtime.as_ref(),
            &prepared.activation_context,
            pane_id,
            MemberSessionPhase::LaunchOnly(self.cli_commands),
        ) {
            self.cleanup_failure();
            self.emit_stage(
                MemberActivationStage::LaunchSession,
                StepStatus::Failed,
                Some(err.to_string()),
            );
            return Err(("launch_session".to_string(), err));
        }

        let (step_message, stage_message) = match self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => {
                unreachable!("initialize should use initialize_create_pane_and_launch")
            }
            SharedMemberActivationWrapper::AddAgent { .. } => {
                ("launched session verified", "launched session verified")
            }
            SharedMemberActivationWrapper::Resume { .. } => {
                ("cli session launched", "cli session launched")
            }
        };
        self.record_step_success("launch_session", step_message);
        self.emit_stage(
            MemberActivationStage::LaunchSession,
            StepStatus::Succeeded,
            Some(stage_message.to_string()),
        );
        Ok(())
    }

    fn capture_session_identity(
        &mut self,
        prepared: &PreparedMemberActivation,
        pane_id: &str,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::CaptureSessionIdentity,
            StepStatus::Running,
            None,
        );
        if let Err(err) = run_member_session_phase(
            self.orchestrator.runtime.as_ref(),
            &prepared.activation_context,
            pane_id,
            MemberSessionPhase::CaptureOnly,
        )
        .map(|detected| {
            self.runtime_state.session_id = detected.session_id;
            self.runtime_state.jsonl_path = detected.jsonl_path;
        }) {
            self.cleanup_failure();
            self.emit_stage(
                MemberActivationStage::CaptureSessionIdentity,
                StepStatus::Failed,
                Some(err.to_string()),
            );
            return Err(("launch_session".to_string(), err));
        }
        self.emit_stage(
            MemberActivationStage::CaptureSessionIdentity,
            StepStatus::Succeeded,
            Some(capture_session_identity_message(
                &prepared.member,
                &self.runtime_state,
            )),
        );
        Ok(())
    }

    fn initialize_create_pane_and_launch(
        &mut self,
        prepared: &PreparedMemberActivation,
        per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
    ) -> Result<(), (String, CoordinationError)> {
        match self.orchestrator.acquire_initialize_member_pane(
            &prepared.activation_context,
            self.cli_commands,
            self.tmux_layout,
            per_project_anchor_panes,
            &mut self.runtime_state,
        ) {
            Ok(_) => Ok(()),
            Err(err) => Err(("create_panes".to_string(), err)),
        }
    }

    fn initialize_capture_session_identity(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<(), (String, CoordinationError)> {
        let pane_id = self.load_initialize_pane_id(prepared, "launch_sessions")?;

        match self
            .orchestrator
            .capture_initialized_member_session_identity(
                &prepared.activation_context,
                &pane_id,
                &mut self.runtime_state,
            ) {
            Ok(()) => Ok(()),
            Err(err) => Err(("launch_sessions".to_string(), err)),
        }
    }

    fn load_initialize_pane_id(
        &mut self,
        prepared: &PreparedMemberActivation,
        failed_step: &'static str,
    ) -> Result<String, (String, CoordinationError)> {
        if let Some(pane_id) = self.runtime_state.pane_id.clone() {
            return Ok(pane_id);
        }

        let runtime = MemberRuntimeStore::load(
            &self.orchestrator.teams_dir,
            &prepared.activation_context.team_name,
            &prepared.member.name,
        )
        .map_err(|err| (failed_step.to_string(), err))?;
        let Some(pane_id) = runtime.pane_id else {
            return Err((
                failed_step.to_string(),
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    prepared.member.name, prepared.activation_context.team_name
                )),
            ));
        };
        self.runtime_state.pane_id = Some(pane_id.clone());
        Ok(pane_id)
    }

    fn join_mesh(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(MemberActivationStage::JoinMesh, StepStatus::Running, None);
        if prepared.member.cli_tool == CliTool::Claude {
            self.record_step_success("join_mesh", "not required for claude");
            self.emit_stage(
                MemberActivationStage::JoinMesh,
                StepStatus::Succeeded,
                Some("not required for claude".to_string()),
            );
            return Ok(());
        }

        let project_id = prepared.member.project_path.display().to_string();
        match join_mesh_if_required(
            self.orchestrator.runtime.as_ref(),
            &prepared.activation_context.team_name,
            &prepared.member.name,
            project_id.as_str(),
            prepared.member.cli_tool,
            &prepared.activation_context.member.model,
        ) {
            Ok(joined) => {
                self.runtime_state.mesh_joined = joined;
                self.record_step_success("join_mesh", "mesh joined");
                self.emit_stage(
                    MemberActivationStage::JoinMesh,
                    StepStatus::Succeeded,
                    Some("mesh joined".to_string()),
                );
                Ok(())
            }
            Err(err) => {
                self.cleanup_failure();
                self.emit_stage(
                    MemberActivationStage::JoinMesh,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
                Err(("join_mesh".to_string(), err))
            }
        }
    }

    fn start_member_daemon(
        &mut self,
        prepared: &PreparedMemberActivation,
        pane_id: &str,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::StartMemberDaemon,
            StepStatus::Running,
            None,
        );
        if prepared.member.cli_tool == CliTool::Claude {
            self.record_step_success("start_daemon", "not required for claude");
            self.emit_stage(
                MemberActivationStage::StartMemberDaemon,
                StepStatus::Succeeded,
                Some("not required for claude".to_string()),
            );
            return Ok(());
        }

        let daemon_result = match &self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => start_member_daemon_if_required(
                self.orchestrator.runtime.as_ref(),
                &prepared.activation_context.team_name,
                &prepared.member.name,
                pane_id,
                prepared.member.cli_tool,
                MemberDaemonStartPolicy::StartFresh,
                None,
            )
            .map(|pid| {
                self.runtime_state.daemon_pid = pid;
            }),
            SharedMemberActivationWrapper::AddAgent { request } => self
                .start_member_daemon_for_wrapper(
                    &request.team_name,
                    &request.agent.name,
                    prepared.member.cli_tool,
                    pane_id,
                    MemberDaemonStartPolicy::StartFresh,
                ),
            SharedMemberActivationWrapper::Resume { request, .. } => self
                .start_member_daemon_for_wrapper(
                    &request.team_name,
                    &prepared.member.name,
                    prepared.member.cli_tool,
                    pane_id,
                    MemberDaemonStartPolicy::ReplaceStalePid {
                        previous_daemon_pid: prepared
                            .previous_runtime
                            .as_ref()
                            .and_then(|runtime| runtime.daemon_pid),
                    },
                ),
        };

        match daemon_result {
            Ok(()) => {
                self.record_step_success("start_daemon", "mesh daemon started");
                self.emit_stage(
                    MemberActivationStage::StartMemberDaemon,
                    StepStatus::Succeeded,
                    Some("mesh daemon started".to_string()),
                );
                Ok(())
            }
            Err(err) => {
                self.cleanup_failure();
                self.emit_stage(
                    MemberActivationStage::StartMemberDaemon,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
                Err(("start_daemon".to_string(), err))
            }
        }
    }

    fn deliver_onboarding(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::DeliverOnboarding,
            StepStatus::Running,
            None,
        );
        let delivery_result = match &self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => {
                unreachable!("initialize onboarding is deferred at the wrapper level")
            }
            SharedMemberActivationWrapper::AddAgent { request } => self
                .orchestrator
                .prepare_add_agent_onboarding_entry(request)
                .and_then(|entry| {
                    self.orchestrator
                        .deliver_onboarding_entries(entry.into_iter().collect())
                }),
            SharedMemberActivationWrapper::Resume { request, .. } => {
                self.deliver_resume_onboarding(request, prepared)
            }
        };

        match delivery_result {
            Ok(()) => {
                self.record_step_success("send_onboarding", "onboarding delivered");
                self.emit_stage(
                    MemberActivationStage::DeliverOnboarding,
                    StepStatus::Succeeded,
                    Some("onboarding delivered".to_string()),
                );
                Ok(())
            }
            Err(err) => {
                self.cleanup_failure();
                self.emit_stage(
                    MemberActivationStage::DeliverOnboarding,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
                Err(("send_onboarding".to_string(), err))
            }
        }
    }

    fn commit_runtime(
        &mut self,
        prepared: &PreparedMemberActivation,
    ) -> Result<(), (String, CoordinationError)> {
        self.emit_stage(
            MemberActivationStage::CommitRuntime,
            StepStatus::Running,
            None,
        );
        let commit_result = match &self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => {
                unreachable!("initialize runtime commits are staged per phase")
            }
            SharedMemberActivationWrapper::AddAgent { request } => self
                .orchestrator
                .update_roster_with_agent(request, &mut self.runtime_state),
            SharedMemberActivationWrapper::Resume { .. } => {
                self.orchestrator.commit_member_runtime(
                    &prepared.activation_context,
                    RuntimeCommitPatch::from_pending_resume_state(
                        &self.runtime_state,
                        Utc::now(),
                        HealthState::Healthy,
                    ),
                )
            }
        };

        match commit_result {
            Ok(()) => {
                let (step, message) = match self.wrapper {
                    SharedMemberActivationWrapper::Initialize { .. } => {
                        unreachable!("initialize runtime commits are staged per phase")
                    }
                    SharedMemberActivationWrapper::AddAgent { .. } => {
                        ("update_roster", "team roster updated")
                    }
                    SharedMemberActivationWrapper::Resume { .. } => {
                        ("update_runtime", "runtime state updated")
                    }
                };
                self.record_step_success(step, message);
                self.emit_stage(
                    MemberActivationStage::CommitRuntime,
                    StepStatus::Succeeded,
                    Some(message.to_string()),
                );
                Ok(())
            }
            Err(err) => {
                self.cleanup_failure();
                self.emit_stage(
                    MemberActivationStage::CommitRuntime,
                    StepStatus::Failed,
                    Some(err.to_string()),
                );
                let failed_step = match self.wrapper {
                    SharedMemberActivationWrapper::Initialize { .. } => {
                        unreachable!("initialize runtime commits are staged per phase")
                    }
                    SharedMemberActivationWrapper::AddAgent { .. } => "update_roster",
                    SharedMemberActivationWrapper::Resume { .. } => "update_runtime",
                };
                Err((failed_step.to_string(), err))
            }
        }
    }

    fn cleanup_failure(&mut self) {
        match &self.wrapper {
            SharedMemberActivationWrapper::Initialize { .. } => {}
            SharedMemberActivationWrapper::AddAgent { request } => self
                .orchestrator
                .cleanup_add_agent_failure(request, &self.runtime_state),
            SharedMemberActivationWrapper::Resume { request, .. } => self
                .orchestrator
                .cleanup_resume_failure(request, &self.runtime_state),
        }
    }

    fn emit_stage(
        &mut self,
        stage: MemberActivationStage,
        status: StepStatus,
        message: Option<String>,
    ) {
        if let SharedMemberActivationWrapper::Resume {
            request,
            emit_progress: Some(emit),
            member_index,
            member_count,
            ..
        } = &mut self.wrapper
        {
            emit(
                request.member_name.as_str(),
                *member_index,
                *member_count,
                stage,
                status,
                message,
            );
        }
    }

    fn record_step_success(&mut self, step: &str, message: &str) {
        mark_step_succeeded(step, message, &mut self.succeeded_steps, &mut self.steps);
    }

    fn start_member_daemon_for_wrapper(
        &mut self,
        team_name: &str,
        member_name: &str,
        cli_tool: CliTool,
        pane_id: &str,
        policy: MemberDaemonStartPolicy,
    ) -> Result<(), CoordinationError> {
        if let Some(new_pid) = start_member_daemon_if_required(
            self.orchestrator.runtime.as_ref(),
            team_name,
            member_name,
            pane_id,
            cli_tool,
            policy,
            Some(&mut self.warnings),
        )? {
            self.runtime_state.daemon_pid = Some(new_pid);
            if matches!(&self.wrapper, SharedMemberActivationWrapper::Resume { .. }) {
                self.runtime_state.new_daemon_pid = Some(new_pid);
            }
        }
        Ok(())
    }

    fn deliver_resume_onboarding(
        &mut self,
        request: &ResumeMemberRequest,
        prepared: &PreparedMemberActivation,
    ) -> Result<(), CoordinationError> {
        if let Some(entry) = self.orchestrator.prepare_resume_onboarding_entry(
            request,
            &prepared.member,
            &prepared.lead_name,
        ) {
            self.orchestrator.deliver_onboarding_entries(vec![entry])?;
            return Ok(());
        }

        if prepared.member.cli_tool != CliTool::Claude {
            return Ok(());
        }

        let message = DeliveryRenderer::render_onboarding(
            &request.team_name,
            &prepared.member.name,
            &prepared.lead_name,
            RoleContext {
                role_id: prepared.member.role_id.as_deref(),
                communication_style: prepared.member.communication_style.as_deref(),
                instructions: prepared.member.instructions.as_deref(),
                behavioral_contract: prepared.member.behavioral_contract.as_ref(),
                quality_gates: prepared.member.quality_gates.as_deref(),
                handoff_expectations: prepared.member.handoff_expectations.as_deref(),
                definition_of_done: prepared.member.definition_of_done.as_deref(),
                capabilities: prepared.member.capabilities.as_deref(),
            },
        );
        self.orchestrator
            .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: prepared.member.name.clone(),
                team_name: request.team_name.clone(),
                message,
                sender_name: Some(prepared.lead_name.clone()),
                operational_context: None,
            }))
            .map(|_| ())
    }

    fn add_agent_request(&self) -> &'b AddAgentRequest {
        match &self.wrapper {
            SharedMemberActivationWrapper::AddAgent { request } => request,
            SharedMemberActivationWrapper::Initialize { .. }
            | SharedMemberActivationWrapper::Resume { .. } => {
                panic!("expected add-agent activation request")
            }
        }
    }

    fn resume_request(&self) -> &'b ResumeMemberRequest {
        match &self.wrapper {
            SharedMemberActivationWrapper::Resume { request, .. } => request,
            SharedMemberActivationWrapper::Initialize { .. }
            | SharedMemberActivationWrapper::AddAgent { .. } => {
                panic!("expected resume activation request")
            }
        }
    }

    fn initialize_request(
        &self,
    ) -> (
        &'b InitializeTeamRequest,
        &'b crate::coordination::requests::AgentSetupConfig,
        MemberRole,
    ) {
        match &self.wrapper {
            SharedMemberActivationWrapper::Initialize {
                request,
                member,
                role,
            } => (request, member, *role),
            SharedMemberActivationWrapper::AddAgent { .. }
            | SharedMemberActivationWrapper::Resume { .. } => {
                panic!("expected initialize activation request")
            }
        }
    }

    fn initialize_member_skips_launch(&self) -> bool {
        matches!(
            &self.wrapper,
            SharedMemberActivationWrapper::Initialize {
                request,
                role: MemberRole::Lead,
                ..
            } if request.lead_mode == crate::coordination::requests::LeadMode::AttachExisting
        )
    }

    fn resume_team_daemon_ownership(&self) -> ResumeTeamDaemonOwnership {
        match &self.wrapper {
            SharedMemberActivationWrapper::Resume {
                team_daemon_ownership,
                ..
            } => *team_daemon_ownership,
            SharedMemberActivationWrapper::Initialize { .. }
            | SharedMemberActivationWrapper::AddAgent { .. } => ResumeTeamDaemonOwnership::Caller,
        }
    }

    fn failed_resume_report(
        &mut self,
        failed_step: &str,
        err: CoordinationError,
    ) -> ResumeAgentReport {
        let request = self.resume_request();
        failed_resume_report(
            &request.team_name,
            &request.member_name,
            failed_step,
            err,
            std::mem::take(&mut self.succeeded_steps),
            &mut self.steps,
            std::mem::take(&mut self.warnings),
            self.runtime_state.pane_id.clone(),
            self.runtime_state.reused_pane,
        )
    }
}
