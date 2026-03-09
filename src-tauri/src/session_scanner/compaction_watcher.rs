//! Event-driven watcher for the canonical Codex compaction signal log.

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::coordination::compaction_events::{
    emit_compaction_signal_consumed, emit_compaction_signal_replayed,
    emit_compaction_watcher_missed_event_recovered, signal_event,
    CompactionWatcherMissedEventRecovered,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::compaction_signal::signal_log_path_for_team;
use crate::coordination::stores::{
    CompactionSignalKind, CompactionSignalLog, CompactionSignalRecord,
};
use crate::session_scanner::cli_tool::CliTool;

const SIGNAL_WATCHER_STATE_VERSION: u32 = 2;
const SIGNAL_WATCHER_STATE_FILE: &str = "signal-watcher-state.json";
const DEFAULT_RECONCILIATION_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_LOOP_TICK: Duration = Duration::from_millis(250);
const RECENT_SIGNAL_ID_LIMIT: usize = 512;

pub trait CompactionSignalProcessor: Send + Sync + 'static {
    fn process_signal(&self, signal: &CompactionSignalRecord) -> Result<(), String>;
}

impl<F: ?Sized> CompactionSignalProcessor for F
where
    F: Fn(&CompactionSignalRecord) -> Result<(), String> + Send + Sync + 'static,
{
    fn process_signal(&self, signal: &CompactionSignalRecord) -> Result<(), String> {
        self(signal)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompactionSignalWatcherConfig {
    pub reconciliation_interval: Duration,
    pub loop_tick: Duration,
}

impl Default for CompactionSignalWatcherConfig {
    fn default() -> Self {
        Self {
            reconciliation_interval: DEFAULT_RECONCILIATION_INTERVAL,
            loop_tick: DEFAULT_LOOP_TICK,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedCompactionSignalWatcherState {
    version: u32,
    #[serde(default)]
    last_consumed_offset: u64,
    #[serde(default)]
    last_event_at: Option<String>,
    #[serde(default)]
    last_reconciliation_at: Option<String>,
    #[serde(default)]
    reconciliation_poll_count: u64,
    #[serde(default)]
    missed_event_recovery_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSignalWatcherDiagnostics {
    pub last_consumed_offset: u64,
    pub last_event_at: Option<String>,
    pub last_reconciliation_at: Option<String>,
    pub reconciliation_poll_count: u64,
    pub missed_event_recovery_count: u64,
}

#[derive(Debug, Default)]
struct RuntimeCompactionSignalWatcherState {
    last_consumed_offset: u64,
    recent_signal_ids: HashSet<String>,
    recent_signal_order: VecDeque<String>,
    last_event_at: Option<String>,
    last_reconciliation_at: Option<String>,
    reconciliation_poll_count: u64,
    missed_event_recovery_count: u64,
}

impl RuntimeCompactionSignalWatcherState {
    fn remember_signal_id(&mut self, signal_id: String) {
        if self.recent_signal_ids.contains(&signal_id) {
            return;
        }

        self.recent_signal_ids.insert(signal_id.clone());
        self.recent_signal_order.push_back(signal_id);

        while self.recent_signal_order.len() > RECENT_SIGNAL_ID_LIMIT {
            if let Some(removed) = self.recent_signal_order.pop_front() {
                self.recent_signal_ids.remove(&removed);
            }
        }
    }
}

struct CompactionSignalWatcherCore<P: ?Sized> {
    teams_dir: PathBuf,
    team_name: String,
    processor: Arc<P>,
    state: Mutex<RuntimeCompactionSignalWatcherState>,
}

impl<P: CompactionSignalProcessor + ?Sized> CompactionSignalWatcherCore<P> {
    fn new_at(
        teams_dir: impl Into<PathBuf>,
        team_name: impl Into<String>,
        processor: Arc<P>,
    ) -> Result<Self, CoordinationError> {
        let teams_dir = teams_dir.into();
        let team_name = team_name.into();
        let persisted = load_persisted_state(&teams_dir, &team_name)?;

        Ok(Self {
            teams_dir,
            team_name,
            processor,
            state: Mutex::new(RuntimeCompactionSignalWatcherState {
                last_consumed_offset: persisted.last_consumed_offset,
                last_event_at: persisted.last_event_at,
                last_reconciliation_at: persisted.last_reconciliation_at,
                reconciliation_poll_count: persisted.reconciliation_poll_count,
                missed_event_recovery_count: persisted.missed_event_recovery_count,
                ..RuntimeCompactionSignalWatcherState::default()
            }),
        })
    }

    fn signal_path(&self) -> PathBuf {
        signal_log_path_for_team(&self.teams_dir, &self.team_name)
    }

    fn signal_dir(&self) -> PathBuf {
        self.signal_path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.teams_dir.join(&self.team_name))
    }

    fn process_available_signals(&self, replayed: bool) -> Result<usize, CoordinationError> {
        let signal_path = self.signal_path();
        let file_len = match fs::metadata(&signal_path) {
            Ok(metadata) => metadata.len(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(CoordinationError::Io(err)),
        };

        let start_offset = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.last_consumed_offset > file_len {
                state.last_consumed_offset = file_len;
                let persisted = PersistedCompactionSignalWatcherState {
                    version: SIGNAL_WATCHER_STATE_VERSION,
                    last_consumed_offset: state.last_consumed_offset,
                    last_event_at: None,
                    last_reconciliation_at: None,
                    reconciliation_poll_count: 0,
                    missed_event_recovery_count: 0,
                };
                save_persisted_state(&self.teams_dir, &self.team_name, &persisted)?;
                return Ok(0);
            }
            state.last_consumed_offset
        };

        let items = CompactionSignalLog::read_items_from_offset(
            &self.teams_dir,
            &self.team_name,
            start_offset,
        )?;
        if items.is_empty() {
            return Ok(0);
        }

        let mut recovered_count = 0usize;
        let mut recovered_context: Option<CompactionSignalRecord> = None;

        for item in items {
            let already_seen = {
                let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                state.recent_signal_ids.contains(&item.record.signal_id)
            };

            if already_seen {
                self.commit_offset(item.next_offset)?;
                continue;
            }

            self.processor
                .process_signal(&item.record)
                .map_err(|error| CoordinationError::StoreError(error.to_string()))?;

            emit_compaction_signal_consumed(signal_from_record(&item.record));
            if replayed {
                emit_compaction_signal_replayed(signal_from_record(&item.record));
                recovered_count += 1;
                recovered_context.get_or_insert_with(|| item.record.clone());
            }

            self.remember_signal_and_commit(item.record.signal_id.clone(), item.next_offset)?;
        }

        if replayed && recovered_count > 0 {
            let context = recovered_context.expect("recovered_count guarantees context");
            self.note_missed_event_recovery(recovered_count as u64)?;
            emit_compaction_watcher_missed_event_recovered(CompactionWatcherMissedEventRecovered {
                tool: CliTool::Codex,
                recovered_count,
                team_name: None,
                member_name: None,
                session_id: Some(context.session_id),
                pane_id: Some(context.pane_id),
            });
        }

        Ok(recovered_count)
    }

    fn commit_offset(&self, next_offset: u64) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.last_consumed_offset = next_offset;
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }

    fn remember_signal_and_commit(
        &self,
        signal_id: String,
        next_offset: u64,
    ) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.remember_signal_id(signal_id);
        state.last_consumed_offset = next_offset;
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }

    fn note_notify_event(&self) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.last_event_at = Some(chrono::Utc::now().to_rfc3339());
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }

    fn note_reconciliation_poll(&self) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.last_reconciliation_at = Some(chrono::Utc::now().to_rfc3339());
        state.reconciliation_poll_count = state.reconciliation_poll_count.saturating_add(1);
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }

    fn note_missed_event_recovery(&self, recovered_count: u64) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.missed_event_recovery_count = state
            .missed_event_recovery_count
            .saturating_add(recovered_count);
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }

    #[cfg(test)]
    fn force_last_consumed_offset_for_test(&self, offset: u64) -> Result<(), CoordinationError> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.last_consumed_offset = offset;
        save_persisted_state(
            &self.teams_dir,
            &self.team_name,
            &PersistedCompactionSignalWatcherState::from_runtime_state(&state),
        )
    }
}

pub struct CompactionSignalWatcher {
    shutdown: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl CompactionSignalWatcher {
    pub fn start_at<P: CompactionSignalProcessor + ?Sized>(
        teams_dir: impl Into<PathBuf>,
        team_name: impl Into<String>,
        processor: Arc<P>,
        config: CompactionSignalWatcherConfig,
    ) -> Result<Self, CoordinationError> {
        let core = Arc::new(CompactionSignalWatcherCore::new_at(
            teams_dir, team_name, processor,
        )?);
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join_handle = thread::spawn(move || {
            if let Err(error) = run_watcher_loop(core, thread_shutdown, config, ready_tx) {
                tracing::warn!(error = %error, "compaction signal watcher loop exited with error");
            }
        });

        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(CoordinationError::StoreError(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(CoordinationError::StoreError(
                    "compaction signal watcher did not become ready before timeout".to_string(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(CoordinationError::StoreError(
                    "compaction signal watcher exited before signaling readiness".to_string(),
                ));
            }
        }

        Ok(Self {
            shutdown,
            join_handle: Some(join_handle),
        })
    }
}

impl Drop for CompactionSignalWatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_watcher_loop<P: CompactionSignalProcessor + ?Sized>(
    core: Arc<CompactionSignalWatcherCore<P>>,
    shutdown: Arc<AtomicBool>,
    config: CompactionSignalWatcherConfig,
    ready_tx: mpsc::SyncSender<Result<(), String>>,
) -> Result<(), CoordinationError> {
    let signal_path = core.signal_path();
    let signal_dir = core.signal_dir();
    fs::create_dir_all(&signal_dir)?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let _ = tx.send(res);
        },
        Config::default(),
    )
    .map_err(|error| CoordinationError::StoreError(error.to_string()))?;

    watcher
        .watch(&signal_dir, RecursiveMode::NonRecursive)
        .map_err(|error| CoordinationError::StoreError(error.to_string()))?;
    let mut file_watch_active = try_watch_signal_file(&mut watcher, &signal_path);
    core.process_available_signals(true)?;
    let _ = ready_tx.send(Ok(()));
    let mut last_reconcile = Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        match rx.recv_timeout(config.loop_tick) {
            Ok(Ok(event)) => {
                if !file_watch_active && signal_path.exists() {
                    file_watch_active = try_watch_signal_file(&mut watcher, &signal_path);
                }
                if should_process_signal_event(&event, &signal_path) {
                    core.note_notify_event()?;
                    core.process_available_signals(false)?;
                }
            }
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "compaction signal watcher received notify error");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if last_reconcile.elapsed() >= config.reconciliation_interval {
            if !file_watch_active && signal_path.exists() {
                file_watch_active = try_watch_signal_file(&mut watcher, &signal_path);
            }
            core.note_reconciliation_poll()?;
            core.process_available_signals(true)?;
            last_reconcile = Instant::now();
        }
    }

    Ok(())
}

fn try_watch_signal_file(watcher: &mut RecommendedWatcher, signal_path: &Path) -> bool {
    if !signal_path.exists() {
        return false;
    }

    watcher
        .watch(signal_path, RecursiveMode::NonRecursive)
        .map(|_| true)
        .unwrap_or_else(|error| {
            tracing::debug!(
                path = %signal_path.display(),
                error = %error,
                "failed to register direct watch on compaction signal file; directory watch remains active"
            );
            false
        })
}

fn should_process_signal_event(event: &Event, signal_path: &Path) -> bool {
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
        return false;
    }

    event.paths.iter().any(|path| path == signal_path)
}

fn signal_from_record(
    record: &CompactionSignalRecord,
) -> crate::coordination::compaction_events::CompactionSignalEvent {
    signal_event(
        record.tool,
        Some(record.session_id.as_str()),
        Some(record.pane_id.as_str()),
        Some(record.project_path.as_str()),
        Some(Path::new(record.jsonl_path.as_str())),
        Some(record.transcript_timestamp),
        Some(match record.signal_kind {
            CompactionSignalKind::Compacted => {
                crate::coordination::compaction_events::CompactionSignalKind::Compacted
            }
            CompactionSignalKind::ContextCompacted => {
                crate::coordination::compaction_events::CompactionSignalKind::ContextCompacted
            }
        }),
    )
}

fn watcher_state_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir
        .join(team_name)
        .join("state")
        .join("compaction")
        .join(SIGNAL_WATCHER_STATE_FILE)
}

fn load_persisted_state(
    teams_dir: &Path,
    team_name: &str,
) -> Result<PersistedCompactionSignalWatcherState, CoordinationError> {
    let state_path = watcher_state_path(teams_dir, team_name);
    let raw = match fs::read_to_string(&state_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PersistedCompactionSignalWatcherState {
                version: SIGNAL_WATCHER_STATE_VERSION,
                last_consumed_offset: 0,
                last_event_at: None,
                last_reconciliation_at: None,
                reconciliation_poll_count: 0,
                missed_event_recovery_count: 0,
            });
        }
        Err(err) => return Err(CoordinationError::Io(err)),
    };

    serde_json::from_str(&raw).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to parse compaction watcher state for team '{team_name}': {error}"
        ))
    })
}

fn save_persisted_state(
    teams_dir: &Path,
    team_name: &str,
    state: &PersistedCompactionSignalWatcherState,
) -> Result<(), CoordinationError> {
    let state_path = watcher_state_path(teams_dir, team_name);
    let parent = state_path.parent().ok_or_else(|| {
        CoordinationError::StoreError("invalid compaction watcher state path".to_string())
    })?;
    fs::create_dir_all(parent)?;
    let mut payload = state.clone();
    payload.version = SIGNAL_WATCHER_STATE_VERSION;
    let raw = serde_json::to_string_pretty(&payload).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize compaction watcher state for team '{team_name}': {error}"
        ))
    })?;
    fs::write(state_path, raw.as_bytes())?;
    Ok(())
}

impl PersistedCompactionSignalWatcherState {
    fn from_runtime_state(state: &RuntimeCompactionSignalWatcherState) -> Self {
        Self {
            version: SIGNAL_WATCHER_STATE_VERSION,
            last_consumed_offset: state.last_consumed_offset,
            last_event_at: state.last_event_at.clone(),
            last_reconciliation_at: state.last_reconciliation_at.clone(),
            reconciliation_poll_count: state.reconciliation_poll_count,
            missed_event_recovery_count: state.missed_event_recovery_count,
        }
    }
}

pub fn load_compaction_signal_watcher_diagnostics_at(
    teams_dir: &Path,
    team_name: &str,
) -> Result<CompactionSignalWatcherDiagnostics, CoordinationError> {
    let state = load_persisted_state(teams_dir, team_name)?;
    Ok(CompactionSignalWatcherDiagnostics {
        last_consumed_offset: state.last_consumed_offset,
        last_event_at: state.last_event_at,
        last_reconciliation_at: state.last_reconciliation_at,
        reconciliation_poll_count: state.reconciliation_poll_count,
        missed_event_recovery_count: state.missed_event_recovery_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use chrono::TimeZone;
    use uuid::Uuid;

    use crate::coordination::stores::compaction_signal::signal_log_path_for_team;
    use crate::coordination::stores::CompactionSignalRecord;

    #[derive(Default)]
    struct RecordingProcessor {
        delivered: Mutex<Vec<CompactionSignalRecord>>,
    }

    impl RecordingProcessor {
        fn delivered(&self) -> Vec<CompactionSignalRecord> {
            self.delivered
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        }
    }

    impl CompactionSignalProcessor for RecordingProcessor {
        fn process_signal(&self, signal: &CompactionSignalRecord) -> Result<(), String> {
            self.delivered
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(signal.clone());
            Ok(())
        }
    }

    fn sample_signal() -> CompactionSignalRecord {
        CompactionSignalRecord {
            version: 1,
            signal_id: Uuid::new_v4().to_string(),
            emitted_at: chrono::Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 30, 0)
                .single()
                .expect("datetime"),
            tool: CliTool::Codex,
            session_id: "sess-123".to_string(),
            pane_id: "%217".to_string(),
            project_path: "/home/mstie/projects/taurhaus".to_string(),
            jsonl_path: "/home/mstie/.codex/sessions/2026/03/08/rollout.jsonl".to_string(),
            jsonl_offset: 18_423,
            transcript_timestamp: chrono::Utc
                .with_ymd_and_hms(2026, 3, 8, 20, 29, 59)
                .single()
                .expect("datetime"),
            signal_kind: CompactionSignalKind::ContextCompacted,
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
    fn watcher_processes_appended_records_via_notify() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let processor = Arc::new(RecordingProcessor::default());
        let signal_path = signal_log_path_for_team(tmp.path(), "taurhaus-team");
        std::fs::create_dir_all(signal_path.parent().expect("signal dir"))
            .expect("create signal dir");
        std::fs::write(&signal_path, b"").expect("precreate signal file");
        let _watcher = CompactionSignalWatcher::start_at(
            tmp.path(),
            "taurhaus-team",
            processor.clone(),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_secs(60),
                loop_tick: Duration::from_millis(25),
            },
        )
        .expect("start watcher");

        CompactionSignalLog::append(tmp.path(), "taurhaus-team", &sample_signal())
            .expect("append signal");

        wait_until(Duration::from_secs(3), || processor.delivered().len() == 1);
    }

    #[test]
    fn reconciliation_catches_missed_events() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let processor = Arc::new(RecordingProcessor::default());
        let core =
            CompactionSignalWatcherCore::new_at(tmp.path(), "taurhaus-team", processor.clone())
                .expect("build core");

        CompactionSignalLog::append(tmp.path(), "taurhaus-team", &sample_signal())
            .expect("append signal");

        let recovered = core
            .process_available_signals(true)
            .expect("reconciliation pass");

        assert_eq!(recovered, 1);
        assert_eq!(processor.delivered().len(), 1);
    }

    #[test]
    fn replay_is_idempotent_when_offset_rewinds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let processor = Arc::new(RecordingProcessor::default());
        let core =
            CompactionSignalWatcherCore::new_at(tmp.path(), "taurhaus-team", processor.clone())
                .expect("build core");

        CompactionSignalLog::append(tmp.path(), "taurhaus-team", &sample_signal())
            .expect("append signal");

        assert_eq!(
            core.process_available_signals(false).expect("first pass"),
            0
        );
        assert_eq!(processor.delivered().len(), 1);

        core.force_last_consumed_offset_for_test(0)
            .expect("rewind offset for replay");

        let recovered = core.process_available_signals(true).expect("replayed pass");

        assert_eq!(recovered, 0);
        assert_eq!(processor.delivered().len(), 1);
    }

    #[test]
    fn persisted_offset_prevents_restart_reprocessing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first_processor = Arc::new(RecordingProcessor::default());
        let first_core = CompactionSignalWatcherCore::new_at(
            tmp.path(),
            "taurhaus-team",
            first_processor.clone(),
        )
        .expect("build first core");

        CompactionSignalLog::append(tmp.path(), "taurhaus-team", &sample_signal())
            .expect("append signal");
        first_core
            .process_available_signals(true)
            .expect("first reconciliation");
        assert_eq!(first_processor.delivered().len(), 1);

        let second_processor = Arc::new(RecordingProcessor::default());
        let second_core = CompactionSignalWatcherCore::new_at(
            tmp.path(),
            "taurhaus-team",
            second_processor.clone(),
        )
        .expect("build second core");

        let recovered = second_core
            .process_available_signals(true)
            .expect("restart reconciliation");

        assert_eq!(recovered, 0);
        assert!(second_processor.delivered().is_empty());
    }

    #[test]
    fn diagnostics_report_persisted_health_counters() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let processor = Arc::new(RecordingProcessor::default());
        let core =
            CompactionSignalWatcherCore::new_at(tmp.path(), "taurhaus-team", processor.clone())
                .expect("build core");

        CompactionSignalLog::append(tmp.path(), "taurhaus-team", &sample_signal())
            .expect("append signal");

        core.note_notify_event().expect("persist notify event");
        core.note_reconciliation_poll()
            .expect("persist reconcile poll");
        let recovered = core
            .process_available_signals(true)
            .expect("reconciliation pass");

        assert_eq!(recovered, 1);

        let diagnostics =
            load_compaction_signal_watcher_diagnostics_at(tmp.path(), "taurhaus-team")
                .expect("load diagnostics");
        assert!(diagnostics.last_consumed_offset > 0);
        assert!(diagnostics.last_event_at.is_some());
        assert!(diagnostics.last_reconciliation_at.is_some());
        assert_eq!(diagnostics.reconciliation_poll_count, 1);
        assert_eq!(diagnostics.missed_event_recovery_count, 1);
    }
}
