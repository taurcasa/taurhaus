//! Coordination orchestrator service.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;

use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent, MemberAddedEvent, MemberRemovedEvent, TeamCreatedEvent,
    TeamDisbandedEvent,
};
use crate::coordination::backend::{BackendKind, CoordinationBackend};
use crate::coordination::domain::{HealthState, Member, MemberRole, Team};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    DeliveryMethod, DeliveryRequest, DeliveryResult, OperatorNoticeDelivery, ResumeMemberRequest,
    ResumeTeamMemberFailure, ResumeTeamReport, TeardownMode, TeardownRequest,
};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{
    DiscoveredTeam, MemberRuntimeRecord, MemberRuntimeStore, TeamConfig, TeamConfigStore,
};
use crate::coordination::validation::{validate_member_name, validate_team_name};
use crate::session_scanner::cli_tool::CliTool;

/// Aggregated status view for a single team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamStatus {
    pub config: TeamConfig,
    pub members_runtime: Vec<(String, MemberRuntimeRecord)>,
}

/// Discovered team status used to restore mesh tabs on app reopen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTeamStatus {
    pub team_name: String,
    pub lead_project_path: Option<PathBuf>,
}

/// Team discovery payload with recoverable warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDiscoveryStatus {
    pub teams: Vec<DiscoveredTeamStatus>,
    pub warnings: Vec<String>,
}

/// Result of disbanding a team with idempotent semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisbandTeamResult {
    pub team_name: String,
    pub disbanded: bool,
    pub already_disbanded: bool,
}

/// Result of removing a member with teardown diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMemberResult {
    pub team_name: String,
    pub member_name: String,
    pub removed: bool,
    pub steps: Vec<RemoveMemberStepResult>,
    pub warnings: Vec<String>,
}

/// Result of a bounded team self-heal pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSelfHealResult {
    pub team_name: String,
    pub runtime_candidate_found: bool,
    pub member_liveness_reconciled: bool,
    pub team_daemon_ensured: bool,
}

/// Per-step teardown status for runtime member removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMemberStepResult {
    pub step: String,
    pub success: bool,
    pub message: Option<String>,
}

/// Top-level coordination service entrypoint.
pub struct CoordinationOrchestrator {
    pub(crate) teams_dir: PathBuf,
    pub(crate) audit_log: Vec<AuditEvent>,
    pub(crate) backend: Arc<dyn CoordinationBackend>,
    pub(crate) runtime: Arc<dyn CoordinationRuntime>,
}

impl std::fmt::Debug for CoordinationOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinationOrchestrator")
            .field("teams_dir", &self.teams_dir)
            .field("audit_log_len", &self.audit_log.len())
            .field("backend_kind", &self.backend.kind())
            .finish()
    }
}

impl CoordinationOrchestrator {
    pub fn new(teams_dir: PathBuf, backend: Arc<dyn CoordinationBackend>) -> Self {
        Self::new_with_runtime(teams_dir, backend, Arc::new(SystemCoordinationRuntime))
    }

    pub fn new_with_runtime(
        teams_dir: PathBuf,
        backend: Arc<dyn CoordinationBackend>,
        runtime: Arc<dyn CoordinationRuntime>,
    ) -> Self {
        Self {
            teams_dir,
            audit_log: Vec::new(),
            backend,
            runtime,
        }
    }

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
        let config = match TeamConfigStore::load(&self.teams_dir, name) {
            Ok(config) => config,
            Err(CoordinationError::NotFound(_)) => {
                return Ok(DisbandTeamResult {
                    team_name: name.to_string(),
                    disbanded: false,
                    already_disbanded: true,
                });
            }
            Err(err) => return Err(err),
        };

        let runtime_by_member = match MemberRuntimeStore::load_all(&self.teams_dir, name) {
            Ok(records) => records.into_iter().collect::<HashMap<_, _>>(),
            Err(err) => {
                tracing::warn!(
                    team = %name,
                    error = %err,
                    "failed to load runtime records during disband teardown"
                );
                HashMap::new()
            }
        };

        for member in config.members.iter().filter(|member| {
            should_teardown_member_on_team_cleanup(member, runtime_by_member.get(&member.name))
        }) {
            self.teardown_member_resources_best_effort(
                name,
                &member.name,
                Some(member.project_path.as_path()),
                runtime_by_member.get(&member.name),
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
            schema_version: 1,
            member_name: member.name.clone(),
            pane_id: None,
            session_id: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
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
                match self.backend.deliver(DeliveryRequest::OperatorNotice(
                    OperatorNoticeDelivery {
                        member_name: lead_name.clone(),
                        team_name: team_name.to_string(),
                        message: notice,
                        sender_name: None,
                    },
                )) {
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
        validate_team_name(&request.team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        self.reconcile_team_liveness(&request.team_name)?;

        let ordered_members = ordered_members_for_team_resume(&config.members);
        let total_members = ordered_members.len();
        let mut resumed_members = Vec::new();
        let mut failed_members = Vec::new();
        let mut warnings = Vec::new();

        for member in ordered_members {
            let member_request = ResumeMemberRequest {
                team_name: request.team_name.clone(),
                member_name: member.name.clone(),
                context_mode: request.context_mode,
            };
            let report = self.resume_member_with_cli_commands_and_layout(
                &member_request,
                cli_commands,
                tmux_layout,
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

        let operator_name = removal_actor_identity();
        let (started_team_daemon, team_daemon_warning) = match self
            .runtime
            .spawn_team_daemon(&request.team_name, &operator_name)
        {
            Ok(pid) => {
                tracing::info!(
                    team = %request.team_name,
                    operator = %operator_name,
                    pid = pid,
                    "team daemon ensured running after team resume"
                );
                (true, None)
            }
            Err(err) => {
                tracing::warn!(
                    team = %request.team_name,
                    operator = %operator_name,
                    error = %err,
                    "failed to ensure team daemon is running after team resume"
                );
                (false, Some(err.to_string()))
            }
        };

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

    /// Reconcile member liveness for a team using pane + daemon state.
    ///
    /// This is a write-on-drift repair pass for explicit recovery and background
    /// self-heal flows. It is intentionally not used on UI-critical snapshot paths.
    pub fn reconcile_team_liveness(&mut self, team_name: &str) -> Result<(), CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members_by_name = config
            .members
            .into_iter()
            .map(|member| (member.name.clone(), member))
            .collect::<HashMap<_, _>>();
        let runtime_records = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;

        for (member_name, mut runtime) in runtime_records {
            let Some(member) = members_by_name.get(&member_name) else {
                continue;
            };

            let (offline_detected, reason) = match runtime.pane_id.as_deref() {
                None => (true, "missing_pane_id"),
                Some(pane_id) => {
                    if !self.runtime.pane_exists(pane_id)? {
                        (true, "pane_missing")
                    } else if self.runtime.pane_is_dead(pane_id)? {
                        (true, "pane_dead")
                    } else if self.runtime.pane_is_shell(pane_id)? {
                        (true, "pane_shell")
                    } else {
                        (false, "pane_active")
                    }
                }
            };

            if offline_detected {
                // Write only when the persisted health is stale.
                if runtime.health == HealthState::SessionDead {
                    continue;
                }

                runtime.health = HealthState::SessionDead;
                runtime.session_id = None;

                if member.cli_tool != CliTool::Claude {
                    if let Some(pid) = runtime.daemon_pid {
                        match self.runtime.is_process_running_by_pid(pid) {
                            Ok(true) => {
                                if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                                    tracing::warn!(
                                        team = %team_name,
                                        member = %member_name,
                                        pid = pid,
                                        error = %err,
                                        "failed to terminate stale daemon during liveness reconciliation"
                                    );
                                }
                            }
                            Ok(false) => {}
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    error = %err,
                                    "failed to check daemon pid during liveness reconciliation"
                                );
                            }
                        }
                        runtime.daemon_pid = None;
                    }
                }

                MemberRuntimeStore::save(&self.teams_dir, team_name, &member_name, &runtime)?;
                tracing::info!(
                    team = %team_name,
                    member = %member_name,
                    reason,
                    "reconciled member liveness drift to offline"
                );
                continue;
            }

            let mut runtime_changed = false;
            if member.cli_tool != CliTool::Claude {
                let pane_id = runtime.pane_id.as_deref();
                let discovered_daemon_pids = if let Some(pane_id) = pane_id {
                    match self.runtime.find_existing_mesh_daemon_pids(
                        pane_id,
                        team_name,
                        &member_name,
                    ) {
                        Ok(pids) => pids,
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pane_id = %pane_id,
                                error = %err,
                                "failed to discover existing mesh daemons during liveness reconciliation"
                            );
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                };

                let mut retained_daemon_pid = None;
                let daemon_needs_restart = match runtime.daemon_pid {
                    Some(pid) => match self.runtime.is_process_running_by_pid(pid) {
                        Ok(true) => match self.runtime.mesh_daemon_uses_current_binary(pid) {
                            Ok(true) => {
                                retained_daemon_pid = Some(pid);
                                false
                            }
                            Ok(false) => {
                                if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                                    tracing::warn!(
                                        team = %team_name,
                                        member = %member_name,
                                        pid = pid,
                                        error = %err,
                                        "failed to terminate binary-drifted mesh daemon during liveness reconciliation"
                                    );
                                }
                                runtime.daemon_pid = None;
                                runtime_changed = true;
                                tracing::info!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    "detected running mesh daemon binary drift during liveness reconciliation"
                                );
                                true
                            }
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pid = pid,
                                    error = %err,
                                    "failed to verify mesh daemon binary identity during liveness reconciliation"
                                );
                                false
                            }
                        },
                        Ok(false) => {
                            runtime.daemon_pid = None;
                            runtime_changed = true;
                            false
                        }
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = pid,
                                error = %err,
                                "failed to verify daemon pid during liveness reconciliation"
                            );
                            false
                        }
                    },
                    None => false,
                };

                if retained_daemon_pid.is_none() && !discovered_daemon_pids.is_empty() {
                    retained_daemon_pid = discovered_daemon_pids.first().copied();
                    runtime.daemon_pid = retained_daemon_pid;
                    runtime_changed = true;
                    if let Some(pid) = retained_daemon_pid {
                        tracing::info!(
                            team = %team_name,
                            member = %member_name,
                            pane_id = %runtime.pane_id.as_deref().unwrap_or_default(),
                            pid = pid,
                            "adopted existing mesh daemon during liveness reconciliation"
                        );
                    }
                }

                if let Some(retained_pid) = retained_daemon_pid {
                    for duplicate_pid in discovered_daemon_pids
                        .into_iter()
                        .filter(|pid| *pid != retained_pid)
                    {
                        if let Err(err) = self.runtime.terminate_process_by_pid(duplicate_pid) {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = duplicate_pid,
                                retained_pid = retained_pid,
                                error = %err,
                                "failed to terminate duplicate mesh daemon during liveness reconciliation"
                            );
                        } else {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pid = duplicate_pid,
                                retained_pid = retained_pid,
                                "terminated duplicate mesh daemon during liveness reconciliation"
                            );
                        }
                    }
                } else if daemon_needs_restart || runtime.daemon_pid.is_none() {
                    if let Some(pane_id) = pane_id {
                        match self
                            .runtime
                            .spawn_mesh_daemon(pane_id, team_name, &member_name)
                        {
                            Ok(pid) => {
                                runtime.daemon_pid = Some(pid);
                                runtime_changed = true;
                                tracing::info!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    pid = pid,
                                    "restarted mesh daemon during liveness reconciliation"
                                );
                            }
                            Err(err) => {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    error = %err,
                                    "failed to restart mesh daemon during liveness reconciliation"
                                );
                            }
                        }
                    }
                }
            }

            if runtime.health != HealthState::SessionDead && !runtime_changed {
                continue;
            }

            runtime.health = HealthState::Healthy;
            runtime.last_seen_at = Some(Utc::now());
            MemberRuntimeStore::save(&self.teams_dir, team_name, &member_name, &runtime)?;
            tracing::info!(
                team = %team_name,
                member = %member_name,
                reason,
                "reconciled member liveness drift to healthy"
            );
        }

        Ok(())
    }

    /// Run a bounded self-heal pass for a single team.
    ///
    /// This repairs per-member liveness drift and ensures the team daemon is
    /// running, but only when the persisted runtime indicates there is active or
    /// recoverable team state worth healing.
    pub fn trigger_team_self_heal(
        &mut self,
        team_name: &str,
    ) -> Result<TeamSelfHealResult, CoordinationError> {
        validate_team_name(team_name)?;

        let initial_status = self.get_team_status_fast(team_name)?;
        let team_daemon_binary_drifted =
            !self.runtime.team_daemon_uses_current_binary(team_name)?;
        let runtime_candidate_found = team_is_self_heal_candidate(&initial_status.members_runtime)
            || team_daemon_binary_drifted;
        if !runtime_candidate_found {
            return Ok(TeamSelfHealResult {
                team_name: team_name.to_string(),
                runtime_candidate_found: false,
                member_liveness_reconciled: false,
                team_daemon_ensured: false,
            });
        }

        self.reconcile_team_liveness(team_name)?;

        if team_daemon_binary_drifted {
            tracing::info!(
                team = %team_name,
                "detected running team daemon binary drift during self-heal"
            );
            self.stop_team_daemon_best_effort(team_name);
        }

        let refreshed_status = self.get_team_status_fast(team_name)?;
        let team_daemon_ensured = team_daemon_binary_drifted
            || team_should_ensure_daemon(&refreshed_status.members_runtime);
        if team_daemon_ensured {
            self.ensure_team_daemon_running_best_effort(team_name);
        }

        Ok(TeamSelfHealResult {
            team_name: team_name.to_string(),
            runtime_candidate_found,
            member_liveness_reconciled: true,
            team_daemon_ensured,
        })
    }

    fn reconcile_team_runtime_state(&self, team_name: &str) -> Result<(), CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let member_names = config
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<HashSet<_>>();
        let runtime_records = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;

        for (member_name, mut runtime) in runtime_records {
            if !member_names.contains(&member_name) {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    "orphan runtime record found during startup reconciliation"
                );
                self.teardown_member_resources_best_effort(
                    team_name,
                    &member_name,
                    None,
                    Some(&runtime),
                );
                if let Err(err) =
                    MemberRuntimeStore::delete(&self.teams_dir, team_name, &member_name)
                {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        error = %err,
                        "failed to delete orphan runtime record during startup reconciliation"
                    );
                }
                continue;
            }

            let Some(pid) = runtime.daemon_pid else {
                continue;
            };

            match self.runtime.is_process_running_by_pid(pid) {
                Ok(true) => {}
                Ok(false) => {
                    runtime.daemon_pid = None;
                    runtime.health = HealthState::SessionDead;
                    MemberRuntimeStore::save(&self.teams_dir, team_name, &member_name, &runtime)?;
                    tracing::info!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        "cleared stale daemon pid during startup reconciliation"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        error = %err,
                        "failed to verify daemon pid during startup reconciliation"
                    );
                }
            }
        }

        Ok(())
    }

    fn teardown_member_resources_best_effort(
        &self,
        team_name: &str,
        member_name: &str,
        member_project_path: Option<&Path>,
        runtime: Option<&MemberRuntimeRecord>,
    ) -> TeardownDiagnostics {
        let mut diagnostics = TeardownDiagnostics::default();
        let pane_id = runtime.and_then(|record| record.pane_id.as_deref());

        let mut daemon_pids = Vec::new();
        if let Some(pid) = runtime.and_then(|record| record.daemon_pid) {
            daemon_pids.push(pid);
        }
        if let Some(pane_id) = pane_id {
            match self
                .runtime
                .find_existing_mesh_daemon_pids(pane_id, team_name, member_name)
            {
                Ok(found_pids) => daemon_pids.extend(found_pids),
                Err(err) => {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pane_id = %pane_id,
                        error = %err,
                        "failed to discover mesh daemons during teardown"
                    );
                    diagnostics.steps.push(step_failed(
                        "discover_daemon",
                        format!("failed to discover daemon state for pane {pane_id}: {err}"),
                    ));
                    diagnostics.warnings.push(format!(
                        "failed to discover daemon state for pane {pane_id}: {err}"
                    ));
                }
            }
        }
        daemon_pids.sort_unstable();
        daemon_pids.dedup();

        if daemon_pids.is_empty() {
            diagnostics
                .steps
                .push(step_succeeded("terminate_daemon", "no daemon pid recorded"));
        } else {
            let mut terminated = Vec::new();
            for pid in daemon_pids {
                if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        error = %err,
                        "failed to terminate daemon during teardown"
                    );
                    diagnostics.steps.push(step_failed(
                        "terminate_daemon",
                        format!("failed to terminate daemon pid {pid}: {err}"),
                    ));
                    diagnostics
                        .warnings
                        .push(format!("failed to terminate daemon pid {pid}: {err}"));
                } else {
                    terminated.push(pid);
                }
            }

            if !terminated.is_empty() {
                diagnostics.steps.push(step_succeeded(
                    "terminate_daemon",
                    format!(
                        "terminated daemon pid{} {}",
                        if terminated.len() == 1 { "" } else { "s" },
                        terminated
                            .iter()
                            .map(u32::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }

        if let Err(err) = self
            .runtime
            .clear_mesh_daemon_pid_file(team_name, member_name)
        {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to clear daemon pid file during teardown"
            );
            diagnostics.steps.push(step_failed(
                "clear_daemon_pid_file",
                format!("failed to clear daemon pid file: {err}"),
            ));
            diagnostics
                .warnings
                .push(format!("failed to clear daemon pid file: {err}"));
        } else {
            diagnostics.steps.push(step_succeeded(
                "clear_daemon_pid_file",
                "daemon pid file cleared",
            ));
        }

        if let Err(err) = self.backend.teardown(TeardownRequest {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            mode: TeardownMode::Graceful,
        }) {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to leave mesh during teardown"
            );
            diagnostics.steps.push(step_failed(
                "leave_mesh",
                format!("failed to leave mesh: {err}"),
            ));
            diagnostics
                .warnings
                .push(format!("failed to leave mesh membership: {err}"));
        } else {
            diagnostics
                .steps
                .push(step_succeeded("leave_mesh", "mesh presence removed"));
        }

        if let Some(pane_id) = pane_id {
            match member_project_path {
                Some(project_path) => {
                    let project_path = project_path.display().to_string();
                    match self
                        .runtime
                        .pane_belongs_to_project(pane_id, project_path.as_str())
                    {
                        Ok(true) => {
                            diagnostics.steps.push(step_succeeded(
                                "verify_pane_ownership",
                                format!("pane {pane_id} matched project {project_path}"),
                            ));
                            if let Err(err) = self.runtime.kill_aitx_pane(pane_id) {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    error = %err,
                                    "failed to kill pane during teardown"
                                );
                                diagnostics.steps.push(step_failed(
                                    "kill_pane",
                                    format!("failed to kill pane {pane_id}: {err}"),
                                ));
                                diagnostics
                                    .warnings
                                    .push(format!("failed to kill pane {pane_id}: {err}"));
                            } else {
                                diagnostics.steps.push(step_succeeded(
                                    "kill_pane",
                                    format!("pane {pane_id} terminated"),
                                ));
                            }
                        }
                        Ok(false) => {
                            diagnostics.steps.push(step_failed(
                                "verify_pane_ownership",
                                format!(
                                    "pane {pane_id} did not match expected project {project_path}"
                                ),
                            ));
                            diagnostics.warnings.push(format!(
                                "skipped pane teardown for {pane_id}: ownership mismatch for {project_path}"
                            ));
                            diagnostics.steps.push(step_failed(
                                "kill_pane",
                                format!(
                                    "skipped pane kill for {pane_id} due to ownership mismatch"
                                ),
                            ));
                        }
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pane_id = %pane_id,
                                error = %err,
                                "failed to verify pane ownership during teardown"
                            );
                            diagnostics.steps.push(step_failed(
                                "verify_pane_ownership",
                                format!("failed to verify pane ownership for {pane_id}: {err}"),
                            ));
                            diagnostics.warnings.push(format!(
                                "skipped pane teardown for {pane_id}: ownership check failed ({err})"
                            ));
                            diagnostics.steps.push(step_failed(
                                "kill_pane",
                                format!(
                                    "skipped pane kill for {pane_id} because ownership check failed"
                                ),
                            ));
                        }
                    }
                }
                None => {
                    diagnostics.steps.push(step_failed(
                        "verify_pane_ownership",
                        format!("no project path recorded for member '{member_name}'"),
                    ));
                    diagnostics.warnings.push(format!(
                        "skipped pane teardown for {pane_id}: missing project path for ownership check"
                    ));
                    diagnostics.steps.push(step_failed(
                        "kill_pane",
                        format!("skipped pane kill for {pane_id} because project path is missing"),
                    ));
                }
            }
        } else {
            diagnostics.steps.push(step_succeeded(
                "verify_pane_ownership",
                "no pane id recorded",
            ));
            diagnostics
                .steps
                .push(step_succeeded("kill_pane", "no pane id recorded"));
        }

        diagnostics
    }

    pub(crate) fn ensure_team_daemon_running_best_effort(&self, team_name: &str) {
        let operator_name = removal_actor_identity();
        match self.runtime.spawn_team_daemon(team_name, &operator_name) {
            Ok(pid) => tracing::info!(
                team = %team_name,
                operator = %operator_name,
                pid = pid,
                "team daemon ensured running"
            ),
            Err(err) => tracing::warn!(
                team = %team_name,
                operator = %operator_name,
                error = %err,
                "failed to ensure team daemon is running"
            ),
        }
    }

    pub(crate) fn stop_team_daemon_best_effort(&self, team_name: &str) {
        if let Err(err) = self.runtime.stop_team_daemon(team_name) {
            tracing::warn!(
                team = %team_name,
                error = %err,
                "failed to stop team daemon during teardown"
            );
        }
    }

    /// Drain buffered audit events and clear the in-memory log.
    pub fn drain_audit_log(&mut self) -> Vec<AuditEvent> {
        std::mem::take(&mut self.audit_log)
    }

    /// Flush buffered audit events to tracing and clear the in-memory buffer.
    pub fn flush_audit_to_log(&mut self) {
        for event in self.audit_log.drain(..) {
            let event_type = event.event_type();
            let json = serde_json::to_string(&event)
                .unwrap_or_else(|err| format!("{{\"error\":\"{err}\"}}"));
            tracing::info!(target: "coordination_audit", event_type, "{json}");
        }
    }

    /// Route a delivery request through the backend and emit audit events.
    pub fn deliver_message(
        &mut self,
        request: DeliveryRequest,
    ) -> Result<DeliveryResult, CoordinationError> {
        let (team_name, member_name) = delivery_meta(&request);
        let delivery_type = delivery_type_name(&request).to_string();
        let attempted_method = default_method_for_backend(self.backend.kind());
        let team_name_owned = team_name.to_string();
        let member_name_owned = member_name.to_string();

        validate_team_name(team_name)?;
        validate_member_name(member_name)?;

        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        if !config
            .members
            .iter()
            .any(|member| member.name == member_name)
        {
            return Err(CoordinationError::NotFound(format!(
                "member '{member_name}' not found in team '{team_name}'"
            )));
        }

        self.audit_log
            .push(AuditEvent::DeliveryAttempted(DeliveryAttemptedEvent {
                team_name: team_name_owned.clone(),
                member_name: member_name_owned.clone(),
                delivery_type: delivery_type.clone(),
                method: attempted_method,
                attempted_at: Utc::now(),
            }));

        match self.backend.deliver(request) {
            Ok(result) => {
                self.audit_log
                    .push(AuditEvent::DeliverySucceeded(DeliverySucceededEvent {
                        team_name: team_name_owned.clone(),
                        member_name: member_name_owned.clone(),
                        delivery_type,
                        method: result.method,
                        succeeded_at: Utc::now(),
                    }));

                if let Ok(mut runtime) =
                    MemberRuntimeStore::load(&self.teams_dir, &team_name_owned, &member_name_owned)
                {
                    runtime.last_seen_at = Some(Utc::now());
                    if let Err(err) = MemberRuntimeStore::save(
                        &self.teams_dir,
                        &team_name_owned,
                        &member_name_owned,
                        &runtime,
                    ) {
                        tracing::warn!(
                            team_name = %team_name_owned,
                            member_name = %member_name_owned,
                            error = %err,
                            "failed to persist runtime last_seen after successful delivery"
                        );
                    }
                }

                Ok(result)
            }
            Err(err) => {
                self.audit_log
                    .push(AuditEvent::DeliveryFailed(DeliveryFailedEvent {
                        team_name: team_name_owned,
                        member_name: member_name_owned,
                        delivery_type,
                        error: err.to_string(),
                        failed_at: Utc::now(),
                    }));
                Err(err)
            }
        }
    }

    /// Record a lease-claim audit event.
    pub fn record_lease_claimed(
        &mut self,
        team_name: &str,
        member_name: &str,
        pid: u32,
        instance_uuid: &str,
    ) {
        self.audit_log
            .push(AuditEvent::LeaseClaimed(LeaseClaimedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                owner_pid: pid,
                instance_uuid: instance_uuid.to_string(),
                claimed_at: Utc::now(),
            }));
    }

    /// Record a lease-reclaim audit event.
    pub fn record_lease_reclaimed(
        &mut self,
        team_name: &str,
        member_name: &str,
        previous_pid: u32,
        new_pid: u32,
    ) {
        self.audit_log
            .push(AuditEvent::LeaseReclaimed(LeaseReclaimedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                previous_pid,
                new_pid,
                reclaimed_at: Utc::now(),
            }));
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct TeardownDiagnostics {
    steps: Vec<RemoveMemberStepResult>,
    warnings: Vec<String>,
}

fn step_succeeded(step: &str, message: impl Into<String>) -> RemoveMemberStepResult {
    RemoveMemberStepResult {
        step: step.to_string(),
        success: true,
        message: Some(message.into()),
    }
}

fn step_failed(step: &str, message: impl Into<String>) -> RemoveMemberStepResult {
    RemoveMemberStepResult {
        step: step.to_string(),
        success: false,
        message: Some(message.into()),
    }
}

fn removal_actor_identity() -> String {
    std::env::var("TAURHAUS_OPERATOR")
        .ok()
        .and_then(non_empty_trimmed)
        .or_else(|| std::env::var("USER").ok().and_then(non_empty_trimmed))
        .or_else(|| std::env::var("USERNAME").ok().and_then(non_empty_trimmed))
        .unwrap_or_else(|| "unknown".to_string())
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn render_member_removed_notice(
    team_name: &str,
    removed_member: &str,
    removed_by: &str,
    cleanup_is_partial: bool,
    warning_count: usize,
) -> String {
    let cleanup = if cleanup_is_partial {
        format!(
            "partial ({warning_count} warning{})",
            if warning_count == 1 { "" } else { "s" }
        )
    } else {
        "complete".to_string()
    };

    format!(
        "[taurhaus] member_removed: '{removed_member}' was removed from team '{team_name}' by '{removed_by}'. Cleanup: {cleanup}."
    )
}

fn infer_backend_kind(tool: CliTool) -> BackendKind {
    match tool {
        CliTool::Claude => BackendKind::ClaudeNative,
        CliTool::Codex | CliTool::Gemini => BackendKind::MeshBridged,
    }
}

fn delivery_meta(req: &DeliveryRequest) -> (&str, &str) {
    match req {
        DeliveryRequest::Bootstrap(payload) => (&payload.team_name, &payload.member_name),
        DeliveryRequest::RecoveryNudge(payload) => (&payload.team_name, &payload.member_name),
        DeliveryRequest::OperatorNotice(payload) => (&payload.team_name, &payload.member_name),
    }
}

fn delivery_type_name(req: &DeliveryRequest) -> &'static str {
    match req {
        DeliveryRequest::Bootstrap(_) => "bootstrap",
        DeliveryRequest::RecoveryNudge(_) => "recovery_nudge",
        DeliveryRequest::OperatorNotice(_) => "operator_notice",
    }
}

fn default_method_for_backend(kind: BackendKind) -> DeliveryMethod {
    match kind {
        BackendKind::ClaudeNative => DeliveryMethod::NativeMessageApi,
        BackendKind::MeshBridged => DeliveryMethod::TmuxInjection,
    }
}

fn discovered_team_to_status(team: DiscoveredTeam) -> DiscoveredTeamStatus {
    DiscoveredTeamStatus {
        team_name: team.team_name,
        lead_project_path: team.lead_project_path,
    }
}

fn should_teardown_member_on_team_cleanup(
    member: &Member,
    runtime: Option<&MemberRuntimeRecord>,
) -> bool {
    if member.role != MemberRole::Lead {
        return true;
    }

    if member.cli_tool != CliTool::Claude {
        return true;
    }

    runtime.is_some_and(|record| {
        record.daemon_pid.is_some() || (record.pane_id.is_some() && record.attached_at.is_some())
    })
}

fn team_is_self_heal_candidate(runtime_records: &[(String, MemberRuntimeRecord)]) -> bool {
    runtime_records.iter().any(|(_, record)| {
        record.health != HealthState::SessionDead
            || record.daemon_pid.is_some()
            || record.pane_id.is_some()
            || record.session_id.is_some()
            || record.attached_at.is_some()
    })
}

fn team_should_ensure_daemon(runtime_records: &[(String, MemberRuntimeRecord)]) -> bool {
    runtime_records.iter().any(|(_, record)| {
        record.health != HealthState::SessionDead
            || record.daemon_pid.is_some()
            || record.session_id.is_some()
    })
}

fn ordered_members_for_team_resume(members: &[Member]) -> Vec<Member> {
    let Some(lead) = members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
    else {
        return members.to_vec();
    };

    let mut ordered = vec![lead.clone()];
    ordered.extend(
        members
            .iter()
            .filter(|member| member.name != lead.name && member.project_path == lead.project_path)
            .cloned(),
    );
    ordered.extend(
        members
            .iter()
            .filter(|member| member.name != lead.name && member.project_path != lead.project_path)
            .cloned(),
    );
    ordered
}

#[cfg(test)]
mod tests;
