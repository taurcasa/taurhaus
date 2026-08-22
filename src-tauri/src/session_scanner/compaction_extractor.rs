//! Event-oriented Codex compaction signal extractor.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, Utc};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{cli_tool::CliTool, RuntimeSession};
use crate::coordination::compaction_events::{
    emit_compaction_extractor_failed, emit_compaction_extractor_heartbeat,
    emit_compaction_signal_emitted, CompactionExtractorFailedEvent,
    CompactionExtractorHeartbeatEvent, CompactionSignalEvent,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::roster::get_team_roster_with_attachments;
use crate::coordination::stores::{
    CompactionSignalKind, CompactionSignalLog, CompactionSignalRecord, MemberRuntimeStore,
    TeamConfigStore,
};

const EXTRACTOR_SCHEMA_VERSION: u32 = 2;
const EXTRACTOR_STATE_FILENAME: &str = "extractor-state.json";
const PAIRED_SIGNAL_WINDOW_MS: i64 = 2_000;
const DEFAULT_EXTRACTOR_RECONCILIATION_INTERVAL: Duration = Duration::from_secs(5);
const EXTRACTOR_HEARTBEAT_INTERVAL_SECS: i64 = 60;
const EXTRACTOR_CHECKPOINT_RETENTION_SECS: i64 = 60;
const EXTRACTOR_CHECKPOINT_REFRESH_SECS: i64 = 30;

struct CompactionSignalExtractorService {
    shutdown: Arc<AtomicBool>,
    command_tx: mpsc::Sender<ExtractorServiceCommand>,
    join_handle: Option<JoinHandle<()>>,
}

enum ExtractorServiceCommand {
    SessionsUpdated(Vec<RuntimeSession>),
    Notify(Result<Event, notify::Error>),
    Stop,
}

impl Drop for CompactionSignalExtractorService {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.command_tx.send(ExtractorServiceCommand::Stop);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

static EXTRACTOR_SERVICE: OnceLock<Mutex<Option<CompactionSignalExtractorService>>> =
    OnceLock::new();
static EXTRACTOR_HEARTBEATS: OnceLock<Mutex<HashMap<PathBuf, DateTime<Utc>>>> = OnceLock::new();

fn extractor_service_slot() -> &'static Mutex<Option<CompactionSignalExtractorService>> {
    EXTRACTOR_SERVICE.get_or_init(|| Mutex::new(None))
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedCodexTranscript {
    team_name: String,
    member_name: String,
    session_id: String,
    pane_id: String,
    project_path: String,
    jsonl_path: PathBuf,
    cli_tool: CliTool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSignalBoundary {
    timestamp: DateTime<Utc>,
    jsonl_offset: u64,
    signal_kind: CompactionSignalKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExtractorFileCheckpoint {
    offset: u64,
    #[serde(default)]
    last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExtractorRecentBoundary {
    timestamp: DateTime<Utc>,
    jsonl_offset: u64,
    signal_kind: CompactionSignalKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionExtractorFileDiagnostics {
    pub jsonl_path: String,
    pub offset: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionExtractorDiagnostics {
    pub heartbeat_at: Option<String>,
    pub last_processed_signal_id: Option<String>,
    pub last_processed_jsonl_path: Option<String>,
    pub last_processed_jsonl_offset: Option<u64>,
    pub active_files: Vec<CompactionExtractorFileDiagnostics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LastProcessedSignal {
    signal_id: String,
    jsonl_path: String,
    jsonl_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExtractorState {
    version: u32,
    #[serde(default)]
    file_offsets: BTreeMap<String, ExtractorFileCheckpoint>,
    #[serde(default)]
    last_emitted_boundary_by_file: BTreeMap<String, ExtractorRecentBoundary>,
    #[serde(default)]
    last_processed_signal: Option<LastProcessedSignal>,
    #[serde(default)]
    heartbeat_at: Option<DateTime<Utc>>,
    #[serde(default)]
    last_error_by_file: BTreeMap<String, String>,
}

impl Default for ExtractorState {
    fn default() -> Self {
        Self {
            version: EXTRACTOR_SCHEMA_VERSION,
            file_offsets: BTreeMap::new(),
            last_emitted_boundary_by_file: BTreeMap::new(),
            last_processed_signal: None,
            heartbeat_at: None,
            last_error_by_file: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TeamExtractionStats {
    tracked_file_count: usize,
    emitted_signal_count: usize,
}

pub fn start_compaction_extractor_service_at(
    teams_dir: impl Into<PathBuf>,
    initial_sessions: Vec<RuntimeSession>,
) -> Result<(), CoordinationError> {
    start_compaction_extractor_service_with_reconciliation_at(
        teams_dir,
        initial_sessions,
        DEFAULT_EXTRACTOR_RECONCILIATION_INTERVAL,
    )
}

pub fn update_active_runtime_sessions(sessions: &[RuntimeSession]) {
    let Some(service) = extractor_service_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .map(|service| service.command_tx.clone())
    else {
        return;
    };

    let codex_sessions = sessions
        .iter()
        .filter(|session| session.cli_tool == CliTool::Codex)
        .cloned()
        .collect::<Vec<_>>();
    let _ = service.send(ExtractorServiceCommand::SessionsUpdated(codex_sessions));
}

fn start_compaction_extractor_service_with_reconciliation_at(
    teams_dir: impl Into<PathBuf>,
    initial_sessions: Vec<RuntimeSession>,
    reconciliation_interval: Duration,
) -> Result<(), CoordinationError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = shutdown.clone();
    let (command_tx, command_rx) = mpsc::channel();
    let loop_command_tx = command_tx.clone();
    let teams_dir = teams_dir.into();
    let loop_teams_dir = teams_dir.clone();
    let initial_sessions = initial_sessions
        .into_iter()
        .filter(|session| session.cli_tool == CliTool::Codex)
        .collect::<Vec<_>>();
    let join_handle = thread::spawn(move || {
        if let Err(error) = run_extractor_service_loop(
            loop_teams_dir,
            initial_sessions,
            loop_command_tx,
            command_rx,
            thread_shutdown,
            reconciliation_interval,
        ) {
            tracing::warn!(error = %error, "compaction extractor loop exited with error");
        }
    });

    let mut slot = extractor_service_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *slot = Some(CompactionSignalExtractorService {
        shutdown,
        command_tx,
        join_handle: Some(join_handle),
    });
    Ok(())
}

#[cfg(test)]
pub fn start_compaction_extractor_service_for_test(
    teams_dir: impl Into<PathBuf>,
    initial_sessions: Vec<RuntimeSession>,
    reconciliation_interval: Duration,
) -> Result<(), CoordinationError> {
    start_compaction_extractor_service_with_reconciliation_at(
        teams_dir,
        initial_sessions,
        reconciliation_interval,
    )
}

pub(crate) fn stop_compaction_extractor_service() {
    let mut slot = extractor_service_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *slot = None;
}

#[cfg(test)]
pub fn stop_compaction_extractor_service_for_test() {
    stop_compaction_extractor_service();
}

#[cfg(test)]
pub fn compaction_extractor_service_is_running_for_test() -> bool {
    extractor_service_slot()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .is_some()
}

fn run_extractor_service_loop(
    teams_dir: PathBuf,
    mut active_sessions: Vec<RuntimeSession>,
    command_tx: mpsc::Sender<ExtractorServiceCommand>,
    command_rx: mpsc::Receiver<ExtractorServiceCommand>,
    shutdown: Arc<AtomicBool>,
    reconciliation_interval: Duration,
) -> Result<(), CoordinationError> {
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let _ = command_tx.send(ExtractorServiceCommand::Notify(res));
        },
        Config::default(),
    )
    .map_err(|error| CoordinationError::StoreError(error.to_string()))?;

    let mut watched_paths = HashSet::new();
    reconcile_watched_transcripts(&mut watcher, &mut watched_paths, &active_sessions);
    extract_compaction_signals_at(&active_sessions, &teams_dir, Utc::now());

    let loop_timeout = reconciliation_interval.max(Duration::from_millis(1));

    while !shutdown.load(Ordering::Relaxed) {
        match command_rx.recv_timeout(loop_timeout) {
            Ok(ExtractorServiceCommand::SessionsUpdated(sessions)) => {
                active_sessions = sessions;
                reconcile_watched_transcripts(&mut watcher, &mut watched_paths, &active_sessions);
            }
            Ok(ExtractorServiceCommand::Notify(Ok(event))) => {
                if should_process_transcript_event(&event, &watched_paths) {
                    extract_compaction_signals_at(&active_sessions, &teams_dir, Utc::now());
                }
            }
            Ok(ExtractorServiceCommand::Notify(Err(error))) => {
                tracing::warn!(error = %error, "compaction extractor received notify error");
            }
            Ok(ExtractorServiceCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                reconcile_watched_transcripts(&mut watcher, &mut watched_paths, &active_sessions);
                extract_compaction_signals_at(&active_sessions, &teams_dir, Utc::now());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn reconcile_watched_transcripts(
    watcher: &mut RecommendedWatcher,
    watched_paths: &mut HashSet<PathBuf>,
    sessions: &[RuntimeSession],
) {
    let desired_paths = sessions
        .iter()
        .filter(|session| session.cli_tool == CliTool::Codex)
        .filter_map(|session| session.jsonl_path.as_deref())
        .map(PathBuf::from)
        .collect::<HashSet<_>>();

    let stale_paths = watched_paths
        .iter()
        .filter(|path| !desired_paths.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    for path in stale_paths {
        let _ = watcher.unwatch(&path);
        watched_paths.remove(&path);
    }

    for path in desired_paths {
        if watched_paths.contains(&path) {
            continue;
        }

        match watcher.watch(&path, RecursiveMode::NonRecursive) {
            Ok(()) => {
                watched_paths.insert(path);
            }
            Err(error) => {
                tracing::debug!(
                    path = %path.display(),
                    error = %error,
                    "failed to register compaction transcript watch; reconciliation fallback remains active"
                );
            }
        }
    }
}

fn should_process_transcript_event(event: &Event, watched_paths: &HashSet<PathBuf>) -> bool {
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
        return false;
    }

    event.paths.iter().any(|path| watched_paths.contains(path))
}

pub fn extract_compaction_signals_at(
    sessions: &[RuntimeSession],
    teams_dir: &Path,
    emitted_at: DateTime<Utc>,
) {
    sync_managed_codex_runtime_bindings(sessions, teams_dir);
    let transcripts = load_managed_codex_transcripts_from_runtime(teams_dir);
    let active_file_count = transcripts.len();

    let mut grouped = BTreeMap::<String, Vec<ManagedCodexTranscript>>::new();
    for transcript in transcripts {
        grouped
            .entry(transcript.team_name.clone())
            .or_default()
            .push(transcript);
    }

    let mut tracked_offset_count = 0usize;
    let mut pending_signal_count = 0usize;

    for (team_name, transcripts) in grouped {
        match extract_compaction_signals_for_team(teams_dir, &team_name, &transcripts, emitted_at) {
            Ok(stats) => {
                tracked_offset_count += stats.tracked_file_count;
                pending_signal_count += stats.emitted_signal_count;
            }
            Err(error) => {
                tracing::warn!(
                    team_name = team_name,
                    error = %error,
                    "failed to extract Codex compaction signals for team"
                );
            }
        }
    }

    if should_emit_extractor_heartbeat(teams_dir, emitted_at) {
        emit_compaction_extractor_heartbeat(CompactionExtractorHeartbeatEvent {
            tool: CliTool::Codex,
            active_file_count,
            tracked_offset_count,
            pending_signal_count,
        });
    }
}

fn should_emit_extractor_heartbeat(teams_dir: &Path, now: DateTime<Utc>) -> bool {
    let mut heartbeats = EXTRACTOR_HEARTBEATS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if heartbeats.get(teams_dir).is_some_and(|previous| {
        now.signed_duration_since(*previous).num_seconds() < EXTRACTOR_HEARTBEAT_INTERVAL_SECS
    }) {
        return false;
    }
    heartbeats.insert(teams_dir.to_path_buf(), now);
    true
}

fn extract_compaction_signals_for_team(
    teams_dir: &Path,
    team_name: &str,
    transcripts: &[ManagedCodexTranscript],
    emitted_at: DateTime<Utc>,
) -> Result<TeamExtractionStats, CoordinationError> {
    let mut state = load_extractor_state(teams_dir, team_name)?;
    let previous_state = state.clone();
    let mut seen_paths = HashSet::new();
    let mut emitted_signal_count = 0usize;

    for transcript in transcripts {
        if !seen_paths.insert(transcript.jsonl_path.clone()) {
            continue;
        }
        let path_key = transcript.jsonl_path.display().to_string();
        if let Some(checkpoint) = state.file_offsets.get_mut(&path_key) {
            if checkpoint_last_seen_refresh_is_due(checkpoint.last_seen_at, emitted_at) {
                checkpoint.last_seen_at = Some(emitted_at);
            }
        }
        let file_len = match fs::metadata(&transcript.jsonl_path) {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                state
                    .last_error_by_file
                    .insert(path_key.clone(), error.to_string());
                emit_compaction_extractor_failed(CompactionExtractorFailedEvent {
                    tool: transcript.cli_tool,
                    jsonl_path: path_key,
                    stage: "stat".to_string(),
                    error_message: error.to_string(),
                });
                continue;
            }
        };

        let start_offset = match state.file_offsets.get_mut(&path_key) {
            Some(checkpoint) if checkpoint.offset > file_len => {
                checkpoint.offset = 0;
                0
            }
            Some(checkpoint) => checkpoint.offset,
            None => {
                state.file_offsets.insert(
                    path_key.clone(),
                    ExtractorFileCheckpoint {
                        offset: file_len,
                        last_seen_at: Some(emitted_at),
                    },
                );
                state.last_error_by_file.remove(&path_key);
                continue;
            }
        };

        if start_offset == file_len {
            state.last_error_by_file.remove(&path_key);
            continue;
        }

        let (boundaries, committed_offset) =
            match read_appended_compaction_boundaries(&transcript.jsonl_path, start_offset) {
                Ok(result) => result,
                Err(error) => {
                    state
                        .last_error_by_file
                        .insert(path_key.clone(), error.to_string());
                    emit_compaction_extractor_failed(CompactionExtractorFailedEvent {
                        tool: transcript.cli_tool,
                        jsonl_path: path_key,
                        stage: "read_appended_boundaries".to_string(),
                        error_message: error.to_string(),
                    });
                    continue;
                }
            };

        state.file_offsets.insert(
            path_key.clone(),
            ExtractorFileCheckpoint {
                offset: committed_offset,
                last_seen_at: Some(emitted_at),
            },
        );
        state.last_error_by_file.remove(&path_key);

        for boundary in normalize_paired_boundaries(boundaries) {
            if should_suppress_paired_boundary(
                state.last_emitted_boundary_by_file.get(&path_key),
                &boundary,
            ) {
                state.last_emitted_boundary_by_file.insert(
                    path_key.clone(),
                    ExtractorRecentBoundary {
                        timestamp: boundary.timestamp,
                        jsonl_offset: boundary.jsonl_offset,
                        signal_kind: boundary.signal_kind,
                    },
                );
                continue;
            }

            let record = CompactionSignalRecord {
                version: EXTRACTOR_SCHEMA_VERSION,
                signal_id: Uuid::new_v4().to_string(),
                emitted_at,
                tool: transcript.cli_tool,
                session_id: transcript.session_id.clone(),
                pane_id: transcript.pane_id.clone(),
                project_path: transcript.project_path.clone(),
                jsonl_path: transcript.jsonl_path.display().to_string(),
                jsonl_offset: boundary.jsonl_offset,
                transcript_timestamp: boundary.timestamp,
                signal_kind: boundary.signal_kind,
            };

            CompactionSignalLog::append(teams_dir, team_name, &record)?;
            state.last_processed_signal = Some(LastProcessedSignal {
                signal_id: record.signal_id.clone(),
                jsonl_path: record.jsonl_path.clone(),
                jsonl_offset: record.jsonl_offset,
            });
            state.last_emitted_boundary_by_file.insert(
                path_key.clone(),
                ExtractorRecentBoundary {
                    timestamp: boundary.timestamp,
                    jsonl_offset: boundary.jsonl_offset,
                    signal_kind: boundary.signal_kind,
                },
            );
            emitted_signal_count += 1;

            emit_compaction_signal_emitted(CompactionSignalEvent {
                tool: record.tool,
                team_name: Some(team_name.to_string()),
                member_name: None,
                session_id: Some(record.session_id.clone()),
                pane_id: Some(record.pane_id.clone()),
                project_path: Some(record.project_path.clone()),
                jsonl_path: Some(record.jsonl_path.clone()),
                compaction_timestamp: Some(record.transcript_timestamp),
                signal_kind: Some(match record.signal_kind {
                    CompactionSignalKind::Compacted => {
                        crate::coordination::compaction_events::CompactionSignalKind::Compacted
                    }
                    CompactionSignalKind::ContextCompacted => crate::coordination::compaction_events::CompactionSignalKind::ContextCompacted,
                }),
            });
        }
    }

    state
        .file_offsets
        .retain(|_, checkpoint| checkpoint_is_within_retention_window(checkpoint, emitted_at));
    let retained_paths = state.file_offsets.keys().cloned().collect::<HashSet<_>>();
    state
        .last_emitted_boundary_by_file
        .retain(|path, _| retained_paths.contains(path));
    state
        .last_error_by_file
        .retain(|path, _| retained_paths.contains(path));
    if extractor_heartbeat_is_due(state.heartbeat_at, emitted_at) {
        state.heartbeat_at = Some(emitted_at);
    }
    if state != previous_state {
        save_extractor_state(teams_dir, team_name, &state)?;
    }

    Ok(TeamExtractionStats {
        tracked_file_count: state.file_offsets.len(),
        emitted_signal_count,
    })
}

fn checkpoint_is_within_retention_window(
    checkpoint: &ExtractorFileCheckpoint,
    now: DateTime<Utc>,
) -> bool {
    checkpoint.last_seen_at.is_some_and(|last_seen_at| {
        now.signed_duration_since(last_seen_at).num_seconds() <= EXTRACTOR_CHECKPOINT_RETENTION_SECS
    })
}

fn checkpoint_last_seen_refresh_is_due(
    previous: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    previous.is_none_or(|previous| {
        now.signed_duration_since(previous).num_seconds() >= EXTRACTOR_CHECKPOINT_REFRESH_SECS
    })
}

fn extractor_heartbeat_is_due(previous: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    previous.is_none_or(|previous| {
        now.signed_duration_since(previous).num_seconds() >= EXTRACTOR_HEARTBEAT_INTERVAL_SECS
    })
}

fn load_managed_codex_transcripts_from_runtime(teams_dir: &Path) -> Vec<ManagedCodexTranscript> {
    let team_names = match TeamConfigStore::list(teams_dir) {
        Ok(team_names) => team_names,
        Err(error) => {
            tracing::warn!(error = %error, "failed to list teams while loading compaction runtime transcripts");
            return Vec::new();
        }
    };

    let mut transcripts = Vec::new();
    for team_name in team_names {
        let roster = match get_team_roster_with_attachments(teams_dir, &team_name) {
            Ok(roster) => roster,
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team roster while loading compaction runtime transcripts");
                continue;
            }
        };

        for member in roster
            .into_iter()
            .filter(|member| member.configured_cli_tool == CliTool::Codex)
        {
            let Some(session_id) = member.session_id.clone() else {
                continue;
            };
            let Some(pane_id) = member.pane_id.clone() else {
                continue;
            };
            let Some(jsonl_path) = member.jsonl_path.clone() else {
                continue;
            };
            let project_path = member
                .attached_project_path
                .as_ref()
                .unwrap_or(&member.configured_project_path)
                .display()
                .to_string();
            transcripts.push(ManagedCodexTranscript {
                team_name: team_name.clone(),
                member_name: member.member_name,
                session_id,
                pane_id,
                project_path,
                jsonl_path,
                cli_tool: CliTool::Codex,
            });
        }
    }

    transcripts
}

fn sync_managed_codex_runtime_bindings(sessions: &[RuntimeSession], teams_dir: &Path) {
    let codex_sessions = sessions
        .iter()
        .filter(|session| session.cli_tool == CliTool::Codex)
        .filter(|session| runtime_session_has_compaction_identity(session))
        .cloned()
        .collect::<Vec<_>>();
    let team_names = match TeamConfigStore::list(teams_dir) {
        Ok(team_names) => team_names,
        Err(error) => {
            tracing::warn!(error = %error, "failed to list teams while syncing compaction runtime jsonl paths");
            return;
        }
    };

    for team_name in team_names {
        let roster = match get_team_roster_with_attachments(teams_dir, &team_name) {
            Ok(roster) => roster,
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team roster while syncing compaction runtime jsonl paths");
                continue;
            }
        };

        for member in roster
            .into_iter()
            .filter(|member| member.configured_cli_tool == CliTool::Codex)
        {
            let Some(mut runtime) = member.runtime_record() else {
                continue;
            };
            let Some(matched_session) =
                select_runtime_session_for_member(&runtime, &codex_sessions)
            else {
                continue;
            };
            let matched_session_id = matched_session
                .session_id
                .clone()
                .or_else(|| runtime.session_id.clone());
            let matched_jsonl_path = matched_session
                .jsonl_path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| runtime.jsonl_path.clone());
            if runtime.session_id == matched_session_id && runtime.jsonl_path == matched_jsonl_path
            {
                continue;
            }
            runtime.session_id = matched_session_id;
            runtime.jsonl_path = matched_jsonl_path;
            if let Err(error) =
                MemberRuntimeStore::save(teams_dir, &team_name, &member.member_name, &runtime)
            {
                tracing::warn!(team_name = team_name, member_name = member.member_name, error = %error, "failed to persist compaction runtime binding");
            }
        }
    }
}

fn runtime_session_has_compaction_identity(session: &RuntimeSession) -> bool {
    session.session_id.is_some() || session.jsonl_path.is_some()
}

fn select_runtime_session_for_member<'a>(
    runtime: &crate::coordination::stores::MemberRuntimeRecord,
    sessions: &'a [RuntimeSession],
) -> Option<&'a RuntimeSession> {
    let mut best: Option<(&RuntimeSession, u8)> = None;

    for session in sessions {
        let pane_matches = runtime.pane_id.as_deref() == session.tmux_pane.as_deref();
        let session_matches = runtime.session_id.as_deref() == session.session_id.as_deref();
        let score = match (pane_matches, session_matches) {
            (true, true) => 4u8,
            (true, false) => 3u8,
            (false, true) => 2u8,
            (false, false) => 0u8,
        };
        if score == 0 {
            continue;
        }

        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some((session, score)),
        }
    }

    best.map(|(session, _)| session)
}

fn read_appended_compaction_boundaries(
    jsonl_path: &Path,
    start_offset: u64,
) -> std::io::Result<(Vec<ParsedSignalBoundary>, u64)> {
    let file = File::open(jsonl_path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start_offset))?;

    let mut boundaries = Vec::new();
    let mut line = String::new();
    let mut committed_offset = start_offset;

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
        if line.is_empty() {
            continue;
        }

        if let Some(boundary) = parse_signal_boundary(&line, committed_offset) {
            boundaries.push(boundary);
        }
    }

    Ok((boundaries, committed_offset))
}

fn parse_signal_boundary(line: &str, jsonl_offset: u64) -> Option<ParsedSignalBoundary> {
    let parsed: Value = serde_json::from_str(line).ok()?;
    let timestamp = parsed
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))?;

    let signal_kind = match parsed.get("type").and_then(Value::as_str) {
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

    Some(ParsedSignalBoundary {
        timestamp,
        jsonl_offset,
        signal_kind,
    })
}

fn normalize_paired_boundaries(boundaries: Vec<ParsedSignalBoundary>) -> Vec<ParsedSignalBoundary> {
    let mut normalized: Vec<ParsedSignalBoundary> = Vec::new();

    for boundary in boundaries {
        if let Some(previous) = normalized.last_mut() {
            let within_pair_window = boundary
                .timestamp
                .signed_duration_since(previous.timestamp)
                .num_milliseconds()
                .abs()
                <= PAIRED_SIGNAL_WINDOW_MS;

            if previous.signal_kind == CompactionSignalKind::Compacted
                && boundary.signal_kind == CompactionSignalKind::ContextCompacted
                && within_pair_window
            {
                previous.timestamp = boundary.timestamp;
                previous.jsonl_offset = boundary.jsonl_offset;
                previous.signal_kind = CompactionSignalKind::ContextCompacted;
                continue;
            }
        }

        normalized.push(boundary);
    }

    normalized
}

fn should_suppress_paired_boundary(
    previous: Option<&ExtractorRecentBoundary>,
    boundary: &ParsedSignalBoundary,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };

    previous.signal_kind == CompactionSignalKind::Compacted
        && boundary.signal_kind == CompactionSignalKind::ContextCompacted
        && boundary
            .timestamp
            .signed_duration_since(previous.timestamp)
            .num_milliseconds()
            .abs()
            <= PAIRED_SIGNAL_WINDOW_MS
}

fn load_extractor_state(
    teams_dir: &Path,
    team_name: &str,
) -> Result<ExtractorState, CoordinationError> {
    let path = extractor_state_path(teams_dir, team_name);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExtractorState::default())
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    };

    let mut state: ExtractorState = serde_json::from_str(&raw).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to parse compaction extractor state for team '{team_name}': {error}"
        ))
    })?;
    for checkpoint in state.file_offsets.values_mut() {
        if checkpoint.last_seen_at.is_none() {
            checkpoint.last_seen_at = state.heartbeat_at;
        }
    }
    Ok(state)
}

fn save_extractor_state(
    teams_dir: &Path,
    team_name: &str,
    state: &ExtractorState,
) -> Result<(), CoordinationError> {
    let _lock = crate::coordination::stores::lock::acquire_team_lock(teams_dir, team_name)?;

    let mut normalized = state.clone();
    normalized.version = EXTRACTOR_SCHEMA_VERSION;

    let compaction_dir = extractor_state_dir(teams_dir, team_name);
    fs::create_dir_all(&compaction_dir)?;

    let path = extractor_state_path(teams_dir, team_name);
    let tmp_path = compaction_dir.join("extractor-state.json.tmp");
    let payload = serde_json::to_string_pretty(&normalized).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize compaction extractor state for team '{team_name}': {error}"
        ))
    })?;

    fs::write(&tmp_path, payload)?;
    if let Err(err) = fs::rename(&tmp_path, &path) {
        if is_windows_unsupported_rename_error(&err) {
            if let Err(write_err) = fs::write(&path, normalized_payload_bytes(state, team_name)?) {
                let _ = fs::remove_file(&tmp_path);
                return Err(CoordinationError::Io(write_err));
            }
            let _ = fs::remove_file(&tmp_path);
            return Ok(());
        }

        let _ = fs::remove_file(&tmp_path);
        return Err(CoordinationError::Io(err));
    }

    Ok(())
}

fn extractor_state_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join("state").join("compaction")
}

fn extractor_state_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    extractor_state_dir(teams_dir, team_name).join(EXTRACTOR_STATE_FILENAME)
}

pub fn load_compaction_extractor_diagnostics_at(
    teams_dir: &Path,
    team_name: &str,
) -> Result<CompactionExtractorDiagnostics, CoordinationError> {
    let state = load_extractor_state(teams_dir, team_name)?;
    let ExtractorState {
        file_offsets,
        last_processed_signal,
        heartbeat_at,
        last_error_by_file,
        ..
    } = state;
    let mut active_files = file_offsets
        .into_iter()
        .map(
            |(jsonl_path, checkpoint)| CompactionExtractorFileDiagnostics {
                last_error: last_error_by_file.get(&jsonl_path).cloned(),
                jsonl_path,
                offset: checkpoint.offset,
            },
        )
        .collect::<Vec<_>>();
    active_files.sort_by(|left, right| left.jsonl_path.cmp(&right.jsonl_path));

    Ok(CompactionExtractorDiagnostics {
        heartbeat_at: heartbeat_at.map(|value| value.to_rfc3339()),
        last_processed_signal_id: last_processed_signal
            .as_ref()
            .map(|value| value.signal_id.clone()),
        last_processed_jsonl_path: last_processed_signal
            .as_ref()
            .map(|value| value.jsonl_path.clone()),
        last_processed_jsonl_offset: last_processed_signal
            .as_ref()
            .map(|value| value.jsonl_offset),
        active_files,
    })
}

fn normalized_payload_bytes(
    state: &ExtractorState,
    team_name: &str,
) -> Result<Vec<u8>, CoordinationError> {
    let mut normalized = state.clone();
    normalized.version = EXTRACTOR_SCHEMA_VERSION;
    serde_json::to_vec_pretty(&normalized).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize compaction extractor state for team '{team_name}': {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::commands::logging::{
        clear_test_tap, install_global_sink, install_test_tap, LogFileState,
    };
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    fn sample_transcript(team_name: &str, jsonl_path: &Path) -> ManagedCodexTranscript {
        ManagedCodexTranscript {
            team_name: team_name.to_string(),
            member_name: "architect".to_string(),
            session_id: "sess-123".to_string(),
            pane_id: "%217".to_string(),
            project_path: "/home/user/projects/taurhaus".to_string(),
            jsonl_path: jsonl_path.to_path_buf(),
            cli_tool: CliTool::Codex,
        }
    }

    fn write_runtime_and_config(
        teams_dir: &Path,
        team_name: &str,
        pane_id: &str,
        project_path: &str,
    ) {
        let config = crate::coordination::stores::TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 0, 0)
                .single()
                .expect("datetime"),
            members: vec![crate::coordination::domain::Member {
                name: "architect".to_string(),
                role: crate::coordination::domain::MemberRole::Agent,
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
                project_path: PathBuf::from(project_path),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            }],
            extra: Default::default(),
        };
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save config");
        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            "architect",
            &crate::coordination::stores::MemberRuntimeRecord {
                schema_version: 3,
                member_name: "architect".to_string(),
                cli_tool: Some(CliTool::Codex),
                project_path: Some(PathBuf::from(project_path)),
                pane_id: Some(pane_id.to_string()),
                session_id: Some("sess-123".to_string()),
                jsonl_path: None,
                daemon_pid: None,
                health: crate::coordination::domain::HealthState::Healthy,
                delivery_lease: None,
                attached_at: Some(
                    Utc.with_ymd_and_hms(2026, 3, 8, 19, 55, 0)
                        .single()
                        .expect("datetime"),
                ),
                last_seen_at: Some(
                    Utc.with_ymd_and_hms(2026, 3, 8, 20, 4, 0)
                        .single()
                        .expect("datetime"),
                ),
            },
        )
        .expect("save runtime");
    }

    fn write_jsonl(path: &Path, lines: &[&str]) {
        let mut body = lines.join("\n");
        if !body.is_empty() {
            body.push('\n');
        }
        fs::write(path, body).expect("write jsonl");
    }

    fn append_raw(path: &Path, text: &str) {
        use std::io::Write;

        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .expect("open append");
        file.write_all(text.as_bytes()).expect("append");
    }

    #[test]
    fn extractor_state_round_trips_offsets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = ExtractorState {
            version: EXTRACTOR_SCHEMA_VERSION,
            file_offsets: BTreeMap::from([(
                "/tmp/session.jsonl".to_string(),
                ExtractorFileCheckpoint {
                    offset: 42,
                    last_seen_at: Some(
                        Utc.with_ymd_and_hms(2026, 3, 8, 20, 5, 0)
                            .single()
                            .expect("datetime"),
                    ),
                },
            )]),
            last_emitted_boundary_by_file: BTreeMap::from([(
                "/tmp/session.jsonl".to_string(),
                ExtractorRecentBoundary {
                    timestamp: Utc
                        .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                        .single()
                        .expect("datetime"),
                    jsonl_offset: 42,
                    signal_kind: CompactionSignalKind::ContextCompacted,
                },
            )]),
            last_processed_signal: Some(LastProcessedSignal {
                signal_id: "sig-1".to_string(),
                jsonl_path: "/tmp/session.jsonl".to_string(),
                jsonl_offset: 42,
            }),
            heartbeat_at: Some(
                Utc.with_ymd_and_hms(2026, 3, 8, 20, 6, 0)
                    .single()
                    .expect("datetime"),
            ),
            last_error_by_file: BTreeMap::from([(
                "/tmp/session.jsonl".to_string(),
                "boom".to_string(),
            )]),
        };

        save_extractor_state(tmp.path(), "taurhaus-team", &state).expect("save");
        let loaded = load_extractor_state(tmp.path(), "taurhaus-team").expect("load");

        assert_eq!(loaded, state);
    }

    #[test]
    fn extractor_heartbeat_is_sampled_at_most_once_per_minute() {
        // Regression: 27770fbd emitted compaction.extractor.heartbeat on every scanner
        // publication; this host recorded 265 events in roughly two minutes.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let log_state =
            LogFileState::new(tmp.path().join("extractor.log.jsonl")).expect("create log state");
        install_global_sink(&log_state);
        let (sender, receiver) = std::sync::mpsc::channel();
        install_test_tap(sender);

        let first = Utc
            .with_ymd_and_hms(2026, 3, 8, 20, 0, 0)
            .single()
            .expect("datetime");
        extract_compaction_signals_at(&[], tmp.path(), first);
        extract_compaction_signals_at(&[], tmp.path(), first + chrono::Duration::seconds(59));

        let heartbeat_count = receiver
            .try_iter()
            .filter(|event| event["event"] == "compaction.extractor.heartbeat")
            .count();
        clear_test_tap();
        assert_eq!(heartbeat_count, 1);
    }

    #[test]
    fn normalize_paired_records_emits_single_canonical_context_compacted_signal() {
        let boundaries = vec![
            ParsedSignalBoundary {
                timestamp: Utc
                    .with_ymd_and_hms(2026, 3, 8, 20, 0, 0)
                    .single()
                    .expect("datetime"),
                jsonl_offset: 10,
                signal_kind: CompactionSignalKind::Compacted,
            },
            ParsedSignalBoundary {
                timestamp: Utc
                    .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                    .single()
                    .expect("datetime"),
                jsonl_offset: 40,
                signal_kind: CompactionSignalKind::ContextCompacted,
            },
        ];

        let normalized = normalize_paired_boundaries(boundaries);

        assert_eq!(
            normalized,
            vec![ParsedSignalBoundary {
                timestamp: Utc
                    .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                    .single()
                    .expect("datetime"),
                jsonl_offset: 40,
                signal_kind: CompactionSignalKind::ContextCompacted,
            }]
        );
    }

    #[test]
    fn suppresses_cross_pass_context_compacted_pair_after_compacted_signal() {
        let previous = ExtractorRecentBoundary {
            timestamp: Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 0, 0)
                .single()
                .expect("datetime"),
            jsonl_offset: 10,
            signal_kind: CompactionSignalKind::Compacted,
        };
        let boundary = ParsedSignalBoundary {
            timestamp: Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
            jsonl_offset: 40,
            signal_kind: CompactionSignalKind::ContextCompacted,
        };

        assert!(should_suppress_paired_boundary(Some(&previous), &boundary));
    }

    #[test]
    fn extractor_skips_partial_trailing_line_until_completed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        let transcript = sample_transcript("taurhaus-team", &jsonl_path);

        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 5)
                .single()
                .expect("datetime"),
        )
        .expect("prime state");

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}"#,
        );
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        )
        .expect("process partial");
        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert!(records.is_empty());

        append_raw(&jsonl_path, "\n");
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[transcript],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 12)
                .single()
                .expect("datetime"),
        )
        .expect("process completed line");

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].signal_kind, CompactionSignalKind::Compacted);
    }

    #[test]
    fn extractor_restart_recovery_resumes_from_saved_offset() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        let transcript = sample_transcript("taurhaus-team", &jsonl_path);

        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
        )
        .expect("prime state");

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}"#,
        );
        append_raw(&jsonl_path, "\n");

        let stats = extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        )
        .expect("extract first appended event");
        assert_eq!(stats.emitted_signal_count, 1);

        let before_restart = load_extractor_state(&teams_dir, "taurhaus-team").expect("load");
        assert_eq!(
            before_restart
                .file_offsets
                .get(&jsonl_path.display().to_string())
                .expect("offset checkpoint")
                .offset,
            fs::metadata(&jsonl_path).expect("metadata").len()
        );

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:20Z","type":"event_msg","payload":{"type":"context_compacted"}}"#,
        );
        append_raw(&jsonl_path, "\n");

        let stats = extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[transcript],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 21)
                .single()
                .expect("datetime"),
        )
        .expect("extract after restart");
        assert_eq!(stats.emitted_signal_count, 1);

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].signal_kind, CompactionSignalKind::Compacted);
        assert_eq!(
            records[1].signal_kind,
            CompactionSignalKind::ContextCompacted
        );
    }

    #[test]
    fn extractor_offset_survives_one_empty_scan() {
        // Regression: 27770fbd retained checkpoints only for paths present in the current
        // scan, so one inventory blackout re-baselined a known transcript to EOF.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        let transcript = sample_transcript("taurhaus-team", &jsonl_path);
        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
        )
        .expect("prime state");
        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}
"#,
        );

        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 5)
                .single()
                .expect("datetime"),
        )
        .expect("empty scan");
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[transcript],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        )
        .expect("recovered scan");

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read signals");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].signal_kind, CompactionSignalKind::Compacted);
    }

    #[test]
    fn extractor_prunes_checkpoints_after_transcript_is_deleted() {
        // Regression: a89ea4c removed active-scan pruning without adding a bounded
        // replacement, so deleted rollout paths accumulated forever in state and diagnostics.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let deleted_path = tmp.path().join("deleted-session.jsonl");
        let path_key = deleted_path.display().to_string();
        let state = ExtractorState {
            version: EXTRACTOR_SCHEMA_VERSION,
            file_offsets: BTreeMap::from([(
                path_key.clone(),
                ExtractorFileCheckpoint {
                    offset: 42,
                    last_seen_at: Some(
                        Utc.with_ymd_and_hms(2026, 3, 8, 19, 0, 0)
                            .single()
                            .expect("datetime"),
                    ),
                },
            )]),
            last_emitted_boundary_by_file: BTreeMap::from([(
                path_key.clone(),
                ExtractorRecentBoundary {
                    timestamp: Utc
                        .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                        .single()
                        .expect("datetime"),
                    jsonl_offset: 42,
                    signal_kind: CompactionSignalKind::Compacted,
                },
            )]),
            last_processed_signal: None,
            heartbeat_at: None,
            last_error_by_file: BTreeMap::from([(path_key, "deleted".to_string())]),
        };
        save_extractor_state(&teams_dir, "taurhaus-team", &state).expect("seed state");

        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 1, 0)
                .single()
                .expect("datetime"),
        )
        .expect("prune deleted transcript");

        let pruned = load_extractor_state(&teams_dir, "taurhaus-team").expect("load state");
        assert!(pruned.file_offsets.is_empty());
        assert!(pruned.last_emitted_boundary_by_file.is_empty());
        assert!(pruned.last_error_by_file.is_empty());
        assert!(
            load_compaction_extractor_diagnostics_at(&teams_dir, "taurhaus-team")
                .expect("load diagnostics")
                .active_files
                .is_empty()
        );
    }

    #[test]
    fn extractor_prunes_stale_checkpoint_while_transcript_still_exists() {
        // Regression: 9f723d3 retained every checkpoint whose transcript still existed,
        // so historical rollout files made state, diagnostics, and per-pass stats grow forever.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let stale_path = tmp.path().join("stale-session.jsonl");
        write_jsonl(
            &stale_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        let transcript = sample_transcript("taurhaus-team", &stale_path);
        let first_seen = Utc
            .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
            .single()
            .expect("datetime");
        extract_compaction_signals_for_team(&teams_dir, "taurhaus-team", &[transcript], first_seen)
            .expect("prime state");

        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[],
            first_seen + chrono::Duration::minutes(2),
        )
        .expect("prune stale checkpoint");

        let pruned = load_extractor_state(&teams_dir, "taurhaus-team").expect("load state");
        assert!(pruned.file_offsets.is_empty());
        assert!(
            load_compaction_extractor_diagnostics_at(&teams_dir, "taurhaus-team")
                .expect("load diagnostics")
                .active_files
                .is_empty()
        );
    }

    #[test]
    fn extractor_does_not_rewrite_unchanged_state_before_next_heartbeat() {
        // Regression: a89ea4c rewrote the full extractor checkpoint map on every notify
        // event by updating heartbeat_at even when no transcript state had changed.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        let transcript = sample_transcript("taurhaus-team", &jsonl_path);
        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        let first = Utc
            .with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
            .single()
            .expect("datetime");
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            first,
        )
        .expect("prime state");
        let state_path = extractor_state_path(&teams_dir, "taurhaus-team");
        let before = fs::read(&state_path).expect("read initial state");

        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[transcript],
            first + chrono::Duration::seconds(1),
        )
        .expect("unchanged pass");

        assert_eq!(fs::read(state_path).expect("read unchanged state"), before);
    }

    #[test]
    fn empty_scan_does_not_clear_known_runtime_binding() {
        // Regression: a11c347d assigned None from an unmatched scan into the persisted
        // session/jsonl binding even though the managed pane still existed.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        write_runtime_and_config(
            &teams_dir,
            "taurhaus-team",
            "%217",
            "/home/user/projects/taurhaus",
        );
        let mut runtime = MemberRuntimeStore::load(&teams_dir, "taurhaus-team", "architect")
            .expect("load runtime");
        runtime.jsonl_path = Some(jsonl_path.clone());
        MemberRuntimeStore::save(&teams_dir, "taurhaus-team", "architect", &runtime)
            .expect("save binding");

        sync_managed_codex_runtime_bindings(&[], &teams_dir);

        let stored = MemberRuntimeStore::load(&teams_dir, "taurhaus-team", "architect")
            .expect("reload runtime");
        assert_eq!(stored.session_id.as_deref(), Some("sess-123"));
        assert_eq!(stored.jsonl_path.as_deref(), Some(jsonl_path.as_path()));
    }

    #[test]
    fn runtime_binding_ignores_candidate_without_identity_or_transcript() {
        // Regression: a89ea4c removed the candidate prefilter, allowing a session with
        // no id and no transcript to shadow an id-less session that had a real rollout path.
        let runtime = crate::coordination::stores::MemberRuntimeRecord {
            schema_version: 3,
            member_name: "architect".to_string(),
            cli_tool: Some(CliTool::Codex),
            project_path: Some(PathBuf::from("/home/user/projects/taurhaus")),
            pane_id: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: crate::coordination::domain::HealthState::Healthy,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
        };
        let session = |jsonl_path: Option<&str>| RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: super::super::SessionState::Active,
            session_id: None,
            jsonl_path: jsonl_path.map(ToOwned::to_owned),
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: super::super::ActivityConfidence::Low,
            activity_attribution: super::super::ActivityAttribution::Unattributed,
            project_unattributed_active: false,
            group_kind: super::super::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        };
        let sessions = [session(None), session(Some("/tmp/real-rollout.jsonl"))];
        let candidates = sessions
            .iter()
            .filter(|session| runtime_session_has_compaction_identity(session))
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            select_runtime_session_for_member(&runtime, &candidates)
                .and_then(|matched| matched.jsonl_path.as_deref()),
            Some("/tmp/real-rollout.jsonl")
        );
    }

    #[test]
    fn extractor_emits_single_signal_when_pair_arrives_across_two_passes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        let transcript = sample_transcript("taurhaus-team", &jsonl_path);

        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
        )
        .expect("prime state");

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10.000Z","type":"compacted"}"#,
        );
        append_raw(&jsonl_path, "\n");

        let first_stats = extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            std::slice::from_ref(&transcript),
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 10)
                .single()
                .expect("datetime"),
        )
        .expect("extract compacted");
        assert_eq!(first_stats.emitted_signal_count, 1);

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10.030Z","type":"event_msg","payload":{"type":"context_compacted"}}"#,
        );
        append_raw(&jsonl_path, "\n");

        let second_stats = extract_compaction_signals_for_team(
            &teams_dir,
            "taurhaus-team",
            &[transcript],
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        )
        .expect("extract context_compacted");
        assert_eq!(second_stats.emitted_signal_count, 0);

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].signal_kind, CompactionSignalKind::Compacted);
    }

    #[test]
    fn runtime_sessions_route_into_team_signal_log() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");

        write_runtime_and_config(
            &teams_dir,
            "taurhaus-team",
            "%217",
            "/home/user/projects/taurhaus",
        );
        write_jsonl(
            &jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );

        let session = RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%217".to_string()),
            tmux_window_name: Some("work".to_string()),
            state: super::super::SessionState::Active,
            session_id: Some("sess-123".to_string()),
            jsonl_path: Some(jsonl_path.display().to_string()),
            recent_io: true,
            last_output_age_secs: Some(1),
            activity_confidence: super::super::ActivityConfidence::High,
            activity_attribution: super::super::ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: super::super::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: Some("architect".to_string()),
        };

        extract_compaction_signals_at(
            std::slice::from_ref(&session),
            &teams_dir,
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
        );

        append_raw(
            &jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}"#,
        );
        append_raw(&jsonl_path, "\n");

        extract_compaction_signals_at(
            &[session],
            &teams_dir,
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        );

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].pane_id, "%217");
        assert_eq!(records[0].session_id, "sess-123");
    }

    #[test]
    fn extractor_ignores_same_project_scanner_session_without_matching_runtime_record() {
        // Regression: commit 8f3ac2a matched Codex transcripts back to teams by project path,
        // so a second same-project session could be treated as managed even when no runtime
        // member record owned its pane/session.
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let tracked_jsonl_path = tmp.path().join("tracked-session.jsonl");
        let stray_jsonl_path = tmp.path().join("stray-session.jsonl");

        write_runtime_and_config(
            &teams_dir,
            "taurhaus-team",
            "%217",
            "/home/user/projects/taurhaus",
        );
        write_jsonl(
            &tracked_jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );
        write_jsonl(
            &stray_jsonl_path,
            &[r#"{"timestamp":"2026-03-08T20:00:00Z","type":"message"}"#],
        );

        let tracked_session = RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%217".to_string()),
            tmux_window_name: Some("work".to_string()),
            state: super::super::SessionState::Active,
            session_id: Some("sess-123".to_string()),
            jsonl_path: Some(tracked_jsonl_path.display().to_string()),
            recent_io: true,
            last_output_age_secs: Some(1),
            activity_confidence: super::super::ActivityConfidence::High,
            activity_attribution: super::super::ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: super::super::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: Some("architect".to_string()),
        };
        let stray_session = RuntimeSession {
            pid: 99,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/8".to_string(),
            args: "codex".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%999".to_string()),
            tmux_window_name: Some("work-2".to_string()),
            state: super::super::SessionState::Active,
            session_id: Some("sess-999".to_string()),
            jsonl_path: Some(stray_jsonl_path.display().to_string()),
            recent_io: true,
            last_output_age_secs: Some(1),
            activity_confidence: super::super::ActivityConfidence::High,
            activity_attribution: super::super::ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: super::super::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: Some("stray".to_string()),
        };

        let sessions = [tracked_session.clone(), stray_session.clone()];
        extract_compaction_signals_at(
            &sessions,
            &teams_dir,
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 1)
                .single()
                .expect("datetime"),
        );

        let runtime = MemberRuntimeStore::load(&teams_dir, "taurhaus-team", "architect")
            .expect("load runtime");
        assert_eq!(runtime.jsonl_path, Some(tracked_jsonl_path.clone()));

        append_raw(
            &tracked_jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}"#,
        );
        append_raw(&tracked_jsonl_path, "\n");
        append_raw(
            &stray_jsonl_path,
            r#"{"timestamp":"2026-03-08T20:00:10Z","type":"compacted"}"#,
        );
        append_raw(&stray_jsonl_path, "\n");

        extract_compaction_signals_at(
            &[tracked_session, stray_session],
            &teams_dir,
            Utc.with_ymd_and_hms(2026, 3, 8, 20, 0, 11)
                .single()
                .expect("datetime"),
        );

        let records = CompactionSignalLog::read_from_offset(&teams_dir, "taurhaus-team", 0)
            .expect("read records");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].pane_id, "%217");
        assert_eq!(records[0].session_id, "sess-123");
        assert_eq!(
            records[0].jsonl_path,
            tracked_jsonl_path.display().to_string()
        );
    }
}
