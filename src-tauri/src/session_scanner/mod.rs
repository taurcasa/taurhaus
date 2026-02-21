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
use std::collections::HashMap;
use std::sync::Mutex;

/// State of a Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Claude is actively working (JSONL mtime < 5s ago).
    Active,
    /// Session is waiting for user input (JSONL mtime > 10s ago, process alive).
    Idle,
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

    map.insert(pid, StateTracker {
        reported: result,
        prev_raw: raw,
    });

    result
}

/// Remove stale PID entries from the state tracker.
fn retain_state_trackers(active_pids: &[u32]) {
    let mut guard = STATE_TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.retain(|pid, _| active_pids.contains(pid));
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
/// - **Proc IO**: rchar delta > 500 bytes for 2+ consecutive polls (thinking)
///
/// **Reported state** — applies bidirectional hysteresis on top: a state
/// change only takes effect after 2 consecutive polls agree on the new state.
pub fn scan_sessions() -> Vec<ClaudeSession> {
    let processes = process::scan_processes();
    let pane_map = tmux::list_panes();

    let sessions: Vec<ClaudeSession> = processes
        .into_iter()
        .map(|proc| {
            let tmux = pane_map.get(&proc.tty);
            let idle_result = idle::detect_idle(&proc.project_path, proc.cli_tool);

            // Raw state from all three signals (OR)
            let raw_state = if idle_result.state == SessionState::Active
                || proc_io::is_process_active(proc.pid)
            {
                SessionState::Active
            } else {
                SessionState::Idle
            };

            // Apply bidirectional hysteresis
            let state = apply_hysteresis(proc.pid, raw_state);

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
                session_id: idle_result.session_id,
                jsonl_path: idle_result.jsonl_path,
            }
        })
        .collect();

    // Clean up stale PID entries from both trackers
    let active_pids: Vec<u32> = sessions.iter().map(|s| s.pid).collect();
    proc_io::retain_pids(&active_pids);
    retain_state_trackers(&active_pids);

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

    processes
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
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            jsonl_path: Some("/home/user/.claude/projects/-home-user-projects-foo/abc-123.jsonl".to_string()),
        };

        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["pid"], 1234);
        assert_eq!(json["project_path"], "/home/user/projects/foo");
        assert_eq!(json["cli_tool"], "claude");
        assert_eq!(json["state"], "active");
        assert_eq!(json["tmux_pane"], "%3");
        assert_eq!(json["session_id"], "abc-123");
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
                    jsonl_path: Some("/home/user/.claude/projects/proj-a/sess-aaa.jsonl".to_string()),
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

        // First session: has tmux, is active
        let a = &sessions[0];
        assert_eq!(a.pid, 100);
        assert_eq!(a.project_path, "/home/user/proj-a");
        assert_eq!(a.cli_tool, CliTool::Claude);
        assert_eq!(a.tmux_session.as_deref(), Some("main"));
        assert_eq!(a.tmux_pane.as_deref(), Some("%0"));
        assert_eq!(a.state, SessionState::Active);
        assert_eq!(a.session_id.as_deref(), Some("sess-aaa"));

        // Second session: no tmux mapping, idle
        let b = &sessions[1];
        assert_eq!(b.pid, 200);
        assert_eq!(b.project_path, "/home/user/proj-b");
        assert_eq!(b.cli_tool, CliTool::Claude);
        assert!(b.tmux_session.is_none());
        assert!(b.tmux_pane.is_none());
        assert_eq!(b.state, SessionState::Idle);
        assert!(b.session_id.is_none());
    }

    #[test]
    fn scan_sessions_empty_when_no_processes() {
        let sessions = scan_sessions_with(
            &|| vec![],
            &|| HashMap::new(),
            &|_| idle::IdleResult {
                state: SessionState::Active,
                session_id: None,
                jsonl_path: None,
            },
        );
        assert!(sessions.is_empty());
    }

    // -----------------------------------------------------------------------
    // Hysteresis unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn hysteresis_first_observation_reports_raw() {
        // New PID → report whatever the raw state is
        assert_eq!(apply_hysteresis(900_001, SessionState::Idle), SessionState::Idle);
        // Clean up
        retain_state_trackers(&[]);
    }

    #[test]
    fn hysteresis_holds_state_on_single_change() {
        let pid = 900_002;

        // Establish baseline: idle
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        // Single active reading → still reports idle (held)
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Idle);

        // Back to idle → still idle (no change ever happened)
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        retain_state_trackers(&[]);
    }

    #[test]
    fn hysteresis_switches_after_two_consecutive() {
        let pid = 900_003;

        // Baseline: idle
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        // First active reading → held
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Idle);

        // Second consecutive active → switch!
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Active);

        retain_state_trackers(&[]);
    }

    #[test]
    fn hysteresis_works_in_both_directions() {
        let pid = 900_004;

        // Start idle
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        // Switch to active (2 consecutive)
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Idle);
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Active);

        // Now try to go back to idle — single reading is held
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Active);

        // Second consecutive idle → switch back
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        retain_state_trackers(&[]);
    }

    #[test]
    fn hysteresis_absorbs_alternating_readings() {
        let pid = 900_005;

        // Baseline: idle
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        // Alternating: active, idle, active, idle → never switches
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Idle);
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);
        assert_eq!(apply_hysteresis(pid, SessionState::Active), SessionState::Idle);
        assert_eq!(apply_hysteresis(pid, SessionState::Idle), SessionState::Idle);

        retain_state_trackers(&[]);
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

        // Retain only other PIDs
        retain_state_trackers(&[1]);

        // Should be gone
        {
            let guard = STATE_TRACKERS.lock().unwrap();
            assert!(!guard.as_ref().unwrap().contains_key(&pid));
        }
    }
}
