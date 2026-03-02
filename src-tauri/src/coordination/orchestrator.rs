//! Coordination orchestrator service.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::commands::coordination::{
    AddAgentReport, AddAgentRequest, InitializeReport, InitializeTeamRequest, StepProgress,
    StepStatus,
};
use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent, MemberAddedEvent, MemberRemovedEvent, TeamCreatedEvent,
    TeamDisbandedEvent,
};
use crate::coordination::backend::{BackendKind, CoordinationBackend};
use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, Team};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    DeliveryMethod, DeliveryRequest, DeliveryResult, OperatorNoticeDelivery,
};
use crate::coordination::stores::{
    DiscoveredTeam, MemberRuntimeRecord, MemberRuntimeStore, TeamConfig, TeamConfigStore,
};
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

    /// Initialize a team via the high-level multi-step pipeline.
    ///
    /// Pipeline steps:
    /// 1. validate_configuration
    /// 2. create_team
    /// 3. add_lead
    /// 4. create_panes (stubbed)
    /// 5. launch_sessions (stubbed)
    /// 6. join_mesh (stubbed)
    /// 7. start_daemons (stubbed)
    /// 8. send_onboarding (render + deliver)
    pub fn initialize_team(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<InitializeReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

        if let Err(err) = self.validate_initialize_configuration(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "validate_configuration",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "validate_configuration",
            "configuration validated",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.create_team(&request.team_name, request.team_description.clone()) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_team",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_team",
            "team created",
            &mut succeeded_steps,
            &mut steps,
        );

        let lead_member = match member_from_agent_setup(
            &request.lead,
            crate::coordination::domain::MemberRole::Lead,
        ) {
            Ok(member) => member,
            Err(err) => {
                return Ok(failed_initialize_report(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                ))
            }
        };
        if let Err(err) = self.add_member(&request.team_name, lead_member) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "add_lead",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("add_lead", "lead added", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.create_panes_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_panes",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_panes",
            "agent panes created",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_sessions_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "launch_sessions",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "launch_sessions",
            "cli sessions launched",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemons_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "start_daemons",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "start_daemons",
            "mesh daemons started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.send_onboarding_messages(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "send_onboarding",
            "onboarding messages sent",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(InitializeReport {
            team_name: request.team_name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps,
        })
    }

    /// Hot-add a single agent to an already running team.
    ///
    /// Pipeline steps:
    /// 1. validate
    /// 2. create_pane (stubbed)
    /// 3. launch_session (stubbed)
    /// 4. join_mesh (stubbed)
    /// 5. start_daemon (stubbed)
    /// 6. send_onboarding (render + backend delivery)
    /// 7. update_roster
    pub fn add_agent_to_team(
        &mut self,
        request: &AddAgentRequest,
    ) -> Result<AddAgentReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

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

        if let Err(err) = self.create_pane_for_agent_stub(request) {
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

        if let Err(err) = self.launch_session_for_agent_stub(request) {
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

        if let Err(err) = self.join_mesh_for_agent_stub(request) {
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

        if let Err(err) = self.start_daemon_for_agent_stub(request) {
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

        if let Err(err) = self.update_roster_with_agent(request) {
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

    fn validate_initialize_configuration(
        &self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_non_empty("lead name", &request.lead.name)?;
        validate_non_empty("lead cli tool", &request.lead.cli_tool)?;

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

    fn create_panes_stub(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for (idx, agent) in request.agents.iter().enumerate() {
            let member =
                member_from_agent_setup(agent, crate::coordination::domain::MemberRole::Agent)?;
            self.add_member(&request.team_name, member.clone())?;

            if let Ok(mut runtime) =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)
            {
                runtime.pane_id = Some(format!("%{}", idx + 1));
                runtime.attached_at = Some(Utc::now());
                runtime.health = HealthState::Healthy;
                MemberRuntimeStore::save(
                    &self.teams_dir,
                    &request.team_name,
                    &member.name,
                    &runtime,
                )?;
            }
        }
        Ok(())
    }

    fn launch_sessions_stub(
        &self,
        _request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn join_mesh_stub(&self, _request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn start_daemons_stub(
        &self,
        _request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn send_onboarding_messages(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for agent in &request.agents {
            let cli_tool = parse_cli_tool(&agent.cli_tool)?;
            if cli_tool == CliTool::Claude {
                continue;
            }

            let onboarding = DeliveryRenderer::render_onboarding(
                &request.team_name,
                &agent.name,
                &request.lead.name,
            );
            self.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        }
        Ok(())
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

    fn create_pane_for_agent_stub(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn launch_session_for_agent_stub(
        &self,
        _request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn join_mesh_for_agent_stub(&self, request: &AddAgentRequest) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn start_daemon_for_agent_stub(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn send_onboarding_for_agent(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let cli_tool = parse_cli_tool(&request.agent.cli_tool)?;
        if cli_tool == CliTool::Claude {
            return Ok(());
        }
        let team = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = team
            .members
            .iter()
            .find(|member| member.role == crate::coordination::domain::MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        let onboarding = DeliveryRenderer::render_onboarding(
            &request.team_name,
            &request.agent.name,
            &lead_name,
        );
        self.backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        Ok(())
    }

    fn update_roster_with_agent(
        &mut self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let member = member_from_agent_setup(
            &request.agent,
            crate::coordination::domain::MemberRole::Agent,
        )?;
        self.add_member(&request.team_name, member)
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
        return Err(CoordinationError::Validation(format!(
            "{field} must not be empty"
        )));
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

fn mark_step_succeeded(
    step: &str,
    message: &str,
    succeeded_steps: &mut Vec<String>,
    steps: &mut Vec<StepProgress>,
) {
    succeeded_steps.push(step.to_string());
    steps.push(StepProgress {
        step: step.to_string(),
        status: StepStatus::Succeeded,
        message: Some(message.to_string()),
    });
}

fn failed_initialize_report(
    team_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> InitializeReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    InitializeReport {
        team_name: team_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

fn failed_add_agent_report(
    team_name: &str,
    member_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> AddAgentReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    AddAgentReport {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

fn parse_cli_tool(raw: &str) -> Result<CliTool, CoordinationError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude_native" => Ok(CliTool::Claude),
        "codex" | "mesh" | "mesh_bridged" => Ok(CliTool::Codex),
        "gemini" => Ok(CliTool::Gemini),
        other => Err(CoordinationError::Validation(format!(
            "unsupported cli tool '{other}'"
        ))),
    }
}

fn member_from_agent_setup(
    setup: &crate::commands::coordination::AgentSetupConfig,
    role: crate::coordination::domain::MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    Ok(Member {
        name: setup.name.clone(),
        role,
        instructions: setup.description.clone(),
        project_path: PathBuf::from(&setup.project_id),
        cli_tool: parse_cli_tool(&setup.cli_tool)?,
    })
}

fn discovered_team_to_status(team: DiscoveredTeam) -> DiscoveredTeamStatus {
    DiscoveredTeamStatus {
        team_name: team.team_name,
        lead_project_path: team.lead_project_path,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::coordination::{
        AddAgentRequest, AgentSetupConfig, InitializeTeamRequest, LeadMode,
    };
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

    fn initialize_request(team_name: &str) -> InitializeTeamRequest {
        InitializeTeamRequest {
            team_name: team_name.to_string(),
            team_description: Some("init pipeline test".to_string()),
            lead_mode: LeadMode::LaunchNew,
            lead: AgentSetupConfig {
                name: "team-lead".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                project_id: "/tmp/lead".to_string(),
                description: Some("lead".to_string()),
            },
            agents: vec![
                AgentSetupConfig {
                    name: "frontend-dev".to_string(),
                    cli_tool: "codex".to_string(),
                    model: "gpt-5.3".to_string(),
                    project_id: "/tmp/frontend".to_string(),
                    description: Some("frontend".to_string()),
                },
                AgentSetupConfig {
                    name: "reviewer".to_string(),
                    cli_tool: "gemini".to_string(),
                    model: "pro".to_string(),
                    project_id: "/tmp/reviewer".to_string(),
                    description: Some("review".to_string()),
                },
            ],
        }
    }

    fn add_agent_request(team_name: &str, agent_name: &str, cli_tool: &str) -> AddAgentRequest {
        AddAgentRequest {
            team_name: team_name.to_string(),
            agent: AgentSetupConfig {
                name: agent_name.to_string(),
                cli_tool: cli_tool.to_string(),
                model: "model".to_string(),
                project_id: format!("/tmp/{agent_name}"),
                description: Some("hot-added".to_string()),
            },
        }
    }

    fn create_running_team(orchestrator: &mut CoordinationOrchestrator, team_name: &str) {
        orchestrator
            .create_team(team_name, Some("running".to_string()))
            .expect("create team");
        orchestrator
            .add_member(
                team_name,
                Member {
                    name: "team-lead".to_string(),
                    role: MemberRole::Lead,
                    instructions: Some("lead".to_string()),
                    project_path: PathBuf::from("/tmp/lead"),
                    cli_tool: CliTool::Claude,
                },
            )
            .expect("add lead");
        orchestrator
            .add_member(team_name, sample_member("existing-dev", CliTool::Codex))
            .expect("add existing member");
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
    fn discover_teams_resolves_lead_project_anchor() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";
        create_running_team(&mut orchestrator, team_name);

        let discovery = orchestrator
            .discover_teams()
            .expect("discover should succeed");
        assert_eq!(discovery.warnings.len(), 0);
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, team_name);
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some(std::path::Path::new("/tmp/lead"))
        );
    }

    #[test]
    fn discover_teams_skips_corrupt_folder_with_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let valid_team = "alpha";
        create_running_team(&mut orchestrator, valid_team);

        let broken_dir = tmp.path().join("broken-team");
        std::fs::create_dir_all(&broken_dir).expect("create broken dir");
        std::fs::write(broken_dir.join("config.json"), "{ broken json").expect("write broken");

        let discovery = orchestrator
            .discover_teams()
            .expect("discover should succeed");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, valid_team);
        assert_eq!(discovery.warnings.len(), 1);
        assert!(discovery.warnings[0].contains("broken-team"));

        let teams = orchestrator.list_teams().expect("list should succeed");
        assert_eq!(teams, vec![valid_team.to_string()]);
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
        let result = orchestrator
            .disband_team(team_name, Some("cleanup".to_string()))
            .expect("disband should succeed");
        assert!(result.disbanded);
        assert!(!result.already_disbanded);

        assert!(
            !tmp.path().join(team_name).exists(),
            "team directory should be removed"
        );
    }

    #[test]
    fn disband_nonexistent_team_returns_already_disbanded() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        let result = orchestrator
            .disband_team("missing-team", None)
            .expect("idempotent disband should succeed");
        assert!(!result.disbanded);
        assert!(result.already_disbanded);
    }

    #[test]
    fn disband_is_idempotent_and_does_not_invoke_backend_controls() {
        let tmp = TempDir::new().expect("tempdir");
        let fake = Arc::new(FakeBackend::default());
        let backend: Arc<dyn CoordinationBackend> = fake.clone();
        let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");
        let first = orchestrator
            .disband_team(team_name, Some("cleanup".to_string()))
            .expect("first disband");
        let second = orchestrator
            .disband_team(team_name, Some("cleanup".to_string()))
            .expect("second disband");

        assert!(first.disbanded);
        assert!(!first.already_disbanded);
        assert!(!second.disbanded);
        assert!(second.already_disbanded);
        assert_eq!(
            fake.call_counts(),
            (0, 0, 0, 0),
            "disband should not touch backend session controls"
        );
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
        fake.set_deliver_error(CoordinationError::Backend(
            "simulated delivery failure".to_string(),
        ));
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
        assert!(
            !orchestrator.drain_audit_log().is_empty(),
            "sanity: event should exist"
        );

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

    #[test]
    fn invalid_team_name_is_rejected_for_create() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        let err = orchestrator
            .create_team("bad/name", None)
            .expect_err("path separators must be rejected");
        match err {
            CoordinationError::Validation(message) => assert!(message.contains("must not contain")),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn invalid_member_name_is_rejected_for_add_member() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final";

        orchestrator
            .create_team(team_name, None)
            .expect("create should succeed");

        let err = orchestrator
            .add_member(team_name, sample_member("bad/member", CliTool::Codex))
            .expect_err("invalid member name should fail");
        match err {
            CoordinationError::Validation(message) => assert!(message.contains("path separators")),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn deliver_to_nonexistent_team_fails_without_delivery_audit_events() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);

        let err = orchestrator
            .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: "codex-reviewer".to_string(),
                team_name: "missing-team".to_string(),
                message: "status?".to_string(),
            }))
            .expect_err("delivery should fail");
        assert_not_found(err);

        let event_types: Vec<&str> = orchestrator
            .drain_audit_log()
            .into_iter()
            .map(|event| event.event_type())
            .collect();
        assert!(
            event_types.is_empty(),
            "no delivery audit event should be emitted before team lookup succeeds"
        );
    }

    #[test]
    fn initialize_team_full_success_path() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let request = initialize_request("architecture-final-init");

        let report = orchestrator
            .initialize_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.team_name, "architecture-final-init");
        assert!(report.failed_step.is_none());
        assert!(!report.retryable);
        assert_eq!(
            report.succeeded_steps,
            vec![
                "validate_configuration",
                "create_team",
                "add_lead",
                "create_panes",
                "launch_sessions",
                "join_mesh",
                "start_daemons",
                "send_onboarding",
            ]
        );
    }

    #[test]
    fn initialize_team_duplicate_team_returns_partial_failure_report() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let request = initialize_request("architecture-final-init");

        orchestrator
            .create_team("architecture-final-init", None)
            .expect("seed team");

        let report = orchestrator
            .initialize_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.failed_step.as_deref(), Some("create_team"));
        assert!(report.retryable);
        assert_eq!(report.succeeded_steps, vec!["validate_configuration"]);
        assert_eq!(report.steps[0].step, "validate_configuration");
        assert_eq!(report.steps[1].step, "create_team");
        assert_eq!(report.steps[1].status, StepStatus::Failed);
    }

    #[test]
    fn initialize_team_agent_addition_failure_is_partial() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let mut request = initialize_request("architecture-final-init");
        request.agents[1].name = "bad/member".to_string();

        let report = orchestrator
            .initialize_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.failed_step.as_deref(), Some("create_panes"));
        assert!(report.retryable);
        assert_eq!(
            report.succeeded_steps,
            vec!["validate_configuration", "create_team", "add_lead"]
        );
        assert_eq!(
            report
                .steps
                .iter()
                .map(|step| step.step.as_str())
                .collect::<Vec<_>>(),
            vec![
                "validate_configuration",
                "create_team",
                "add_lead",
                "create_panes",
            ]
        );
    }

    #[test]
    fn initialize_team_steps_are_ordered() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let request = initialize_request("architecture-final-order");

        let report = orchestrator
            .initialize_team(&request)
            .expect("pipeline should return report");
        let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
        assert_eq!(
            step_names,
            vec![
                "validate_configuration",
                "create_team",
                "add_lead",
                "create_panes",
                "launch_sessions",
                "join_mesh",
                "start_daemons",
                "send_onboarding",
            ]
        );
    }

    #[test]
    fn add_agent_to_team_full_success() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final-hot-add";
        create_running_team(&mut orchestrator, team_name);
        let request = add_agent_request(team_name, "new-agent", "codex");

        let report = orchestrator
            .add_agent_to_team(&request)
            .expect("pipeline should return report");
        assert!(report.failed_step.is_none());
        assert!(!report.retryable);
        assert_eq!(report.member_name, "new-agent");
        assert_eq!(
            report.succeeded_steps,
            vec![
                "validate",
                "create_pane",
                "launch_session",
                "join_mesh",
                "start_daemon",
                "send_onboarding",
                "update_roster",
            ]
        );

        let status = orchestrator
            .get_team_status(team_name)
            .expect("status should load");
        assert!(status
            .config
            .members
            .iter()
            .any(|member| member.name == "new-agent"));
    }

    #[test]
    fn add_agent_duplicate_name_rejected() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final-hot-add";
        create_running_team(&mut orchestrator, team_name);
        let request = add_agent_request(team_name, "existing-dev", "codex");

        let report = orchestrator
            .add_agent_to_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.failed_step.as_deref(), Some("validate"));
        assert!(report.retryable);
        assert!(report.succeeded_steps.is_empty());
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].status, StepStatus::Failed);
    }

    #[test]
    fn add_agent_team_not_found_fails_before_pipeline_progress() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let request = add_agent_request("missing-team", "new-agent", "codex");

        let report = orchestrator
            .add_agent_to_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.failed_step.as_deref(), Some("validate"));
        assert!(report.succeeded_steps.is_empty());
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "validate");
    }

    #[test]
    fn add_agent_mid_flow_failure_preserves_existing_team_state() {
        let tmp = TempDir::new().expect("tempdir");
        let fake = Arc::new(FakeBackend::default());
        fake.set_deliver_error(CoordinationError::Backend(
            "simulated onboarding failure".to_string(),
        ));
        let backend: Arc<dyn CoordinationBackend> = fake;
        let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
        let team_name = "architecture-final-hot-add";
        create_running_team(&mut orchestrator, team_name);
        let request = add_agent_request(team_name, "new-agent", "codex");

        let before = orchestrator
            .get_team_status(team_name)
            .expect("status before")
            .config
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();

        let report = orchestrator
            .add_agent_to_team(&request)
            .expect("pipeline should return report");
        assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
        assert!(report.retryable);

        let after = orchestrator
            .get_team_status(team_name)
            .expect("status after")
            .config
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(before, after, "existing team roster should be unchanged");
        assert!(!after.contains(&"new-agent".to_string()));
    }

    #[test]
    fn add_agent_step_ordering_is_stable() {
        let tmp = TempDir::new().expect("tempdir");
        let mut orchestrator = new_orchestrator(&tmp);
        let team_name = "architecture-final-hot-add-order";
        create_running_team(&mut orchestrator, team_name);
        let request = add_agent_request(team_name, "new-agent", "codex");

        let report = orchestrator
            .add_agent_to_team(&request)
            .expect("pipeline should return report");
        let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
        assert_eq!(
            step_names,
            vec![
                "validate",
                "create_pane",
                "launch_session",
                "join_mesh",
                "start_daemon",
                "send_onboarding",
                "update_roster",
            ]
        );
    }
}
