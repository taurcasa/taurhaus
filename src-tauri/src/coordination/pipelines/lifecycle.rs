use super::*;

use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::{
    MemberActivationContext, MemberActivationDeliveryPolicy, MemberActivationRosterPolicy,
    MemberActivationRuntimeCommitPolicy,
};
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::reinjection::CompactionReinjectionService;
use crate::coordination::requests::{
    AddAgentRequest, AgentSetupConfig, DeliveryRequest, DeliveryResult, InitializeTeamRequest,
    OperatorNoticeDelivery, ResumeMemberRequest, TeardownMode, TeardownRequest,
};
use crate::coordination::stores::lock::acquire_team_lock;
use crate::coordination::stores::{
    MemberRuntimeSnapshot, MemberRuntimeStore, RuntimeCommitOutcome, TeamConfigStore,
};
use crate::session_scanner::cli_tool::CliTool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedOnboardingDelivery {
    pub(super) policy: MemberActivationDeliveryPolicy,
    pub(super) member_name: String,
    pub(super) team_name: String,
    pub(super) sender_name: String,
    pub(super) message: String,
}

impl CoordinationOrchestrator {
    pub(super) fn cleanup_initialize_failure(&mut self, team_name: &str) {
        let _ = self.disband_team(
            team_name,
            Some("initialization failed — cleaning up".to_string()),
        );
    }

    pub(super) fn cleanup_add_agent_failure(
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

        if let Err(err) = self
            .runtime
            .clear_mesh_daemon_pid_file(&request.team_name, &request.agent.name)
        {
            tracing::warn!(
                team = %request.team_name,
                member = %request.agent.name,
                error = %err,
                "hot-add rollback: failed to clear daemon pid file"
            );
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
            match TeamConfigStore::load(&self.teams_dir, &request.team_name) {
                Ok(mut config) => {
                    config
                        .members
                        .retain(|member| member.name != request.agent.name);
                    if let Err(err) =
                        TeamConfigStore::save(&self.teams_dir, &request.team_name, &config)
                    {
                        tracing::warn!(
                            team = %request.team_name,
                            member = %request.agent.name,
                            error = %err,
                            "hot-add rollback: failed to save team config after removing member"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        team = %request.team_name,
                        member = %request.agent.name,
                        error = %err,
                        "hot-add rollback: failed to load team config for member removal"
                    );
                }
            }

            if let Err(err) =
                MemberRuntimeStore::delete(&self.teams_dir, &request.team_name, &request.agent.name)
            {
                tracing::warn!(
                    team = %request.team_name,
                    member = %request.agent.name,
                    error = %err,
                    "hot-add rollback: failed to delete member runtime"
                );
            }
        }
    }

    pub(super) fn cleanup_resume_failure(
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

        if let Err(err) = self
            .runtime
            .clear_mesh_daemon_pid_file(&request.team_name, &request.member_name)
        {
            tracing::warn!(
                team = %request.team_name,
                member = %request.member_name,
                error = %err,
                "resume rollback: failed to clear daemon pid file"
            );
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

    pub(super) fn update_roster_with_agent(
        &mut self,
        request: &AddAgentRequest,
        runtime_state: &mut PendingRuntimeState,
    ) -> Result<(), CoordinationError> {
        let desired_member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        let mut config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        if let Some(existing) = config
            .members
            .iter_mut()
            .find(|member| member.name == desired_member.name)
        {
            *existing = desired_member;
            TeamConfigStore::save(&self.teams_dir, &request.team_name, &config)?;
        } else {
            self.add_member(&request.team_name, desired_member)?;
            runtime_state.member_added = true;
        }

        let context =
            MemberActivationContext::for_add_agent(&request.team_name, &lead_name, &request.agent)?;
        self.commit_member_runtime(
            &context,
            RuntimeCommitPatch::from_pending_runtime_state(runtime_state),
        )
    }

    pub(super) fn commit_member_runtime(
        &self,
        context: &MemberActivationContext,
        patch: RuntimeCommitPatch,
    ) -> Result<(), CoordinationError> {
        let expected = match MemberRuntimeStore::load(
            &self.teams_dir,
            &context.team_name,
            &context.member.name,
        ) {
            Ok(runtime) => MemberRuntimeSnapshot::capture(&runtime),
            Err(CoordinationError::NotFound(_))
                if matches!(
                    context.roster_policy,
                    MemberActivationRosterPolicy::CreateMember
                ) =>
            {
                MemberRuntimeSnapshot::absent(&default_runtime_record(&context.member.name))
            }
            Err(err) => return Err(err),
        };

        let outcome = self.commit_member_runtime_if_unchanged(context, patch, &expected)?;
        match outcome {
            RuntimeCommitOutcome::Committed => {
                if matches!(
                    context.runtime_commit_policy,
                    MemberActivationRuntimeCommitPolicy::FinalizeAtEnd
                ) {
                    self.sync_team_config_metadata(&context.team_name)?;
                }
            }
            RuntimeCommitOutcome::Skipped { .. } => {
                return Err(CoordinationError::Conflict(format!(
                    "runtime changed while activating member '{}'",
                    context.member.name
                )));
            }
        }
        Ok(())
    }

    pub(super) fn commit_member_runtime_if_unchanged(
        &self,
        context: &MemberActivationContext,
        patch: RuntimeCommitPatch,
        expected: &MemberRuntimeSnapshot,
    ) -> Result<RuntimeCommitOutcome, CoordinationError> {
        let guard = acquire_team_lock(&self.teams_dir, &context.team_name)?;

        let outcome = MemberRuntimeStore::commit_if_unchanged(
            &guard,
            &self.teams_dir,
            &context.team_name,
            &context.member.name,
            expected,
            |runtime| {
                runtime.cli_tool.get_or_insert(context.member.cli_tool);
                if runtime.project_path.is_none() {
                    runtime.project_path = Some(context.member.project_path.clone());
                }
                if let Some(pane_id) = patch.pane_id {
                    runtime.pane_id = pane_id;
                }
                if let Some(pane_pid) = patch.pane_pid {
                    runtime.pane_pid = pane_pid;
                }
                if let Some(pane_start_time) = patch.pane_start_time {
                    runtime.pane_start_time = pane_start_time;
                }
                if let Some(session_id) = patch.session_id {
                    runtime.session_id = session_id;
                }
                if let Some(jsonl_path) = patch.jsonl_path {
                    runtime.jsonl_path = jsonl_path;
                }
                if let Some(daemon_pid) = patch.daemon_pid {
                    runtime.daemon_pid = daemon_pid;
                }
                if let Some(attached_at) = patch.attached_at {
                    runtime.attached_at = attached_at;
                }
                if let Some(health) = patch.health {
                    runtime.health = health;
                }
                if let Some(launch_account) = patch.launch_account.as_ref() {
                    runtime.launch_account = launch_account.clone().unwrap_or_default();
                }
                if let Some(applied_effort) = patch.applied_effort {
                    // The launch renderer is the authority on what survived
                    // base-command overrides and validation.
                    runtime.applied_effort = applied_effort;
                }
                // A committed launch reached a level, so an earlier failed
                // effort-switch budget no longer applies.
                runtime.effort_resume_failure = None;
            },
        );
        drop(guard);
        outcome
    }

    pub(super) fn sync_team_config_metadata(
        &self,
        team_name: &str,
    ) -> Result<(), CoordinationError> {
        let config_path = self.teams_dir.join(team_name).join("config.json");
        let config = TeamConfigStore::load(&self.teams_dir, team_name).inspect_err(|err| {
            log_team_config_sync_error(team_name, "load", &config_path, err);
        })?;
        TeamConfigStore::save(&self.teams_dir, team_name, &config).inspect_err(|err| {
            log_team_config_sync_error(team_name, "save", &config_path, err);
        })
    }
}

fn prepare_agent_onboarding_delivery(
    context: MemberActivationContext,
    member: &AgentSetupConfig,
) -> Option<PreparedOnboardingDelivery> {
    let role_context = RoleContext {
        role_id: member.role_id.as_deref(),
        communication_style: member.communication_style.as_deref(),
        instructions: agent_instructions(member),
        behavioral_contract: member.behavioral_contract.as_ref(),
        quality_gates: member.quality_gates.as_deref(),
        handoff_expectations: member.handoff_expectations.as_deref(),
        definition_of_done: member.definition_of_done.as_deref(),
        capabilities: member.capabilities.as_deref(),
    };
    let has_role_context = agent_has_role_context(member);
    prepare_onboarding_delivery(context, has_role_context, role_context)
}

fn prepare_member_onboarding_delivery(
    context: MemberActivationContext,
    member: &Member,
) -> Option<PreparedOnboardingDelivery> {
    let role_context = RoleContext {
        role_id: member.role_id.as_deref(),
        communication_style: member.communication_style.as_deref(),
        instructions: member.instructions.as_deref(),
        behavioral_contract: member.behavioral_contract.as_ref(),
        quality_gates: member.quality_gates.as_deref(),
        handoff_expectations: member.handoff_expectations.as_deref(),
        definition_of_done: member.definition_of_done.as_deref(),
        capabilities: member.capabilities.as_deref(),
    };
    let has_role_context = member_has_role_context(member);
    prepare_onboarding_delivery(context, has_role_context, role_context)
}

fn prepare_onboarding_delivery(
    context: MemberActivationContext,
    has_role_context: bool,
    role_context: RoleContext<'_>,
) -> Option<PreparedOnboardingDelivery> {
    let MemberActivationContext {
        team_name,
        lead,
        member,
        delivery_policy,
        ..
    } = context;
    let message = render_onboarding_message(
        &team_name,
        &member.name,
        &lead.name,
        member.cli_tool,
        has_role_context,
        role_context,
    )?;
    Some(PreparedOnboardingDelivery {
        policy: delivery_policy,
        member_name: member.name,
        team_name,
        sender_name: lead.name,
        message,
    })
}

fn render_onboarding_message(
    team_name: &str,
    member_name: &str,
    lead_name: &str,
    cli_tool: CliTool,
    has_role_context: bool,
    role_context: RoleContext<'_>,
) -> Option<String> {
    DeliveryRenderer::render_for_tool(
        cli_tool,
        team_name,
        member_name,
        lead_name,
        has_role_context,
        role_context,
    )
}

fn log_team_config_sync_error(
    team_name: &str,
    operation: &str,
    path: &std::path::Path,
    err: &CoordinationError,
) {
    let mut fields = Map::new();
    fields.insert(
        "team_name".to_string(),
        Value::String(team_name.to_string()),
    );
    fields.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    fields.insert("error".to_string(), Value::String(err.to_string()));
    fields.insert(
        "raw_os_error".to_string(),
        err.raw_os_error()
            .map(|code| Value::Number(code.into()))
            .unwrap_or(Value::Null),
    );
    emit_global(
        "warn",
        "coordination",
        "coordination.team_config.sync_failed",
        Some("Team config metadata sync failed".to_string()),
        fields,
    );
    tracing::warn!(
        team = %team_name,
        operation,
        path = %path.display(),
        error = %err,
        raw_os_error = ?err.raw_os_error(),
        "team config metadata sync failed"
    );
}

impl CoordinationOrchestrator {
    pub(super) fn deliver_onboarding_entries(
        &mut self,
        entries: Vec<PreparedOnboardingDelivery>,
    ) -> Result<Vec<DeliveryResult>, CoordinationError> {
        let mut deferred_entries = Vec::new();
        let mut results = Vec::new();

        for entry in entries {
            match entry.policy {
                MemberActivationDeliveryPolicy::Immediate => {
                    results.push(self.deliver_prepared_onboarding(entry)?);
                }
                MemberActivationDeliveryPolicy::DeferredBarrier => deferred_entries.push(entry),
            }
        }

        for entry in deferred_entries {
            results.push(self.deliver_prepared_onboarding(entry)?);
        }

        Ok(results)
    }

    pub(super) fn prepare_initialize_onboarding_entries(
        &self,
        request: &InitializeTeamRequest,
    ) -> Result<Vec<PreparedOnboardingDelivery>, CoordinationError> {
        let mut entries = Vec::with_capacity(1 + request.agents.len());

        let lead_context = MemberActivationContext::for_initialize_member(
            &request.team_name,
            &request.lead.name,
            &request.lead,
            MemberRole::Lead,
        )?;
        if let Some(entry) = prepare_agent_onboarding_delivery(lead_context, &request.lead) {
            entries.push(entry);
        }

        for agent in &request.agents {
            let context = MemberActivationContext::for_initialize_member(
                &request.team_name,
                &request.lead.name,
                agent,
                MemberRole::Agent,
            )?;
            if let Some(entry) = prepare_agent_onboarding_delivery(context, agent) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    pub(super) fn prepare_resume_onboarding_entry(
        &self,
        request: &ResumeMemberRequest,
        member: &Member,
        lead_name: &str,
    ) -> Option<PreparedOnboardingDelivery> {
        let context =
            MemberActivationContext::for_resume_member(&request.team_name, lead_name, member);
        let mut entry = prepare_member_onboarding_delivery(context, member)?;
        CompactionReinjectionService::append_member_lease_context(
            &mut entry.message,
            &self.teams_dir,
            &request.team_name,
            &member.name,
        );
        Some(entry)
    }

    pub(super) fn prepare_add_agent_onboarding_entry(
        &self,
        request: &AddAgentRequest,
    ) -> Result<Option<PreparedOnboardingDelivery>, CoordinationError> {
        let team = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = team
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());
        let context =
            MemberActivationContext::for_add_agent(&request.team_name, &lead_name, &request.agent)?;
        Ok(prepare_agent_onboarding_delivery(context, &request.agent))
    }

    fn deliver_prepared_onboarding(
        &mut self,
        entry: PreparedOnboardingDelivery,
    ) -> Result<DeliveryResult, CoordinationError> {
        self.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: entry.member_name,
            team_name: entry.team_name,
            message: entry.message,
            sender_name: Some(entry.sender_name),
            operational_context: None,
        }))
    }
}
