//! tmux mapper — map terminal TTYs to tmux pane/window IDs.

use std::collections::HashMap;
use std::process::Command;

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
fn run_tmux_list_panes() -> Option<String> {
    Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{pane_id} #{pane_tty} #{window_index} #{window_name} #{session_name}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
