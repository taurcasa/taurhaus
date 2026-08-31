//! Daemon-owned scheduler for managed-task deadline actions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::coordination::state::CoordinationState;

const INITIAL_DELAY: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_secs(30);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) struct DeadlineScheduler {
    handle: Option<thread::JoinHandle<()>>,
}

impl DeadlineScheduler {
    pub(crate) fn start(state: CoordinationState, shutdown: Arc<AtomicBool>) -> Self {
        Self::start_with_cadence(state, shutdown, INITIAL_DELAY, CHECK_INTERVAL)
    }

    fn start_with_cadence(
        state: CoordinationState,
        shutdown: Arc<AtomicBool>,
        initial_delay: Duration,
        check_interval: Duration,
    ) -> Self {
        let handle = thread::Builder::new()
            .name("coordination-deadlines".to_string())
            .spawn(move || {
                if wait_or_shutdown(&shutdown, initial_delay) {
                    return;
                }
                while !shutdown.load(Ordering::Relaxed) {
                    match state.run_background_task_deadline_pass() {
                        Ok(summary) => {
                            if summary.team_errors > 0 {
                                tracing::warn!(
                                    teams_scanned = summary.teams_scanned,
                                    team_errors = summary.team_errors,
                                    "daemon task-deadline pass completed with errors"
                                );
                            }
                        }
                        Err(error) => {
                            tracing::warn!(error = %error, "daemon task-deadline pass failed");
                        }
                    }
                    if wait_or_shutdown(&shutdown, check_interval) {
                        return;
                    }
                }
            })
            .unwrap_or_else(|error| panic!("failed to start deadline scheduler: {error}"));
        Self {
            handle: Some(handle),
        }
    }

    pub(crate) fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn wait_or_shutdown(shutdown: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(STOP_POLL_INTERVAL));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::{Duration as ChronoDuration, Utc};

    use super::DeadlineScheduler;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig, TeamConfigStore,
    };
    use crate::session_scanner::cli_tool::CliTool;

    fn member(name: &str, role: MemberRole) -> Member {
        Member {
            name: name.to_string(),
            role,
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
            project_path: PathBuf::from("/tmp/deadline-project"),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    fn seed_overdue_task(root: &std::path::Path) -> (PathBuf, chrono::DateTime<Utc>) {
        let teams_dir = root.join("teams");
        TeamConfigStore::save(
            &teams_dir,
            "deadline-team",
            &TeamConfig {
                schema_version: 1,
                name: "deadline-team".to_string(),
                description: None,
                created_at: Utc::now(),
                members: vec![member("builder", MemberRole::Agent)],
                extra: Default::default(),
            },
        )
        .expect("seed team config");

        let assigned_at = Utc::now() - ChronoDuration::minutes(30);
        OperationalContextSnapshotStore::save(
            &teams_dir,
            &OperationalContextSnapshot {
                version: 1,
                team_name: "deadline-team".to_string(),
                member_name: "builder".to_string(),
                updated_at: assigned_at,
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Run the migration".to_string(),
                    status: "in_progress".to_string(),
                    deadline_minutes: Some(20),
                    assigned_at: Some(assigned_at),
                    nudged_at: None,
                    stale_at: None,
                },
                assignment_footer: OperationalAssignmentFooterSnapshot::default(),
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "/tmp/deadline-project".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("seed operational snapshot");

        let task_dir = root.join("tasks/deadline-team");
        std::fs::create_dir_all(&task_dir).expect("create task dir");
        std::fs::write(
            task_dir.join("42.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "id": "42",
                "subject": "Run the migration",
                "status": "in_progress",
                "owner": "builder",
                "metadata": { "deadline_minutes": 20 },
            }))
            .expect("serialize task"),
        )
        .expect("seed task");
        (teams_dir, assigned_at)
    }

    #[test]
    fn registered_daemon_scheduler_fires_the_deadline_pass() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let log_state =
            crate::commands::logging::LogFileState::new(temp.path().join("taurhaus.log.jsonl"))
                .expect("log state");
        crate::commands::logging::install_global_sink(&log_state);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(event_tx);

        let (teams_dir, _assigned_at) = seed_overdue_task(temp.path());
        let fake = FakeBackend::default();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = CoordinationState::with_components_and_runtime(
            teams_dir.clone(),
            BackendSelector::m0(),
            Arc::new(move |_kind, _teams_dir| {
                Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime.clone()),
        );
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler = DeadlineScheduler::start_with_cadence(
            state,
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let event = loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let record = event_rx
                .recv_timeout(remaining)
                .expect("deadline event within two seconds");
            if record["event"] == "deadline.task.staled" {
                break record;
            }
        };
        shutdown.store(true, Ordering::Relaxed);
        scheduler.join();
        crate::commands::logging::clear_test_tap();

        assert_eq!(event["component"], "coordination");
        assert_eq!(event["fields"]["team"], "deadline-team");
        assert_eq!(event["fields"]["member"], "builder");
        assert_eq!(event["fields"]["task_id"], "42");
        assert_eq!(event["fields"]["deadline_minutes"], 20);

        let snapshot =
            OperationalContextSnapshotStore::load(&teams_dir, "deadline-team", "builder")
                .expect("load snapshot")
                .expect("snapshot exists");
        assert_eq!(snapshot.task.status, "stale");
        assert!(snapshot.task.stale_at.is_some());

        let task: serde_json::Value = serde_json::from_slice(
            &std::fs::read(temp.path().join("tasks/deadline-team/42.json")).expect("read task"),
        )
        .expect("parse task");
        assert_eq!(task["status"], "stale");
    }
}
