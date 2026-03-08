//! Codex post-compaction detection integrated into the session scanner poll loop.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{cli_tool::CliTool, RuntimeSession};
use crate::coordination::compaction_events::{
    emit_compaction_extractor_failed, emit_compaction_signal_emitted, signal_event,
    CompactionExtractorFailedEvent,
};
use crate::coordination::compaction_processor::{
    CompactionSignalProcessOutcome, CompactionSignalProcessor,
};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{
    CompactionSignalKind as StoreSignalKind, CompactionSignalRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCodexCompactionSignal {
    pub signal: CompactionSignalRecord,
}

#[derive(Debug, Default)]
struct CompactionWatcherState {
    offsets: HashMap<PathBuf, u64>,
    pending: VecDeque<PendingCodexCompactionSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactionSignalKind {
    Compacted,
    ContextCompacted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexCompactionEvent {
    session_id: String,
    timestamp: DateTime<Utc>,
    kind: CompactionSignalKind,
}

static WATCHER_STATE: OnceLock<Mutex<CompactionWatcherState>> = OnceLock::new();

fn watcher_state() -> &'static Mutex<CompactionWatcherState> {
    WATCHER_STATE.get_or_init(|| Mutex::new(CompactionWatcherState::default()))
}

pub fn process_codex_compaction_events(sessions: &[RuntimeSession]) {
    let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
    process_codex_compaction_events_at(sessions, &teams_dir);
    let runtime = SystemCoordinationRuntime;
    deliver_pending_codex_compaction_reinjections_at(sessions, &teams_dir, &runtime, Utc::now());
}

pub fn process_codex_compaction_events_at(sessions: &[RuntimeSession], _teams_dir: &Path) {
    let mut active_paths = HashSet::new();
    let mut processed_paths = HashSet::new();

    for session in sessions
        .iter()
        .filter(|session| session.cli_tool == CliTool::Codex)
    {
        let Some(session_id) = session.session_id.as_deref() else {
            continue;
        };
        let Some(jsonl_path) = session.jsonl_path.as_deref() else {
            continue;
        };

        let path = PathBuf::from(jsonl_path);
        active_paths.insert(path.clone());
        if !processed_paths.insert(path.clone()) {
            continue;
        }

        let observed_jsonl_len = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                emit_compaction_extractor_failed(CompactionExtractorFailedEvent {
                    tool: CliTool::Codex,
                    jsonl_path: path.display().to_string(),
                    stage: "stat".to_string(),
                    error_message: error.to_string(),
                });
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "failed to stat Codex JSONL while processing compaction events"
                );
                continue;
            }
        };

        let Some(read_start) = track_read_start(&path) else {
            continue;
        };

        let (appended_lines, committed_offset) = match read_appended_lines(&path, read_start) {
            Ok(result) => result,
            Err(error) => {
                emit_compaction_extractor_failed(CompactionExtractorFailedEvent {
                    tool: CliTool::Codex,
                    jsonl_path: path.display().to_string(),
                    stage: "read_appended_lines".to_string(),
                    error_message: error.to_string(),
                });
                tracing::warn!(path = %path.display(), error = %error, "failed to read appended Codex JSONL lines");
                continue;
            }
        };
        set_tracked_offset(&path, committed_offset);

        let events = detect_compaction_events(&appended_lines, session_id);
        if events.is_empty() {
            continue;
        }

        for event in events {
            let Some(pane_id) = session.tmux_pane.clone() else {
                continue;
            };

            let signal = CompactionSignalRecord {
                version: 1,
                signal_id: Uuid::new_v4().to_string(),
                emitted_at: Utc::now(),
                tool: CliTool::Codex,
                session_id: event.session_id,
                pane_id,
                project_path: session.project_path.clone(),
                jsonl_path: path.display().to_string(),
                jsonl_offset: observed_jsonl_len,
                transcript_timestamp: event.timestamp,
                signal_kind: store_signal_kind(event.kind),
            };
            emit_compaction_signal_emitted(signal_event(
                signal.tool,
                Some(&signal.session_id),
                Some(&signal.pane_id),
                Some(&signal.project_path),
                Some(Path::new(&signal.jsonl_path)),
                Some(signal.transcript_timestamp),
                Some(event_signal_kind(signal.signal_kind)),
            ));
            enqueue_pending(PendingCodexCompactionSignal { signal });
        }
    }

    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.offsets.retain(|path, _| active_paths.contains(path));
}

pub fn drain_pending_codex_compaction_reinjections() -> Vec<PendingCodexCompactionSignal> {
    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.pending.drain(..).collect()
}

fn track_read_start(path: &Path) -> Option<u64> {
    let file_len = std::fs::metadata(path).ok()?.len();
    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    match guard.offsets.get_mut(path) {
        Some(offset) if *offset > file_len => {
            *offset = file_len;
            None
        }
        Some(offset) if *offset == file_len => None,
        Some(offset) => Some(*offset),
        None => {
            guard.offsets.insert(path.to_path_buf(), file_len);
            None
        }
    }
}

fn set_tracked_offset(path: &Path, offset: u64) {
    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.offsets.insert(path.to_path_buf(), offset);
}

fn read_appended_lines(path: &Path, start: u64) -> std::io::Result<(Vec<String>, u64)> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start))?;

    let mut lines = Vec::new();
    let mut line = String::new();
    let mut committed_offset = start;
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        if !line.ends_with('\n') {
            break;
        }
        committed_offset += bytes_read as u64;
        while matches!(line.chars().last(), Some('\n' | '\r')) {
            line.pop();
        }
        if !line.is_empty() {
            lines.push(line.clone());
        }
    }
    Ok((lines, committed_offset))
}

fn detect_compaction_events(lines: &[String], session_id: &str) -> Vec<CodexCompactionEvent> {
    let mut events = Vec::new();

    for line in lines {
        let Some(candidate) = parse_codex_compaction_record(line, session_id) else {
            continue;
        };

        let skip_paired_context = matches!(candidate.kind, CompactionSignalKind::ContextCompacted)
            && events
                .last()
                .is_some_and(|previous: &CodexCompactionEvent| {
                    previous.kind == CompactionSignalKind::Compacted
                        && previous.session_id == candidate.session_id
                        && candidate
                            .timestamp
                            .signed_duration_since(previous.timestamp)
                            .num_milliseconds()
                            .abs()
                            <= 2_000
                });

        if !skip_paired_context {
            events.push(candidate);
        }
    }

    events
}

fn parse_codex_compaction_record(line: &str, session_id: &str) -> Option<CodexCompactionEvent> {
    let parsed: Value = serde_json::from_str(line).ok()?;
    let timestamp = parsed
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))?;

    let kind = match parsed.get("type").and_then(Value::as_str) {
        Some("compacted") => CompactionSignalKind::Compacted,
        Some("event_msg")
            if parsed
                .get("payload")
                .and_then(|payload| payload.get("type"))
                .and_then(Value::as_str)
                == Some("context_compacted") =>
        {
            CompactionSignalKind::ContextCompacted
        }
        _ => return None,
    };

    Some(CodexCompactionEvent {
        session_id: session_id.to_string(),
        timestamp,
        kind,
    })
}

fn deliver_pending_codex_compaction_reinjections_at(
    _sessions: &[RuntimeSession],
    teams_dir: &Path,
    runtime: &dyn CoordinationRuntime,
    now: DateTime<Utc>,
) {
    for pending in drain_pending_codex_compaction_reinjections() {
        if let CompactionSignalProcessOutcome::Failed {
            team_name,
            member_name,
            error_message,
        } =
            CompactionSignalProcessor::process_signal_at(&pending.signal, teams_dir, runtime, now)
        {
            tracing::warn!(
                team_name = team_name,
                member_name = member_name,
                pane_id = pending.signal.pane_id,
                session_id = pending.signal.session_id,
                error = error_message,
                "failed to process Codex compaction signal"
            );
        }
    }
}

fn store_signal_kind(kind: CompactionSignalKind) -> StoreSignalKind {
    match kind {
        CompactionSignalKind::Compacted => StoreSignalKind::Compacted,
        CompactionSignalKind::ContextCompacted => StoreSignalKind::ContextCompacted,
    }
}

fn event_signal_kind(
    kind: StoreSignalKind,
) -> crate::coordination::compaction_events::CompactionSignalKind {
    match kind {
        StoreSignalKind::Compacted => {
            crate::coordination::compaction_events::CompactionSignalKind::Compacted
        }
        StoreSignalKind::ContextCompacted => {
            crate::coordination::compaction_events::CompactionSignalKind::ContextCompacted
        }
    }
}

fn enqueue_pending(event: PendingCodexCompactionSignal) {
    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.pending.push_back(event);
}

#[cfg(test)]
fn reset_test_state() {
    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.offsets.clear();
    guard.pending.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::stores::{
        CompactionDeliveryResult, MemberCompactionState, MemberCompactionStore,
        MemberRuntimeRecord, MemberRuntimeStore, OperationalAssignmentFooterSnapshot,
        OperationalContextSnapshot, OperationalContextSnapshotStore, OperationalOwnershipSnapshot,
        OperationalTaskSnapshot, OperationalWorkingSetSnapshot, TeamConfig, TeamConfigStore,
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
                id: "678".to_string(),
                subject: "Implement Codex compaction watcher".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec![
                    "src-tauri/src/session_scanner/compaction.rs".to_string()
                ],
                adjacent_fix_policy: "local validation only".to_string(),
                validation_expectation: "cargo check --tests".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: project_path.to_string(),
                focal_files: vec!["src-tauri/src/session_scanner/compaction.rs".to_string()],
            },
        }
    }

    fn save_team_fixture(
        teams_dir: &Path,
        team_name: &str,
        member: &Member,
        runtime_session_id: Option<&str>,
        runtime_pane_id: Option<&str>,
    ) {
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
            pane_id: runtime_pane_id.map(ToOwned::to_owned),
            session_id: runtime_session_id.map(ToOwned::to_owned),
            daemon_pid: Some(42),
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

    fn write_jsonl(path: &Path, lines: &[&str]) {
        let body = lines.join("\n");
        std::fs::write(path, format!("{body}\n")).expect("write jsonl");
    }

    fn append_jsonl(path: &Path, lines: &[&str]) {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open jsonl for append");
        for line in lines {
            writeln!(file, "{line}").expect("append jsonl line");
        }
        file.flush().expect("flush appended jsonl");
    }

    fn append_raw(path: &Path, chunk: &str) {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open jsonl for raw append");
        file.write_all(chunk.as_bytes())
            .expect("append raw jsonl chunk");
        file.flush().expect("flush raw jsonl chunk");
    }

    fn sample_session(
        project_path: &str,
        jsonl_path: &Path,
        session_id: &str,
        tmux_pane: &str,
    ) -> RuntimeSession {
        RuntimeSession {
            pid: 1234,
            project_path: project_path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex resume --last".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("main".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some(tmux_pane.to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: super::super::SessionState::Idle,
            session_id: Some(session_id.to_string()),
            jsonl_path: Some(jsonl_path.display().to_string()),
            recent_io: false,
            last_output_age_secs: Some(0),
            activity_confidence: super::super::ActivityConfidence::Low,
            activity_attribution: super::super::ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: super::super::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn parse_codex_compaction_record_detects_compacted_line() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let line = r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#;
        let parsed = parse_codex_compaction_record(line, "session-1").expect("compaction line");

        assert_eq!(parsed.session_id, "session-1");
        assert_eq!(parsed.timestamp, timestamp("2026-03-08T13:46:41.037Z"));
        assert_eq!(parsed.kind, CompactionSignalKind::Compacted);
    }

    #[test]
    fn parse_codex_compaction_record_detects_context_compacted_line() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let line = r#"{"timestamp":"2026-03-08T13:46:41.038Z","type":"event_msg","payload":{"type":"context_compacted"}}"#;
        let parsed =
            parse_codex_compaction_record(line, "session-2").expect("context compacted line");

        assert_eq!(parsed.kind, CompactionSignalKind::ContextCompacted);
        assert_eq!(parsed.timestamp, timestamp("2026-03-08T13:46:41.038Z"));
    }

    #[test]
    fn parse_codex_compaction_record_ignores_non_compaction_lines() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let line = r#"{"timestamp":"2026-03-08T13:46:40.000Z","type":"event_msg","payload":{"type":"token_count"}}"#;
        assert!(parse_codex_compaction_record(line, "session-3").is_none());
    }

    #[test]
    fn parse_codex_compaction_record_ignores_invalid_timestamp() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let line =
            r#"{"timestamp":"not-a-time","type":"compacted","payload":{"replacement_history":[]}}"#;
        assert!(parse_codex_compaction_record(line, "session-3").is_none());
    }

    #[test]
    fn detect_compaction_events_ignores_noise_and_collapses_paired_context_event() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let lines = vec![
            r#"{"timestamp":"2026-03-08T13:46:40.000Z","type":"event_msg","payload":{"type":"token_count"}}"#.to_string(),
            r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#.to_string(),
            r#"{"timestamp":"2026-03-08T13:46:41.038Z","type":"event_msg","payload":{"type":"context_compacted"}}"#.to_string(),
            r#"{"timestamp":"2026-03-08T13:46:42.000Z","type":"agent_message_delta","payload":{"delta":"done"}}"#.to_string(),
        ];

        let events = detect_compaction_events(&lines, "session-1");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, CompactionSignalKind::Compacted);
        assert_eq!(events[0].timestamp, timestamp("2026-03-08T13:46:41.037Z"));
    }

    #[test]
    fn detect_compaction_events_keeps_context_event_when_not_a_close_pair() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let lines = vec![
            r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#.to_string(),
            r#"{"timestamp":"2026-03-08T13:46:44.500Z","type":"event_msg","payload":{"type":"context_compacted"}}"#.to_string(),
        ];

        let events = detect_compaction_events(&lines, "session-1");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, CompactionSignalKind::Compacted);
        assert_eq!(events[1].kind, CompactionSignalKind::ContextCompacted);
    }

    #[test]
    fn first_observation_baselines_eof_without_replaying_history() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        process_codex_compaction_events_at(&[session], &teams_dir);

        assert!(drain_pending_codex_compaction_reinjections().is_empty());
    }

    #[test]
    fn appended_compaction_enqueues_single_pending_reinjection() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);

        append_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
                r#"{"timestamp":"2026-03-08T13:46:41.038Z","type":"event_msg","payload":{"type":"context_compacted"}}"#,
            ],
        );

        process_codex_compaction_events_at(&[session], &teams_dir);
        let pending = drain_pending_codex_compaction_reinjections();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].signal.pane_id, "%7");
        assert_eq!(pending[0].signal.session_id, "session-1");
        assert_eq!(pending[0].signal.project_path, project_path);
        assert_eq!(
            pending[0].signal.transcript_timestamp,
            timestamp("2026-03-08T13:46:41.037Z")
        );
    }

    #[test]
    fn display_scan_unattributed_codex_sessions_still_drive_compaction_detection() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member_a = sample_member("developer2", project_path);
        let member_b = sample_member("developer3", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member_a,
            Some("session-1"),
            Some("%7"),
        );
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member_b,
            Some("session-2"),
            Some("%8"),
        );

        let jsonl_path_a = tmp.path().join("session-a.jsonl");
        let jsonl_path_b = tmp.path().join("session-b.jsonl");
        write_jsonl(
            &jsonl_path_a,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );
        write_jsonl(
            &jsonl_path_b,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let processes = vec![
            crate::session_scanner::process::ProcessInfo {
                pid: 910_001,
                project_path: project_path.to_string(),
                tty: "/dev/pts/21".to_string(),
                args: "codex".to_string(),
                cli_tool: CliTool::Codex,
            },
            crate::session_scanner::process::ProcessInfo {
                pid: 910_002,
                project_path: project_path.to_string(),
                tty: "/dev/pts/22".to_string(),
                args: "codex".to_string(),
                cli_tool: CliTool::Codex,
            },
        ];
        let pane_map = HashMap::from([
            (
                "/dev/pts/21".to_string(),
                crate::session_scanner::tmux::TmuxPane {
                    pane_id: "%7".to_string(),
                    tty: "/dev/pts/21".to_string(),
                    window_index: "1".to_string(),
                    window_name: "mesh-a".to_string(),
                    session_name: "0".to_string(),
                },
            ),
            (
                "/dev/pts/22".to_string(),
                crate::session_scanner::tmux::TmuxPane {
                    pane_id: "%8".to_string(),
                    tty: "/dev/pts/22".to_string(),
                    window_index: "2".to_string(),
                    window_name: "mesh-b".to_string(),
                    session_name: "0".to_string(),
                },
            ),
        ]);
        let sessions_per_project_tool =
            HashMap::from([((project_path.to_string(), CliTool::Codex), 2usize)]);

        let idle_detector = |proc: &crate::session_scanner::process::ProcessInfo| {
            if proc.pid == 910_001 {
                crate::session_scanner::idle::IdleResult {
                    state: crate::session_scanner::SessionState::Active,
                    session_id: Some("session-1".to_string()),
                    jsonl_path: Some(jsonl_path_a.display().to_string()),
                    last_output_age_secs: Some(0),
                }
            } else {
                crate::session_scanner::idle::IdleResult {
                    state: crate::session_scanner::SessionState::Active,
                    session_id: Some("session-2".to_string()),
                    jsonl_path: Some(jsonl_path_b.display().to_string()),
                    last_output_age_secs: Some(0),
                }
            }
        };

        let (sessions, ..) = super::super::classify_display_runtime_sessions_with(
            processes.clone(),
            pane_map.clone(),
            &sessions_per_project_tool,
            &idle_detector,
        );
        assert_eq!(sessions.len(), 2);
        assert!(sessions
            .iter()
            .all(|session| session.project_unattributed_active));
        assert!(sessions.iter().all(|session| session.session_id.is_some()));
        assert!(sessions.iter().all(|session| session.jsonl_path.is_some()));

        process_codex_compaction_events_at(&sessions, &teams_dir);
        append_jsonl(
            &jsonl_path_b,
            &[
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );

        let (sessions, ..) = super::super::classify_display_runtime_sessions_with(
            processes,
            pane_map,
            &sessions_per_project_tool,
            &idle_detector,
        );
        process_codex_compaction_events_at(&sessions, &teams_dir);

        let pending = drain_pending_codex_compaction_reinjections();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].signal.pane_id, "%8");
        assert_eq!(pending[0].signal.session_id, "session-2");
    }

    #[test]
    fn partial_trailing_line_is_re_read_on_next_poll() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
        );
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);
        assert!(drain_pending_codex_compaction_reinjections().is_empty());

        append_raw(&jsonl_path, "\n");
        process_codex_compaction_events_at(&[session], &teams_dir);

        let pending = drain_pending_codex_compaction_reinjections();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].signal.transcript_timestamp,
            timestamp("2026-03-08T13:46:41.037Z")
        );
    }

    #[test]
    fn already_handled_compaction_is_not_requeued() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );

        let compaction_timestamp = timestamp("2026-03-08T13:46:41.037Z");
        MemberCompactionStore::save(
            &teams_dir,
            "taurhaus-team",
            "developer2",
            &MemberCompactionState {
                version: 1,
                member_name: "developer2".to_string(),
                last_session_id: "session-1".to_string(),
                last_compaction_timestamp: compaction_timestamp,
                last_delivery_result: CompactionDeliveryResult::Injected,
            },
        )
        .expect("save compaction state");

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);
        append_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );
        process_codex_compaction_events_at(&[session], &teams_dir);

        assert!(drain_pending_codex_compaction_reinjections().is_empty());
    }

    #[test]
    fn new_session_with_same_timestamp_is_requeued() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-2"),
            Some("%7"),
        );

        let duplicate_timestamp = timestamp("2026-03-08T13:46:41.037Z");
        MemberCompactionStore::save(
            &teams_dir,
            "taurhaus-team",
            "developer2",
            &MemberCompactionState {
                version: 1,
                member_name: "developer2".to_string(),
                last_session_id: "session-1".to_string(),
                last_compaction_timestamp: duplicate_timestamp,
                last_delivery_result: CompactionDeliveryResult::Injected,
            },
        )
        .expect("save compaction state");

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-2", "%7");
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);
        append_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );
        process_codex_compaction_events_at(&[session], &teams_dir);

        let pending = drain_pending_codex_compaction_reinjections();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].signal.session_id, "session-2");
    }

    #[test]
    fn same_session_with_new_timestamp_is_requeued() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );

        MemberCompactionStore::save(
            &teams_dir,
            "taurhaus-team",
            "developer2",
            &MemberCompactionState {
                version: 1,
                member_name: "developer2".to_string(),
                last_session_id: "session-1".to_string(),
                last_compaction_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
                last_delivery_result: CompactionDeliveryResult::Injected,
            },
        )
        .expect("save compaction state");

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        process_codex_compaction_events_at(std::slice::from_ref(&session), &teams_dir);
        append_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:43.250Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );
        process_codex_compaction_events_at(&[session], &teams_dir);

        let pending = drain_pending_codex_compaction_reinjections();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].signal.transcript_timestamp,
            timestamp("2026-03-08T13:46:43.250Z")
        );
    }
}
