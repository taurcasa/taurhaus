//! Session scanner — detects CLI tool sessions running in tmux.
//!
//! Combines three detection strategies:
//! 1. Process scanning (ps + /proc) — find claude processes and their project paths
//! 2. tmux mapping — map terminal TTYs to tmux pane/window IDs
//! 3. Idle detection — check JSONL + subagent mtime to determine active vs idle
//!
//! State changes use bidirectional hysteresis: a transition (idle↔active)
//! only takes effect after 2 consecutive polls agree on the new state.
//! This eliminates flickering from transient signals in either direction.
//!
//! Warning:
//! - `DisplaySession` is the UI-safe view and intentionally strips transcript
//!   metadata such as `session_id` and `jsonl_path`.
//! - Coordination and other transcript-aware logic must use
//!   `RuntimeSession` via `scan_sessions_for_runtime()`.

pub mod cli_tool;
pub mod compaction_extractor;
pub mod compaction_watcher;
pub mod control;
pub mod idle;
pub mod proc_io;
pub mod process;
pub mod tmux;

pub use cli_tool::CliTool;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// State of a Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Claude is actively working (JSONL mtime < 5s ago).
    Active,
    /// Session is waiting for user input (JSONL mtime > 10s ago, process alive).
    Idle,
}

/// Confidence level for reported activity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivityConfidence {
    /// Process-level signal or deterministic file ownership.
    High,
    /// Project-scoped file signal used with single-session attribution.
    Medium,
    /// No direct attribution signal available.
    #[default]
    Low,
}

/// Attribution quality for the reported activity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActivityAttribution {
    /// Activity was attributed to this exact process/session.
    Attributed,
    /// Project shows activity, but this process cannot be proven as owner.
    Unattributed,
    /// No active signal observed.
    #[default]
    None,
}

/// Grouping metadata used by sidebar session indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionGroupKind {
    MeshTeam,
    #[default]
    Standalone,
}

/// A detected CLI tool session for UI/display consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplaySession {
    /// Process ID of the CLI tool.
    pub pid: u32,
    /// Absolute path to the project directory (from /proc/PID/cwd).
    pub project_path: String,
    /// Terminal device (e.g., "/dev/pts/2").
    pub tty: String,
    /// The full command line args.
    pub args: String,
    /// Which CLI tool this session belongs to.
    pub cli_tool: CliTool,
    /// tmux session name (if mapped).
    pub tmux_session: Option<String>,
    /// tmux window index (if mapped).
    pub tmux_window: Option<String>,
    /// tmux pane ID (e.g., "%0") (if mapped).
    pub tmux_pane: Option<String>,
    /// tmux window name (if mapped).
    pub tmux_window_name: Option<String>,
    /// Session state: Active or Idle.
    pub state: SessionState,
    /// Whether proc-level IO/network detection reported recent active work.
    #[serde(default)]
    pub recent_io: bool,
    /// Seconds since the latest session output file change, if known.
    #[serde(default)]
    pub last_output_age_secs: Option<u64>,
    /// Confidence score for this session's current activity classification.
    #[serde(default)]
    pub activity_confidence: ActivityConfidence,
    /// Attribution quality for the current activity signal.
    #[serde(default)]
    pub activity_attribution: ActivityAttribution,
    /// Project has active session-file signal that could not be tied to this PID.
    #[serde(default)]
    pub project_unattributed_active: bool,
    /// Grouping mode used by session indicators.
    #[serde(default)]
    pub group_kind: SessionGroupKind,
    /// Stable grouping key when the session belongs to a managed team.
    #[serde(default)]
    pub group_id: Option<String>,
    /// User-facing grouping label when the session belongs to a managed team.
    #[serde(default)]
    pub group_label: Option<String>,
    /// Managed team member name associated with this session.
    #[serde(default)]
    pub member_name: Option<String>,
}

/// A detected CLI tool session with runtime transcript metadata preserved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSession {
    pub pid: u32,
    pub project_path: String,
    pub tty: String,
    pub args: String,
    pub cli_tool: CliTool,
    pub tmux_session: Option<String>,
    pub tmux_window: Option<String>,
    pub tmux_pane: Option<String>,
    pub tmux_window_name: Option<String>,
    pub state: SessionState,
    pub session_id: Option<String>,
    pub jsonl_path: Option<String>,
    #[serde(default)]
    pub recent_io: bool,
    #[serde(default)]
    pub last_output_age_secs: Option<u64>,
    #[serde(default)]
    pub activity_confidence: ActivityConfidence,
    #[serde(default)]
    pub activity_attribution: ActivityAttribution,
    #[serde(default)]
    pub project_unattributed_active: bool,
    #[serde(default)]
    pub group_kind: SessionGroupKind,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub group_label: Option<String>,
    #[serde(default)]
    pub member_name: Option<String>,
}

impl From<RuntimeSession> for DisplaySession {
    fn from(session: RuntimeSession) -> Self {
        Self {
            pid: session.pid,
            project_path: session.project_path,
            tty: session.tty,
            args: session.args,
            cli_tool: session.cli_tool,
            tmux_session: session.tmux_session,
            tmux_window: session.tmux_window,
            tmux_pane: session.tmux_pane,
            tmux_window_name: session.tmux_window_name,
            state: session.state,
            recent_io: session.recent_io,
            last_output_age_secs: session.last_output_age_secs,
            activity_confidence: session.activity_confidence,
            activity_attribution: session.activity_attribution,
            project_unattributed_active: session.project_unattributed_active,
            group_kind: session.group_kind,
            group_id: session.group_id,
            group_label: session.group_label,
            member_name: session.member_name,
        }
    }
}

// ---------------------------------------------------------------------------
// Bidirectional state hysteresis
// ---------------------------------------------------------------------------

/// Per-PID state tracker for bidirectional hysteresis.
///
/// State changes only take effect after 2 consecutive polls agree on the new
/// state. This prevents flickering in both directions (idle→active and
/// active→idle).
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
#[cfg(test)]
static RUNTIME_IDLE_DETECTOR_OVERRIDE: OnceLock<
    Mutex<Option<fn(&process::ProcessInfo) -> idle::IdleResult>>,
> = OnceLock::new();
#[cfg(test)]
static DISPLAY_SCAN_COMPACTION_HOOK: OnceLock<Mutex<Option<fn(&[RuntimeSession])>>> =
    OnceLock::new();
#[cfg(test)]
static DISPLAY_SCAN_COMPLETED_HOOK: OnceLock<Mutex<Option<fn(usize)>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Default)]
struct ScanCompletionMetrics {
    process_scan_ms: u64,
    tmux_ms: u64,
    process_cache_hit: bool,
    tmux_cache_hit: bool,
    classify_ms: u64,
    idle_ms: u64,
    process_signal_ms: u64,
    ownership_ms: u64,
    total_ms: u64,
}

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

fn detect_runtime_idle_for_process(proc: &process::ProcessInfo) -> idle::IdleResult {
    #[cfg(test)]
    if let Some(detector) = RUNTIME_IDLE_DETECTOR_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .copied()
    {
        return detector(proc);
    }

    idle::detect_runtime_idle(&proc.project_path, proc.pid, proc.cli_tool)
}

fn process_display_scan_compaction(runtime_sessions: &[RuntimeSession]) {
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

fn finalize_display_scan(
    display_sessions: Vec<DisplaySession>,
    runtime_sessions_for_compaction: Option<&[RuntimeSession]>,
    metrics: ScanCompletionMetrics,
) -> Vec<DisplaySession> {
    if let Some(runtime_sessions) = runtime_sessions_for_compaction {
        process_display_scan_compaction(runtime_sessions);
    }

    let active_pids: Vec<u32> = display_sessions.iter().map(|s| s.pid).collect();
    proc_io::retain_pids(&active_pids);
    retain_state_trackers(&active_pids);
    emit_scan_completed(metrics, display_sessions.len());
    display_sessions
}

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
fn decode_daemon_session_response<T>(
    response: crate::daemon::protocol::DaemonResponse,
) -> Option<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if response.error.is_some() {
        return None;
    }

    match response.result {
        Some(value) => serde_json::from_value(value).ok(),
        None => Some(Vec::new()),
    }
}

#[cfg(target_os = "windows")]
fn scan_sessions_via_daemon<T>(method: &str) -> Option<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    const DAEMON_ADDR: &str = "127.0.0.1:17233";
    const DAEMON_TIMEOUT: Duration = Duration::from_millis(500);

    let request =
        crate::daemon::protocol::DaemonRequest::new("windows-session-scan", method, Value::Null)
            .with_auth(crate::daemon::auth::read_auth_token());

    let mut stream = TcpStream::connect(DAEMON_ADDR).ok()?;
    stream.set_nodelay(true).ok()?;
    stream.set_read_timeout(Some(DAEMON_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(DAEMON_TIMEOUT)).ok()?;

    let payload = serde_json::to_string(&request).ok()?;
    stream.write_all(payload.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.flush().ok()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.trim().is_empty() {
        return None;
    }

    let response = serde_json::from_str(&line).ok()?;
    decode_daemon_session_response(response)
}

#[cfg(target_os = "windows")]
fn scan_display_sessions_via_daemon() -> Option<Vec<DisplaySession>> {
    scan_sessions_via_daemon(crate::daemon::protocol::method::LIST_DISPLAY_SESSIONS)
}

#[cfg(target_os = "windows")]
fn scan_runtime_sessions_via_daemon() -> Option<Vec<RuntimeSession>> {
    scan_sessions_via_daemon(crate::daemon::protocol::method::LIST_RUNTIME_SESSIONS)
}

/// Notify scanner cache that tmux layout metadata likely changed.
///
/// Call this after tmux launch/stop operations so the next scan forces a
/// fresh `list-panes` read even when process PIDs are otherwise stable.
pub fn notify_tmux_changed() {
    TMUX_CHANGE_EPOCH.fetch_add(1, Ordering::Relaxed);
}

fn scan_inputs_with_cache<F, G, H>(
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
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());

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
            .is_some_and(|ts| now.duration_since(ts) < TMUX_CACHE_MAX_AGE);

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
///
/// Returns the state to report. Only changes from the previously reported
/// state when 2 consecutive raw readings agree on the new state.
fn apply_hysteresis(pid: u32, raw: SessionState) -> SessionState {
    let mut guard = STATE_TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);

    let result = match map.get(&pid) {
        Some(tracker) => {
            if raw == tracker.prev_raw && raw != tracker.reported {
                // Two consecutive readings of the new state → switch
                raw
            } else {
                // Hold the current reported state
                tracker.reported
            }
        }
        None => {
            // First observation for this PID — report as-is
            raw
        }
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

/// Remove stale PID entries from the state tracker.
fn retain_state_trackers(active_pids: &[u32]) {
    let mut guard = STATE_TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
    }
}

#[cfg(test)]
fn clear_scan_cache() {
    if let Some(cache) = SCAN_CACHE.get() {
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        *guard = ScannerCache::default();
    }
    TMUX_CHANGE_EPOCH.store(0, Ordering::Relaxed);
}

/// Compute a process's raw state from tool-specific signals.
///
/// For Claude/Gemini we keep the existing behavior: project-level file signal
/// OR process-level signal marks the process active.
///
/// Codex needs special handling for multi-session projects: the file signal is
/// project-scoped (shared transcript activity), so using it directly marks all
/// Codex sessions active when only one is working. When multiple Codex
/// sessions are present for a project, we ignore the shared file signal and
/// rely on per-PID IO activity to disambiguate each session.
struct ActivityDecision {
    raw_state: SessionState,
    confidence: ActivityConfidence,
    attribution: ActivityAttribution,
    project_unattributed_active: bool,
    #[cfg(test)]
    keep_session_metadata: bool,
}

fn compute_activity_decision(
    file_active: bool,
    process_active: bool,
    sessions_for_tool_in_project: usize,
    deterministic_file_owner: bool,
) -> ActivityDecision {
    if process_active {
        return ActivityDecision {
            raw_state: SessionState::Active,
            confidence: ActivityConfidence::High,
            attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            #[cfg(test)]
            keep_session_metadata: true,
        };
    }

    if file_active {
        if sessions_for_tool_in_project <= 1 {
            return ActivityDecision {
                raw_state: SessionState::Active,
                confidence: ActivityConfidence::Medium,
                attribution: ActivityAttribution::Attributed,
                project_unattributed_active: false,
                #[cfg(test)]
                keep_session_metadata: true,
            };
        }

        if deterministic_file_owner {
            return ActivityDecision {
                raw_state: SessionState::Active,
                confidence: ActivityConfidence::High,
                attribution: ActivityAttribution::Attributed,
                project_unattributed_active: false,
                #[cfg(test)]
                keep_session_metadata: true,
            };
        }

        return ActivityDecision {
            raw_state: SessionState::Idle,
            confidence: ActivityConfidence::Low,
            attribution: ActivityAttribution::Unattributed,
            project_unattributed_active: true,
            #[cfg(test)]
            keep_session_metadata: false,
        };
    }

    ActivityDecision {
        raw_state: SessionState::Idle,
        confidence: ActivityConfidence::Low,
        attribution: ActivityAttribution::None,
        project_unattributed_active: false,
        #[cfg(test)]
        keep_session_metadata: sessions_for_tool_in_project <= 1,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan for all running Claude Code sessions.
///
/// Orchestrates process scanning, tmux mapping, idle detection, and
/// proc IO activity tracking.
///
/// **Raw state** — Active if ANY of these is true (OR):
/// - **JSONL mtime**: main transcript modified < 5s ago (tool use, streaming)
/// - **Subagent mtime**: subagent file modified < 5s ago (compaction)
/// - **Proc IO** (Claude): rchar delta > 500 bytes for 2+ consecutive polls
/// - **TCP sockets** (Gemini only): ESTABLISHED connection to remote port 443
/// - **Codex (single session/project)**: session file mtime OR proc IO hysteresis
/// - **Codex (multi session/project)**: proc IO hysteresis only (to avoid
///   broadcasting one session's file activity to all Codex sessions)
///
/// **Reported state** — applies bidirectional hysteresis on top: a state
/// change only takes effect after 2 consecutive polls agree on the new state.
pub fn scan_sessions_for_display() -> Vec<DisplaySession> {
    #[cfg(target_os = "windows")]
    if let Some(display_sessions) = scan_display_sessions_via_daemon() {
        let runtime_sessions = scan_runtime_sessions_via_daemon();
        return finalize_display_scan(
            display_sessions,
            runtime_sessions.as_deref(),
            ScanCompletionMetrics::default(),
        );
    }

    let scan_started = Instant::now();
    let (processes, pane_map, process_cache_hit, tmux_cache_hit, process_scan_ms, tmux_ms) =
        scan_inputs_with_cache(
            scan_started,
            &process::scan_process_ids_cached,
            &process::scan_processes,
            &tmux::list_panes,
        );

    let mut sessions_per_project_tool: HashMap<(String, CliTool), usize> = HashMap::new();
    for proc in &processes {
        *sessions_per_project_tool
            .entry((proc.project_path.clone(), proc.cli_tool))
            .or_default() += 1;
    }

    let classify_started = Instant::now();
    let (sessions, idle_ms, process_signal_ms, ownership_ms) =
        classify_display_runtime_sessions_with(
            processes,
            pane_map,
            &sessions_per_project_tool,
            &detect_runtime_idle_for_process,
        );

    let classify_ms = classify_started.elapsed().as_millis() as u64;
    let total_ms = scan_started.elapsed().as_millis() as u64;
    tracing::debug!(
        process_scan_ms,
        tmux_ms,
        process_cache_hit,
        tmux_cache_hit,
        classify_ms,
        idle_ms = idle_ms.as_millis() as u64,
        process_signal_ms = process_signal_ms.as_millis() as u64,
        ownership_ms = ownership_ms.as_millis() as u64,
        total_ms,
        sessions = sessions.len(),
        "session_scanner metrics"
    );
    let display_sessions: Vec<DisplaySession> =
        sessions.iter().cloned().map(DisplaySession::from).collect();
    finalize_display_scan(
        display_sessions,
        Some(&sessions),
        ScanCompletionMetrics {
            process_scan_ms,
            tmux_ms,
            process_cache_hit,
            tmux_cache_hit,
            classify_ms,
            idle_ms: idle_ms.as_millis() as u64,
            process_signal_ms: process_signal_ms.as_millis() as u64,
            ownership_ms: ownership_ms.as_millis() as u64,
            total_ms,
        },
    )
}

fn classify_display_runtime_sessions_with<H>(
    processes: Vec<process::ProcessInfo>,
    pane_map: HashMap<String, tmux::TmuxPane>,
    sessions_per_project_tool: &HashMap<(String, CliTool), usize>,
    idle_detector: &H,
) -> (Vec<RuntimeSession>, Duration, Duration, Duration)
where
    H: Fn(&process::ProcessInfo) -> idle::IdleResult,
{
    let mut idle_ms = Duration::default();
    let mut process_signal_ms = Duration::default();
    let mut ownership_ms = Duration::default();

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux = pane_map.get(&proc.tty);

            let idle_started = Instant::now();
            let idle_result = idle_detector(&proc);
            idle_ms += idle_started.elapsed();
            let file_active = idle_result.state == SessionState::Active;
            let sessions_for_tool_in_project = sessions_per_project_tool
                .get(&(proc.project_path.clone(), proc.cli_tool))
                .copied()
                .unwrap_or(1);

            // Raw state from multiple signals (OR).
            // Claude: file mtime OR consecutive-poll IO hysteresis (sustained rchar).
            // Gemini: file mtime OR TCP socket to :443 (Gemini closes connections when idle).
            // Codex: per-PID IO hysteresis always; project-level file mtime only
            //   when this is the sole Codex session for the project.
            let process_signal_started = Instant::now();
            let (process_active, recent_io) = match proc.cli_tool {
                CliTool::Claude => {
                    let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                    (recent_io, recent_io)
                }
                CliTool::Gemini => (proc_io::has_api_connections(proc.pid), false),
                CliTool::Codex => {
                    let recent_io = proc_io::is_process_active_hysteresis(proc.pid);
                    (recent_io, recent_io)
                }
            };
            process_signal_ms += process_signal_started.elapsed();

            // Deterministic fallback in multi-session projects:
            // if file signal is active but process signal is quiet, check whether this
            // PID currently holds the session file open.
            let ownership_started = Instant::now();
            let deterministic_file_owner = file_active
                && !process_active
                && sessions_for_tool_in_project > 1
                && idle_result
                    .jsonl_path
                    .as_deref()
                    .is_some_and(|p| crate::platform::process_has_open_path(proc.pid, p));
            ownership_ms += ownership_started.elapsed();

            let decision = compute_activity_decision(
                file_active,
                process_active,
                sessions_for_tool_in_project,
                deterministic_file_owner,
            );

            // Keep runtime metadata intact here. DisplaySession strips it at the
            // type boundary, but scanner-side compaction watching still needs the
            // full `(session_id, jsonl_path)` pair even when UI attribution is
            // intentionally marked unattributed.
            let state = apply_hysteresis(proc.pid, decision.raw_state);
            let (activity_confidence, activity_attribution, project_unattributed_active) =
                if state == SessionState::Active {
                    (decision.confidence, decision.attribution, false)
                } else if decision.project_unattributed_active {
                    (
                        ActivityConfidence::Low,
                        ActivityAttribution::Unattributed,
                        true,
                    )
                } else {
                    (ActivityConfidence::Low, ActivityAttribution::None, false)
                };

            RuntimeSession {
                pid: proc.pid,
                project_path: proc.project_path,
                tty: proc.tty,
                args: proc.args,
                cli_tool: proc.cli_tool,
                tmux_session: tmux.map(|t| t.session_name.clone()),
                tmux_window: tmux.map(|t| t.window_index.clone()),
                tmux_pane: tmux.map(|t| t.pane_id.clone()),
                tmux_window_name: tmux.map(|t| t.window_name.clone()),
                state,
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
                recent_io,
                last_output_age_secs: idle_result.last_output_age_secs,
                activity_confidence,
                activity_attribution,
                project_unattributed_active,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
            }
        })
        .collect();

    // Deduplicate: when a tool runs via an fnm/node shim, both the shim
    // process and the native binary appear in `ps` output sharing the same
    // TTY. Keep only one session per (tty, cli_tool) pair.
    //
    // We prefer the HIGHEST PID per group — the child process (native binary)
    // is the one that actually owns API sockets and does meaningful IO.
    // The shim (lower PID, parent) is just a launcher with no sockets.
    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    (sessions, idle_ms, process_signal_ms, ownership_ms)
}

/// Scan for runtime reconciliation/session-id detection without hiding session metadata.
///
/// Coordination uses this path when it needs exact `(pane, tool) -> session_id`
/// correlation. Unlike the UI-facing `scan_sessions_for_display()`, this keeps session ids
/// even when activity attribution is ambiguous in multi-session projects.
pub fn scan_sessions_for_runtime() -> Vec<RuntimeSession> {
    #[cfg(target_os = "windows")]
    if let Some(sessions) = scan_runtime_sessions_via_daemon() {
        return sessions;
    }

    let scan_started = Instant::now();
    let (processes, pane_map, ..) = scan_inputs_with_cache(
        scan_started,
        &process::scan_process_ids_cached,
        &process::scan_processes,
        &tmux::list_panes,
    );

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux = pane_map.get(&proc.tty);
            let idle_result = detect_runtime_idle_for_process(&proc);

            RuntimeSession {
                pid: proc.pid,
                project_path: proc.project_path,
                tty: proc.tty,
                args: proc.args,
                cli_tool: proc.cli_tool,
                tmux_session: tmux.map(|t| t.session_name.clone()),
                tmux_window: tmux.map(|t| t.window_index.clone()),
                tmux_pane: tmux.map(|t| t.pane_id.clone()),
                tmux_window_name: tmux.map(|t| t.window_name.clone()),
                state: idle_result.state,
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
                recent_io: false,
                last_output_age_secs: idle_result.last_output_age_secs,
                activity_confidence: ActivityConfidence::Low,
                activity_attribution: ActivityAttribution::None,
                project_unattributed_active: false,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
            }
        })
        .collect();

    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    compaction_extractor::update_active_runtime_sessions(&sessions);
    sessions
}

/// Testable version of scan_sessions that accepts injectable functions.
pub fn scan_sessions_with<F, G, H>(
    process_scanner: &F,
    tmux_lister: &G,
    idle_detector: &H,
) -> Vec<DisplaySession>
where
    F: Fn() -> Vec<process::ProcessInfo>,
    G: Fn() -> HashMap<String, tmux::TmuxPane>,
    H: Fn(&str) -> idle::IdleResult,
{
    let processes = process_scanner();
    let pane_map = tmux_lister();

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            // Look up tmux pane by TTY
            let tmux = pane_map.get(&proc.tty);

            // Check idle state via JSONL mtime
            let idle_result = idle_detector(&proc.project_path);

            RuntimeSession {
                pid: proc.pid,
                project_path: proc.project_path,
                tty: proc.tty,
                args: proc.args,
                cli_tool: proc.cli_tool,
                tmux_session: tmux.map(|t| t.session_name.clone()),
                tmux_window: tmux.map(|t| t.window_index.clone()),
                tmux_pane: tmux.map(|t| t.pane_id.clone()),
                tmux_window_name: tmux.map(|t| t.window_name.clone()),
                state: idle_result.state,
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
                recent_io: false,
                last_output_age_secs: idle_result.last_output_age_secs,
                activity_confidence: ActivityConfidence::Low,
                activity_attribution: ActivityAttribution::None,
                project_unattributed_active: false,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
            }
        })
        .collect();

    // Deduplicate: same logic as scan_sessions (see comment there)
    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    sessions.into_iter().map(DisplaySession::from).collect()
}

#[cfg(test)]
fn scan_sessions_for_runtime_with<F, G>(process_scanner: &F, tmux_lister: &G) -> Vec<RuntimeSession>
where
    F: Fn() -> Vec<process::ProcessInfo>,
    G: Fn() -> HashMap<String, tmux::TmuxPane>,
{
    let processes = process_scanner();
    let pane_map = tmux_lister();

    let mut sessions: Vec<RuntimeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux = pane_map.get(&proc.tty);
            let idle_result = detect_runtime_idle_for_process(&proc);

            RuntimeSession {
                pid: proc.pid,
                project_path: proc.project_path,
                tty: proc.tty,
                args: proc.args,
                cli_tool: proc.cli_tool,
                tmux_session: tmux.map(|t| t.session_name.clone()),
                tmux_window: tmux.map(|t| t.window_index.clone()),
                tmux_pane: tmux.map(|t| t.pane_id.clone()),
                tmux_window_name: tmux.map(|t| t.window_name.clone()),
                state: idle_result.state,
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
                recent_io: false,
                last_output_age_secs: idle_result.last_output_age_secs,
                activity_confidence: ActivityConfidence::Low,
                activity_attribution: ActivityAttribution::None,
                project_unattributed_active: false,
                group_kind: SessionGroupKind::Standalone,
                group_id: None,
                group_label: None,
                member_name: None,
            }
        })
        .collect();

    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    sessions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    static SCAN_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static TEST_COMPACTION_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPLETED_SESSION_COUNT: AtomicUsize = AtomicUsize::new(0);
    static TEST_COMPACTION_SESSION_IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

    fn set_runtime_idle_detector_override(
        detector: Option<fn(&process::ProcessInfo) -> idle::IdleResult>,
    ) {
        *RUNTIME_IDLE_DETECTOR_OVERRIDE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = detector;
    }

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
        TEST_COMPACTION_SESSION_COUNT.store(sessions.len(), Ordering::SeqCst);
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
        TEST_COMPLETED_SESSION_COUNT.store(session_count, Ordering::SeqCst);
    }

    #[test]
    fn session_state_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&SessionState::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&SessionState::Idle).unwrap(),
            "\"idle\""
        );
    }

    #[test]
    fn runtime_session_serializes_to_json() {
        let session = RuntimeSession {
            pid: 1234,
            project_path: "/home/user/projects/foo".to_string(),
            tty: "/dev/pts/2".to_string(),
            args: "claude --dangerously-skip-permissions".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: Some("0".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%3".to_string()),
            tmux_window_name: Some("foo".to_string()),
            state: SessionState::Active,
            session_id: Some("abc-123".to_string()),
            jsonl_path: Some(
                "/home/user/.claude/projects/-home-user-projects-foo/abc-123.jsonl".to_string(),
            ),
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["pid"], 1234);
        assert_eq!(json["project_path"], "/home/user/projects/foo");
        assert_eq!(json["cli_tool"], "claude");
        assert_eq!(json["state"], "active");
        assert_eq!(json["tmux_pane"], "%3");
        assert_eq!(json["session_id"], "abc-123");
        assert_eq!(json["activity_confidence"], "high");
        assert_eq!(json["activity_attribution"], "attributed");
        assert_eq!(json["group_kind"], "standalone");
    }

    #[test]
    fn display_session_strips_runtime_metadata_on_serialize() {
        let session = DisplaySession {
            pid: 1234,
            project_path: "/home/user/projects/foo".to_string(),
            tty: "/dev/pts/2".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: SessionState::Idle,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::Low,
            activity_attribution: ActivityAttribution::None,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["state"], "idle");
        assert!(json["tmux_session"].is_null());
        assert!(json.get("session_id").is_none());
        assert!(json.get("jsonl_path").is_none());
    }

    #[test]
    fn runtime_session_sanitizes_to_display_session() {
        let runtime = RuntimeSession {
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
        };

        let display = DisplaySession::from(runtime);
        let json = serde_json::to_value(&display).unwrap();

        assert_eq!(display.pid, 42);
        assert_eq!(display.tmux_pane.as_deref(), Some("%7"));
        assert!(json.get("session_id").is_none());
        assert!(json.get("jsonl_path").is_none());
    }

    #[test]
    fn decode_daemon_display_session_response_returns_sessions() {
        // Regression: Windows host-side session consumers must accept daemon-
        // backed session lists because the local Windows scanner is stubbed.
        let session = DisplaySession {
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
            recent_io: false,
            last_output_age_secs: Some(1),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        };

        let decoded: Vec<DisplaySession> = decode_daemon_session_response(
            crate::daemon::protocol::DaemonResponse::ok("list", vec![session.clone()]),
        )
        .expect("daemon session list should decode");

        assert_eq!(decoded, vec![session]);
    }

    #[test]
    fn decode_daemon_session_response_rejects_daemon_errors() {
        let response = crate::daemon::protocol::DaemonResponse {
            id: "list".to_string(),
            result: None,
            error: Some(crate::daemon::protocol::DaemonError {
                code: "UNAVAILABLE".to_string(),
                message: "daemon unavailable".to_string(),
            }),
        };

        let decoded: Option<Vec<DisplaySession>> = decode_daemon_session_response(response);
        assert!(decoded.is_none());
    }

    #[test]
    fn finalize_display_scan_processes_runtime_compaction_and_emits_completion() {
        let _guard = SCAN_CACHE_TEST_LOCK.lock().expect("lock");
        TEST_COMPACTION_SESSION_COUNT.store(0, Ordering::SeqCst);
        TEST_COMPLETED_SESSION_COUNT.store(0, Ordering::SeqCst);
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
        assert_eq!(TEST_COMPACTION_SESSION_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(TEST_COMPLETED_SESSION_COUNT.load(Ordering::SeqCst), 1);
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
    fn scan_sessions_combines_all_sources() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 100,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    args: "claude --continue".to_string(),
                    cli_tool: CliTool::Claude,
                },
                process::ProcessInfo {
                    pid: 200,
                    project_path: "/home/user/proj-b".to_string(),
                    tty: "/dev/pts/5".to_string(),
                    args: "claude".to_string(),
                    cli_tool: CliTool::Claude,
                },
            ]
        };

        let mock_tmux = || {
            let mut map = HashMap::new();
            map.insert(
                "/dev/pts/1".to_string(),
                tmux::TmuxPane {
                    pane_id: "%0".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    window_index: "0".to_string(),
                    window_name: "proj-a".to_string(),
                    session_name: "main".to_string(),
                },
            );
            // pts/5 has no tmux mapping — simulate process outside tmux
            map
        };

        let mock_idle = |path: &str| -> idle::IdleResult {
            if path.contains("proj-a") {
                idle::IdleResult {
                    state: SessionState::Active,
                    session_id: Some("sess-aaa".to_string()),
                    jsonl_path: Some(
                        "/home/user/.claude/projects/proj-a/sess-aaa.jsonl".to_string(),
                    ),
                    last_output_age_secs: None,
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: None,
                    jsonl_path: None,
                    last_output_age_secs: None,
                }
            }
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 2);

        // Find sessions by PID (order may vary due to descending PID sort)
        let a = sessions
            .iter()
            .find(|s| s.pid == 100)
            .expect("proj-a session");
        assert_eq!(a.project_path, "/home/user/proj-a");
        assert_eq!(a.cli_tool, CliTool::Claude);
        assert_eq!(a.tmux_session.as_deref(), Some("main"));
        assert_eq!(a.tmux_pane.as_deref(), Some("%0"));
        assert_eq!(a.state, SessionState::Active);
        let b = sessions
            .iter()
            .find(|s| s.pid == 200)
            .expect("proj-b session");
        assert_eq!(b.project_path, "/home/user/proj-b");
        assert_eq!(b.cli_tool, CliTool::Claude);
        assert!(b.tmux_session.is_none());
        assert!(b.tmux_pane.is_none());
        assert_eq!(b.state, SessionState::Idle);
        let display_json = serde_json::to_value(a).unwrap();
        assert!(display_json.get("session_id").is_none());
        let display_json = serde_json::to_value(b).unwrap();
        assert!(display_json.get("session_id").is_none());
    }

    #[test]
    fn scan_sessions_empty_when_no_processes() {
        let sessions = scan_sessions_with(&|| vec![], &|| HashMap::new(), &|_| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        });
        assert!(sessions.is_empty());
    }

    #[test]
    fn scan_sessions_for_runtime_uses_distinct_codex_metadata_per_pid_in_multi_session_project() {
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 100,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/1".to_string(),
                    args: "codex".to_string(),
                    cli_tool: CliTool::Codex,
                },
                process::ProcessInfo {
                    pid: 200,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/2".to_string(),
                    args: "codex resume --last".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || {
            HashMap::from([
                (
                    "/dev/pts/1".to_string(),
                    tmux::TmuxPane {
                        pane_id: "%1".to_string(),
                        tty: "/dev/pts/1".to_string(),
                        window_index: "1".to_string(),
                        window_name: "proj-a".to_string(),
                        session_name: "main".to_string(),
                    },
                ),
                (
                    "/dev/pts/2".to_string(),
                    tmux::TmuxPane {
                        pane_id: "%2".to_string(),
                        tty: "/dev/pts/2".to_string(),
                        window_index: "2".to_string(),
                        window_name: "proj-a".to_string(),
                        session_name: "main".to_string(),
                    },
                ),
            ])
        };

        fn runtime_idle_by_pid(proc: &process::ProcessInfo) -> idle::IdleResult {
            assert_eq!(proc.project_path, "/home/user/proj-a");
            assert_eq!(proc.cli_tool, CliTool::Codex);
            if proc.pid == 100 {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: Some("rollout-123".to_string()),
                    jsonl_path: Some("/tmp/rollout-123.jsonl".to_string()),
                    last_output_age_secs: Some(42),
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: Some("rollout-456".to_string()),
                    jsonl_path: Some("/tmp/rollout-456.jsonl".to_string()),
                    last_output_age_secs: Some(41),
                }
            }
        }

        set_runtime_idle_detector_override(Some(runtime_idle_by_pid));
        let sessions = scan_sessions_for_runtime_with(&mock_processes, &mock_tmux);
        set_runtime_idle_detector_override(None);
        assert_eq!(sessions.len(), 2);
        let first = sessions.iter().find(|session| session.pid == 100).unwrap();
        let second = sessions.iter().find(|session| session.pid == 200).unwrap();
        assert_eq!(first.session_id.as_deref(), Some("rollout-123"));
        assert_eq!(second.session_id.as_deref(), Some("rollout-456"));
        assert_eq!(first.jsonl_path.as_deref(), Some("/tmp/rollout-123.jsonl"));
        assert_eq!(second.jsonl_path.as_deref(), Some("/tmp/rollout-456.jsonl"));
    }

    // -----------------------------------------------------------------------
    // Hysteresis unit tests
    // -----------------------------------------------------------------------

    /// Remove a single PID from the global state tracker.
    /// Using `retain_state_trackers(&[])` in tests is racy because it clears
    /// the ENTIRE map — concurrent tests lose their state mid-sequence.
    fn remove_state_tracker(pid: u32) {
        let mut guard = STATE_TRACKERS.lock().unwrap();
        if let Some(map) = guard.as_mut() {
            map.remove(&pid);
        }
    }

    #[test]
    fn hysteresis_first_observation_reports_raw() {
        // New PID → report whatever the raw state is
        assert_eq!(
            apply_hysteresis(900_001, SessionState::Idle),
            SessionState::Idle
        );
        // Clean up
        remove_state_tracker(900_001);
    }

    #[test]
    fn hysteresis_holds_state_on_single_change() {
        let pid = 900_002;

        // Establish baseline: idle
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        // Single active reading → still reports idle (held)
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );

        // Back to idle → still idle (no change ever happened)
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        remove_state_tracker(pid);
    }

    #[test]
    fn hysteresis_switches_after_two_consecutive() {
        let pid = 900_003;

        // Baseline: idle
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        // First active reading → held
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );

        // Second consecutive active → switch!
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Active
        );

        remove_state_tracker(pid);
    }

    #[test]
    fn hysteresis_works_in_both_directions() {
        let pid = 900_004;

        // Start idle
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        // Switch to active (2 consecutive)
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Idle
        );
        assert_eq!(
            apply_hysteresis(pid, SessionState::Active),
            SessionState::Active
        );

        // Now try to go back to idle — single reading is held
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Active
        );

        // Second consecutive idle → switch back
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        remove_state_tracker(pid);
    }

    #[test]
    fn hysteresis_absorbs_alternating_readings() {
        let pid = 900_005;

        // Baseline: idle
        assert_eq!(
            apply_hysteresis(pid, SessionState::Idle),
            SessionState::Idle
        );

        // Alternating: active, idle, active, idle → never switches
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

        // Verify it's tracked
        {
            let guard = STATE_TRACKERS.lock().unwrap();
            assert!(guard.as_ref().unwrap().contains_key(&pid));
        }

        // Remove only this test's PID (using remove_state_tracker to avoid
        // wiping concurrent tests' state via retain_state_trackers).
        remove_state_tracker(pid);

        // Should be gone
        {
            let guard = STATE_TRACKERS.lock().unwrap();
            assert!(!guard.as_ref().unwrap().contains_key(&pid));
        }
    }

    #[test]
    fn multi_session_file_signal_becomes_unattributed_without_owner() {
        let d = compute_activity_decision(true, false, 3, false);
        assert_eq!(d.raw_state, SessionState::Idle);
        assert_eq!(d.attribution, ActivityAttribution::Unattributed);
        assert!(d.project_unattributed_active);
        assert!(!d.keep_session_metadata);
    }

    #[test]
    fn single_session_file_signal_is_attributed_medium_confidence() {
        let d = compute_activity_decision(true, false, 1, false);
        assert_eq!(d.raw_state, SessionState::Active);
        assert_eq!(d.confidence, ActivityConfidence::Medium);
        assert_eq!(d.attribution, ActivityAttribution::Attributed);
        assert!(d.keep_session_metadata);
    }

    #[test]
    fn process_signal_is_high_confidence_attributed() {
        let d = compute_activity_decision(false, true, 3, false);
        assert_eq!(d.raw_state, SessionState::Active);
        assert_eq!(d.confidence, ActivityConfidence::High);
        assert_eq!(d.attribution, ActivityAttribution::Attributed);
    }

    #[test]
    fn deterministic_owner_resolves_multi_session_file_signal() {
        let d = compute_activity_decision(true, false, 3, true);
        assert_eq!(d.raw_state, SessionState::Active);
        assert_eq!(d.confidence, ActivityConfidence::High);
        assert_eq!(d.attribution, ActivityAttribution::Attributed);
        assert!(!d.project_unattributed_active);
        assert!(d.keep_session_metadata);
    }

    #[test]
    fn scan_sessions_deduplicates_same_tty_same_tool() {
        // Simulates the fnm shim scenario: node shim + native binary
        // both detected as Codex, same TTY (same tmux pane).
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 500,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    args: "node /path/to/bin/codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
                process::ProcessInfo {
                    pid: 501,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    args: "/path/to/codex/codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || {
            let mut map = HashMap::new();
            map.insert(
                "/dev/pts/3".to_string(),
                tmux::TmuxPane {
                    pane_id: "%5".to_string(),
                    tty: "/dev/pts/3".to_string(),
                    window_index: "2".to_string(),
                    window_name: "proj-a".to_string(),
                    session_name: "0".to_string(),
                },
            );
            map
        };

        let mock_idle = |_: &str| idle::IdleResult {
            state: SessionState::Idle,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(
            sessions.len(),
            1,
            "should deduplicate same-TTY same-tool processes"
        );
        assert_eq!(sessions[0].cli_tool, CliTool::Codex);
        assert_eq!(sessions[0].tmux_pane.as_deref(), Some("%5"));
        // Higher PID (native binary) wins over lower PID (shim)
        assert_eq!(sessions[0].pid, 501);
    }

    #[test]
    fn scan_sessions_keeps_different_tools_on_same_tty() {
        // Different tools on different TTYs should NOT be deduped.
        // (Same TTY + different tool would be unusual but should also be kept.)
        let mock_processes = || {
            vec![
                process::ProcessInfo {
                    pid: 600,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/4".to_string(),
                    args: "claude --continue".to_string(),
                    cli_tool: CliTool::Claude,
                },
                process::ProcessInfo {
                    pid: 700,
                    project_path: "/home/user/proj-a".to_string(),
                    tty: "/dev/pts/5".to_string(),
                    args: "codex --yolo".to_string(),
                    cli_tool: CliTool::Codex,
                },
            ]
        };

        let mock_tmux = || HashMap::new();
        let mock_idle = |_: &str| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
        };

        let sessions = scan_sessions_with(&mock_processes, &mock_tmux, &mock_idle);
        assert_eq!(sessions.len(), 2, "different tools should not be deduped");
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
        let mut map = HashMap::new();
        map.insert(
            tty.to_string(),
            tmux::TmuxPane {
                pane_id: "%1".to_string(),
                tty: tty.to_string(),
                window_index: "0".to_string(),
                window_name: "project".to_string(),
                session_name: "taurhaus".to_string(),
            },
        );
        map
    }

    #[test]
    fn scanner_cache_hit_reuses_process_and_tmux_data() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || vec![42];
        let process_scan = || {
            full_process_calls.fetch_add(1, Ordering::Relaxed);
            vec![process_info(42, "/dev/pts/1")]
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, Ordering::Relaxed);
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
        assert_eq!(full_process_calls.load(Ordering::Relaxed), 1);
        assert_eq!(tmux_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn scanner_cache_invalidates_on_pid_change() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let pid_scan_calls = AtomicUsize::new(0);
        let process_ids = || {
            let call = pid_scan_calls.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                vec![42]
            } else {
                vec![43]
            }
        };
        let process_scan = || {
            full_process_calls.fetch_add(1, Ordering::Relaxed);
            vec![process_info(43, "/dev/pts/1")]
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, Ordering::Relaxed);
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

        assert_eq!(full_process_calls.load(Ordering::Relaxed), 2);
        assert_eq!(tmux_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn scanner_cache_invalidates_on_tmux_change_epoch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let _lock = SCAN_CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        clear_scan_cache();

        let full_process_calls = AtomicUsize::new(0);
        let tmux_calls = AtomicUsize::new(0);
        let process_ids = || vec![42];
        let process_scan = || {
            full_process_calls.fetch_add(1, Ordering::Relaxed);
            vec![process_info(42, "/dev/pts/1")]
        };
        let tmux_scan = || {
            tmux_calls.fetch_add(1, Ordering::Relaxed);
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
        assert_eq!(full_process_calls.load(Ordering::Relaxed), 1);
        assert_eq!(tmux_calls.load(Ordering::Relaxed), 2);
    }
}
