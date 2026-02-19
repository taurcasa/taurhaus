//! Session scanner — detects Claude Code sessions running in tmux.
//!
//! Combines three detection strategies:
//! 1. Process scanning (ps + /proc) — find claude processes and their project paths
//! 2. tmux mapping — map terminal TTYs to tmux pane/window IDs
//! 3. Idle detection — check JSONL transcript mtime to determine active vs idle

pub mod idle;
pub mod process;
pub mod tmux;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State of a Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Claude is actively working (JSONL mtime < 5s ago).
    Active,
    /// Session is waiting for user input (JSONL mtime > 10s ago, process alive).
    Idle,
}

/// A detected Claude Code session with all available metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeSession {
    /// Process ID of the claude CLI.
    pub pid: u32,
    /// Absolute path to the project directory (from /proc/PID/cwd).
    pub project_path: String,
    /// Terminal device (e.g., "/dev/pts/2").
    pub tty: String,
    /// The full command line args.
    pub args: String,
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
    /// Claude Code session ID (from JSONL filename, if found).
    pub session_id: Option<String>,
    /// Path to the active JSONL transcript file (if found).
    pub jsonl_path: Option<String>,
}

/// Scan for all running Claude Code sessions.
///
/// Orchestrates process scanning, tmux mapping, and idle detection
/// into a single comprehensive scan.
pub fn scan_sessions() -> Vec<ClaudeSession> {
    scan_sessions_with(
        &process::scan_processes,
        &tmux::list_panes,
        &idle::detect_idle,
    )
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
                },
                process::ProcessInfo {
                    pid: 200,
                    project_path: "/home/user/proj-b".to_string(),
                    tty: "/dev/pts/5".to_string(),
                    args: "claude".to_string(),
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
        assert_eq!(a.tmux_session.as_deref(), Some("main"));
        assert_eq!(a.tmux_pane.as_deref(), Some("%0"));
        assert_eq!(a.state, SessionState::Active);
        assert_eq!(a.session_id.as_deref(), Some("sess-aaa"));

        // Second session: no tmux mapping, idle
        let b = &sessions[1];
        assert_eq!(b.pid, 200);
        assert_eq!(b.project_path, "/home/user/proj-b");
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
}
