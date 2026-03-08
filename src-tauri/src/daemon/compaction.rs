use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::coordination::compaction_processor::{
    CompactionSignalProcessOutcome, CompactionSignalProcessor,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::{TeamConfig, TeamConfigStore};
use crate::session_scanner::compaction_extractor;
use crate::session_scanner::compaction_watcher::{
    CompactionSignalWatcher, CompactionSignalWatcherConfig,
};
use crate::session_scanner::RuntimeSession;

const DEFAULT_SCAN_INTERVAL: Duration = Duration::from_millis(500);

type SessionSupplier = dyn Fn() -> Vec<RuntimeSession> + Send + Sync + 'static;
type SignalProcessor = dyn Fn(&crate::coordination::stores::CompactionSignalRecord) -> Result<(), String>
    + Send
    + Sync
    + 'static;

pub struct DaemonCompactionRuntime {
    shutdown: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
    watchers: Arc<Mutex<HashMap<String, CompactionSignalWatcher>>>,
}

impl DaemonCompactionRuntime {
    pub fn maybe_start() -> Result<Option<Self>, CoordinationError> {
        if !running_under_wsl() {
            return Ok(None);
        }

        let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
        let supplier: Arc<SessionSupplier> =
            Arc::new(crate::session_scanner::scan_sessions_for_runtime);
        let runtime = Self::start_with_supplier_and_processor(
            teams_dir,
            supplier,
            DEFAULT_SCAN_INTERVAL,
            CompactionSignalWatcherConfig::default(),
            Arc::new(
                |signal: &crate::coordination::stores::CompactionSignalRecord| {
                    match CompactionSignalProcessor::process_signal(signal) {
                        CompactionSignalProcessOutcome::Failed { error_message, .. } => {
                            Err(error_message)
                        }
                        _ => Ok(()),
                    }
                },
            ),
        )?;
        Ok(Some(runtime))
    }

    fn start_with_supplier_and_processor(
        teams_dir: PathBuf,
        session_supplier: Arc<SessionSupplier>,
        scan_interval: Duration,
        watcher_config: CompactionSignalWatcherConfig,
        processor: Arc<SignalProcessor>,
    ) -> Result<Self, CoordinationError> {
        let initial_sessions = session_supplier();
        compaction_extractor::start_compaction_extractor_service_at(
            teams_dir.clone(),
            initial_sessions,
        )?;

        let watchers = Arc::new(Mutex::new(HashMap::new()));
        reconcile_team_watchers(&teams_dir, &watchers, watcher_config, processor.clone())?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_teams_dir = teams_dir.clone();
        let thread_watchers = watchers.clone();
        let thread_supplier = session_supplier.clone();
        let thread_processor = processor.clone();
        let join_handle = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                let _ = thread_supplier();
                if let Err(error) = reconcile_team_watchers(
                    &thread_teams_dir,
                    &thread_watchers,
                    watcher_config,
                    thread_processor.clone(),
                ) {
                    tracing::warn!(error = %error, "daemon compaction watcher reconcile failed");
                }
                thread::sleep(scan_interval);
            }
        });

        Ok(Self {
            shutdown,
            join_handle: Some(join_handle),
            watchers,
        })
    }
}

impl Drop for DaemonCompactionRuntime {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
        self.watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        compaction_extractor::stop_compaction_extractor_service();
    }
}

fn running_under_wsl() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WSL_DISTRO_NAME").is_some()
}

fn reconcile_team_watchers(
    teams_dir: &Path,
    watchers: &Arc<Mutex<HashMap<String, CompactionSignalWatcher>>>,
    watcher_config: CompactionSignalWatcherConfig,
    processor: Arc<SignalProcessor>,
) -> Result<(), CoordinationError> {
    let mut guard = watchers.lock().unwrap_or_else(|error| error.into_inner());
    let mut desired = HashMap::new();
    for team_name in TeamConfigStore::list(teams_dir)? {
        match team_has_managed_codex_member(teams_dir, &team_name) {
            Ok(true) => {
                desired.insert(team_name, ());
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    team_name,
                    error = %error,
                    "skipping compaction watcher startup for team without valid config"
                );
            }
        }
    }

    guard.retain(|team_name, _| desired.contains_key(team_name));
    for team_name in desired.into_keys() {
        if guard.contains_key(&team_name) {
            continue;
        }
        let watcher_processor = {
            let processor = processor.clone();
            Arc::new(
                move |signal: &crate::coordination::stores::CompactionSignalRecord| {
                    processor(signal)
                },
            )
        };
        let watcher = CompactionSignalWatcher::start_at(
            teams_dir.to_path_buf(),
            team_name.clone(),
            watcher_processor,
            watcher_config,
        )?;
        guard.insert(team_name, watcher);
    }

    Ok(())
}

fn team_has_managed_codex_member(
    teams_dir: &Path,
    team_name: &str,
) -> Result<bool, CoordinationError> {
    let config: TeamConfig = TeamConfigStore::load(teams_dir, team_name)?;
    Ok(config
        .members
        .iter()
        .any(|member| member.cli_tool == crate::session_scanner::cli_tool::CliTool::Codex))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Instant;

    use chrono::{DateTime, Utc};

    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::stores::{
        MemberRuntimeRecord, MemberRuntimeStore, MeshInboxStore,
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfigStore,
    };
    use crate::session_scanner::cli_tool::CliTool;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, SessionGroupKind, SessionState,
    };

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(name: &str, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: Some(format!("{name}-role")),
            role_name: Some(format!("{name} role")),
            focus_area: Some("Keep task execution aligned".to_string()),
            context_summary: Some("Maintains project context".to_string()),
            behavior_summary: Some("Stay concrete and report blockers".to_string()),
            instructions: Some("Implement assigned work".to_string()),
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
        }
    }

    fn sample_snapshot(
        team_name: &str,
        member_name: &str,
        project_path: &str,
    ) -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            version: 1,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            updated_at: timestamp("2026-03-08T14:10:00Z"),
            task: OperationalTaskSnapshot {
                id: "736".to_string(),
                subject: "Move Windows compaction into daemon".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec!["src-tauri/src/daemon/compaction.rs".to_string()],
                adjacent_fix_policy: "local validation only".to_string(),
                validation_expectation: "just check-quick".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: project_path.to_string(),
                focal_files: vec!["src-tauri/src/daemon/compaction.rs".to_string()],
            },
        }
    }

    fn save_team_fixture(teams_dir: &Path, team_name: &str, member: &Member) {
        let config = TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: timestamp("2026-03-08T14:00:00Z"),
            members: vec![member.clone()],
        };
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save team config");

        let runtime = MemberRuntimeRecord {
            schema_version: 3,
            member_name: member.name.clone(),
            cli_tool: Some(member.cli_tool),
            project_path: Some(member.project_path.clone()),
            pane_id: Some("%7".to_string()),
            session_id: Some("session-1".to_string()),
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::Healthy,
            delivery_lease: None,
            attached_at: Some(timestamp("2026-03-08T14:01:00Z")),
            last_seen_at: Some(timestamp("2026-03-08T14:02:00Z")),
        };
        MemberRuntimeStore::save(teams_dir, team_name, &member.name, &runtime)
            .expect("save runtime");
        OperationalContextSnapshotStore::save(
            teams_dir,
            &sample_snapshot(
                team_name,
                &member.name,
                &member.project_path.display().to_string(),
            ),
        )
        .expect("save snapshot");
    }

    fn sample_session(project_path: &str, jsonl_path: &Path) -> RuntimeSession {
        RuntimeSession {
            pid: 1234,
            project_path: project_path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex resume --last".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("main".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            session_id: Some("session-1".to_string()),
            jsonl_path: Some(jsonl_path.display().to_string()),
            recent_io: false,
            last_output_age_secs: Some(0),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("condition was not met before timeout");
    }

    #[test]
    fn daemon_runtime_delivers_codex_compaction_without_app_local_pipeline() {
        let _guard = TEST_LOCK.lock().expect("lock");
        compaction_extractor::stop_compaction_extractor_service_for_test();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let team_name = "taurhaus-team";
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(&teams_dir, team_name, &member);

        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(
            &jsonl_path,
            concat!(
                "{\"timestamp\":\"2026-03-08T13:46:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/home/mstie/projects/taurhaus\"}}\n"
            ),
        )
        .expect("write baseline jsonl");

        let sessions = Arc::new(Mutex::new(vec![sample_session(project_path, &jsonl_path)]));
        let supplier: Arc<SessionSupplier> = {
            let sessions = sessions.clone();
            Arc::new(move || {
                sessions
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone()
            })
        };
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex"));

        let processor_teams_dir = teams_dir.clone();
        let processor_runtime = runtime.clone();
        let _runtime = DaemonCompactionRuntime::start_with_supplier_and_processor(
            teams_dir.clone(),
            supplier,
            Duration::from_millis(25),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_millis(100),
                loop_tick: Duration::from_millis(25),
            },
            Arc::new(
                move |signal: &crate::coordination::stores::CompactionSignalRecord| {
                    match crate::coordination::compaction_processor::CompactionSignalProcessor::process_signal_at(
                        signal,
                        &processor_teams_dir,
                        processor_runtime.as_ref(),
                        timestamp("2026-03-08T13:46:42Z"),
                    ) {
                        CompactionSignalProcessOutcome::Failed { error_message, .. } => {
                            Err(error_message)
                        }
                        _ => Ok(()),
                    }
                },
            ),
        )
        .expect("start daemon compaction runtime");

        std::thread::sleep(Duration::from_millis(80));
        std::fs::OpenOptions::new()
            .append(true)
            .open(&jsonl_path)
            .expect("open jsonl for append")
            .write_all(
                br#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}
"#,
            )
            .expect("append compaction line");

        wait_until(Duration::from_secs(3), || {
            MeshInboxStore::load(&teams_dir, team_name, "developer2")
                .map(|messages| !messages.is_empty())
                .unwrap_or(false)
        });
    }

    #[test]
    fn daemon_runtime_skips_orphaned_team_dirs_without_failing() {
        let _guard = TEST_LOCK.lock().expect("lock");
        compaction_extractor::stop_compaction_extractor_service_for_test();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(teams_dir.join("default")).expect("create orphaned team dir");

        let member = sample_member("developer2", "/home/mstie/projects/taurhaus");
        save_team_fixture(&teams_dir, "taurhaus-team", &member);

        let runtime = DaemonCompactionRuntime::start_with_supplier_and_processor(
            teams_dir.clone(),
            Arc::new(Vec::new),
            Duration::from_millis(25),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_millis(100),
                loop_tick: Duration::from_millis(25),
            },
            Arc::new(|_| Ok(())),
        )
        .expect("start daemon compaction runtime");

        let guard = runtime
            .watchers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(guard.contains_key("taurhaus-team"));
        assert!(!guard.contains_key("default"));
        drop(guard);
        drop(runtime);

        let listed = TeamConfigStore::list(&teams_dir).expect("list teams");
        assert!(listed.contains(&"default".to_string()));
    }
}
