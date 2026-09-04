use super::*;

use chrono::Utc;

use crate::coordination::audit::{AuditEvent, MemberAddedEvent};
use crate::coordination::backend::BackendKind;
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::MemberActivationContext;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::pipelines::members::{
    InitializeMemberActivationStage, SharedMemberActivationExecutor,
};
use crate::coordination::requests::{
    InitializeReport, InitializeTeamRequest, LeadMode, StepProgress, StepStatus,
};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfig, TeamConfigStore};
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
        self.initialize_team_with_cli_commands_and_layout_and_progress(
            request,
            cli_commands,
            tmux_layout,
            None,
        )
    }

    pub fn initialize_team_with_cli_commands_and_layout_and_progress(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        mut emit_progress: Option<InitializeProgressEmitter<'_>>,
    ) -> Result<InitializeReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

        emit_initialize_step_progress(
            "validate_configuration",
            StepStatus::Running,
            None,
            &mut emit_progress,
        );
        if let Err(err) = self.validate_initialize_configuration(request) {
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                "validate_configuration",
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }
        mark_initialize_step_succeeded(
            "validate_configuration",
            "configuration validated",
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
        );

        emit_initialize_step_progress("create_team", StepStatus::Running, None, &mut emit_progress);
        if let Err(err) = self.create_team(&request.team_name, request.team_description.clone()) {
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                "create_team",
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }
        mark_initialize_step_succeeded(
            "create_team",
            "team created",
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
        );

        emit_initialize_step_progress("add_lead", StepStatus::Running, None, &mut emit_progress);
        let lead_member = match member_from_agent_setup(&request.lead, MemberRole::Lead) {
            Ok(member) => member,
            Err(err) => {
                self.cleanup_initialize_failure(&request.team_name);
                return Ok(failed_initialize_report_with_progress(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                    &mut emit_progress,
                ));
            }
        };
        let agent_members = match request
            .agents
            .iter()
            .map(|agent| member_from_agent_setup(agent, MemberRole::Agent))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(members) => members,
            Err(err) => {
                self.cleanup_initialize_failure(&request.team_name);
                return Ok(failed_initialize_report_with_progress(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                    &mut emit_progress,
                ));
            }
        };
        if let Err(err) = self.seed_initialize_roster(
            &request.team_name,
            request.team_description.clone(),
            lead_member,
            &agent_members,
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                "add_lead",
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }
        mark_initialize_step_succeeded(
            "add_lead",
            "lead added",
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
        );

        let total_members = 1 + request.agents.len();
        let mut per_project_anchor_panes = std::collections::HashMap::<String, String>::new();
        let mut initialize_members = Vec::with_capacity(total_members);
        initialize_members.push((&request.lead, MemberRole::Lead));
        initialize_members.extend(
            request
                .agents
                .iter()
                .map(|agent| (agent, MemberRole::Agent)),
        );

        if let Err((failed_step, err)) = self.run_initialize_stage_pass(
            request,
            &initialize_members,
            "create_panes",
            InitializeMemberActivationStage::CreatePanes,
            cli_commands,
            tmux_layout,
            &mut per_project_anchor_panes,
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
            |_| Ok(()),
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                &failed_step,
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }

        if let Err((failed_step, err)) = self.run_initialize_stage_pass(
            request,
            &initialize_members,
            "launch_sessions",
            InitializeMemberActivationStage::LaunchSessions,
            cli_commands,
            tmux_layout,
            &mut per_project_anchor_panes,
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
            |orchestrator| orchestrator.sync_team_config_metadata(&request.team_name),
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                &failed_step,
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }

        if let Err((failed_step, err)) = self.run_initialize_stage_pass(
            request,
            &initialize_members,
            "join_mesh",
            InitializeMemberActivationStage::JoinMesh,
            cli_commands,
            tmux_layout,
            &mut per_project_anchor_panes,
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
            |_| Ok(()),
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                &failed_step,
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }

        if let Err((failed_step, err)) = self.run_initialize_stage_pass(
            request,
            &initialize_members,
            "start_daemons",
            InitializeMemberActivationStage::StartDaemons,
            cli_commands,
            tmux_layout,
            &mut per_project_anchor_panes,
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
            |_| Ok(()),
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                &failed_step,
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }

        emit_initialize_step_progress(
            "send_onboarding",
            StepStatus::Running,
            None,
            &mut emit_progress,
        );
        if let Err(err) = self.send_onboarding_messages(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report_with_progress(
                &request.team_name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
                &mut emit_progress,
            ));
        }
        mark_initialize_step_succeeded(
            "send_onboarding",
            "onboarding messages sent",
            &mut succeeded_steps,
            &mut steps,
            &mut emit_progress,
        );

        self.ensure_team_daemon_after_initialize(request);

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
        if request.lead_mode == LeadMode::AttachExisting && should_use_mesh_sidecar(&request.lead)?
        {
            return Err(CoordinationError::Validation(format!(
                "attach-existing is not supported yet for '{}' leads; use launch-new",
                request.lead.cli_tool
            )));
        }

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

    fn seed_initialize_roster(
        &mut self,
        team_name: &str,
        team_description: Option<String>,
        lead_member: crate::coordination::domain::Member,
        agent_members: &[crate::coordination::domain::Member],
    ) -> Result<(), CoordinationError> {
        let created_at = Utc::now();
        let mut members = Vec::with_capacity(1 + agent_members.len());
        members.push(lead_member);
        members.extend(agent_members.iter().cloned());

        TeamConfigStore::save(
            &self.teams_dir,
            team_name,
            &TeamConfig {
                schema_version: 1,
                name: team_name.to_string(),
                description: team_description,
                created_at,
                members: members.clone(),
                extra: Default::default(),
            },
        )?;

        for member in members {
            MemberRuntimeStore::save(
                &self.teams_dir,
                team_name,
                &member.name,
                &crate::coordination::stores::MemberRuntimeRecord {
                    schema_version: 3,
                    member_name: member.name.clone(),
                    cli_tool: Some(member.cli_tool),
                    project_path: Some(member.project_path.clone()),
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
                    applied_effort: None,
                    effort_resume_failure: None,
                    launch_account: Default::default(),
                    extra: Default::default(),
                },
            )?;

            self.audit_log
                .push(AuditEvent::MemberAdded(MemberAddedEvent {
                    team_name: team_name.to_string(),
                    member_name: member.name,
                    role: member.role,
                    backend: backend_kind_for_member_tool(member.cli_tool),
                    added_at: created_at,
                }));
        }

        Ok(())
    }

    pub(super) fn acquire_initialize_member_pane(
        &self,
        context: &MemberActivationContext,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<String, CoordinationError> {
        let launch = build_member_activation_launch_command(context, cli_commands)?;
        record_context_launch_telemetry(&self.teams_dir, context, &launch);
        let pane_id = launch_member_pane(
            self.runtime.as_ref(),
            per_project_anchor_panes,
            tmux_layout,
            &context.member.project_path.to_string_lossy(),
            &launch.command,
        )?;
        let account = launch.account_result();
        runtime_state.launch_account = (!account.is_empty()).then_some(account);
        runtime_state.applied_effort = launch.applied_effort.clone();
        runtime_state.pane_id = Some(pane_id.clone());
        capture_member_pane_identity(self.runtime.as_ref(), &pane_id, runtime_state)?;
        runtime_state.session_id = None;
        runtime_state.jsonl_path = None;
        runtime_state.daemon_pid = None;
        runtime_state.attached_at = Some(Utc::now());
        runtime_state.health = Some(HealthState::Healthy);
        self.commit_member_runtime(
            context,
            RuntimeCommitPatch {
                pane_id: Some(Some(pane_id.clone())),
                pane_pid: Some(runtime_state.pane_pid),
                pane_start_time: Some(runtime_state.pane_start_time),
                session_id: Some(None),
                jsonl_path: Some(None),
                daemon_pid: Some(None),
                attached_at: Some(runtime_state.attached_at),
                health: Some(HealthState::Healthy),
                launch_account: Some(runtime_state.launch_account.clone()),
                applied_effort: Some(runtime_state.applied_effort.clone()),
            },
        )?;
        Ok(pane_id)
    }

    pub(super) fn capture_initialized_member_session_identity(
        &self,
        context: &MemberActivationContext,
        pane_id: &str,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        let detected = run_member_session_phase(
            self.runtime.as_ref(),
            &self.teams_dir,
            context,
            pane_id,
            MemberSessionPhase::CaptureOnly,
            runtime_state,
        )?;
        runtime_state.session_id = detected.session_id.clone();
        runtime_state.jsonl_path = detected.jsonl_path.clone();
        let Some(session_id) = detected.session_id else {
            return Ok(());
        };

        self.commit_member_runtime(
            context,
            RuntimeCommitPatch {
                session_id: Some(Some(session_id)),
                jsonl_path: Some(detected.jsonl_path),
                ..Default::default()
            },
        )
    }

    fn send_onboarding_messages(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        let entries = self.prepare_initialize_onboarding_entries(request)?;
        self.deliver_onboarding_entries(entries).map(|_| ())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_initialize_stage_pass<F>(
        &mut self,
        request: &InitializeTeamRequest,
        initialize_members: &[(&crate::coordination::requests::AgentSetupConfig, MemberRole)],
        step: &str,
        activation_stage: InitializeMemberActivationStage,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
        succeeded_steps: &mut Vec<String>,
        steps: &mut Vec<StepProgress>,
        emit_progress: &mut Option<InitializeProgressEmitter<'_>>,
        after_stage: F,
    ) -> Result<(), (String, CoordinationError)>
    where
        F: FnOnce(&mut CoordinationOrchestrator) -> Result<(), CoordinationError>,
    {
        emit_initialize_step_progress(step, StepStatus::Running, None, emit_progress);
        let mut best_effort_message = None;
        for (member, role) in initialize_members {
            if let Some(message) = SharedMemberActivationExecutor::for_initialize(
                self,
                request,
                member,
                *role,
                cli_commands,
                tmux_layout,
            )
            .run_initialize_stage(activation_stage, per_project_anchor_panes)?
            {
                best_effort_message = Some(message);
            }
        }
        after_stage(self).map_err(|err| (step.to_string(), err))?;
        mark_initialize_step_succeeded(
            step,
            best_effort_message
                .as_deref()
                .unwrap_or_else(|| initialize_stage_success_message(step)),
            succeeded_steps,
            steps,
            emit_progress,
        );
        Ok(())
    }
}

fn emit_initialize_step_progress(
    step: &str,
    status: StepStatus,
    message: Option<String>,
    emit_progress: &mut Option<InitializeProgressEmitter<'_>>,
) {
    if let Some(emit) = emit_progress.as_deref_mut() {
        emit(step, status, message);
    }
}

fn mark_initialize_step_succeeded(
    step: &str,
    message: &str,
    succeeded_steps: &mut Vec<String>,
    steps: &mut Vec<StepProgress>,
    emit_progress: &mut Option<InitializeProgressEmitter<'_>>,
) {
    mark_step_succeeded(step, message, succeeded_steps, steps);
    emit_initialize_step_progress(
        step,
        StepStatus::Succeeded,
        Some(message.to_string()),
        emit_progress,
    );
}

fn failed_initialize_report_with_progress(
    team_name: &str,
    step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
    emit_progress: &mut Option<InitializeProgressEmitter<'_>>,
) -> InitializeReport {
    emit_initialize_step_progress(
        step,
        StepStatus::Failed,
        Some(err.to_string()),
        emit_progress,
    );
    failed_initialize_report(team_name, step, err, succeeded_steps, steps)
}

fn initialize_stage_success_message(stage: &str) -> &'static str {
    match stage {
        "create_panes" => "panes opened and sessions started",
        "launch_sessions" => "launched sessions verified",
        "join_mesh" => "mesh joined",
        "start_daemons" => "mesh daemons started",
        _ => "step completed",
    }
}

fn launch_member_pane(
    runtime: &dyn crate::coordination::runtime::CoordinationRuntime,
    per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
    tmux_layout: &str,
    project_path: &str,
    launch_cmd: &str,
) -> Result<String, CoordinationError> {
    if tmux_layout == "per_project" {
        if let Some(anchor_pane) = per_project_anchor_panes.get(project_path) {
            return runtime.create_aitx_pane_and_launch_in_target(
                project_path,
                anchor_pane,
                launch_cmd,
            );
        }
    }

    let pane_id = runtime.create_aitx_pane_and_launch(project_path, tmux_layout, launch_cmd)?;
    if tmux_layout == "per_project" {
        per_project_anchor_panes
            .entry(project_path.to_string())
            .or_insert_with(|| pane_id.clone());
    }
    Ok(pane_id)
}

fn backend_kind_for_member_tool(tool: CliTool) -> BackendKind {
    if crate::session_scanner::cli_tool::spec(tool)
        .capabilities
        .native_inbox_poller
    {
        BackendKind::ClaudeNative
    } else {
        BackendKind::MeshBridged
    }
}
