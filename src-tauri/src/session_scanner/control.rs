//! tmux control — launch, stop, and navigate to CLI tool sessions.

use std::path::Path;
use std::process::Command;

use crate::daemon::protocol::LaunchMode;
use crate::session_scanner::cli_tool::{self, CliTool};

/// Launch a CLI tool session in a new tmux window.
///
/// Creates a new tmux window named after the project directory,
/// then sends the cd + tool command to it.
///
/// Returns `(window_name, pane_id)` on success.
pub fn launch_in_tmux(
    project_path: &str,
    mode: LaunchMode,
    tool: CliTool,
) -> Result<(String, String), String> {
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

    // Create new tmux window (trailing colon = next available index in session)
    let target = format!("{tmux_session}:");
    let output = Command::new("tmux")
        .args([
            "new-window",
            "-n",
            &window_name,
            "-t",
            &target,
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

    // Build the tool-specific command
    let tool_cmd = build_launch_command(tool, mode);

    // Send the cd + tool command to the new pane.
    // Shell-escape the path to prevent injection via crafted directory names.
    let escaped_path = shell_escape(project_path);
    let keys = format!("cd {escaped_path} && {tool_cmd}");
    run_tmux_send_keys(&pane_id, &keys)?;

    Ok((window_name, pane_id))
}

/// Stop a CLI tool session by sending the exit command to the tmux pane.
///
/// After the tool exits, polls for the process to terminate (pane returns
/// to shell), then kills the pane to clean up. If it's the last pane in the
/// window, tmux automatically closes the window too.
pub fn stop_session(tmux_pane: &str, tool: CliTool) -> Result<(), String> {
    let config = cli_tool::config_for(tool);
    run_tmux_send_keys(tmux_pane, config.exit_command)?;

    // Poll for exit, then kill the pane. Background thread so we don't block IPC.
    let pane = tmux_pane.to_string();
    std::thread::spawn(move || {
        const POLL_MS: u64 = 200;
        const TIMEOUT_MS: u64 = 5000;
        let mut elapsed = 0u64;

        tracing::info!(pane = %pane, "stop_session: polling for exit");

        // Poll until the pane's command becomes a shell (Claude exited)
        // or the pane disappears, or we hit the timeout.
        while elapsed < TIMEOUT_MS {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
            elapsed += POLL_MS;

            match pane_current_command(&pane) {
                Some(cmd) if is_shell(&cmd) => {
                    tracing::info!(pane = %pane, cmd = %cmd, elapsed_ms = elapsed, "stop_session: shell detected, killing pane");
                    break;
                }
                None => {
                    tracing::info!(pane = %pane, "stop_session: pane already gone");
                    return;
                }
                Some(cmd) => {
                    tracing::debug!(pane = %pane, cmd = %cmd, elapsed_ms = elapsed, "stop_session: still running");
                }
            }
        }

        if elapsed >= TIMEOUT_MS {
            tracing::warn!(pane = %pane, "stop_session: timeout, killing pane anyway");
        }

        // Kill the pane (noop if already gone)
        let result = Command::new("tmux")
            .args(["kill-pane", "-t", &pane])
            .output();
        tracing::info!(pane = %pane, success = ?result.as_ref().map(|o| o.status.success()), "stop_session: kill-pane result");
    });

    Ok(())
}

/// Get the current command running in a tmux pane.
fn pane_current_command(pane: &str) -> Option<String> {
    Command::new("tmux")
        .args(["display-message", "-p", "-t", pane, "#{pane_current_command}"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check if a command name is a shell (meaning Claude Code has exited).
fn is_shell(cmd: &str) -> bool {
    matches!(cmd, "zsh" | "bash" | "fish" | "sh" | "dash")
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

/// Build the launch command string for a given tool and launch mode.
pub fn build_launch_command(tool: CliTool, mode: LaunchMode) -> String {
    match tool {
        CliTool::Claude => match mode {
            LaunchMode::Continue => {
                "claude --dangerously-skip-permissions --continue".to_string()
            }
            LaunchMode::Fresh => "claude --dangerously-skip-permissions".to_string(),
            LaunchMode::Resume => {
                "claude --dangerously-skip-permissions --resume".to_string()
            }
        },
        CliTool::Codex => match mode {
            LaunchMode::Continue => "codex --full-auto".to_string(),
            LaunchMode::Fresh => "codex --full-auto".to_string(),
            LaunchMode::Resume => "codex resume --last".to_string(),
        },
        CliTool::Gemini => match mode {
            LaunchMode::Continue => "gemini --sandbox --resume".to_string(),
            LaunchMode::Fresh => "gemini --sandbox".to_string(),
            LaunchMode::Resume => "gemini --sandbox --resume".to_string(),
        },
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

/// Escape a string for safe use in a POSIX shell command.
///
/// Wraps the string in single quotes and escapes any embedded single quotes
/// using the `'\''` technique (end quote, escaped quote, start quote).
/// This prevents shell interpretation of spaces, semicolons, backticks,
/// `$()`, and all other metacharacters.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Send keys to a tmux pane (with Enter).
///
/// Sends the text first, waits briefly for the terminal to process it,
/// then sends Enter separately. Without this delay, fast terminals can
/// receive Enter before the text is fully rendered in the prompt.
fn run_tmux_send_keys(pane: &str, keys: &str) -> Result<(), String> {
    // Send the text
    let output = Command::new("tmux")
        .args(["send-keys", "-t", pane, keys])
        .output()
        .map_err(|e| format!("Failed to send keys to tmux pane {pane}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux send-keys failed: {stderr}"));
    }

    // Brief pause so the terminal processes the text before Enter (matches aitx @ 200ms)
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Send Enter
    let output = Command::new("tmux")
        .args(["send-keys", "-t", pane, "Enter"])
        .output()
        .map_err(|e| format!("Failed to send Enter to tmux pane {pane}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux send-keys Enter failed: {stderr}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Claude command tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_claude_continue_command() {
        assert_eq!(
            build_launch_command(CliTool::Claude, LaunchMode::Continue),
            "claude --dangerously-skip-permissions --continue"
        );
    }

    #[test]
    fn build_claude_fresh_command() {
        assert_eq!(
            build_launch_command(CliTool::Claude, LaunchMode::Fresh),
            "claude --dangerously-skip-permissions"
        );
    }

    #[test]
    fn build_claude_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Claude, LaunchMode::Resume),
            "claude --dangerously-skip-permissions --resume"
        );
    }

    // -----------------------------------------------------------------------
    // Codex command tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_codex_fresh_command() {
        assert_eq!(
            build_launch_command(CliTool::Codex, LaunchMode::Fresh),
            "codex --full-auto"
        );
    }

    #[test]
    fn build_codex_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Codex, LaunchMode::Resume),
            "codex resume --last"
        );
    }

    // -----------------------------------------------------------------------
    // Gemini command tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_gemini_fresh_command() {
        assert_eq!(
            build_launch_command(CliTool::Gemini, LaunchMode::Fresh),
            "gemini --sandbox"
        );
    }

    #[test]
    fn build_gemini_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Gemini, LaunchMode::Resume),
            "gemini --sandbox --resume"
        );
    }

    #[test]
    fn launch_rejects_nonexistent_path() {
        let result = launch_in_tmux("/nonexistent/path/12345", LaunchMode::Continue, CliTool::Claude);
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

    // -----------------------------------------------------------------------
    // Shell escaping tests
    // -----------------------------------------------------------------------

    #[test]
    fn shell_escape_simple_path() {
        assert_eq!(shell_escape("/home/user/project"), "'/home/user/project'");
    }

    #[test]
    fn shell_escape_path_with_spaces() {
        assert_eq!(
            shell_escape("/home/user/my project"),
            "'/home/user/my project'"
        );
    }

    #[test]
    fn shell_escape_path_with_special_chars() {
        // Semicolons, backticks, dollar signs — all neutralized by single quotes
        assert_eq!(
            shell_escape("/tmp/foo; echo pwned"),
            "'/tmp/foo; echo pwned'"
        );
        assert_eq!(
            shell_escape("/tmp/foo$(rm -rf /)"),
            "'/tmp/foo$(rm -rf /)'"
        );
        assert_eq!(
            shell_escape("/tmp/foo`id`bar"),
            "'/tmp/foo`id`bar'"
        );
    }

    #[test]
    fn shell_escape_path_with_single_quotes() {
        // Single quotes within the path get the '\'' treatment
        assert_eq!(
            shell_escape("/home/user/it's a project"),
            "'/home/user/it'\\''s a project'"
        );
    }

    #[test]
    fn shell_escape_path_with_parentheses() {
        assert_eq!(
            shell_escape("/home/user/project (v2)"),
            "'/home/user/project (v2)'"
        );
    }

    #[test]
    fn shell_escape_empty_string() {
        assert_eq!(shell_escape(""), "''");
    }
}
