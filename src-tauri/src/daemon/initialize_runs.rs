//! Daemon-owned lifecycle registry for asynchronous team initialization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::coordination::requests::{InitializeReport, StepProgress};
pub(crate) use crate::daemon::protocol::{
    CoordinationInitializeOutcome as InitializeRunOutcome,
    CoordinationInitializeStatus as InitializeRunStatus,
};

pub(crate) const INITIALIZE_RUN_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
struct InitializeRunRecord {
    steps: Vec<StepProgress>,
    outcome: InitializeRunOutcome,
    terminal_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct InitializeRunRegistry {
    records: Arc<Mutex<HashMap<String, InitializeRunRecord>>>,
    ttl: Duration,
}

impl Default for InitializeRunRegistry {
    fn default() -> Self {
        Self::with_ttl(INITIALIZE_RUN_TTL)
    }
}

impl InitializeRunRegistry {
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    pub(crate) fn start(&self) -> String {
        self.start_at(Instant::now())
    }

    fn start_at(&self, now: Instant) -> String {
        let run_id = format!("init_{}", uuid::Uuid::new_v4().simple());
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
        records.insert(
            run_id.clone(),
            InitializeRunRecord {
                steps: Vec::new(),
                outcome: InitializeRunOutcome::Running,
                terminal_at: None,
            },
        );
        run_id
    }

    pub(crate) fn record_step(&self, run_id: &str, step: StepProgress) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "team initialization run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("team initialization run '{run_id}' was not found"))?;
        if record.outcome != InitializeRunOutcome::Running {
            return Err(format!(
                "team initialization run '{run_id}' is already terminal"
            ));
        }
        record.steps.push(step);
        Ok(())
    }

    pub(crate) fn complete(&self, run_id: &str, report: InitializeReport) -> Result<(), String> {
        self.complete_at(run_id, report, Instant::now())
    }

    fn complete_at(
        &self,
        run_id: &str,
        report: InitializeReport,
        now: Instant,
    ) -> Result<(), String> {
        self.finish_at(run_id, InitializeRunOutcome::Completed { report }, now)
    }

    pub(crate) fn fail(&self, run_id: &str, error: String) -> Result<(), String> {
        self.finish_at(
            run_id,
            InitializeRunOutcome::Failed { error },
            Instant::now(),
        )
    }

    fn finish_at(
        &self,
        run_id: &str,
        outcome: InitializeRunOutcome,
        now: Instant,
    ) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "team initialization run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("team initialization run '{run_id}' was not found"))?;
        if record.outcome != InitializeRunOutcome::Running {
            return Err(format!(
                "team initialization run '{run_id}' is already terminal"
            ));
        }
        record.outcome = outcome;
        record.terminal_at = Some(now);
        Ok(())
    }

    pub(crate) fn status(&self, run_id: &str) -> Option<InitializeRunStatus> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, Instant::now());
        records.get(run_id).map(|record| InitializeRunStatus {
            run_id: run_id.to_string(),
            steps: record.steps.clone(),
            outcome: record.outcome.clone(),
        })
    }

    fn prune_at(&self, now: Instant) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
    }

    fn prune_locked(
        records: &mut HashMap<String, InitializeRunRecord>,
        ttl: Duration,
        now: Instant,
    ) {
        records.retain(|_, record| {
            record
                .terminal_at
                .is_none_or(|terminal_at| now.saturating_duration_since(terminal_at) <= ttl)
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{InitializeRunOutcome, InitializeRunRegistry};
    use crate::coordination::requests::{InitializeReport, StepProgress, StepStatus};

    fn progress(step: &str, status: StepStatus) -> StepProgress {
        StepProgress {
            step: step.to_string(),
            status,
            message: None,
        }
    }

    fn report() -> InitializeReport {
        InitializeReport {
            team_name: "daemon-init".to_string(),
            succeeded_steps: vec!["validate_configuration".to_string()],
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps: vec![progress("validate_configuration", StepStatus::Succeeded)],
        }
    }

    #[test]
    fn run_lifecycle_mirrors_steps_and_completed_outcome() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start();

        registry
            .record_step(
                &run_id,
                progress("validate_configuration", StepStatus::Running),
            )
            .expect("running step recorded");
        registry
            .record_step(
                &run_id,
                progress("validate_configuration", StepStatus::Succeeded),
            )
            .expect("terminal step recorded");

        let running = registry.status(&run_id).expect("running status");
        assert_eq!(running.steps.len(), 2);
        assert_eq!(running.outcome, InitializeRunOutcome::Running);

        registry.complete(&run_id, report()).expect("run completed");
        let completed = registry.status(&run_id).expect("completed status");
        assert_eq!(
            completed.outcome,
            InitializeRunOutcome::Completed { report: report() }
        );
    }

    #[test]
    fn failed_run_preserves_steps_and_terminal_error() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start();
        registry
            .record_step(&run_id, progress("create_team", StepStatus::Running))
            .expect("step recorded");
        registry
            .fail(&run_id, "pipeline panicked".to_string())
            .expect("run failed");

        let status = registry.status(&run_id).expect("failed status");
        assert_eq!(status.steps.len(), 1);
        assert_eq!(
            status.outcome,
            InitializeRunOutcome::Failed {
                error: "pipeline panicked".to_string()
            }
        );
    }

    #[test]
    fn terminal_runs_expire_after_ttl_but_running_runs_do_not() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(10));
        let started_at = Instant::now();
        let running_id = registry.start_at(started_at);
        let completed_id = registry.start_at(started_at);
        registry
            .complete_at(&completed_id, report(), started_at)
            .expect("run completed");

        registry.prune_at(started_at + Duration::from_secs(11));

        assert!(registry.status(&completed_id).is_none());
        assert!(registry.status(&running_id).is_some());
    }
}
