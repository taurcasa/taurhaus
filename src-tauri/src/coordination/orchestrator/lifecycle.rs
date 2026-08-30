use chrono::Utc;

use crate::coordination::audit::{
    AuditEvent, MemberAddedEvent, MemberRemovedEvent, TeamCreatedEvent, TeamDisbandedEvent,
};
use crate::coordination::domain::{HealthState, Member, MemberRole, Team};
use crate::coordination::errors::CoordinationError;
use crate::coordination::pipelines::{ResumeProgressEmitter, ResumeTeamDaemonOwnership};
use crate::coordination::requests::{
    MemberActivationStage, OperatorNoticeDelivery, ResumeMemberRequest, ResumeTeamMemberFailure,
    ResumeTeamReport, StepStatus,
};
use crate::coordination::roster::get_team_roster_with_attachments;
use crate::coordination::stores::{
    MemberRuntimeRecord, MemberRuntimeStore, TeamConfig, TeamConfigStore,
};
use crate::coordination::validation::{validate_member_name, validate_team_name};

use super::helpers::{
    discovered_team_to_status, infer_backend_kind, ordered_members_for_team_resume,
    should_teardown_member_on_team_cleanup,
};
use super::teardown::{
    removal_actor_identity, render_member_removed_notice, step_failed, step_succeeded,
};
use super::{
    CoordinationOrchestrator, DisbandTeamResult, RemoveMemberResult, TeamDiscoveryStatus,
    TeamStatus,
};

impl CoordinationOrchestrator {
    /// Create a new team with empty membership.
    pub fn create_team(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<Team, CoordinationError> {
        validate_team_name(name)?;

        match TeamConfigStore::load(&self.teams_dir, name) {
            Ok(_) => {
                return Err(CoordinationError::Conflict(format!(
                    "team '{name}' already exists"
                )));
            }
            Err(CoordinationError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }

        let now = Utc::now();
        let config = TeamConfig {
            schema_version: 1,
            name: name.to_string(),
            description: description.clone(),
            created_at: now,
            members: Vec::new(),
            extra: Default::default(),
        };
        TeamConfigStore::save(&self.teams_dir, name, &config)?;

        self.audit_log
            .push(AuditEvent::TeamCreated(TeamCreatedEvent {
                team_name: name.to_string(),
                member_count: 0,
                created_at: now,
            }));

        Ok(Team {
            name: name.to_string(),
            description,
            created_at: now,
            schema_version: 1,
        })
    }

    /// Disband a team and remove all persisted state.
    pub fn disband_team(
        &mut self,
        name: &str,
        reason: Option<String>,
    ) -> Result<DisbandTeamResult, CoordinationError> {
        validate_team_name(name)?;
        match TeamConfigStore::load(&self.teams_dir, name) {
            Ok(_) => {}
            Err(CoordinationError::NotFound(_)) => {
                return Ok(DisbandTeamResult {
                    team_name: name.to_string(),
                    disbanded: false,
                    already_disbanded: true,
                });
            }
            Err(err) => return Err(err),
        }

        let roster = match get_team_roster_with_attachments(&self.teams_dir, name) {
            Ok(roster) => roster,
            Err(err) => {
                tracing::warn!(
                    team = %name,
                    error = %err,
                    "failed to load team roster during disband teardown"
                );
                Vec::new()
            }
        };

        for member in roster
            .iter()
            .filter(|member| should_teardown_member_on_team_cleanup(member))
        {
            let runtime = member.runtime_record();
            self.teardown_member_resources_best_effort(
                name,
                &member.member_name,
                Some(member.configured_project_path.as_path()),
                runtime.as_ref(),
            );
        }

        self.stop_team_daemon_best_effort(name);

        TeamConfigStore::delete(&self.teams_dir, name)?;

        self.audit_log
            .push(AuditEvent::TeamDisbanded(TeamDisbandedEvent {
                team_name: name.to_string(),
                reason,
                disbanded_at: Utc::now(),
            }));
        Ok(DisbandTeamResult {
            team_name: name.to_string(),
            disbanded: true,
            already_disbanded: false,
        })
    }

    /// Add a member to an existing team and initialize runtime state.
    pub fn add_member(&mut self, team_name: &str, member: Member) -> Result<(), CoordinationError> {
        validate_team_name(team_name)?;
        validate_member_name(&member.name)?;

        let mut config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        if config
            .members
            .iter()
            .any(|existing| existing.name == member.name)
        {
            return Err(CoordinationError::Conflict(format!(
                "member '{}' already exists in team '{team_name}'",
                member.name
            )));
        }

        config.members.push(member.clone());
        TeamConfigStore::save(&self.teams_dir, team_name, &config)?;

        let runtime = MemberRuntimeRecord {
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
            extra: Default::default(),
        };
        MemberRuntimeStore::save(&self.teams_dir, team_name, &member.name, &runtime)?;

        self.audit_log
            .push(AuditEvent::MemberAdded(MemberAddedEvent {
                team_name: team_name.to_string(),
                member_name: member.name,
                role: member.role,
                backend: infer_backend_kind(member.cli_tool),
                added_at: Utc::now(),
            }));

        Ok(())
    }

    /// Remove a member from an existing team and clear runtime state.
    pub fn remove_member(
        &mut self,
        team_name: &str,
        member_name: &str,
        reason: Option<String>,
    ) -> Result<RemoveMemberResult, CoordinationError> {
        validate_team_name(team_name)?;
        validate_member_name(member_name)?;

        let mut config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let member = config
            .members
            .iter()
            .find(|candidate| candidate.name == member_name)
            .cloned()
            .ok_or_else(|| {
                CoordinationError::NotFound(format!(
                    "member '{member_name}' not found in team '{team_name}'"
                ))
            })?;
        if member.role == MemberRole::Lead {
            return Err(CoordinationError::Validation(format!(
                "member '{member_name}' is the team lead for '{team_name}' and cannot be removed"
            )));
        }

        let original_len = config.members.len();
        config.members.retain(|member| member.name != member_name);
        if config.members.len() == original_len {
            return Err(CoordinationError::NotFound(format!(
                "member '{member_name}' not found in team '{team_name}'"
            )));
        }

        let runtime = match MemberRuntimeStore::load(&self.teams_dir, team_name, member_name) {
            Ok(record) => Some(record),
            Err(CoordinationError::NotFound(_)) => None,
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    error = %err,
                    "failed to load runtime record for remove_member teardown"
                );
                None
            }
        };

        let teardown = self.teardown_member_resources_best_effort(
            team_name,
            member_name,
            Some(member.project_path.as_path()),
            runtime.as_ref(),
        );

        TeamConfigStore::save(&self.teams_dir, team_name, &config)?;
        let mut steps = teardown.steps;
        let mut warnings = teardown.warnings;
        steps.push(step_succeeded("update_config", "team config updated"));

        MemberRuntimeStore::delete(&self.teams_dir, team_name, member_name)?;
        steps.push(step_succeeded("delete_runtime", "runtime record deleted"));

        let lead_name = config
            .members
            .iter()
            .find(|candidate| candidate.role == MemberRole::Lead)
            .or_else(|| config.members.first())
            .map(|candidate| candidate.name.clone());
        let removed_by = removal_actor_identity();
        let cleanup_is_partial = !warnings.is_empty();
        match lead_name {
            Some(lead_name) => {
                let notice = render_member_removed_notice(
                    team_name,
                    member_name,
                    removed_by.as_str(),
                    cleanup_is_partial,
                    warnings.len(),
                );
                match self.deliver_message(
                    crate::coordination::requests::DeliveryRequest::operator_notice(
                        OperatorNoticeDelivery {
                            member_name: lead_name.clone(),
                            team_name: team_name.to_string(),
                            message: notice,
                            sender_name: None,
                            operational_context: None,
                        },
                    ),
                ) {
                    Ok(_) => {
                        steps.push(step_succeeded(
                            "notify_lead",
                            format!("sent removal notice to team lead '{lead_name}'"),
                        ));
                    }
                    Err(err) => {
                        let warning = format!(
                            "failed to notify team lead '{lead_name}' about removal: {err}"
                        );
                        warnings.push(warning.clone());
                        steps.push(step_failed("notify_lead", warning));
                    }
                }
            }
            None => {
                let warning =
                    "skipped lead notification: no lead member found in team config".to_string();
                warnings.push(warning.clone());
                steps.push(step_failed("notify_lead", warning));
            }
        }

        self.audit_log
            .push(AuditEvent::MemberRemoved(MemberRemovedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                reason,
                removed_at: Utc::now(),
            }));
        Ok(RemoveMemberResult {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            removed: true,
            steps,
            warnings,
        })
    }

    /// Resume all persisted members in a team by reusing the per-member resume flow.
    pub fn resume_team_with_cli_commands_and_layout(
        &mut self,
        request: &crate::coordination::requests::ResumeTeamRequest,
        cli_commands: &crate::models::CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<ResumeTeamReport, CoordinationError> {
        self.resume_team_with_cli_commands_and_layout_and_progress(
            request,
            cli_commands,
            tmux_layout,
            None,
        )
    }

    /// Resume all persisted members in a team by reusing the per-member resume flow.
    pub fn resume_team_with_cli_commands_and_layout_and_progress(
        &mut self,
        request: &crate::coordination::requests::ResumeTeamRequest,
        cli_commands: &crate::models::CliCommandSettings,
        tmux_layout: &str,
        mut emit_progress: Option<ResumeProgressEmitter<'_>>,
    ) -> Result<ResumeTeamReport, CoordinationError> {
        validate_team_name(&request.team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        self.reconcile_team_liveness(&request.team_name)?;

        let ordered_members = ordered_members_for_team_resume(&config.members);
        let total_members = ordered_members.len();
        let mut resumed_members = Vec::new();
        let mut failed_members = Vec::new();
        let mut warnings = Vec::new();
        let mut forward_member_progress =
            |member_name: &str,
             member_index: usize,
             member_count: usize,
             stage: MemberActivationStage,
             status: StepStatus,
             message: Option<String>| {
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

        for (index, member) in ordered_members.into_iter().enumerate() {
            let member_request = ResumeMemberRequest {
                team_name: request.team_name.clone(),
                member_name: member.name.clone(),
                reasoning_effort_override: None,
            };
            let report = self.resume_member_with_cli_commands_and_layout_and_progress_owned(
                &member_request,
                cli_commands,
                tmux_layout,
                index + 1,
                total_members,
                Some(&mut forward_member_progress),
                ResumeTeamDaemonOwnership::Caller,
            )?;

            warnings.extend(
                report
                    .warnings
                    .into_iter()
                    .map(|warning| format!("{}: {warning}", report.member_name)),
            );

            if report.resumed {
                resumed_members.push(report.member_name);
            } else {
                failed_members.push(ResumeTeamMemberFailure {
                    member_name: report.member_name,
                    message: report.message,
                    retryable: report.retryable,
                });
            }
        }

        let (started_team_daemon, team_daemon_warning) =
            self.ensure_team_daemon_after_resume_team(request)?;

        Ok(ResumeTeamReport {
            team_name: request.team_name.clone(),
            resumed: !resumed_members.is_empty(),
            total_members,
            resumed_members,
            failed_members,
            warnings,
            started_team_daemon,
            team_daemon_warning,
        })
    }

    /// Best-effort startup reconciliation for stale runtime process metadata.
    pub fn reconcile_runtime_state_on_startup(&mut self) -> Result<(), CoordinationError> {
        let team_names = TeamConfigStore::list(&self.teams_dir)?;
        for team_name in team_names {
            self.reconcile_team_runtime_state(&team_name)?;
        }
        Ok(())
    }

    /// List all persisted teams.
    pub fn list_teams(&self) -> Result<Vec<String>, CoordinationError> {
        let discovery = TeamConfigStore::discover(&self.teams_dir)?;
        Ok(discovery
            .teams
            .into_iter()
            .map(|team| team.team_name)
            .collect())
    }

    /// Discover existing teams with lead project anchors for UI restore.
    pub fn discover_teams(&self) -> Result<TeamDiscoveryStatus, CoordinationError> {
        let discovery = TeamConfigStore::discover(&self.teams_dir)?;
        Ok(TeamDiscoveryStatus {
            teams: discovery
                .teams
                .into_iter()
                .map(discovered_team_to_status)
                .collect(),
            warnings: discovery.warnings,
        })
    }

    /// Get team config and runtime snapshot without any runtime reconciliation.
    pub fn get_team_status_fast(&self, team_name: &str) -> Result<TeamStatus, CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members_runtime = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;
        Ok(TeamStatus {
            config,
            members_runtime,
        })
    }

    /// Get team config and runtime snapshot.
    pub fn get_team_status(&self, team_name: &str) -> Result<TeamStatus, CoordinationError> {
        self.get_team_status_fast(team_name)
    }
}
