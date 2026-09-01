//! Daemon-owned host for standalone team and roster mutations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddMemberRequest, DisbandTeamReport, DisbandTeamRequest, RemoveMemberRequest, StopMemberReport,
};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::ActiveProjectTeamStore;
use crate::daemon::protocol::{
    CoordinationAddMemberOutcome, CoordinationAddMemberParams, CoordinationAddMemberStatus,
    CoordinationCreateTeamOutcome, CoordinationCreateTeamParams, CoordinationCreateTeamStatus,
    CoordinationDisbandTeamOutcome, CoordinationDisbandTeamParams, CoordinationDisbandTeamStatus,
    CoordinationRemoveMemberOutcome, CoordinationRemoveMemberParams,
    CoordinationRemoveMemberStatus,
};
use crate::session_scanner::cli_tool::CliTool;

const ROSTER_RUN_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RosterRunKind {
    CreateTeam,
    DisbandTeam,
    AddMember,
    RemoveMember,
}

impl RosterRunKind {
    fn id_prefix(self) -> &'static str {
        match self {
            Self::CreateTeam => "create",
            Self::DisbandTeam => "disband",
            Self::AddMember => "member-add",
            Self::RemoveMember => "member-remove",
        }
    }

    fn accepts(self, report: &RosterRunReport) -> bool {
        matches!(
            (self, report),
            (Self::CreateTeam, RosterRunReport::CreateTeam)
                | (Self::DisbandTeam, RosterRunReport::DisbandTeam(_))
                | (Self::AddMember, RosterRunReport::AddMember)
                | (Self::RemoveMember, RosterRunReport::RemoveMember(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RosterRunReport {
    CreateTeam,
    DisbandTeam(DisbandTeamReport),
    AddMember,
    RemoveMember(StopMemberReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RosterRunOutcome {
    Running,
    Completed { report: RosterRunReport },
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RosterRunStatus {
    run_id: String,
    kind: RosterRunKind,
    outcome: RosterRunOutcome,
}

#[derive(Debug)]
struct RosterRunRecord {
    kind: RosterRunKind,
    outcome: RosterRunOutcome,
    terminal_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct RosterRunRegistry {
    records: Arc<Mutex<HashMap<String, RosterRunRecord>>>,
    ttl: Duration,
}

impl Default for RosterRunRegistry {
    fn default() -> Self {
        Self::with_ttl(ROSTER_RUN_TTL)
    }
}

impl RosterRunRegistry {
    fn with_ttl(ttl: Duration) -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    fn start(&self, kind: RosterRunKind) -> String {
        let run_id = format!("{}_{}", kind.id_prefix(), uuid::Uuid::new_v4().simple());
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, Instant::now());
        records.insert(
            run_id.clone(),
            RosterRunRecord {
                kind,
                outcome: RosterRunOutcome::Running,
                terminal_at: None,
            },
        );
        run_id
    }

    fn complete(&self, run_id: &str, report: RosterRunReport) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "roster run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("roster run '{run_id}' was not found"))?;
        if record.outcome != RosterRunOutcome::Running {
            return Err(format!("roster run '{run_id}' is already terminal"));
        }
        if !record.kind.accepts(&report) {
            let error =
                format!("roster run '{run_id}' cannot complete with a different report kind");
            record.outcome = RosterRunOutcome::Failed {
                error: error.clone(),
            };
            record.terminal_at = Some(Instant::now());
            return Err(error);
        }
        record.outcome = RosterRunOutcome::Completed { report };
        record.terminal_at = Some(Instant::now());
        Ok(())
    }

    fn fail(&self, run_id: &str, error: String) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "roster run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("roster run '{run_id}' was not found"))?;
        if record.outcome != RosterRunOutcome::Running {
            return Err(format!("roster run '{run_id}' is already terminal"));
        }
        record.outcome = RosterRunOutcome::Failed { error };
        record.terminal_at = Some(Instant::now());
        Ok(())
    }

    fn status(&self, run_id: &str) -> Option<RosterRunStatus> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, Instant::now());
        records.get(run_id).map(|record| RosterRunStatus {
            run_id: run_id.to_string(),
            kind: record.kind,
            outcome: record.outcome.clone(),
        })
    }

    fn prune_locked(records: &mut HashMap<String, RosterRunRecord>, ttl: Duration, now: Instant) {
        records.retain(|_, record| {
            record
                .terminal_at
                .is_none_or(|terminal_at| now.saturating_duration_since(terminal_at) <= ttl)
        });
    }
}

/// Daemon-local host for standalone create/disband and roster edits.
#[derive(Clone)]
pub(crate) struct RosterOperationsService {
    registry: RosterRunRegistry,
    state: Arc<CoordinationState>,
}

impl RosterOperationsService {
    pub(crate) fn for_process_default(state: Arc<CoordinationState>) -> Self {
        Self {
            registry: RosterRunRegistry::default(),
            state,
        }
    }

    pub(crate) fn start_create_team(
        &self,
        params: CoordinationCreateTeamParams,
    ) -> Result<String, String> {
        self.start_operation(RosterRunKind::CreateTeam, "create", move |state| {
            state
                .with_orchestrator(|orchestrator| {
                    orchestrator
                        .create_team(&params.request.team_name, None)
                        .map(|_| ())
                })
                .map_err(|error| error.to_string())?;
            Ok(RosterRunReport::CreateTeam)
        })
    }

    pub(crate) fn start_disband_team(
        &self,
        params: CoordinationDisbandTeamParams,
    ) -> Result<String, String> {
        self.start_operation(RosterRunKind::DisbandTeam, "disband", move |state| {
            execute_disband_team(state.as_ref(), &params.request)
                .map(RosterRunReport::DisbandTeam)
                .map_err(|error| error.to_string())
        })
    }

    pub(crate) fn start_add_member(
        &self,
        params: CoordinationAddMemberParams,
    ) -> Result<String, String> {
        self.start_operation(RosterRunKind::AddMember, "member-add", move |state| {
            execute_add_member(state.as_ref(), &params.request)
                .map(|()| RosterRunReport::AddMember)
                .map_err(|error| error.to_string())
        })
    }

    pub(crate) fn start_remove_member(
        &self,
        params: CoordinationRemoveMemberParams,
    ) -> Result<String, String> {
        self.start_operation(RosterRunKind::RemoveMember, "member-remove", move |state| {
            let report = execute_remove_member(state.as_ref(), &params.request)
                .map_err(|error| error.to_string())?;
            crate::coordination::stores::active_project::sync_team_from_config(
                state.teams_dir(),
                &report.team_name,
            )
            .map_err(|error| error.to_string())?;
            Ok(RosterRunReport::RemoveMember(report))
        })
    }

    fn start_operation<F>(
        &self,
        kind: RosterRunKind,
        worker_name: &'static str,
        operation: F,
    ) -> Result<String, String>
    where
        F: FnOnce(Arc<CoordinationState>) -> Result<RosterRunReport, String> + Send + 'static,
    {
        let run_id = self.registry.start(kind);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let thread_suffix = run_id
            .split_once('_')
            .map(|(_, suffix)| suffix)
            .unwrap_or(&run_id)
            .chars()
            .take(8)
            .collect::<String>();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-{worker_name}-{thread_suffix}"))
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(state)));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry.complete(&run_id_for_task, report);
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, format!("{worker_name} worker panicked"));
                    }
                }
            });
        match spawn_result {
            Ok(_) => Ok(run_id),
            Err(error) => {
                let message = format!("failed to start {worker_name} worker: {error}");
                let _ = self.registry.fail(&run_id, message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn create_team_status(&self, run_id: &str) -> Option<CoordinationCreateTeamStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != RosterRunKind::CreateTeam {
            return None;
        }
        let outcome = match status.outcome {
            RosterRunOutcome::Running => CoordinationCreateTeamOutcome::Running,
            RosterRunOutcome::Completed {
                report: RosterRunReport::CreateTeam,
            } => CoordinationCreateTeamOutcome::Completed,
            RosterRunOutcome::Failed { error } => CoordinationCreateTeamOutcome::Failed { error },
            RosterRunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationCreateTeamStatus {
            run_id: status.run_id,
            outcome,
        })
    }

    pub(crate) fn disband_team_status(
        &self,
        run_id: &str,
    ) -> Option<CoordinationDisbandTeamStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != RosterRunKind::DisbandTeam {
            return None;
        }
        let outcome = match status.outcome {
            RosterRunOutcome::Running => CoordinationDisbandTeamOutcome::Running,
            RosterRunOutcome::Completed {
                report: RosterRunReport::DisbandTeam(report),
            } => CoordinationDisbandTeamOutcome::Completed { report },
            RosterRunOutcome::Failed { error } => CoordinationDisbandTeamOutcome::Failed { error },
            RosterRunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationDisbandTeamStatus {
            run_id: status.run_id,
            outcome,
        })
    }

    pub(crate) fn add_member_status(&self, run_id: &str) -> Option<CoordinationAddMemberStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != RosterRunKind::AddMember {
            return None;
        }
        let outcome = match status.outcome {
            RosterRunOutcome::Running => CoordinationAddMemberOutcome::Running,
            RosterRunOutcome::Completed {
                report: RosterRunReport::AddMember,
            } => CoordinationAddMemberOutcome::Completed,
            RosterRunOutcome::Failed { error } => CoordinationAddMemberOutcome::Failed { error },
            RosterRunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationAddMemberStatus {
            run_id: status.run_id,
            outcome,
        })
    }

    pub(crate) fn remove_member_status(
        &self,
        run_id: &str,
    ) -> Option<CoordinationRemoveMemberStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != RosterRunKind::RemoveMember {
            return None;
        }
        let outcome = match status.outcome {
            RosterRunOutcome::Running => CoordinationRemoveMemberOutcome::Running,
            RosterRunOutcome::Completed {
                report: RosterRunReport::RemoveMember(report),
            } => CoordinationRemoveMemberOutcome::Completed { report },
            RosterRunOutcome::Failed { error } => CoordinationRemoveMemberOutcome::Failed { error },
            RosterRunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationRemoveMemberStatus {
            run_id: status.run_id,
            outcome,
        })
    }
}

fn execute_disband_team(
    state: &CoordinationState,
    request: &DisbandTeamRequest,
) -> Result<DisbandTeamReport, CoordinationError> {
    let result = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(&request.team_name, None))?;
    ActiveProjectTeamStore::clear_team(state.teams_dir(), &result.team_name)?;
    let message = if result.already_disbanded {
        "team already disbanded"
    } else {
        "team disbanded"
    };
    Ok(DisbandTeamReport {
        team_name: result.team_name,
        disbanded: result.disbanded,
        already_disbanded: result.already_disbanded,
        message: message.to_string(),
    })
}

fn execute_add_member(
    state: &CoordinationState,
    request: &AddMemberRequest,
) -> Result<(), CoordinationError> {
    let cli_tool = CliTool::from_alias(&request.backend_kind).map_err(|_| {
        CoordinationError::Validation(format!(
            "unsupported backend_kind '{}'",
            request.backend_kind.trim()
        ))
    })?;
    state.with_orchestrator(|orchestrator| {
        let team_status = orchestrator.get_team_status(&request.team_name)?;
        let project_path = resolve_member_project_path(
            &team_status.config.members,
            request.project_path.as_deref(),
        )?;
        orchestrator.add_member(
            &request.team_name,
            Member {
                name: request.member_name.clone(),
                role: MemberRole::Agent,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: None,
                reasoning_effort: None,
                project_path,
                cli_tool,
                extra: Default::default(),
            },
        )
    })
}

fn resolve_member_project_path(
    existing_members: &[Member],
    project_path_override: Option<&str>,
) -> Result<PathBuf, CoordinationError> {
    if let Some(project_path) = project_path_override {
        let project_path = project_path.trim();
        if project_path.is_empty() {
            return Err(CoordinationError::Validation(
                "project_path must not be empty".to_string(),
            ));
        }
        return Ok(PathBuf::from(project_path));
    }

    existing_members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| existing_members.first())
        .map(|member| member.project_path.clone())
        .ok_or_else(|| {
            CoordinationError::Validation(
                "project_path must be provided for legacy add-member when team has no members"
                    .to_string(),
            )
        })
}

fn execute_remove_member(
    state: &CoordinationState,
    request: &RemoveMemberRequest,
) -> Result<StopMemberReport, CoordinationError> {
    state
        .with_orchestrator(|orchestrator| {
            orchestrator.remove_member(&request.team_name, &request.member_name, None)
        })
        .map(StopMemberReport::from_remove_member_result)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{RosterOperationsService, RosterRunKind, RosterRunOutcome, RosterRunRegistry};
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::requests::{
        AddMemberRequest, CreateTeamRequest, DisbandTeamRequest, RemoveMemberRequest,
    };
    use crate::coordination::runtime::{
        CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
    };
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{ActiveProjectTeamStore, TeamConfigStore};
    use crate::daemon::protocol::{
        CoordinationAddMemberOutcome, CoordinationAddMemberParams, CoordinationCreateTeamOutcome,
        CoordinationCreateTeamParams, CoordinationDisbandTeamOutcome,
        CoordinationDisbandTeamParams, CoordinationRemoveMemberOutcome,
        CoordinationRemoveMemberParams,
    };

    fn service(
        root: &std::path::Path,
    ) -> (RosterOperationsService, Arc<RecordingCoordinationRuntime>) {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let runtime_for_factory = runtime.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            root.to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        (RosterOperationsService::for_process_default(state), runtime)
    }

    fn wait_until<T>(mut status: impl FnMut() -> Option<T>, running: impl Fn(&T) -> bool) -> T {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = status().expect("run remains registered");
            if !running(&status) {
                return status;
            }
            assert!(Instant::now() < deadline, "roster operation did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn registry_accepts_every_roster_operation_kind() {
        let registry = RosterRunRegistry::with_ttl(Duration::from_secs(600));
        for (kind, prefix) in [
            (RosterRunKind::CreateTeam, "create_"),
            (RosterRunKind::DisbandTeam, "disband_"),
            (RosterRunKind::AddMember, "member-add_"),
            (RosterRunKind::RemoveMember, "member-remove_"),
        ] {
            let run_id = registry.start(kind);
            assert!(run_id.starts_with(prefix));
            let status = registry.status(&run_id).expect("run registered");
            assert_eq!(status.kind, kind);
            assert_eq!(status.outcome, RosterRunOutcome::Running);
        }
    }

    #[test]
    fn create_add_and_remove_execute_through_daemon_state() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (service, _runtime) = service(temp.path());
        let project = temp.path().join("project");

        let run_id = service
            .start_create_team(CoordinationCreateTeamParams {
                request: CreateTeamRequest {
                    team_name: "arch".to_string(),
                },
            })
            .expect("create worker starts");
        let created = wait_until(
            || service.create_team_status(&run_id),
            |status| status.outcome == CoordinationCreateTeamOutcome::Running,
        );
        assert_eq!(created.outcome, CoordinationCreateTeamOutcome::Completed);

        let run_id = service
            .start_add_member(CoordinationAddMemberParams {
                request: AddMemberRequest {
                    team_name: "arch".to_string(),
                    member_name: "builder".to_string(),
                    backend_kind: "codex".to_string(),
                    project_path: Some(project.display().to_string()),
                },
            })
            .expect("add-member worker starts");
        let added = wait_until(
            || service.add_member_status(&run_id),
            |status| status.outcome == CoordinationAddMemberOutcome::Running,
        );
        assert_eq!(added.outcome, CoordinationAddMemberOutcome::Completed);
        assert_eq!(
            TeamConfigStore::load(temp.path(), "arch")
                .expect("team config")
                .members
                .len(),
            1
        );
        crate::coordination::stores::active_project::sync_team_from_config(temp.path(), "arch")
            .expect("seed active project mapping");

        let run_id = service
            .start_remove_member(CoordinationRemoveMemberParams {
                request: RemoveMemberRequest {
                    team_name: "arch".to_string(),
                    member_name: "builder".to_string(),
                },
            })
            .expect("remove-member worker starts");
        let removed = wait_until(
            || service.remove_member_status(&run_id),
            |status| status.outcome == CoordinationRemoveMemberOutcome::Running,
        );
        let CoordinationRemoveMemberOutcome::Completed { report } = removed.outcome else {
            panic!("remove should complete: {:?}", removed.outcome);
        };
        assert!(report.removed);
        assert_eq!(report.message, "member removed with 1 warning");
        assert!(TeamConfigStore::load(temp.path(), "arch")
            .expect("team config")
            .members
            .is_empty());
        // Regression: 03eb3a2c routed roster removal through a daemon worker
        // that omitted the active-project sync performed by stop-member.
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(
                temp.path(),
                &project.display().to_string(),
            )
            .expect("active project mapping"),
            None
        );
    }

    #[test]
    fn disband_owns_teardown_config_delete_and_active_project_cleanup() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (service, runtime) = service(temp.path());
        let create_id = service
            .start_create_team(CoordinationCreateTeamParams {
                request: CreateTeamRequest {
                    team_name: "arch".to_string(),
                },
            })
            .expect("create worker starts");
        let _ = wait_until(
            || service.create_team_status(&create_id),
            |status| status.outcome == CoordinationCreateTeamOutcome::Running,
        );
        ActiveProjectTeamStore::set_active_team(temp.path(), "/work/arch", "arch")
            .expect("active team mapping");

        let run_id = service
            .start_disband_team(CoordinationDisbandTeamParams {
                request: DisbandTeamRequest {
                    team_name: "arch".to_string(),
                },
            })
            .expect("disband worker starts");
        let disbanded = wait_until(
            || service.disband_team_status(&run_id),
            |status| status.outcome == CoordinationDisbandTeamOutcome::Running,
        );
        let CoordinationDisbandTeamOutcome::Completed { report } = disbanded.outcome else {
            panic!("disband should complete: {:?}", disbanded.outcome);
        };
        assert!(report.disbanded);
        assert_eq!(report.message, "team disbanded");
        assert!(TeamConfigStore::load(temp.path(), "arch").is_err());
        assert_eq!(
            ActiveProjectTeamStore::load_active_team(temp.path(), "/work/arch")
                .expect("active mapping load"),
            None
        );
        assert!(runtime.calls().contains(&RuntimeCall::StopTeamDaemon {
            team_name: "arch".to_string(),
        }));
    }
}
