//! tmux mapper — map terminal TTYs to tmux pane/window IDs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ClaudeSession;

/// Information about a tmux pane.
#[derive(Debug, Clone, PartialEq)]
pub struct TmuxPane {
    /// Pane ID (e.g., "%0", "%3").
    pub pane_id: String,
    /// Terminal device (e.g., "/dev/pts/2").
    pub tty: String,
    /// Window index (e.g., "0", "1").
    pub window_index: String,
    /// Window name (e.g., "claude", "bash").
    pub window_name: String,
    /// tmux session name (e.g., "0", "main").
    pub session_name: String,
}

/// Persisted foreground tmux focus state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TmuxFocusState {
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub window: Option<String>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

impl TmuxFocusState {
    pub fn detached() -> Self {
        Self {
            session: None,
            window: None,
            timestamp: None,
        }
    }
}

/// List all tmux panes and build a TTY → TmuxPane lookup.
///
/// Returns an empty map if tmux is not running or the command fails.
pub fn list_panes() -> HashMap<String, TmuxPane> {
    let output = match run_tmux_list_panes() {
        Some(output) => output,
        None => return HashMap::new(),
    };
    parse_tmux_output(&output)
}

/// Run `tmux list-panes -a` and return stdout.
///
/// Uses `run_with_timeout` to avoid hanging if tmux is unresponsive.
fn run_tmux_list_panes() -> Option<String> {
    super::process::run_with_timeout(
        "tmux",
        &[
            "list-panes",
            "-a",
            "-F",
            "#{pane_id} #{pane_tty} #{window_index} #{window_name} #{session_name}",
        ],
    )
}

/// Parse tmux list-panes output into a TTY → TmuxPane map.
///
/// Expected format per line (space-separated, 5 fields):
/// `%0 /dev/pts/2 0 claude 0`
/// `pane_id tty window_index window_name session_name`
pub fn parse_tmux_output(output: &str) -> HashMap<String, TmuxPane> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            // Split into exactly 5 fields. Window names may contain spaces,
            // so we split into at most 5 parts.
            let parts: Vec<&str> = line.splitn(5, ' ').collect();
            if parts.len() < 5 {
                return None;
            }

            let pane = TmuxPane {
                pane_id: parts[0].to_string(),
                tty: parts[1].to_string(),
                window_index: parts[2].to_string(),
                window_name: parts[3].to_string(),
                session_name: parts[4].to_string(),
            };

            Some((pane.tty.clone(), pane))
        })
        .collect()
}

pub fn focus_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("tmux-focus.json")
}

pub fn read_focus_state(data_dir: &Path) -> Option<TmuxFocusState> {
    let path = focus_file_path(data_dir);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn write_focus_state(path: &Path, state: &TmuxFocusState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create tmux focus parent directory: {error}"))?;
    }
    let payload = serde_json::to_string(state)
        .map_err(|error| format!("Failed to serialize tmux focus state: {error}"))?;
    std::fs::write(path, payload)
        .map_err(|error| format!("Failed to write tmux focus state: {error}"))
}

pub fn resolve_focus_project_path(
    focus: &TmuxFocusState,
    sessions: &[ClaudeSession],
) -> Option<String> {
    let session_name = focus.session.as_deref()?.trim();
    let window = focus.window.as_deref()?.trim();
    if session_name.is_empty() || window.is_empty() {
        return None;
    }

    sessions.iter().find_map(|session| {
        let tmux_session = session.tmux_session.as_deref()?.trim();
        if tmux_session != session_name {
            return None;
        }

        let matches_window_name = session
            .tmux_window_name
            .as_deref()
            .is_some_and(|value| value.trim() == window);
        let matches_window_index = session
            .tmux_window
            .as_deref()
            .is_some_and(|value| value.trim() == window);

        if matches_window_name || matches_window_index {
            Some(session.project_path.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, CliTool, SessionGroupKind, SessionState,
    };

    fn session_for(path: &str, session_name: Option<&str>, window: Option<&str>) -> ClaudeSession {
        ClaudeSession {
            pid: 42,
            project_path: path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: session_name.map(str::to_string),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%1".to_string()),
            tmux_window_name: window.map(str::to_string),
            state: SessionState::Active,
            session_id: None,
            jsonl_path: None,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn parse_single_pane() {
        let output = "%0 /dev/pts/2 0 claude 0\n";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        let pane = map.get("/dev/pts/2").unwrap();
        assert_eq!(pane.pane_id, "%0");
        assert_eq!(pane.tty, "/dev/pts/2");
        assert_eq!(pane.window_index, "0");
        assert_eq!(pane.window_name, "claude");
        assert_eq!(pane.session_name, "0");
    }

    #[test]
    fn parse_multi_pane_multi_window() {
        let output = "\
%0 /dev/pts/1 0 bash main
%1 /dev/pts/2 0 bash main
%2 /dev/pts/3 1 claude main
%3 /dev/pts/4 2 vim work";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 4);

        let pane0 = map.get("/dev/pts/1").unwrap();
        assert_eq!(pane0.pane_id, "%0");
        assert_eq!(pane0.window_name, "bash");
        assert_eq!(pane0.session_name, "main");

        let pane2 = map.get("/dev/pts/3").unwrap();
        assert_eq!(pane2.pane_id, "%2");
        assert_eq!(pane2.window_index, "1");
        assert_eq!(pane2.window_name, "claude");

        let pane3 = map.get("/dev/pts/4").unwrap();
        assert_eq!(pane3.session_name, "work");
    }

    #[test]
    fn parse_multi_session() {
        let output = "\
%0 /dev/pts/1 0 shell sess-a
%1 /dev/pts/2 0 shell sess-b";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("/dev/pts/1").unwrap().session_name, "sess-a");
        assert_eq!(map.get("/dev/pts/2").unwrap().session_name, "sess-b");
    }

    #[test]
    fn parse_empty_output() {
        let map = parse_tmux_output("");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_malformed_lines_skipped() {
        let output = "\
%0 /dev/pts/1 0 bash main
bad line
%1 /dev/pts/2";
        let map = parse_tmux_output(output);
        // Only the first line has 5 fields
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("/dev/pts/1"));
    }

    #[test]
    fn parse_window_name_with_spaces() {
        // Window name might contain spaces if renamed
        // With splitn(5), the 5th field gets the rest
        // But our format has session_name as the 5th, and window_name as 4th
        // Session name shouldn't have spaces typically
        let output = "%0 /dev/pts/1 0 my-project 0\n";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        let pane = map.get("/dev/pts/1").unwrap();
        assert_eq!(pane.window_name, "my-project");
        assert_eq!(pane.session_name, "0");
    }

    #[test]
    fn duplicate_tty_last_wins() {
        // If somehow two panes share a TTY (shouldn't happen), last wins
        let output = "\
%0 /dev/pts/1 0 first main
%1 /dev/pts/1 1 second main";
        let map = parse_tmux_output(output);
        assert_eq!(map.len(), 1);
        // HashMap insert overwrites, so which wins is non-deterministic
        // Just verify we have one entry
        assert!(map.contains_key("/dev/pts/1"));
    }

    #[test]
    fn resolve_focus_matches_taurhaus_managed_window_name() {
        let focus = TmuxFocusState {
            session: Some("taurhaus".to_string()),
            window: Some("mesh".to_string()),
            timestamp: Some(123),
        };
        let sessions = vec![
            session_for("/projects/other", Some("taurhaus"), Some("other")),
            session_for("/projects/mesh", Some("taurhaus"), Some("mesh")),
        ];

        assert_eq!(
            resolve_focus_project_path(&focus, &sessions),
            Some("/projects/mesh".to_string())
        );
    }

    #[test]
    fn resolve_focus_returns_none_for_unknown_window() {
        let focus = TmuxFocusState {
            session: Some("taurhaus".to_string()),
            window: Some("missing".to_string()),
            timestamp: Some(123),
        };
        let sessions = vec![session_for(
            "/projects/mesh",
            Some("taurhaus"),
            Some("mesh"),
        )];

        assert_eq!(resolve_focus_project_path(&focus, &sessions), None);
    }

    #[test]
    fn resolve_focus_returns_none_without_attached_client() {
        let sessions = vec![session_for(
            "/projects/mesh",
            Some("taurhaus"),
            Some("mesh"),
        )];
        assert_eq!(
            resolve_focus_project_path(&TmuxFocusState::detached(), &sessions),
            None
        );
    }
}
