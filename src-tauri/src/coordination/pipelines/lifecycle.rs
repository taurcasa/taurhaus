use super::*;

use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentRequest, DeliveryRequest, OperatorNoticeDelivery, ResumeMemberRequest, TeardownMode,
    TeardownRequest,
};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
use crate::session_scanner::cli_tool::CliTool;

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

    pub(super) fn resume_join_mesh(
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

    pub(super) fn resume_start_daemon(
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

    pub(super) fn resume_send_onboarding(
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
        self.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member.name.clone(),
            team_name: request.team_name.clone(),
            message: onboarding,
            sender_name: Some(lead_name.to_string()),
            operational_context: None,
        }))?;
        Ok(())
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

    pub(super) fn join_mesh_for_agent(
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

    pub(super) fn start_daemon_for_agent(
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

    pub(super) fn send_onboarding_for_agent(
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
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
                sender_name: Some(lead_name),
                operational_context: None,
            }))?;
        Ok(())
    }

    pub(super) fn update_roster_with_agent(
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

    pub(super) fn sync_team_config_metadata(
        &self,
        team_name: &str,
    ) -> Result<(), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        TeamConfigStore::save(&self.teams_dir, team_name, &config)
    }
}
