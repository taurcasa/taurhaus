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
}

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

pub(crate) fn publish_compaction_runtime_sessions(runtime_sessions: &[RuntimeSession]) {
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

pub(crate) fn scan_inputs_with_cache<F, G, H>(
    now: Instant,
    process_id_scanner: &F,
    process_scanner: &G,
    tmux_lister: &H,
) -> (
    Vec<process::ProcessInfo>,
    HashMap<String, tmux::TmuxPane>,
    bool,
    bool,
    u64,
    u64,
)
where
    F: Fn() -> Vec<u32>,
    G: Fn() -> Vec<process::ProcessInfo>,
    H: Fn() -> HashMap<String, tmux::TmuxPane>,
{
    let current_pids = process_id_scanner();
    let current_tmux_epoch = TMUX_CHANGE_EPOCH.load(Ordering::Relaxed);

    let cache = SCAN_CACHE.get_or_init(|| Mutex::new(ScannerCache::default()));
    let mut guard = cache.lock().unwrap_or_else(|error| error.into_inner());

    let process_cache_hit = !guard.processes.is_empty() && guard.pid_fingerprint == current_pids;

    let mut process_scan_ms = 0u64;
    let processes = if process_cache_hit {
        guard.processes.clone()
    } else {
        let process_started = Instant::now();
        let fresh = process_scanner();
        process_scan_ms = process_started.elapsed().as_millis() as u64;
        guard.processes = fresh.clone();
        guard.pid_fingerprint = current_pids;
        fresh
    };

    let tmux_cache_hit = process_cache_hit
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

    (
        processes,
        pane_map,
        process_cache_hit,
        tmux_cache_hit,
        process_scan_ms,
        tmux_ms,
    )
}

/// Apply bidirectional hysteresis to a raw state reading.
pub(crate) fn apply_hysteresis(pid: u32, raw: SessionState) -> SessionState {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

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

    result
}

pub(crate) fn retain_state_trackers(active_pids: &[u32]) {
    let mut guard = STATE_TRACKERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

#[cfg(test)]
fn clear_scan_cache() {
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
        ActivityAttribution, ActivityConfidence, CliTool, SessionGroupKind,
    };
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    static SCAN_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static TEST_COMPACTION_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPLETED_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPACTION_SESSION_IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

    fn set_display_scan_compaction_hook(hook: Option<fn(&[RuntimeSession])>) {
        *DISPLAY_SCAN_COMPACTION_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = hook;
    }

    fn set_display_scan_completed_hook(hook: Option<fn(usize)>) {
        *DISPLAY_SCAN_COMPLETED_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = hook;
    }

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

    fn remove_state_tracker(pid: u32) {
        let mut guard = STATE_TRACKERS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(map) = guard.as_mut() {
            map.remove(&pid);
        }
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
        let _guard = SCAN_CACHE_TEST_LOCK.lock().expect("lock");
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
        assert_eq!(
            apply_hysteresis(900_001, SessionState::Idle),
            SessionState::Idle
        );
        remove_state_tracker(900_001);
    }
    #[test]
    fn hysteresis_holds_state_on_single_change() {
        let pid = 900_002;
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_switches_after_two_consecutive() {
        let pid = 900_003;
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Active
        );
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_works_in_both_directions() {
        let pid = 900_004;
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Active
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Active
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        remove_state_tracker(pid);
    }
    #[test]
    fn hysteresis_absorbs_alternating_readings() {
        let pid = 900_005;
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );
        remove_state_tracker(pid);
    }
    #[test]
    fn retain_state_trackers_cleans_up() {
        let pid = 900_006;
        apply_hysteresis(pid, SessionState::Idle);
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
        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || vec![42];
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            vec![process_info(42, "/dev/pts/1")]
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, AtomicOrdering::Relaxed);
            tmux_map("/dev/pts/1")
        };

        let now = Instant::now();
        let (_, _, process_hit_1, tmux_hit_1, _, _) =
            scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        let (_, _, process_hit_2, tmux_hit_2, _, _) = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert!(!process_hit_1);
        assert!(!tmux_hit_1);
        assert!(process_hit_2);
        assert!(tmux_hit_2);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(tmux_calls.load(AtomicOrdering::Relaxed), 1);
    }
    #[test]
    fn scanner_cache_invalidates_on_pid_change() {
        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let pid_scan_calls = AtomicUsize::new(0);
        let process_ids = || {
            let call = pid_scan_calls.fetch_add(1, AtomicOrdering::Relaxed);
            if call == 0 {
                vec![42]
            } else {
                vec![43]
            }
        };
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            vec![process_info(43, "/dev/pts/1")]
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
        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || vec![42];
        let process_scan = || {
            full_process_calls.fetch_add(1, AtomicOrdering::Relaxed);
            vec![process_info(42, "/dev/pts/1")]
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, AtomicOrdering::Relaxed);
            tmux_map("/dev/pts/1")
        };

        let now = Instant::now();
        let _ = scan_inputs_with_cache(now, &process_ids, &process_scan, &tmux_scan);
        notify_tmux_changed();
        let (_, _, process_hit, tmux_hit, _, _) = scan_inputs_with_cache(
            now + Duration::from_millis(100),
            &process_ids,
            &process_scan,
            &tmux_scan,
        );

        assert!(process_hit);
        assert!(!tmux_hit);
        assert_eq!(full_process_calls.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(tmux_calls.load(AtomicOrdering::Relaxed), 2);
    }
}
