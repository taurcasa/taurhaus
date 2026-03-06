//! Session scanner — detects Claude Code sessions running in tmux.
//!
//! Combines three detection strategies:
//! 1. Process scanning (ps + /proc) — find claude processes and their project paths
//! 2. tmux mapping — map terminal TTYs to tmux pane/window IDs
//! 3. Idle detection — check JSONL + subagent mtime to determine active vs idle
//!
//! State changes use bidirectional hysteresis: a transition (idle↔active)
//! only takes effect after 2 consecutive polls agree on the new state.
//! This eliminates flickering from transient signals in either direction.

pub mod cli_tool;
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

/// A detected CLI tool session with all available metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeSession {
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
    /// Session ID (from JSONL filename, if found).
    pub session_id: Option<String>,
    /// Path to the active JSONL transcript file (if found).
    pub jsonl_path: Option<String>,
    /// Confidence score for this session's current activity classification.
    #[serde(default)]
    pub activity_confidence: ActivityConfidence,
    /// Attribution quality for the current activity signal.
    #[serde(default)]
    pub activity_attribution: ActivityAttribution,
    /// Project has active session-file signal that could not be tied to this PID.
    #[serde(default)]
    pub project_unattributed_active: bool,
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

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
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
                keep_session_metadata: true,
            };
        }

        if deterministic_file_owner {
            return ActivityDecision {
                raw_state: SessionState::Active,
                confidence: ActivityConfidence::High,
                attribution: ActivityAttribution::Attributed,
                project_unattributed_active: false,
                keep_session_metadata: true,
            };
        }

        return ActivityDecision {
            raw_state: SessionState::Idle,
            confidence: ActivityConfidence::Low,
            attribution: ActivityAttribution::Unattributed,
            project_unattributed_active: true,
            keep_session_metadata: false,
        };
    }

    ActivityDecision {
        raw_state: SessionState::Idle,
        confidence: ActivityConfidence::Low,
        attribution: ActivityAttribution::None,
        project_unattributed_active: false,
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
pub fn scan_sessions() -> Vec<ClaudeSession> {
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
    let mut idle_ms = Duration::default();
    let mut process_signal_ms = Duration::default();
    let mut ownership_ms = Duration::default();

    let mut sessions: Vec<ClaudeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux = pane_map.get(&proc.tty);

            let idle_started = Instant::now();
            let idle_result = idle::detect_idle(&proc.project_path, proc.cli_tool);
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
            let process_active = match proc.cli_tool {
                CliTool::Claude => proc_io::is_process_active_hysteresis(proc.pid),
                CliTool::Gemini => proc_io::has_api_connections(proc.pid),
                CliTool::Codex => proc_io::is_process_active_hysteresis(proc.pid),
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

            // Hide shared session metadata when activity attribution is unavailable.
            let (session_id, jsonl_path) = if decision.keep_session_metadata {
                (idle_result.session_id, idle_result.jsonl_path)
            } else {
                (None, None)
            };

            // Apply bidirectional hysteresis
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

            ClaudeSession {
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
                session_id,
                jsonl_path,
                activity_confidence,
                activity_attribution,
                project_unattributed_active,
            }
        })
        .collect();

    // Deduplicate: when a tool runs via an fnm/node shim, both the shim
    // process and the native binary appear in `ps` output sharing the same
    // TTY.  Keep only one session per (tty, cli_tool) pair.
    //
    // We prefer the HIGHEST PID per group — the child process (native binary)
    // is the one that actually owns API sockets and does meaningful IO.
    // The shim (lower PID, parent) is just a launcher with no sockets.
    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    // Clean up stale PID entries from both trackers
    let active_pids: Vec<u32> = sessions.iter().map(|s| s.pid).collect();
    proc_io::retain_pids(&active_pids);
    retain_state_trackers(&active_pids);

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
    let mut fields = Map::new();
    fields.insert(
        "process_scan_ms".to_string(),
        json_number_u64(process_scan_ms),
    );
    fields.insert("tmux_ms".to_string(), json_number_u64(tmux_ms));
    fields.insert(
        "process_cache_hit".to_string(),
        Value::Bool(process_cache_hit),
    );
    fields.insert("tmux_cache_hit".to_string(), Value::Bool(tmux_cache_hit));
    fields.insert("classify_ms".to_string(), json_number_u64(classify_ms));
    fields.insert(
        "idle_ms".to_string(),
        json_number_u64(idle_ms.as_millis() as u64),
    );
    fields.insert(
        "process_signal_ms".to_string(),
        json_number_u64(process_signal_ms.as_millis() as u64),
    );
    fields.insert(
        "ownership_ms".to_string(),
        json_number_u64(ownership_ms.as_millis() as u64),
    );
    fields.insert("duration_ms".to_string(), json_number_u64(total_ms));
    fields.insert(
        "session_count".to_string(),
        Value::Number(serde_json::Number::from(sessions.len())),
    );
    crate::commands::logging::emit_global(
        "debug",
        "backend",
        "session_scanner.scan.completed",
        Some("Session scanner cycle completed".to_string()),
        fields,
    );

    sessions
}

/// Testable version of scan_sessions that accepts injectable functions.
pub fn scan_sessions_with<F, G, H>(
    process_scanner: &F,
    tmux_lister: &G,
    idle_detector: &H,
) -> Vec<ClaudeSession>
where
    F: Fn() -> Vec<process::ProcessInfo>,
    G: Fn() -> HashMap<String, tmux::TmuxPane>,
    H: Fn(&str) -> idle::IdleResult,
{
    let processes = process_scanner();
    let pane_map = tmux_lister();

    let mut sessions: Vec<ClaudeSession> = processes
        .into_iter()
        .map(|proc| {
            // Look up tmux pane by TTY
            let tmux = pane_map.get(&proc.tty);

            // Check idle state via JSONL mtime
            let idle_result = idle_detector(&proc.project_path);

            ClaudeSession {
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
                activity_confidence: ActivityConfidence::Low,
                activity_attribution: ActivityAttribution::None,
                project_unattributed_active: false,
            }
        })
        .collect();

    // Deduplicate: same logic as scan_sessions (see comment there)
    sessions.sort_by(|a, b| b.pid.cmp(&a.pid));
    let mut seen = std::collections::HashSet::<(String, CliTool)>::new();
    sessions.retain(|s| seen.insert((s.tty.clone(), s.cli_tool)));

    sessions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static SCAN_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

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
    fn claude_session_serializes_to_json() {
        let session = ClaudeSession {
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
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
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
    }

    #[test]
    fn claude_session_with_no_tmux_serializes() {
        let session = ClaudeSession {
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
            session_id: None,
            jsonl_path: None,
            activity_confidence: ActivityConfidence::Low,
            activity_attribution: ActivityAttribution::None,
            project_unattributed_active: false,
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["state"], "idle");
        assert!(json["tmux_session"].is_null());
        assert!(json["session_id"].is_null());
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
                }
            } else {
                idle::IdleResult {
                    state: SessionState::Idle,
                    session_id: None,
                    jsonl_path: None,
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
        assert_eq!(a.session_id.as_deref(), Some("sess-aaa"));

        let b = sessions
            .iter()
            .find(|s| s.pid == 200)
            .expect("proj-b session");
        assert_eq!(b.project_path, "/home/user/proj-b");
        assert_eq!(b.cli_tool, CliTool::Claude);
        assert!(b.tmux_session.is_none());
        assert!(b.tmux_pane.is_none());
        assert_eq!(b.state, SessionState::Idle);
        assert!(b.session_id.is_none());
    }

    #[test]
    fn scan_sessions_empty_when_no_processes() {
        let sessions = scan_sessions_with(&|| vec![], &|| HashMap::new(), &|_| idle::IdleResult {
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
        });
        assert!(sessions.is_empty());
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
