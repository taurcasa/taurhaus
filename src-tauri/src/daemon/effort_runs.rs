//! Daemon-owned host for task-arrival effort intents.

use std::sync::Arc;

use crate::coordination::state::CoordinationState;
use crate::daemon::coordination_runs::{
    daemon_launch_resolver, CoordinationRunKind, CoordinationRunRegistry, CoordinationRunReport,
    PrepareLaunchInputs, RunOutcome,
};
use crate::daemon::protocol::{
    CoordinationApplyTaskEffortOutcome, CoordinationApplyTaskEffortParams,
    CoordinationApplyTaskEffortReport, CoordinationApplyTaskEffortStatus,
};

#[derive(Clone)]
pub(crate) struct EffortOperationsService {
    registry: CoordinationRunRegistry,
    state: Arc<CoordinationState>,
    prepare_launch_inputs: Arc<PrepareLaunchInputs>,
}

impl EffortOperationsService {
    pub(crate) fn for_process_default(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
    ) -> Self {
        let prepare_launch_inputs = daemon_launch_resolver();
        Self::with_state_and_prepare(state, registry, prepare_launch_inputs)
    }

    fn with_state_and_prepare(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
        prepare_launch_inputs: Arc<PrepareLaunchInputs>,
    ) -> Self {
        Self {
            registry,
            state,
            prepare_launch_inputs,
        }
    }

    pub(crate) fn start(
        &self,
        params: CoordinationApplyTaskEffortParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::ApplyTaskEffort);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-effort-{}", &run_id[7..15]))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    state
                        .apply_task_effort_for_project_with_launch_resolution(
                            &params.project_path,
                            &mut cli_commands,
                            &params.tmux_layout,
                            &mut |root, tool, commands| prepare_launch_inputs(root, tool, commands),
                        )
                        .map(|outcome| CoordinationApplyTaskEffortReport {
                            switched: outcome.switched,
                            failed: outcome.failed,
                            skipped_teams: outcome.skipped_teams,
                        })
                        .map_err(|error| error.to_string())
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry.complete(
                            &run_id_for_task,
                            CoordinationRunReport::ApplyTaskEffort(report),
                        );
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, "task-effort worker panicked".to_string());
                    }
                }
            });
        match spawn_result {
            Ok(_) => Ok(run_id),
            Err(error) => {
                let message = format!("failed to start task-effort worker: {error}");
                let _ = self.registry.fail(&run_id, message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn status(&self, run_id: &str) -> Option<CoordinationApplyTaskEffortStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::ApplyTaskEffort {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationApplyTaskEffortOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::ApplyTaskEffort(report),
            } => CoordinationApplyTaskEffortOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationApplyTaskEffortOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationApplyTaskEffortStatus {
            run_id: status.run_id,
            outcome,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::EffortOperationsService;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::state::CoordinationState;
    use crate::daemon::coordination_runs::CoordinationRunRegistry;
    use crate::daemon::protocol::{
        CoordinationApplyTaskEffortOutcome, CoordinationApplyTaskEffortParams,
    };

    #[test]
    fn apply_task_effort_intent_roundtrips_through_the_shared_run_registry() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            temp.path().to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
        ));
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(60));
        let service = EffortOperationsService::with_state_and_prepare(
            state,
            registry,
            Arc::new(|_, _, _| {}),
        );

        let run_id = service
            .start(CoordinationApplyTaskEffortParams {
                project_path: "/tmp/no-team".to_string(),
                cli_commands: crate::models::CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
            })
            .expect("intent accepted");

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = service.status(&run_id).expect("run remains registered");
            match status.outcome {
                CoordinationApplyTaskEffortOutcome::Running => {
                    assert!(Instant::now() < deadline, "task-effort run completed");
                    std::thread::sleep(Duration::from_millis(5));
                }
                CoordinationApplyTaskEffortOutcome::Completed { report } => {
                    assert!(report.switched.is_empty());
                    assert!(report.failed.is_empty());
                    assert!(report.skipped_teams.is_empty());
                    break;
                }
                CoordinationApplyTaskEffortOutcome::Failed { error } => {
                    panic!("task-effort run failed: {error}")
                }
            }
        }
    }
}
