//! Daemon-owned scheduler for managed-task deadline actions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::coordination::state::CoordinationState;

const INITIAL_DELAY: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_secs(30);
const STOP_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SHUTDOWN_JOIN_GRACE: Duration = Duration::from_secs(1);

pub(crate) struct DeadlineScheduler {
    handle: Option<thread::JoinHandle<()>>,
}

impl DeadlineScheduler {
    pub(crate) fn start(state: Arc<CoordinationState>, shutdown: Arc<AtomicBool>) -> Self {
        Self::start_with_cadence(state, shutdown, INITIAL_DELAY, CHECK_INTERVAL)
    }

    fn start_with_cadence(
        state: Arc<CoordinationState>,
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
                    let started = Instant::now();
                    match state.run_background_task_deadline_pass() {
                        Ok(summary) => {
                            emit_pass_completed(
                                summary.teams_scanned,
                                summary.team_errors,
                                started.elapsed(),
                            );
                            if summary.team_errors > 0 {
                                tracing::warn!(
                                    teams_scanned = summary.teams_scanned,
                                    team_errors = summary.team_errors,
                                    "daemon task-deadline pass completed with errors"
                                );
                            }
                        }
                        Err(error) => {
                            emit_pass_failed(&error.to_string(), started.elapsed(), "pass");
                            tracing::warn!(error = %error, "daemon task-deadline pass failed");
                        }
                    }
                    if wait_or_shutdown(&shutdown, check_interval) {
                        return;
                    }
                }
            });
        match handle {
            Ok(handle) => Self {
                handle: Some(handle),
            },
            Err(error) => {
                emit_pass_failed(&error.to_string(), Duration::ZERO, "spawn");
                tracing::warn!(error = %error, "daemon task-deadline scheduler not spawned");
                Self { handle: None }
            }
        }
    }

    pub(crate) fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let deadline = Instant::now() + SHUTDOWN_JOIN_GRACE;
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if !handle.is_finished() {
                // An ordinary shutdown overlapping a slow pass is not a pass
                // failure; the detach itself is the only noteworthy fact.
                tracing::warn!("daemon task-deadline scheduler detached during shutdown");
                return;
            }
            let _ = handle.join();
        }
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

fn emit_pass_completed(teams_scanned: usize, team_errors: usize, duration: Duration) {
    let mut fields = Map::new();
    fields.insert("teams_scanned".to_string(), Value::from(teams_scanned));
    fields.insert("team_errors".to_string(), Value::from(team_errors));
    fields.insert(
        "duration_ms".to_string(),
        Value::from(duration_millis(duration)),
    );
    // A 30-second heartbeat is periodic health, not an operator-actionable
    // change: debug per the log-level policy. Errors surface separately.
    taurhaus_lib::logging::emit_global(
        "debug",
        "coordination",
        "deadline.pass.completed",
        Some("Daemon task-deadline pass completed".to_string()),
        fields,
    );
}

fn emit_pass_failed(error: &str, duration: Duration, phase: &str) {
    let mut fields = Map::new();
    fields.insert("error".to_string(), Value::String(error.to_string()));
    fields.insert("phase".to_string(), Value::String(phase.to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::from(duration_millis(duration)),
    );
    taurhaus_lib::logging::emit_global(
        "warn",
        "coordination",
        "deadline.pass.failed",
        Some("Daemon task-deadline pass failed".to_string()),
        fields,
    );
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

    fn install_log_tap(root: &std::path::Path) -> std::sync::mpsc::Receiver<serde_json::Value> {
        let log_state =
            crate::commands::logging::LogFileState::new(root.join("taurhaus.log.jsonl"))
                .expect("log state");
        crate::commands::logging::install_global_sink(&log_state);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(event_tx);
        event_rx
    }

    fn receive_event(
        event_rx: &std::sync::mpsc::Receiver<serde_json::Value>,
        expected: &str,
    ) -> serde_json::Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let record = event_rx
                .recv_timeout(remaining)
                .unwrap_or_else(|_| panic!("{expected} event within two seconds"));
            if record["event"] == expected {
                return record;
            }
        }
    }

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
            account_id: None,
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
        let event_rx = install_log_tap(temp.path());

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
            Arc::new(state),
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let event = receive_event(&event_rx, "deadline.task.staled");
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

    // Regression: 34fdeead moved deadline execution into the daemon but emitted
    // no successful-pass record, so the paid E2E lane could mistake an unrelated
    // app self-heal event for proof that the daemon scheduler was alive.
    #[test]
    fn daemon_scheduler_emits_a_record_for_each_completed_pass() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let teams_dir = temp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        let state = CoordinationState::with_components_and_runtime(
            teams_dir,
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
        );
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler = DeadlineScheduler::start_with_cadence(
            Arc::new(state),
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let event = receive_event(&event_rx, "deadline.pass.completed");
        shutdown.store(true, Ordering::Relaxed);
        scheduler.join();
        crate::commands::logging::clear_test_tap();

        assert_eq!(event["component"], "coordination");
        assert_eq!(event["fields"]["teams_scanned"], 0);
        assert_eq!(event["fields"]["team_errors"], 0);
        assert!(event["fields"]["duration_ms"].is_number());
    }

    // Regression: 34fdeead reported daemon deadline-pass failures only through
    // tracing stderr, which production launchers discard instead of retaining in
    // the canonical JSONL log.
    #[test]
    fn daemon_scheduler_emits_a_record_when_a_pass_fails() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let teams_dir = temp.path().join("teams-not-a-directory");
        std::fs::write(&teams_dir, "not a directory").expect("invalid teams root");
        let state = CoordinationState::with_components_and_runtime(
            teams_dir,
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
        );
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler = DeadlineScheduler::start_with_cadence(
            Arc::new(state),
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let event = receive_event(&event_rx, "deadline.pass.failed");
        shutdown.store(true, Ordering::Relaxed);
        scheduler.join();
        crate::commands::logging::clear_test_tap();

        assert_eq!(event["component"], "coordination");
        assert!(event["fields"]["error"].is_string());
        assert!(event["fields"]["duration_ms"].is_number());
    }
}
