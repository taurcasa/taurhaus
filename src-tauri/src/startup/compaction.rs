use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::coordination::compaction_processor::{
    CompactionSignalProcessOutcome, CompactionSignalProcessor,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::TeamConfigStore;
use crate::session_scanner::compaction_extractor;
use crate::session_scanner::compaction_watcher::CompactionSignalWatcher;
use crate::session_scanner::scan_sessions_for_runtime;

#[allow(dead_code)]
pub struct CompactionWatcherState(pub Mutex<Vec<CompactionSignalWatcher>>);

pub(crate) fn initialize(app: &mut tauri::App) -> Result<(), CoordinationError> {
    if cfg!(target_os = "windows") {
        return Ok(());
    }

    let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
    let initial_sessions = scan_sessions_for_runtime();
    compaction_extractor::start_compaction_extractor_service_at(
        teams_dir.clone(),
        initial_sessions,
    )?;

    let watchers = start_team_watchers(&teams_dir)?;
    app.manage(CompactionWatcherState(Mutex::new(watchers)));
    Ok(())
}

fn start_team_watchers(
    teams_dir: &PathBuf,
) -> Result<Vec<CompactionSignalWatcher>, CoordinationError> {
    let processor = Arc::new(
        |signal: &crate::coordination::stores::CompactionSignalRecord| {
            match CompactionSignalProcessor::process_signal(signal) {
                CompactionSignalProcessOutcome::Failed { error_message, .. } => Err(error_message),
                _ => Ok(()),
            }
        },
    );

    let mut watchers = Vec::new();
    for team_name in TeamConfigStore::list(teams_dir)? {
        match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    team_name,
                    error = %error,
                    "skipping compaction watcher startup for team without valid config"
                );
                continue;
            }
        }
        let watcher = CompactionSignalWatcher::start_at(
            teams_dir.clone(),
            team_name,
            processor.clone(),
            Default::default(),
        )?;
        watchers.push(watcher);
    }
    Ok(watchers)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use chrono::{DateTime, Utc};

    use crate::coordination::compaction_processor::CompactionSignalProcessOutcome;
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::stores::{
        MemberRuntimeRecord, MemberRuntimeStore, MeshInboxStore,
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig, TeamConfigStore,
    };
    use crate::session_scanner::cli_tool::CliTool;
    use crate::session_scanner::compaction_extractor::{
        start_compaction_extractor_service_for_test, stop_compaction_extractor_service_for_test,
    };
    use crate::session_scanner::compaction_watcher::CompactionSignalWatcherConfig;
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
                id: "735".to_string(),
                subject: "Wire extractor watcher processor".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec![
                    "src-tauri/src/session_scanner/compaction_extractor.rs".to_string(),
                    "src-tauri/src/session_scanner/compaction_watcher.rs".to_string(),
                ],
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
                focal_files: vec![
                    "src-tauri/src/session_scanner/compaction_extractor.rs".to_string(),
                    "src-tauri/src/session_scanner/compaction_watcher.rs".to_string(),
                ],
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
            schema_version: 2,
            member_name: member.name.clone(),
            cli_tool: Some(member.cli_tool),
            project_path: Some(member.project_path.clone()),
            pane_id: Some("%7".to_string()),
            session_id: Some("session-1".to_string()),
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
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("condition was not met before timeout");
    }

    #[test]
    fn extractor_watcher_processor_pipeline_delivers_inbox_message() {
        let _guard = TEST_LOCK.lock().expect("lock");
        stop_compaction_extractor_service_for_test();

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

        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex"));

        let processor_teams_dir = teams_dir.clone();
        let processor_runtime = runtime.clone();
        let _watcher = CompactionSignalWatcher::start_at(
            teams_dir.clone(),
            team_name,
            Arc::new(move |signal: &crate::coordination::stores::CompactionSignalRecord| {
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
            }),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_millis(100),
                loop_tick: Duration::from_millis(25),
            },
        )
        .expect("start watcher");

        start_compaction_extractor_service_for_test(
            teams_dir.clone(),
            vec![sample_session(project_path, &jsonl_path)],
            Duration::from_millis(25),
        )
        .expect("start extractor service");

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

        let inbox = MeshInboxStore::load(&teams_dir, team_name, "developer2").expect("load inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].summary.as_deref(), Some("post_compaction_context"));

        stop_compaction_extractor_service_for_test();
    }

    #[test]
    fn start_team_watchers_skips_orphaned_team_dirs_without_failing() {
        let _guard = TEST_LOCK.lock().expect("lock");

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(teams_dir.join("default")).expect("create orphaned team dir");

        let member = sample_member("developer2", "/home/mstie/projects/taurhaus");
        save_team_fixture(&teams_dir, "taurhaus-team", &member);

        let watchers = start_team_watchers(&teams_dir).expect("start watchers");
        assert_eq!(watchers.len(), 1);
    }
}
