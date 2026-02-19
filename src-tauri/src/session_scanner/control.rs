//! tmux control — launch, stop, and navigate to Claude Code sessions.

use std::path::Path;
use std::process::Command;

use crate::daemon::protocol::LaunchMode;

/// Launch a Claude Code session in a new tmux window.
///
/// Creates a new tmux window named after the project directory,
/// then sends the cd + claude command to it.
///
/// Returns `(window_name, pane_id)` on success.
pub fn launch_in_tmux(project_path: &str, mode: LaunchMode) -> Result<(String, String), String> {
    // Validate project path
    if !Path::new(project_path).is_dir() {
        return Err(format!("Project path does not exist: {project_path}"));
    }

    // Detect active tmux session
    let tmux_session = detect_tmux_session()?;

    // Window name: last component of project path
    let window_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "claude".to_string());

    // Create new tmux window
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-n",
            &window_name,
            "-t",
            &tmux_session,
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .output()
        .map_err(|e| format!("Failed to create tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux new-window failed: {stderr}"));
    }

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Build the claude command
    let claude_cmd = build_claude_command(mode);

    // Send the cd + claude command to the new pane
    let keys = format!("cd {project_path} && {claude_cmd}");
    run_tmux_send_keys(&pane_id, &keys)?;

    Ok((window_name, pane_id))
}

/// Stop a Claude Code session by sending /exit to the tmux pane.
pub fn stop_session(tmux_pane: &str) -> Result<(), String> {
    // Send /exit (graceful Claude Code exit command)
    run_tmux_send_keys(tmux_pane, "/exit")?;
    Ok(())
}

/// Navigate to a specific tmux session/window/pane.
pub fn navigate_to_pane(
    tmux_session: &str,
    tmux_window: &str,
    tmux_pane: &str,
) -> Result<(), String> {
    // Select the window
    let target = format!("{tmux_session}:{tmux_window}");
    let output = Command::new("tmux")
        .args(["select-window", "-t", &target])
        .output()
        .map_err(|e| format!("Failed to select tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux select-window failed: {stderr}"));
    }

    // Select the pane
    let output = Command::new("tmux")
        .args(["select-pane", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("Failed to select tmux pane: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux select-pane failed: {stderr}"));
    }

    Ok(())
}

/// Build the claude command string for a given launch mode.
pub fn build_claude_command(mode: LaunchMode) -> String {
    match mode {
        LaunchMode::Continue => {
            "claude --dangerously-skip-permissions --continue".to_string()
        }
        LaunchMode::Fresh => "claude --dangerously-skip-permissions".to_string(),
        LaunchMode::Resume => {
            "claude --dangerously-skip-permissions --resume".to_string()
        }
    }
}

/// Detect the first available tmux session name.
fn detect_tmux_session() -> Result<String, String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .map_err(|e| format!("tmux not running or not available: {e}"))?;

    if !output.status.success() {
        return Err("tmux is not running. Start tmux first.".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "No tmux sessions found.".to_string())
}

/// Send keys to a tmux pane (with Enter).
fn run_tmux_send_keys(pane: &str, keys: &str) -> Result<(), String> {
    let output = Command::new("tmux")
        .args(["send-keys", "-t", pane, keys, "Enter"])
        .output()
        .map_err(|e| format!("Failed to send keys to tmux pane {pane}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux send-keys failed: {stderr}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_continue_command() {
        assert_eq!(
            build_claude_command(LaunchMode::Continue),
            "claude --dangerously-skip-permissions --continue"
        );
    }

    #[test]
    fn build_fresh_command() {
        assert_eq!(
            build_claude_command(LaunchMode::Fresh),
            "claude --dangerously-skip-permissions"
        );
    }

    #[test]
    fn build_resume_command() {
        assert_eq!(
            build_claude_command(LaunchMode::Resume),
            "claude --dangerously-skip-permissions --resume"
        );
    }

    #[test]
    fn launch_rejects_nonexistent_path() {
        let result = launch_in_tmux("/nonexistent/path/12345", LaunchMode::Continue);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn launch_mode_serializes() {
        assert_eq!(
            serde_json::to_string(&LaunchMode::Continue).unwrap(),
            "\"continue\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchMode::Fresh).unwrap(),
            "\"fresh\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchMode::Resume).unwrap(),
            "\"resume\""
        );
    }

    #[test]
    fn launch_mode_deserializes() {
        let c: LaunchMode = serde_json::from_str("\"continue\"").unwrap();
        assert_eq!(c, LaunchMode::Continue);
        let f: LaunchMode = serde_json::from_str("\"fresh\"").unwrap();
        assert_eq!(f, LaunchMode::Fresh);
        let r: LaunchMode = serde_json::from_str("\"resume\"").unwrap();
        assert_eq!(r, LaunchMode::Resume);
    }
}
