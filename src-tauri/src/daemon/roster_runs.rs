//! Daemon-owned host for standalone team and roster mutations.

use std::path::PathBuf;
use std::sync::Arc;

use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddMemberRequest, DisbandTeamReport, DisbandTeamRequest, RemoveMemberRequest, StopMemberReport,
};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::ActiveProjectTeamStore;
use crate::daemon::coordination_runs::{
    CoordinationRunKind, CoordinationRunRegistry, CoordinationRunReport, RunOutcome,
};
use crate::daemon::protocol::{
    CoordinationAddMemberOutcome, CoordinationAddMemberParams, CoordinationAddMemberStatus,
    CoordinationCreateTeamOutcome, CoordinationCreateTeamParams, CoordinationCreateTeamStatus,
    CoordinationDisbandTeamOutcome, CoordinationDisbandTeamParams, CoordinationDisbandTeamStatus,
    CoordinationRemoveMemberOutcome, CoordinationRemoveMemberParams,
    CoordinationRemoveMemberStatus,
};
use crate::session_scanner::cli_tool::CliTool;

/// Daemon-local host for standalone create/disband and roster edits.
#[derive(Clone)]
pub(crate) struct RosterOperationsService {
    registry: CoordinationRunRegistry,
    state: Arc<CoordinationState>,
}

impl RosterOperationsService {
    pub(crate) fn for_process_default(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
    ) -> Self {
        Self { registry, state }
    }

    pub(crate) fn start_create_team(
        &self,
        params: CoordinationCreateTeamParams,
    ) -> Result<String, String> {
        self.start_operation(CoordinationRunKind::CreateTeam, "create", move |state| {
            state
                .with_orchestrator(|orchestrator| {
                    orchestrator
                        .create_team(&params.request.team_name, None)
                        .map(|_| ())
                })
                .map_err(|error| error.to_string())?;
            Ok(CoordinationRunReport::CreateTeam)
        })
    }

    pub(crate) fn start_disband_team(
        &self,
        params: CoordinationDisbandTeamParams,
    ) -> Result<String, String> {
        self.start_operation(CoordinationRunKind::DisbandTeam, "disband", move |state| {
            execute_disband_team(state.as_ref(), &params.request)
                .map(CoordinationRunReport::DisbandTeam)
                .map_err(|error| error.to_string())
        })
    }

    pub(crate) fn start_add_member(
        &self,
        params: CoordinationAddMemberParams,
    ) -> Result<String, String> {
        self.start_operation(CoordinationRunKind::AddMember, "member-add", move |state| {
            execute_add_member(state.as_ref(), &params.request)
                .map(|()| CoordinationRunReport::AddMember)
                .map_err(|error| error.to_string())
        })
    }

    pub(crate) fn start_remove_member(
        &self,
        params: CoordinationRemoveMemberParams,
    ) -> Result<String, String> {
        self.start_operation(
            CoordinationRunKind::RemoveMember,
            "member-remove",
            move |state| {
                let report = execute_remove_member(state.as_ref(), &params.request)
                    .map_err(|error| error.to_string())?;
                crate::coordination::stores::active_project::sync_team_from_config(
                    state.teams_dir(),
                    &report.team_name,
                )
                .map_err(|error| error.to_string())?;
                Ok(CoordinationRunReport::RemoveMember(report))
            },
        )
    }

    fn start_operation<F>(
        &self,
        kind: CoordinationRunKind,
        worker_name: &'static str,
        operation: F,
    ) -> Result<String, String>
    where
        F: FnOnce(Arc<CoordinationState>) -> Result<CoordinationRunReport, String> + Send + 'static,
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
        if status.kind != CoordinationRunKind::CreateTeam {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationCreateTeamOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::CreateTeam,
            } => CoordinationCreateTeamOutcome::Completed,
            RunOutcome::Failed { error } => CoordinationCreateTeamOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
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
        if status.kind != CoordinationRunKind::DisbandTeam {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationDisbandTeamOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::DisbandTeam(report),
            } => CoordinationDisbandTeamOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationDisbandTeamOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationDisbandTeamStatus {
            run_id: status.run_id,
            outcome,
        })
    }

    pub(crate) fn add_member_status(&self, run_id: &str) -> Option<CoordinationAddMemberStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::AddMember {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationAddMemberOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::AddMember,
            } => CoordinationAddMemberOutcome::Completed,
            RunOutcome::Failed { error } => CoordinationAddMemberOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
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
        if status.kind != CoordinationRunKind::RemoveMember {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationRemoveMemberOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::RemoveMember(report),
            } => CoordinationRemoveMemberOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationRemoveMemberOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
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

    use super::RosterOperationsService;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::requests::{
        AddMemberRequest, CreateTeamRequest, DisbandTeamRequest, RemoveMemberRequest,
    };
    use crate::coordination::runtime::{
        CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
    };
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{ActiveProjectTeamStore, TeamConfigStore};
    use crate::daemon::coordination_runs::{
        CoordinationRunKind, CoordinationRunRegistry, RunOutcome,
    };
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
        (
            RosterOperationsService::for_process_default(state, CoordinationRunRegistry::default()),
            runtime,
        )
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
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(600));
        for (kind, prefix) in [
            (CoordinationRunKind::CreateTeam, "create_"),
            (CoordinationRunKind::DisbandTeam, "disband_"),
            (CoordinationRunKind::AddMember, "member-add_"),
            (CoordinationRunKind::RemoveMember, "member-remove_"),
        ] {
            let run_id = registry.start(kind);
            assert!(run_id.starts_with(prefix));
            let status = registry.status(&run_id).expect("run registered");
            assert_eq!(status.kind, kind);
            assert_eq!(status.outcome, RunOutcome::Running);
        }
    }

    #[test]
    fn add_member_without_project_path_inherits_the_first_members() {
        // The legacy add-member project-path rule moved into the daemon with
        // this slice: an omitted path inherits the lead's (or first
        // member's), and an empty team refuses.
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (service, _runtime) = service(temp.path());
        let project = temp.path().join("project");

        let run_id = service
            .start_create_team(CoordinationCreateTeamParams {
                request: CreateTeamRequest {
                    team_name: "paths".to_string(),
                },
            })
            .expect("create worker starts");
        let created = wait_until(
            || service.create_team_status(&run_id),
            |status| status.outcome == CoordinationCreateTeamOutcome::Running,
        );
        assert_eq!(created.outcome, CoordinationCreateTeamOutcome::Completed);

        let empty_team = service.start_add_member(CoordinationAddMemberParams {
            request: AddMemberRequest {
                team_name: "paths".to_string(),
                member_name: "orphan".to_string(),
                backend_kind: "codex".to_string(),
                project_path: None,
            },
        });
        let run_id = empty_team.expect("worker starts; the refusal is the run outcome");
        let refused = wait_until(
            || service.add_member_status(&run_id),
            |status| status.outcome == CoordinationAddMemberOutcome::Running,
        );
        assert!(
            matches!(refused.outcome, CoordinationAddMemberOutcome::Failed { .. }),
            "an omitted path on an empty team must refuse"
        );

        for (name, path) in [
            ("first", Some(project.display().to_string())),
            ("inheritor", None),
        ] {
            let run_id = service
                .start_add_member(CoordinationAddMemberParams {
                    request: AddMemberRequest {
                        team_name: "paths".to_string(),
                        member_name: name.to_string(),
                        backend_kind: "codex".to_string(),
                        project_path: path,
                    },
                })
                .expect("add-member worker starts");
            let added = wait_until(
                || service.add_member_status(&run_id),
                |status| status.outcome == CoordinationAddMemberOutcome::Running,
            );
            assert_eq!(added.outcome, CoordinationAddMemberOutcome::Completed);
        }
        let config = TeamConfigStore::load(temp.path(), "paths").expect("team config");
        let inheritor = config
            .members
            .iter()
            .find(|member| member.name == "inheritor")
            .expect("inherited member present");
        assert_eq!(
            inheritor.project_path, project,
            "an omitted project_path inherits the first member's path"
        );
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
            ActiveProjectTeamStore::load_active_team(temp.path(), &project.display().to_string(),)
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
