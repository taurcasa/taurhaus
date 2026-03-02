//! Coordination orchestrator service.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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
    DeliveryMethod, DeliveryRequest, DeliveryResult, TeardownMode, TeardownRequest,
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

        for member in config
            .members
            .iter()
            .filter(|member| member.role != MemberRole::Lead)
        {
            self.teardown_member_resources_best_effort(
                name,
                &member.name,
                runtime_by_member.get(&member.name),
            );
        }

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
    ) -> Result<(), CoordinationError> {
        validate_team_name(team_name)?;
        validate_member_name(member_name)?;

        let mut config = TeamConfigStore::load(&self.teams_dir, team_name)?;
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

        self.teardown_member_resources_best_effort(team_name, member_name, runtime.as_ref());

        TeamConfigStore::save(&self.teams_dir, team_name, &config)?;
        MemberRuntimeStore::delete(&self.teams_dir, team_name, member_name)?;

        self.audit_log
            .push(AuditEvent::MemberRemoved(MemberRemovedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                reason,
                removed_at: Utc::now(),
            }));
        Ok(())
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

    /// Get team config and runtime snapshot.
    pub fn get_team_status(&self, team_name: &str) -> Result<TeamStatus, CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members_runtime = MemberRuntimeStore::load_all(&self.teams_dir, team_name)?;
        Ok(TeamStatus {
            config,
            members_runtime,
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
                self.teardown_member_resources_best_effort(team_name, &member_name, Some(&runtime));
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
        runtime: Option<&MemberRuntimeRecord>,
    ) {
        if let Some(pid) = runtime.and_then(|record| record.daemon_pid) {
            if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pid = pid,
                    error = %err,
                    "failed to terminate daemon during teardown"
                );
            }
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
        }

        if let Some(pane_id) = runtime.and_then(|record| record.pane_id.as_deref()) {
            if let Err(err) = self.runtime.kill_aitx_pane(pane_id) {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    error = %err,
                    "failed to kill pane during teardown"
                );
            }
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
                    let _ = MemberRuntimeStore::save(
                        &self.teams_dir,
                        &team_name_owned,
                        &member_name_owned,
                        &runtime,
                    );
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

#[cfg(test)]
mod tests;
