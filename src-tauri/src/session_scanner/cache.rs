use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{
    compaction_extractor, proc_io, process, tmux, DisplaySession, RuntimeSession, SessionState,
};

/// Per-PID state tracker for bidirectional hysteresis.
struct StateTracker {
    /// The state we last reported to the frontend.
    reported: SessionState,
    /// The raw (unfiltered) state from the previous poll.
    prev_raw: SessionState,
}

/// State trackers keyed by PID.
static STATE_TRACKERS: Mutex<Option<HashMap<u32, StateTracker>>> = Mutex::new(None);

/// Max age for cached tmux pane metadata before forced refresh.
const TMUX_CACHE_MAX_AGE: Duration = Duration::from_secs(2);

#[derive(Default)]
struct ScannerCache {
    /// A healthy full scan has populated `processes`/`pid_fingerprint`; a
    /// healthy empty inventory is a valid cached state.
    initialized: bool,
    pid_fingerprint: Vec<u32>,
    processes: Vec<process::ProcessInfo>,
    pane_map: HashMap<String, tmux::TmuxPane>,
    tmux_epoch: u64,
    last_tmux_refresh: Option<Instant>,
}

static SCAN_CACHE: OnceLock<Mutex<ScannerCache>> = OnceLock::new();
static TMUX_CHANGE_EPOCH: AtomicU64 = AtomicU64::new(0);
static LATEST_COMPACTION_RUNTIME_SESSIONS: OnceLock<Mutex<Vec<RuntimeSession>>> = OnceLock::new();
#[allow(clippy::type_complexity)]
#[cfg(test)]
static DISPLAY_SCAN_COMPACTION_HOOK: OnceLock<Mutex<Option<fn(&[RuntimeSession])>>> =
    OnceLock::new();
#[allow(clippy::type_complexity)]
#[cfg(test)]
static DISPLAY_SCAN_COMPLETED_HOOK: OnceLock<Mutex<Option<fn(usize)>>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn set_display_scan_compaction_hook(hook: Option<fn(&[RuntimeSession])>) {
    *DISPLAY_SCAN_COMPACTION_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = hook;
}

#[cfg(test)]
pub(crate) fn set_display_scan_completed_hook(hook: Option<fn(usize)>) {
    *DISPLAY_SCAN_COMPLETED_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = hook;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScanCompletionMetrics {
    pub(crate) process_scan_ms: u64,
    pub(crate) tmux_ms: u64,
    pub(crate) process_cache_hit: bool,
    pub(crate) tmux_cache_hit: bool,
    pub(crate) classify_ms: u64,
    pub(crate) idle_ms: u64,
    pub(crate) process_signal_ms: u64,
    pub(crate) ownership_ms: u64,
    pub(crate) total_ms: u64,
    /// The process inventory could not be read; sessions are the previous inventory.
    pub(crate) degraded: bool,
}

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

pub(crate) fn publish_compaction_runtime_sessions(runtime_sessions: &[RuntimeSession]) {
    // Which subscription a project's Claude session writes to has to be known
    // after that session ends — that is when Resume asks.
    super::claude_accounts::record_claude_transcripts(runtime_sessions);

    *LATEST_COMPACTION_RUNTIME_SESSIONS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = runtime_sessions.to_vec();

    #[cfg(test)]
    if let Some(hook) = DISPLAY_SCAN_COMPACTION_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .copied()
    {
        hook(runtime_sessions);
        return;
    }

    compaction_extractor::update_active_runtime_sessions(runtime_sessions);
}

pub fn latest_compaction_runtime_sessions() -> Vec<RuntimeSession> {
    LATEST_COMPACTION_RUNTIME_SESSIONS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn emit_scan_completed(metrics: ScanCompletionMetrics, session_count: usize) {
    #[cfg(test)]
    if let Some(hook) = DISPLAY_SCAN_COMPLETED_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .copied()
    {
        hook(session_count);
    }

    let mut fields = Map::new();
    fields.insert(
        "process_scan_ms".to_string(),
        json_number_u64(metrics.process_scan_ms),
    );
    fields.insert("tmux_ms".to_string(), json_number_u64(metrics.tmux_ms));
    fields.insert(
        "process_cache_hit".to_string(),
        Value::Bool(metrics.process_cache_hit),
    );
    fields.insert(
        "tmux_cache_hit".to_string(),
        Value::Bool(metrics.tmux_cache_hit),
    );
    fields.insert(
        "classify_ms".to_string(),
        json_number_u64(metrics.classify_ms),
    );
    fields.insert("idle_ms".to_string(), json_number_u64(metrics.idle_ms));
    fields.insert(
        "process_signal_ms".to_string(),
        json_number_u64(metrics.process_signal_ms),
    );
    fields.insert(
        "ownership_ms".to_string(),
        json_number_u64(metrics.ownership_ms),
    );
    fields.insert("duration_ms".to_string(), json_number_u64(metrics.total_ms));
    fields.insert("degraded".to_string(), Value::Bool(metrics.degraded));
    fields.insert(
        "session_count".to_string(),
        Value::Number(serde_json::Number::from(session_count)),
    );
    crate::commands::logging::emit_global(
        "debug",
        "backend",
        "session_scanner.scan.completed",
        Some("Session scanner cycle completed".to_string()),
        fields,
    );
}

pub(crate) fn finalize_display_scan(
    display_sessions: Vec<DisplaySession>,
    runtime_sessions_for_compaction: Option<&[RuntimeSession]>,
    metrics: ScanCompletionMetrics,
) -> Vec<DisplaySession> {
    if metrics.degraded {
        // Degraded scan: the sessions are the previous inventory, not an
        // observation. Publish nothing and prune nothing.
        emit_scan_completed(metrics, display_sessions.len());
        return display_sessions;
    }

    if let Some(runtime_sessions) = runtime_sessions_for_compaction {
        publish_compaction_runtime_sessions(runtime_sessions);
    }

    let active_pids: Vec<u32> = display_sessions.iter().map(|session| session.pid).collect();
    proc_io::retain_pids(&active_pids);
    retain_state_trackers(&active_pids);
    emit_scan_completed(metrics, display_sessions.len());
    display_sessions
}

/// Notify scanner cache that tmux layout metadata likely changed.
pub fn notify_tmux_changed() {
    TMUX_CHANGE_EPOCH.fetch_add(1, Ordering::Relaxed);
}

/// Cached scanner inputs for one cycle.
pub(crate) struct ScanInputs {
    pub(crate) processes: Vec<process::ProcessInfo>,
    pub(crate) pane_map: HashMap<String, tmux::TmuxPane>,
    pub(crate) process_cache_hit: bool,
    pub(crate) tmux_cache_hit: bool,
    pub(crate) process_scan_ms: u64,
    pub(crate) tmux_ms: u64,
    /// The process inventory could not be read this cycle; `processes` is the
    /// previous inventory and the pid fingerprint was left untouched.
    pub(crate) degraded: bool,
}

pub(crate) fn scan_inputs_with_cache<F, G, H>(
    now: Instant,
    process_id_scanner: &F,
    process_scanner: &G,
    tmux_lister: &H,
) -> ScanInputs
where
    F: Fn() -> Option<Vec<u32>>,
    G: Fn() -> process::ProcessScan,
    H: Fn() -> HashMap<String, tmux::TmuxPane>,
{
    // The inventory cost is the fingerprint read plus any full scan it
    // triggers; a degraded fingerprint read is the whole cost of that cycle.
    let process_started = Instant::now();
    let current_pids = process_id_scanner();
    let current_tmux_epoch = TMUX_CHANGE_EPOCH.load(Ordering::Relaxed);

    let cache = SCAN_CACHE.get_or_init(|| Mutex::new(ScannerCache::default()));
    let mut guard = cache.lock().unwrap_or_else(|error| error.into_inner());

    let mut degraded = false;
    let mut process_cache_hit = false;
    let processes = match current_pids {
        None => {
            degraded = true;
            guard.processes.clone()
        }
        Some(current_pids) => {
            process_cache_hit = guard.initialized && guard.pid_fingerprint == current_pids;
            if process_cache_hit {
                guard.processes.clone()
            } else {
                let fresh = process_scanner();
                if fresh.degraded {
                    degraded = true;
                    guard.processes.clone()
                } else {
                    guard.initialized = true;
                    guard.processes = fresh.processes.clone();
                    guard.pid_fingerprint = current_pids;
                    fresh.processes
                }
            }
        }
    };
    let process_scan_ms = process_started.elapsed().as_millis() as u64;

    let tmux_cache_hit = (process_cache_hit || degraded)
        && !guard.pane_map.is_empty()
        && guard.tmux_epoch == current_tmux_epoch
        && guard
            .last_tmux_refresh
            .is_some_and(|timestamp| now.duration_since(timestamp) < TMUX_CACHE_MAX_AGE);

    let mut tmux_ms = 0u64;
    let pane_map = if tmux_cache_hit {
        guard.pane_map.clone()
    } else {
        let tmux_started = Instant::now();
        let fresh = tmux_lister();
        tmux_ms = tmux_started.elapsed().as_millis() as u64;
        guard.pane_map = fresh.clone();
        guard.tmux_epoch = current_tmux_epoch;
        guard.last_tmux_refresh = Some(now);
        fresh
    };

    ScanInputs {
        processes,
        pane_map,
        process_cache_hit,
        tmux_cache_hit,
        process_scan_ms,
        tmux_ms,
        degraded,
    }
}

/// Apply bidirectional hysteresis to a raw state reading.
///
/// Returns the state to report and the previously reported state (`None` on
/// the first observation of a PID) so callers can log transitions.
pub(crate) fn apply_hysteresis(
    pid: u32,
    raw: SessionState,
) -> (SessionState, Option<SessionState>) {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let previous = map.get(&pid).map(|tracker| tracker.reported);
    let result = match map.get(&pid) {
        Some(tracker) => {
            if raw == tracker.prev_raw && raw != tracker.reported {
                raw
            } else {
                tracker.reported
            }
        }
        None => raw,
    };

    map.insert(
        pid,
        StateTracker {
            reported: result,
            prev_raw: raw,
        },
    );

    (result, previous)
}

/// Record a state the tool reported about itself, bypassing hysteresis.
///
/// The tracker is still written so a later fall back to the heuristics resumes
/// from the authoritative value instead of a stale one. Returns the previously
/// reported state, like `apply_hysteresis`.
pub(crate) fn record_authoritative_state(pid: u32, state: SessionState) -> Option<SessionState> {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let previous = map.get(&pid).map(|tracker| tracker.reported);
    map.insert(
        pid,
        StateTracker {
            reported: state,
            prev_raw: state,
        },
    );
    previous
}

pub(crate) fn retain_state_trackers(active_pids: &[u32]) {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

/// Drop one PID's hysteresis tracker (test cleanup only).
#[cfg(test)]
pub(crate) fn remove_state_tracker(pid: u32) {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(map) = guard.as_mut() {
        map.remove(&pid);
    }
}

/// Reported/raw hysteresis state for one PID (test inspection only).
#[cfg(test)]
pub(crate) fn state_tracker_snapshot(pid: u32) -> Option<(SessionState, SessionState)> {
    STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .and_then(|map| map.get(&pid))
        .map(|tracker| (tracker.reported, tracker.prev_raw))
}

#[cfg(test)]
pub(crate) fn clear_scan_cache() {
    if let Some(cache) = SCAN_CACHE.get() {
        let mut guard = cache.lock().unwrap_or_else(|error| error.into_inner());
        *guard = ScannerCache::default();
    }
    TMUX_CHANGE_EPOCH.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, CliTool, SessionGroupKind, SCANNER_TEST_LOCK,
    };
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    static TEST_COMPACTION_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPLETED_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPACTION_SESSION_IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

    fn record_compaction_sessions(sessions: &[RuntimeSession]) {
        TEST_COMPACTION_SESSION_COUNT.store(sessions.len(), AtomicOrdering::SeqCst);
        let session_ids = sessions
            .iter()
            .filter_map(|session| session.session_id.clone())
            .collect::<Vec<_>>();
        *TEST_COMPACTION_SESSION_IDS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = session_ids;
    }

    fn record_completed_session_count(session_count: usize) {
        TEST_COMPLETED_SESSION_COUNT.store(session_count, AtomicOrdering::SeqCst);
    }

    fn reported_state(pid: u32, raw: SessionState) -> SessionState {
        apply_hysteresis(pid, raw).0
    }

    fn process_info(pid: u32, tty: &str) -> process::ProcessInfo {
        process::ProcessInfo {
            pid,
            project_path: "/home/user/project".to_string(),
            tty: tty.to_string(),
            args: "claude --continue".to_string(),
            cli_tool: CliTool::Claude,
        }
    }

    fn healthy_scan(processes: Vec<process::ProcessInfo>) -> process::ProcessScan {
        process::ProcessScan {
            processes,
            degraded: false,
        }
    }

    fn tmux_map(tty: &str) -> HashMap<String, tmux::TmuxPane> {
        HashMap::from([(
            tty.to_string(),
            tmux::TmuxPane {
                pane_id: "%1".to_string(),
                tty: tty.to_string(),
                window_index: "0".to_string(),
                window_name: "project".to_string(),
                session_name: "taurhaus".to_string(),
            },
        )])
    }

    #[test]
    fn finalize_display_scan_processes_runtime_compaction_and_emits_completion() {
        let _guard = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        TEST_COMPACTION_SESSION_COUNT.store(0, AtomicOrdering::SeqCst);
        TEST_COMPLETED_SESSION_COUNT.store(0, AtomicOrdering::SeqCst);
        TEST_COMPACTION_SESSION_IDS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        set_display_scan_compaction_hook(Some(record_compaction_sessions));
        set_display_scan_completed_hook(Some(record_completed_session_count));

        let runtime_sessions = vec![RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            session_id: Some("sess-123".to_string()),
            jsonl_path: Some("/home/user/.codex/sessions/sess-123.jsonl".to_string()),
            recent_io: false,
            last_output_age_secs: Some(1),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }];
        let display_sessions = runtime_sessions
            .iter()
            .cloned()
            .map(DisplaySession::from)
            .collect::<Vec<_>>();

        let finalized = finalize_display_scan(
            display_sessions,
            Some(&runtime_sessions),
            ScanCompletionMetrics::default(),
        );

        set_display_scan_compaction_hook(None);
        set_display_scan_completed_hook(None);

        assert_eq!(finalized.len(), 1);
        assert_eq!(
            TEST_COMPACTION_SESSION_COUNT.load(AtomicOrdering::SeqCst),
            1
        );
        assert_eq!(TEST_COMPLETED_SESSION_COUNT.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(
            TEST_COMPACTION_SESSION_IDS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_slice(),
            ["sess-123"]
        );
    }
    #[test]
    fn hysteresis_first_observation_reports_raw() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert_eq!(
            reported_state(900_001, SessionState::Idle),
            SessionState::Idle
        );
        remove_state_tracker(900_001);
    }
    #[test]
    fn hysteresis_holds_state_on_single_change() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_002;
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_switches_after_two_consecutive() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_003;
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Active
        );
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_works_in_both_directions() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_004;
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Active
        );
        assert_eq!(
            reported_state(pid, SessionState::Idle),
            SessionState::Active
        );
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_absorbs_alternating_readings() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_005;
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(
            reported_state(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(reported_state(pid, SessionState::Idle), SessionState::Idle);
        remove_state_tracker(pid);
    }
    #[test]
    fn retain_state_trackers_cleans_up() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_006;
        reported_state(pid, SessionState::Idle);
        {
            let guard = STATE_TRACKERS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            assert!(guard.as_ref().unwrap().contains_key(&pid));
        }
        remove_state_tracker(pid);
        {
            let guard = STATE_TRACKERS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            assert!(!guard.as_ref().unwrap().contains_key(&pid));
        }
    }
    #[test]
    fn scanner_cache_hit_reuses_process_and_tmux_data() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || Some(vec![42]);
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            healthy_scan(vec![process_info(42, "/dev/pts/1")])
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, AtomicOrdering::Relaxed);
            tmux_map("/dev/pts/1")
        };

        let now = Instant::now();
        let first = scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        let second = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert!(!first.process_cache_hit);
        assert!(!first.tmux_cache_hit);
        assert!(second.process_cache_hit);
        assert!(second.tmux_cache_hit);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(tmux_calls.load(AtomicOrdering::Relaxed), 1);
    }
    #[test]
    fn scanner_cache_invalidates_on_pid_change() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let pid_scan_calls = AtomicUsize::new(0);
        let process_ids = || {
            let call = pid_scan_calls.fetch_add(1, AtomicOrdering::Relaxed);
            if call == 0 {
                Some(vec![42])
            } else {
                Some(vec![43])
            }
        };
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            healthy_scan(vec![process_info(43, "/dev/pts/1")])
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, AtomicOrdering::Relaxed);
            tmux_map("/dev/pts/1")
        };

        let now = Instant::now();
        let _ = scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        let _ = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 2);
        assert_eq!(tmux_calls.load(AtomicOrdering::Relaxed), 2);
    }
    #[test]
    fn scanner_cache_invalidates_on_tmux_change_epoch() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || Some(vec![42]);
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            healthy_scan(vec![process_info(42, "/dev/pts/1")])
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, AtomicOrdering::Relaxed);
            tmux_map("/dev/pts/1")
        };

        let now = Instant::now();
        let _ = scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        notify_tmux_changed();
        let second = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert!(second.process_cache_hit);
        assert!(!second.tmux_cache_hit);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(tmux_calls.load(AtomicOrdering::Relaxed), 2);
    }

    // Regression: latent since 9a66d1c. A timed-out `ps` became an empty
    // inventory, `finalize_display_scan` pruned every state tracker and proc_io
    // entry, and the sessions came back a few seconds later with fresh
    // hysteresis state. A degraded scan must leave trackers untouched.
    #[test]
    fn finalize_display_scan_does_not_prune_trackers_on_degraded() {
        let _guard = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        set_display_scan_compaction_hook(Some(record_compaction_sessions));
        TEST_COMPACTION_SESSION_COUNT.store(usize::MAX, AtomicOrdering::SeqCst);
        let pid = 900_007;
        reported_state(pid, SessionState::Active);
        let no_runtime_sessions: [RuntimeSession; 0] = [];

        let degraded = finalize_display_scan(
            Vec::new(),
            Some(&no_runtime_sessions),
            ScanCompletionMetrics {
                degraded: true,
                ..ScanCompletionMetrics::default()
            },
        );
        assert!(degraded.is_empty());
        {
            let guard = STATE_TRACKERS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            assert!(
                guard.as_ref().unwrap().contains_key(&pid),
                "degraded scan must not prune state trackers"
            );
        }
        assert_eq!(
            TEST_COMPACTION_SESSION_COUNT.load(AtomicOrdering::SeqCst),
            usize::MAX,
            "degraded scan must not publish compaction runtime sessions"
        );

        // Control: a healthy empty scan prunes.
        let _ = finalize_display_scan(
            Vec::new(),
            Some(&no_runtime_sessions),
            ScanCompletionMetrics::default(),
        );
        set_display_scan_compaction_hook(None);
        {
            let guard = STATE_TRACKERS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            assert!(!guard.as_ref().unwrap().contains_key(&pid));
        }
        assert_eq!(
            TEST_COMPACTION_SESSION_COUNT.load(AtomicOrdering::SeqCst),
            0
        );
    }

    #[test]
    fn hysteresis_reports_previous_state_on_transition() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let pid = 900_008;
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            (SessionState::Idle, None)
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            (SessionState::Idle, Some(SessionState::Idle))
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            (SessionState::Active, Some(SessionState::Idle))
        );
        remove_state_tracker(pid);
    }

    // Regression: latent since 9a66d1c. A failed pid fingerprint (`None`) used to
    // become `[]`, miss the cache and run a full scan that failed the same way,
    // so the cache was overwritten with an empty inventory. Both failure points
    // must keep the previous inventory and report the cycle as degraded.
    #[test]
    fn scanner_cache_keeps_previous_inventory_on_degraded_scan() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let process_ids_ok = || Some(vec![42]);
        let process_ids_degraded = || None;
        let process_ids_changed = || Some(vec![42, 43]);
        let process_scan_ok = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            healthy_scan(vec![process_info(42, "/dev/pts/1")])
        };
        let process_scan_degraded = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            process::ProcessScan {
                processes: Vec::new(),
                degraded: true,
            }
        };
        let tmux_scan = || tmux_map("/dev/pts/1");

        let now = Instant::now();
        let healthy = scan_inputs_with_cache(now, &process_ids_ok, &process_scan_ok, &tmux_scan);
        assert!(!healthy.degraded);
        assert_eq!(healthy.processes, vec![process_info(42, "/dev/pts/1")]);

        // Fingerprint read failed: no full scan, previous inventory, degraded.
        let fingerprint_failed = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids_degraded,
            &process_scan_degraded,
            &tmux_scan,
        );
        assert!(fingerprint_failed.degraded);
        assert_eq!(
            fingerprint_failed.processes,
            vec![process_info(42, "/dev/pts/1")]
        );
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);

        // Fingerprint changed but the full scan failed: previous inventory, degraded.
        let scan_failed = scan_inputs_with_cache(
            now + Duration::from_millis(200),
            &process_ids_changed,
            &process_scan_degraded,
            &tmux_scan,
        );
        assert!(scan_failed.degraded);
        assert_eq!(scan_failed.processes, vec![process_info(42, "/dev/pts/1")]);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 2);

        // The fingerprint was left untouched, so the original pids still hit the cache.
        let recovered = scan_inputs_with_cache(
            now + Duration::from_millis(300),
            &process_ids_ok,
            &process_scan_ok,
            &tmux_scan,
        );
        assert!(!recovered.degraded);
        assert!(recovered.process_cache_hit);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 2);
    }

    // Regression: since 06b432d the cache hit required a non-empty inventory,
    // so a healthy empty inventory (no CLI running) never hit and the full
    // /proc walk ran every cycle. Initialization is tracked separately.
    #[test]
    fn scanner_cache_hits_for_healthy_empty_inventory() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let process_ids = || Some(Vec::new());
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            healthy_scan(Vec::new())
        };
        let tmux_scan = HashMap::new;

        let now = Instant::now();
        let first = scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        let second = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert!(!first.degraded);
        assert!(!first.process_cache_hit);
        assert!(second.process_cache_hit);
        assert!(second.processes.is_empty());
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);
    }

    // Regression: `process_scan_ms` only timed the full scan, so a degraded
    // fingerprint read — the whole inventory cost of that cycle — reported 0.
    #[test]
    fn process_scan_ms_includes_fingerprint_read() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let process_ids = || {
            std::thread::sleep(Duration::from_millis(15));
            None
        };
        let process_scan =
            || -> process::ProcessScan { panic!("no full scan after a failed fingerprint read") };
        let tmux_scan = HashMap::new;

        let inputs =
            scan_inputs_with_cache(Instant::now(), &process_ids, &process_scan, &tmux_scan);
        assert!(inputs.degraded);
        assert!(
            inputs.process_scan_ms >= 15,
            "process_scan_ms must include the fingerprint read, got {}",
            inputs.process_scan_ms
        );
    }
}
