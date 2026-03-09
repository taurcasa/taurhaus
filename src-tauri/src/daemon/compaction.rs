use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::coordination::compaction_processor::{
    CompactionSignalProcessOutcome, CompactionSignalProcessor,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::{TeamConfig, TeamConfigStore};
use crate::session_scanner::compaction_extractor;
use crate::session_scanner::compaction_watcher::{
    CompactionSignalWatcher, CompactionSignalWatcherConfig,
};
const TEAM_CONFIG_FILENAME: &str = "config.json";
const TEAM_CONFIG_TMP_FILENAME: &str = "config.json.tmp";

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
        let runtime = Self::start_with_processor(
            teams_dir,
            crate::session_scanner::latest_compaction_runtime_sessions(),
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

    fn start_with_processor(
        teams_dir: PathBuf,
        initial_sessions: Vec<crate::session_scanner::RuntimeSession>,
        watcher_config: CompactionSignalWatcherConfig,
        processor: Arc<SignalProcessor>,
    ) -> Result<Self, CoordinationError> {
        fs::create_dir_all(&teams_dir)?;
        compaction_extractor::start_compaction_extractor_service_at(
            teams_dir.clone(),
            initial_sessions,
        )?;

        let watchers = Arc::new(Mutex::new(HashMap::new()));
        reconcile_team_watchers(&teams_dir, &watchers, watcher_config, processor.clone())?;
        let (topology_tx, topology_rx) = mpsc::channel();
        let topology_watcher = start_team_topology_watcher(&teams_dir, topology_tx)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_teams_dir = teams_dir.clone();
        let thread_watchers = watchers.clone();
        let thread_processor = processor.clone();
        let join_handle = thread::spawn(move || {
            let _topology_watcher = topology_watcher;
            let mut next_watcher_reconcile =
                Instant::now() + watcher_config.reconciliation_interval;

            while !thread_shutdown.load(Ordering::Relaxed) {
                let mut topology_changed = false;
                match topology_rx.recv_timeout(next_loop_wait(
                    next_watcher_reconcile,
                    watcher_config.loop_tick,
                )) {
                    Ok(event) => {
                        topology_changed |= is_team_topology_event(&thread_teams_dir, &event);
                        while let Ok(pending_event) = topology_rx.try_recv() {
                            topology_changed |=
                                is_team_topology_event(&thread_teams_dir, &pending_event);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        tracing::warn!("daemon compaction topology watcher disconnected");
                    }
                }

                let now = Instant::now();
                if topology_changed || now >= next_watcher_reconcile {
                    if let Err(error) = reconcile_team_watchers(
                        &thread_teams_dir,
                        &thread_watchers,
                        watcher_config,
                        thread_processor.clone(),
                    ) {
                        tracing::warn!(error = %error, "daemon compaction watcher reconcile failed");
                    }
                    next_watcher_reconcile = now + watcher_config.reconciliation_interval;
                }
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
    let desired = desired_watcher_teams(teams_dir)?;

    guard.retain(|team_name, _| desired.contains(team_name));
    for team_name in desired {
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

fn start_team_topology_watcher(
    teams_dir: &Path,
    tx: mpsc::Sender<Event>,
) -> Result<RecommendedWatcher, CoordinationError> {
    let teams_dir = teams_dir.to_path_buf();
    let log_teams_dir = teams_dir.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| match result {
            Ok(event) => {
                let _ = tx.send(event);
            }
            Err(error) => {
                tracing::warn!(
                    teams_dir = %log_teams_dir.display(),
                    error = %error,
                    "daemon compaction topology watcher emitted notify error"
                );
            }
        },
        Config::default(),
    )
    .map_err(|error| CoordinationError::StoreError(error.to_string()))?;
    watcher
        .watch(&teams_dir, RecursiveMode::Recursive)
        .map_err(|error| CoordinationError::StoreError(error.to_string()))?;
    Ok(watcher)
}

fn next_loop_wait(next_watcher_reconcile: Instant, max_wait: Duration) -> Duration {
    let now = Instant::now();
    let until_reconcile = next_watcher_reconcile.saturating_duration_since(now);
    until_reconcile.min(max_wait)
}

fn is_team_topology_event(teams_dir: &Path, event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event
        .paths
        .iter()
        .any(|path| is_team_topology_path(teams_dir, path))
}

fn is_team_topology_path(teams_dir: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(teams_dir) else {
        return false;
    };

    let mut components = relative.components();
    let Some(team_name) = components.next() else {
        return false;
    };
    if team_name.as_os_str().is_empty() {
        return false;
    }

    match (components.next(), components.next()) {
        (None, None) => true,
        (Some(file_name), None) => {
            file_name.as_os_str() == TEAM_CONFIG_FILENAME
                || file_name.as_os_str() == TEAM_CONFIG_TMP_FILENAME
        }
        _ => false,
    }
}

fn desired_watcher_teams(teams_dir: &Path) -> Result<BTreeSet<String>, CoordinationError> {
    let mut desired = BTreeSet::new();
    for team_name in TeamConfigStore::list(teams_dir)? {
        match team_has_managed_codex_member(teams_dir, &team_name) {
            Ok(true) => {
                desired.insert(team_name);
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
    Ok(desired)
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
        ActivityAttribution, ActivityConfidence, RuntimeSession, SessionGroupKind, SessionState,
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
    fn daemon_compaction_runtime_bootstrap_and_watchers_deliver_codex_compaction() {
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
            "{\"timestamp\":\"2026-03-08T13:46:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/home/mstie/projects/taurhaus\"}}\n",
        )
        .expect("write baseline jsonl");

        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex"));

        let processor_teams_dir = teams_dir.clone();
        let processor_runtime = runtime.clone();
        let _runtime = DaemonCompactionRuntime::start_with_processor(
            teams_dir.clone(),
            vec![sample_session(project_path, &jsonl_path)],
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

        let runtime = DaemonCompactionRuntime::start_with_processor(
            teams_dir.clone(),
            Vec::new(),
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

    #[test]
    fn desired_watcher_teams_only_includes_teams_with_managed_codex_members() {
        let _guard = TEST_LOCK.lock().expect("lock");

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        save_team_fixture(
            &teams_dir,
            "codex-team",
            &sample_member("developer2", "/home/mstie/projects/taurhaus"),
        );

        let claude_member = Member {
            cli_tool: CliTool::Claude,
            ..sample_member("reviewer", "/home/mstie/projects/taurhaus")
        };
        save_team_fixture(&teams_dir, "claude-team", &claude_member);

        let desired = desired_watcher_teams(&teams_dir).expect("load watcher teams");
        assert!(desired.contains("codex-team"));
        assert!(!desired.contains("claude-team"));
    }

    #[test]
    fn topology_events_start_and_stop_team_watchers_without_waiting_for_fallback_reconcile() {
        let _guard = TEST_LOCK.lock().expect("lock");
        compaction_extractor::stop_compaction_extractor_service_for_test();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        save_team_fixture(
            &teams_dir,
            "alpha",
            &sample_member("developer2", "/home/mstie/projects/taurhaus"),
        );

        let runtime = DaemonCompactionRuntime::start_with_processor(
            teams_dir.clone(),
            Vec::new(),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_secs(5),
                loop_tick: Duration::from_millis(25),
            },
            Arc::new(|_| Ok(())),
        )
        .expect("start daemon compaction runtime");

        wait_until(Duration::from_secs(1), || {
            let guard = runtime
                .watchers
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            guard.contains_key("alpha")
        });

        save_team_fixture(
            &teams_dir,
            "beta",
            &sample_member("developer3", "/home/mstie/projects/taurhaus"),
        );
        wait_until(Duration::from_secs(1), || {
            let guard = runtime
                .watchers
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            guard.contains_key("beta")
        });

        TeamConfigStore::delete(&teams_dir, "beta").expect("delete beta");
        wait_until(Duration::from_secs(1), || {
            let guard = runtime
                .watchers
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            !guard.contains_key("beta")
        });
    }
}
