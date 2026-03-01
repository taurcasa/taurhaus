//! Coordination orchestrator service.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent, MemberAddedEvent, MemberRemovedEvent, TeamCreatedEvent,
    TeamDisbandedEvent,
};
use crate::coordination::backend::{BackendKind, CoordinationBackend};
use crate::coordination::domain::{HealthState, Member, Team};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{DeliveryMethod, DeliveryRequest, DeliveryResult};
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfig, TeamConfigStore};
use crate::session_scanner::cli_tool::CliTool;

/// Aggregated status view for a single team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamStatus {
    pub config: TeamConfig,
    pub members_runtime: Vec<(String, MemberRuntimeRecord)>,
}

/// Top-level coordination service entrypoint.
pub struct CoordinationOrchestrator {
    teams_dir: PathBuf,
    audit_log: Vec<AuditEvent>,
    backend: Arc<dyn CoordinationBackend>,
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
        Self {
            teams_dir,
            audit_log: Vec::new(),
            backend,
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

        self.audit_log.push(AuditEvent::TeamCreated(TeamCreatedEvent {
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
    ) -> Result<(), CoordinationError> {
        validate_team_name(name)?;
        TeamConfigStore::load(&self.teams_dir, name)?;
        TeamConfigStore::delete(&self.teams_dir, name)?;

        self.audit_log
            .push(AuditEvent::TeamDisbanded(TeamDisbandedEvent {
                team_name: name.to_string(),
                reason,
                disbanded_at: Utc::now(),
            }));
        Ok(())
    }

    /// Add a member to an existing team and initialize runtime state.
    pub fn add_member(&mut self, team_name: &str, member: Member) -> Result<(), CoordinationError> {
        validate_team_name(team_name)?;
        validate_member_name(&member.name)?;

        let mut config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        if config.members.iter().any(|existing| existing.name == member.name) {
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
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
        };
        MemberRuntimeStore::save(&self.teams_dir, team_name, &member.name, &runtime)?;

        self.audit_log.push(AuditEvent::MemberAdded(MemberAddedEvent {
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

    /// List all persisted teams.
    pub fn list_teams(&self) -> Result<Vec<String>, CoordinationError> {
        TeamConfigStore::list(&self.teams_dir)
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
        if !config.members.iter().any(|member| member.name == member_name) {
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
                self.audit_log.push(AuditEvent::DeliveryFailed(DeliveryFailedEvent {
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
        self.audit_log.push(AuditEvent::LeaseClaimed(LeaseClaimedEvent {
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

fn validate_team_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("team name", name)?;
    if has_path_separator(name) {
        return Err(CoordinationError::Validation(format!(
            "team name '{name}' must not contain path separators"
        )));
    }
    Ok(())
}

fn validate_member_name(name: &str) -> Result<(), CoordinationError> {
    validate_non_empty("member name", name)?;
    if has_path_separator(name) {
        return Err(CoordinationError::Validation(format!(
            "member name '{name}' must not contain path separators"
        )));
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), CoordinationError> {
    if value.trim().is_empty() {
        return Err(CoordinationError::Validation(format!("{field} must not be empty")));
    }
    Ok(())
}

fn has_path_separator(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::domain::MemberRole;
    use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};

    fn sample_member(name: &str, tool: CliTool) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            instructions: Some("focus on implementation".to_string()),
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: tool,
        }
    }

    fn new_orchestrator(tmp: &TempDir) -> CoordinationOrchestrator {
        CoordinationOrchestrator::new(tmp.path().to_path_buf(), Arc::new(FakeBackend::default()))
    }

    fn new_orchestrator_with_backend(
        tmp: &TempDir,
        backend: Arc<dyn CoordinationBackend>,
    ) -> CoordinationOrchestrator {
        CoordinationOrchestrator::new(tmp.path().to_path_buf(), backend)
    }

    fn assert_conflict(err: CoordinationError) {
        match err {
            CoordinationError::Conflict(_) => {}
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    fn assert_not_found(err: CoordinationError) {
        match err {
            CoordinationError::NotFound(_) => {}
            other => panic!("expected not_found, got {other:?}"),
        }
    }

    #[test]
    fn create_team_then_list() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        orchestrator
            .create_team("architecture-final", Some("desc".to_string()))
            .expect("create should succeed");

        let teams = orchestrator.list_teams().expect("list should succeed");
        assert_eq!(teams, vec!["architecture-final".to_string()]);
    }

    #[test]
    fn create_team_duplicate_returns_conflict() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        orchestrator
            .create_team("architecture-final", None)
            .expect("first create should succeed");
        let err = orchestrator
            .create_team("architecture-final", None)
            .expect_err("duplicate create should fail");
        assert_conflict(err);
    }

    #[test]
    fn disband_team_removes_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .disband_team(team_name, Some("cleanup".to_string()))
            .expect("disband should succeed");

        assert!(
            !tmp.path().join(team_name).exists(),
            "team directory should be removed"
        );
    }

    #[test]
    fn disband_nonexistent_team_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        let err = orchestrator
            .disband_team("missing-team", None)
            .expect_err("expected not_found");
        assert_not_found(err);
    }

    #[test]
    fn add_member_then_get_status() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");

        let status = orchestrator
            .get_team_status(team_name)
            .expect("status should load");
        assert_eq!(status.config.members.len(), 1);
        assert_eq!(status.config.members[0].name, member_name);
        assert_eq!(status.members_runtime.len(), 1);
        assert_eq!(status.members_runtime[0].0, member_name);
        assert_eq!(status.members_runtime[0].1.health, HealthState::SessionDead);
    }

    #[test]
    fn add_duplicate_member_returns_conflict() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member = sample_member("codex-reviewer", CliTool::Codex);

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, member.clone())
            .expect("first add should succeed");
        let err = orchestrator
            .add_member(team_name, member)
            .expect_err("duplicate add should fail");
        assert_conflict(err);
    }

    #[test]
    fn remove_member_cleans_runtime() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");
        orchestrator
            .remove_member(team_name, member_name, Some("cleanup".to_string()))
            .expect("remove should succeed");

        let status = orchestrator
            .get_team_status(team_name)
            .expect("status should load");
        assert!(status.config.members.is_empty());
        assert!(status.members_runtime.is_empty());
    }

    #[test]
    fn remove_nonexistent_member_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        let err = orchestrator
            .remove_member(team_name, "missing-member", None)
            .expect_err("expected not_found");
        assert_not_found(err);
    }

    #[test]
    fn audit_log_captures_events() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");
        orchestrator
            .remove_member(team_name, member_name, None)
            .expect("remove should succeed");

        let event_types: Vec<&str> = orchestrator
            .drain_audit_log()
            .into_iter()
            .map(|event| event.event_type())
            .collect();
        assert_eq!(
            event_types,
            vec!["team_created", "member_added", "member_removed"]
        );
    }

    #[test]
    fn deliver_operator_notice_succeeds() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");

        let result = orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: member_name.to_string(),
                team_name: team_name.to_string(),
                message: "status?".to_string(),
            }))
            .expect("delivery should succeed");
        assert!(result.delivered);

        let event_types: Vec<&str> = orchestrator
            .drain_audit_log()
            .into_iter()
            .map(|event| event.event_type())
            .collect();
        assert_eq!(
            event_types,
            vec![
                "team_created",
                "member_added",
                "delivery_attempted",
                "delivery_succeeded"
            ]
        );
    }

    #[test]
    fn deliver_to_nonexistent_member_fails() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");

        let err = orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: "missing-member".to_string(),
                team_name: team_name.to_string(),
                message: "status?".to_string(),
            }))
            .expect_err("delivery should fail");
        assert_not_found(err);
    }

    #[test]
    fn deliver_updates_runtime_last_seen() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");

        let before = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
            .expect("runtime should exist before delivery");
        assert!(before.last_seen_at.is_none());

        orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: member_name.to_string(),
                team_name: team_name.to_string(),
                message: "status?".to_string(),
            }))
            .expect("delivery should succeed");

        let after = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
            .expect("runtime should exist after delivery");
        assert!(after.last_seen_at.is_some());
    }

    #[test]
    fn deliver_backend_failure_emits_failed_event() {
        let tmp = TempDir::new().expect("tempdir");
        let fake = Arc::new(FakeBackend::default());
        fake.set_deliver_error(CoordinationError::Backend("simulated delivery failure".to_string()));
        let backend: Arc<dyn CoordinationBackend> = fake.clone();
        let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");

        let err = orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: member_name.to_string(),
                team_name: team_name.to_string(),
                message: "status?".to_string(),
            }))
            .expect_err("delivery should fail");
        match err {
            CoordinationError::Backend(msg) => assert!(msg.contains("simulated")),
            other => panic!("expected backend error, got {other:?}"),
        }

        let event_types: Vec<&str> = orchestrator
            .drain_audit_log()
            .into_iter()
            .map(|event| event.event_type())
            .collect();
        assert_eq!(
            event_types,
            vec![
                "team_created",
                "member_added",
                "delivery_attempted",
                "delivery_failed"
            ]
        );
    }

    #[test]
    fn full_lifecycle() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, Some("lifecycle".to_string()))
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member("member-a", CliTool::Codex))
            .expect("add a should succeed");
        orchestrator
            .add_member(team_name, sample_member("member-b", CliTool::Claude))
            .expect("add b should succeed");
        orchestrator
            .remove_member(team_name, "member-a", Some("done".to_string()))
            .expect("remove should succeed");
        orchestrator
            .disband_team(team_name, Some("shutdown".to_string()))
            .expect("disband should succeed");

        let events = orchestrator.drain_audit_log();
        let event_types: Vec<&str> = events.iter().map(|event| event.event_type()).collect();
        assert_eq!(
            event_types,
            vec![
                "team_created",
                "member_added",
                "member_added",
                "member_removed",
                "team_disbanded"
            ]
        );
    }

    #[test]
    fn flush_audit_to_log_clears_buffer() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        orchestrator
            .create_team("architecture-final", Some("desc".to_string()))
            .expect("create should succeed");
        assert!(!orchestrator.drain_audit_log().is_empty(), "sanity: event should exist");

        orchestrator
            .create_team("second-team", Some("desc".to_string()))
            .expect("create should succeed");
        orchestrator.flush_audit_to_log();
        assert!(
            orchestrator.drain_audit_log().is_empty(),
            "flush should clear buffered events"
        );
    }

    #[test]
    fn lease_claimed_emits_event() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        orchestrator.record_lease_claimed("architecture-final", "codex-reviewer", 4242, "inst-1");

        let events = orchestrator.drain_audit_log();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AuditEvent::LeaseClaimed(payload) => {
                assert_eq!(payload.team_name, "architecture-final");
                assert_eq!(payload.member_name, "codex-reviewer");
                assert_eq!(payload.owner_pid, 4242);
                assert_eq!(payload.instance_uuid, "inst-1");
            }
            other => panic!("expected lease_claimed event, got {other:?}"),
        }
    }

    #[test]
    fn lease_reclaimed_emits_event() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        orchestrator.record_lease_reclaimed("architecture-final", "codex-reviewer", 1111, 2222);

        let events = orchestrator.drain_audit_log();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AuditEvent::LeaseReclaimed(payload) => {
                assert_eq!(payload.team_name, "architecture-final");
                assert_eq!(payload.member_name, "codex-reviewer");
                assert_eq!(payload.previous_pid, 1111);
                assert_eq!(payload.new_pid, 2222);
            }
            other => panic!("expected lease_reclaimed event, got {other:?}"),
        }
    }

    #[test]
    fn all_mutations_emit_events() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        let member_name = "codex-reviewer";

        orchestrator
            .create_team(team_name, Some("audit coverage".to_string()))
            .expect("create should succeed");
        orchestrator
            .add_member(team_name, sample_member(member_name, CliTool::Codex))
            .expect("add should succeed");
        orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: member_name.to_string(),
                team_name: team_name.to_string(),
                message: "check status".to_string(),
            }))
            .expect("delivery should succeed");
        orchestrator.record_lease_claimed(team_name, member_name, 4242, "inst-1");
        orchestrator.record_lease_reclaimed(team_name, member_name, 4242, 5252);
        orchestrator
            .remove_member(team_name, member_name, Some("cleanup".to_string()))
            .expect("remove should succeed");
        orchestrator
            .disband_team(team_name, Some("shutdown".to_string()))
            .expect("disband should succeed");

        let event_types: Vec<&str> = orchestrator
            .drain_audit_log()
            .into_iter()
            .map(|event| event.event_type())
            .collect();
        assert_eq!(
            event_types,
            vec![
                "team_created",
                "member_added",
                "delivery_attempted",
                "delivery_succeeded",
                "lease_claimed",
                "lease_reclaimed",
                "member_removed",
                "team_disbanded"
            ]
        );
    }
}
