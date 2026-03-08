//! Codex post-compaction detection integrated into the session scanner poll loop.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{cli_tool::CliTool, ClaudeSession};
use crate::coordination::domain::Member;
use crate::coordination::reinjection::{CompactionReinjectionService, OperationalReinjectionCard};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{
    emit_compaction_delivery_event, emit_compaction_detected_event, is_stale_compaction,
    CompactionDeliveryResult, MemberCompactionState, MemberCompactionStore, MemberRuntimeStore,
    OperationalContextSnapshot, OperationalContextSnapshotStore, TeamConfigStore,
};
use crate::provider::path::normalize_project_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCodexCompactionReinjection {
    pub team_name: String,
    pub member_name: String,
    pub pane_id: String,
    pub session_id: String,
    pub compaction_timestamp: DateTime<Utc>,
    pub card: OperationalReinjectionCard,
    pub rendered_text: String,
}

#[derive(Debug, Default)]
struct CompactionWatcherState {
    offsets: HashMap<PathBuf, u64>,
    pending: VecDeque<PendingCodexCompactionReinjection>,
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

#[derive(Debug, Clone)]
struct ResolvedManagedCodexSession {
    team_name: String,
    member_name: String,
    pane_id: String,
    member: Member,
    snapshot: OperationalContextSnapshot,
}

static WATCHER_STATE: OnceLock<Mutex<CompactionWatcherState>> = OnceLock::new();

fn watcher_state() -> &'static Mutex<CompactionWatcherState> {
    WATCHER_STATE.get_or_init(|| Mutex::new(CompactionWatcherState::default()))
}

pub fn process_codex_compaction_events(sessions: &[ClaudeSession]) {
    let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
    process_codex_compaction_events_at(sessions, &teams_dir);
    let runtime = SystemCoordinationRuntime;
    deliver_pending_codex_compaction_reinjections_at(sessions, &teams_dir, &runtime, Utc::now());
}

pub fn process_codex_compaction_events_at(sessions: &[ClaudeSession], teams_dir: &Path) {
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

        let Some(read_start) = track_read_start(&path) else {
            continue;
        };

        let appended_lines = match read_appended_lines(&path, read_start) {
            Ok(lines) => lines,
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "failed to read appended Codex JSONL lines");
                continue;
            }
        };

        let events = detect_compaction_events(&appended_lines, session_id);
        if events.is_empty() {
            continue;
        }

        let Some(resolved) = resolve_managed_codex_session(teams_dir, session) else {
            tracing::debug!(
                project_path = %session.project_path,
                session_id = session_id,
                tmux_pane = session.tmux_pane.as_deref().unwrap_or(""),
                "skipping Codex compaction event because no managed member resolution was available"
            );
            continue;
        };

        for event in events {
            if already_handled(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &event.session_id,
                event.timestamp,
            ) {
                continue;
            }

            emit_compaction_detected_event(
                &resolved.team_name,
                &resolved.member_name,
                CliTool::Codex,
                &event.session_id,
                event.timestamp,
            );

            let card = CompactionReinjectionService::compose(&resolved.member, &resolved.snapshot);
            let rendered_text = CompactionReinjectionService::render_codex_tmux_text(&card);

            enqueue_pending(PendingCodexCompactionReinjection {
                team_name: resolved.team_name.clone(),
                member_name: resolved.member_name.clone(),
                pane_id: resolved.pane_id.clone(),
                session_id: event.session_id,
                compaction_timestamp: event.timestamp,
                card,
                rendered_text,
            });
        }
    }

    let mut guard = watcher_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    guard.offsets.retain(|path, _| active_paths.contains(path));
}

pub fn drain_pending_codex_compaction_reinjections() -> Vec<PendingCodexCompactionReinjection> {
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
        Some(offset) => {
            let start = *offset;
            *offset = file_len;
            Some(start)
        }
        None => {
            guard.offsets.insert(path.to_path_buf(), file_len);
            None
        }
    }
}

fn read_appended_lines(path: &Path, start: u64) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start))?;

    let mut lines = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        while matches!(line.chars().last(), Some('\n' | '\r')) {
            line.pop();
        }
        if !line.is_empty() {
            lines.push(line.clone());
        }
    }
    Ok(lines)
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

fn resolve_managed_codex_session(
    teams_dir: &Path,
    session: &ClaudeSession,
) -> Option<ResolvedManagedCodexSession> {
    let session_id = session.session_id.as_deref()?;
    let normalized_project = normalize_project_path(&session.project_path);
    let scanner_pane = session.tmux_pane.as_deref();

    let team_names = TeamConfigStore::list(teams_dir).ok()?;
    let mut best_match: Option<ResolvedManagedCodexSession> = None;
    let mut best_score = 0u8;
    let mut ambiguous = false;

    for team_name in team_names {
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team config while resolving Codex compaction");
                continue;
            }
        };
        let runtime_by_member = match MemberRuntimeStore::load_all(teams_dir, &team_name) {
            Ok(records) => records.into_iter().collect::<HashMap<_, _>>(),
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team runtime while resolving Codex compaction");
                continue;
            }
        };

        for member in config.members {
            if member.cli_tool != CliTool::Codex {
                continue;
            }
            if normalize_project_path(&member.project_path.display().to_string())
                != normalized_project
            {
                continue;
            }

            let runtime = runtime_by_member.get(&member.name);
            let runtime_session = runtime.and_then(|record| record.session_id.as_deref());
            let runtime_pane = runtime.and_then(|record| record.pane_id.as_deref());

            let score = if runtime_session == Some(session_id) {
                3
            } else if runtime_pane.is_some() && runtime_pane == scanner_pane {
                2
            } else {
                0
            };
            if score == 0 {
                continue;
            }

            let pane_id = runtime_pane
                .map(ToOwned::to_owned)
                .or_else(|| session.tmux_pane.clone());
            let Some(pane_id) = pane_id else {
                continue;
            };

            let snapshot =
                match OperationalContextSnapshotStore::load(teams_dir, &team_name, &member.name) {
                    Ok(Some(snapshot)) => snapshot,
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::warn!(
                            team_name = team_name,
                            member_name = member.name,
                            error = %error,
                            "failed to load operational snapshot while resolving Codex compaction"
                        );
                        continue;
                    }
                };

            let resolved = ResolvedManagedCodexSession {
                team_name: team_name.clone(),
                member_name: member.name.clone(),
                pane_id,
                member,
                snapshot,
            };

            if score > best_score {
                best_score = score;
                best_match = Some(resolved);
                ambiguous = false;
            } else if score == best_score {
                ambiguous = true;
            }
        }
    }

    if ambiguous {
        None
    } else {
        best_match
    }
}

fn already_handled(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
) -> bool {
    match MemberCompactionStore::load(teams_dir, team_name, member_name) {
        Ok(Some(state)) => {
            state.last_session_id == session_id
                && state.last_compaction_timestamp == compaction_timestamp
        }
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(
                team_name = team_name,
                member_name = member_name,
                error = %error,
                "failed to load compaction state while resolving idempotency"
            );
            false
        }
    }
}

fn deliver_pending_codex_compaction_reinjections_at(
    sessions: &[ClaudeSession],
    teams_dir: &Path,
    runtime: &dyn CoordinationRuntime,
    now: DateTime<Utc>,
) {
    for pending in drain_pending_codex_compaction_reinjections() {
        if let Err(error) = deliver_pending_codex_compaction_reinjection_at(
            sessions, teams_dir, runtime, now, &pending,
        ) {
            tracing::warn!(
                team_name = pending.team_name,
                member_name = pending.member_name,
                pane_id = pending.pane_id,
                session_id = pending.session_id,
                error = %error,
                "failed to deliver Codex post-compaction tmux injection"
            );
            let _ = record_delivery_at(
                teams_dir,
                &pending.team_name,
                &pending.member_name,
                &pending.session_id,
                pending.compaction_timestamp,
                CompactionDeliveryResult::Failed,
            );
        }
    }
}

fn deliver_pending_codex_compaction_reinjection_at(
    sessions: &[ClaudeSession],
    teams_dir: &Path,
    runtime: &dyn CoordinationRuntime,
    now: DateTime<Utc>,
    pending: &PendingCodexCompactionReinjection,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    if is_stale_compaction(pending.compaction_timestamp, now) {
        return record_delivery_at(
            teams_dir,
            &pending.team_name,
            &pending.member_name,
            &pending.session_id,
            pending.compaction_timestamp,
            CompactionDeliveryResult::Stale,
        );
    }

    if already_handled(
        teams_dir,
        &pending.team_name,
        &pending.member_name,
        &pending.session_id,
        pending.compaction_timestamp,
    ) {
        return record_delivery_at(
            teams_dir,
            &pending.team_name,
            &pending.member_name,
            &pending.session_id,
            pending.compaction_timestamp,
            CompactionDeliveryResult::Skipped,
        );
    }

    if !member_is_still_attached(teams_dir, pending)?
        || !session_still_matches_pending(sessions, pending)
        || !pane_is_live_codex(runtime, &pending.pane_id)?
    {
        return record_delivery_at(
            teams_dir,
            &pending.team_name,
            &pending.member_name,
            &pending.session_id,
            pending.compaction_timestamp,
            CompactionDeliveryResult::Skipped,
        );
    }

    runtime.send_tmux_keys_with_enter(&pending.pane_id, &pending.rendered_text)?;
    record_delivery_at(
        teams_dir,
        &pending.team_name,
        &pending.member_name,
        &pending.session_id,
        pending.compaction_timestamp,
        CompactionDeliveryResult::Injected,
    )
}

fn member_is_still_attached(
    teams_dir: &Path,
    pending: &PendingCodexCompactionReinjection,
) -> Result<bool, crate::coordination::errors::CoordinationError> {
    let config = match TeamConfigStore::load(teams_dir, &pending.team_name) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(
                team_name = pending.team_name,
                member_name = pending.member_name,
                error = %error,
                "failed to load team config while validating Codex compaction delivery"
            );
            return Ok(false);
        }
    };

    let Some(member) = config
        .members
        .iter()
        .find(|member| member.name == pending.member_name)
    else {
        return Ok(false);
    };

    if member.cli_tool != CliTool::Codex {
        return Ok(false);
    }

    let runtime = MemberRuntimeStore::load(teams_dir, &pending.team_name, &pending.member_name)?;

    Ok(runtime.pane_id.as_deref() == Some(pending.pane_id.as_str())
        && runtime.session_id.as_deref() == Some(pending.session_id.as_str()))
}

fn session_still_matches_pending(
    sessions: &[ClaudeSession],
    pending: &PendingCodexCompactionReinjection,
) -> bool {
    sessions.iter().any(|session| {
        session.cli_tool == CliTool::Codex
            && session.tmux_pane.as_deref() == Some(pending.pane_id.as_str())
            && session.session_id.as_deref() == Some(pending.session_id.as_str())
    })
}

fn pane_is_live_codex(
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
) -> Result<bool, crate::coordination::errors::CoordinationError> {
    if !runtime.pane_exists(pane_id)? || runtime.pane_is_dead(pane_id)? {
        return Ok(false);
    }

    let command = runtime.pane_current_command(pane_id)?;
    Ok(command
        .as_deref()
        .is_some_and(foreground_command_matches_codex))
}

fn foreground_command_matches_codex(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let first = normalized.split_whitespace().next().unwrap_or_default();
    let first = first.rsplit('/').next().unwrap_or(first);

    first == "codex" || first.ends_with("codex")
}

fn record_delivery_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    MemberCompactionStore::save(
        teams_dir,
        team_name,
        member_name,
        &MemberCompactionState {
            version: 1,
            member_name: member_name.to_string(),
            last_session_id: session_id.to_string(),
            last_compaction_timestamp: compaction_timestamp,
            last_delivery_result: result,
        },
    )?;
    emit_compaction_delivery_event(
        team_name,
        member_name,
        CliTool::Codex,
        session_id,
        compaction_timestamp,
        result,
    );
    Ok(())
}

fn enqueue_pending(event: PendingCodexCompactionReinjection) {
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

    use chrono::TimeZone;
    use tempfile::TempDir;

    use crate::coordination::domain::{HealthState, MemberRole};
    use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
    use crate::coordination::stores::{
        MemberRuntimeRecord, OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot,
        OperationalTaskSnapshot, OperationalWorkingSetSnapshot, TeamConfig,
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
            schema_version: 1,
            member_name: member.name.clone(),
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

    fn sample_session(
        project_path: &str,
        jsonl_path: &Path,
        session_id: &str,
        tmux_pane: &str,
    ) -> ClaudeSession {
        ClaudeSession {
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
        process_codex_compaction_events_at(&[session.clone()], &teams_dir);

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
        assert_eq!(pending[0].team_name, "taurhaus-team");
        assert_eq!(pending[0].member_name, "developer2");
        assert_eq!(pending[0].pane_id, "%7");
        assert_eq!(pending[0].session_id, "session-1");
        assert_eq!(
            pending[0].compaction_timestamp,
            timestamp("2026-03-08T13:46:41.037Z")
        );
        assert!(pending[0]
            .rendered_text
            .contains("[taurhaus] post_compaction_context"));
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
        process_codex_compaction_events_at(&[session.clone()], &teams_dir);
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
        process_codex_compaction_events_at(&[session.clone()], &teams_dir);
        append_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}"#,
            ],
        );
        process_codex_compaction_events_at(&[session], &teams_dir);

        let pending = drain_pending_codex_compaction_reinjections();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].session_id, "session-2");
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
        process_codex_compaction_events_at(&[session.clone()], &teams_dir);
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
            pending[0].compaction_timestamp,
            timestamp("2026-03-08T13:46:43.250Z")
        );
    }

    #[test]
    fn pending_reinjection_injects_into_live_codex_pane_and_records_delivery() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let team_name = "taurhaus-team";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            team_name,
            &member,
            Some("session-1"),
            Some("%7"),
        );

        let pending = PendingCodexCompactionReinjection {
            team_name: team_name.to_string(),
            member_name: member.name.clone(),
            pane_id: "%7".to_string(),
            session_id: "session-1".to_string(),
            compaction_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
            card: CompactionReinjectionService::compose(
                &member,
                &sample_snapshot(team_name, &member.name, project_path),
            ),
            rendered_text: "[taurhaus] post_compaction_context\nTask: #679".to_string(),
        };
        enqueue_pending(pending.clone());

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex --resume"));

        let session = sample_session(
            project_path,
            &tmp.path().join("session.jsonl"),
            "session-1",
            "%7",
        );
        deliver_pending_codex_compaction_reinjections_at(
            &[session],
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:43.000Z"),
        );

        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SendKeys { pane_id, keys }
                if pane_id == "%7" && keys == &pending.rendered_text
        )));

        let stored = MemberCompactionStore::load(&teams_dir, team_name, &member.name)
            .expect("load state")
            .expect("saved state");
        assert_eq!(stored.last_session_id, "session-1");
        assert_eq!(
            stored.last_delivery_result,
            CompactionDeliveryResult::Injected
        );
    }

    #[test]
    fn pending_reinjection_marks_stale_when_delivery_is_too_old() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let team_name = "taurhaus-team";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            team_name,
            &member,
            Some("session-1"),
            Some("%7"),
        );

        enqueue_pending(PendingCodexCompactionReinjection {
            team_name: team_name.to_string(),
            member_name: member.name.clone(),
            pane_id: "%7".to_string(),
            session_id: "session-1".to_string(),
            compaction_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
            card: CompactionReinjectionService::compose(
                &member,
                &sample_snapshot(team_name, &member.name, project_path),
            ),
            rendered_text: "ignored".to_string(),
        });

        let runtime = RecordingCoordinationRuntime::default();
        let session = sample_session(
            project_path,
            &tmp.path().join("session.jsonl"),
            "session-1",
            "%7",
        );
        deliver_pending_codex_compaction_reinjections_at(
            &[session],
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:57.000Z"),
        );

        assert!(!runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::SendKeys { .. })));

        let stored = MemberCompactionStore::load(&teams_dir, team_name, &member.name)
            .expect("load state")
            .expect("saved state");
        assert_eq!(stored.last_delivery_result, CompactionDeliveryResult::Stale);
    }

    #[test]
    fn pending_reinjection_skips_when_pane_is_dead() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let team_name = "taurhaus-team";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            team_name,
            &member,
            Some("session-1"),
            Some("%7"),
        );

        enqueue_pending(PendingCodexCompactionReinjection {
            team_name: team_name.to_string(),
            member_name: member.name.clone(),
            pane_id: "%7".to_string(),
            session_id: "session-1".to_string(),
            compaction_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
            card: CompactionReinjectionService::compose(
                &member,
                &sample_snapshot(team_name, &member.name, project_path),
            ),
            rendered_text: "ignored".to_string(),
        });

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", true);
        runtime.set_pane_current_command("%7", Some("codex"));

        let session = sample_session(
            project_path,
            &tmp.path().join("session.jsonl"),
            "session-1",
            "%7",
        );
        deliver_pending_codex_compaction_reinjections_at(
            &[session],
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:43.000Z"),
        );

        assert!(!runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::SendKeys { .. })));

        let stored = MemberCompactionStore::load(&teams_dir, team_name, &member.name)
            .expect("load state")
            .expect("saved state");
        assert_eq!(
            stored.last_delivery_result,
            CompactionDeliveryResult::Skipped
        );
    }

    #[test]
    fn pending_reinjection_skips_when_live_session_has_moved_past_compaction_boundary() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let team_name = "taurhaus-team";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            team_name,
            &member,
            Some("session-2"),
            Some("%7"),
        );

        enqueue_pending(PendingCodexCompactionReinjection {
            team_name: team_name.to_string(),
            member_name: member.name.clone(),
            pane_id: "%7".to_string(),
            session_id: "session-1".to_string(),
            compaction_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
            card: CompactionReinjectionService::compose(
                &member,
                &sample_snapshot(team_name, &member.name, project_path),
            ),
            rendered_text: "ignored".to_string(),
        });

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex --resume"));

        let session = sample_session(
            project_path,
            &tmp.path().join("session.jsonl"),
            "session-2",
            "%7",
        );
        deliver_pending_codex_compaction_reinjections_at(
            &[session],
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:43.000Z"),
        );

        assert!(!runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::SendKeys { .. })));

        let stored = MemberCompactionStore::load(&teams_dir, team_name, &member.name)
            .expect("load state")
            .expect("saved state");
        assert_eq!(
            stored.last_delivery_result,
            CompactionDeliveryResult::Skipped
        );
    }

    #[test]
    fn resolve_managed_codex_session_prefers_exact_session_match_over_pane_match() {
        let _guard = TEST_LOCK.lock().expect("lock");
        reset_test_state();

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let member_by_pane = sample_member("pane-match", project_path);
        let member_by_session = sample_member("session-match", project_path);

        let config = TeamConfig {
            schema_version: 1,
            name: "taurhaus-team".to_string(),
            description: None,
            created_at: Utc
                .with_ymd_and_hms(2026, 3, 8, 14, 0, 0)
                .single()
                .expect("datetime"),
            members: vec![member_by_pane.clone(), member_by_session.clone()],
        };
        TeamConfigStore::save(&teams_dir, "taurhaus-team", &config).expect("save team config");

        MemberRuntimeStore::save(
            &teams_dir,
            "taurhaus-team",
            &member_by_pane.name,
            &MemberRuntimeRecord {
                schema_version: 1,
                member_name: member_by_pane.name.clone(),
                pane_id: Some("%7".to_string()),
                session_id: Some("other-session".to_string()),
                daemon_pid: None,
                health: HealthState::Healthy,
                delivery_lease: None,
                attached_at: None,
                last_seen_at: None,
            },
        )
        .expect("save pane runtime");
        MemberRuntimeStore::save(
            &teams_dir,
            "taurhaus-team",
            &member_by_session.name,
            &MemberRuntimeRecord {
                schema_version: 1,
                member_name: member_by_session.name.clone(),
                pane_id: Some("%9".to_string()),
                session_id: Some("session-1".to_string()),
                daemon_pid: None,
                health: HealthState::Healthy,
                delivery_lease: None,
                attached_at: None,
                last_seen_at: None,
            },
        )
        .expect("save session runtime");

        OperationalContextSnapshotStore::save(
            &teams_dir,
            &sample_snapshot("taurhaus-team", &member_by_pane.name, project_path),
        )
        .expect("save pane snapshot");
        OperationalContextSnapshotStore::save(
            &teams_dir,
            &sample_snapshot("taurhaus-team", &member_by_session.name, project_path),
        )
        .expect("save session snapshot");

        let jsonl_path = tmp.path().join("session.jsonl");
        write_jsonl(
            &jsonl_path,
            &[
                r#"{"timestamp":"2026-03-08T13:46:00.000Z","type":"session_meta","payload":{"cwd":"/home/mstie/projects/taurhaus"}}"#,
            ],
        );

        let session = sample_session(project_path, &jsonl_path, "session-1", "%7");
        let resolved =
            resolve_managed_codex_session(&teams_dir, &session).expect("resolved managed member");

        assert_eq!(resolved.member_name, "session-match");
        assert_eq!(resolved.pane_id, "%9");
    }
}
