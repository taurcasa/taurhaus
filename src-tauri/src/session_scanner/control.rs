//! tmux control — launch, stop, and navigate to CLI tool sessions.

use std::path::Path;
use std::process::Command;

use crate::daemon::protocol::LaunchMode;
use crate::session_scanner::cli_tool::{self, CliTool};

/// Launch a CLI tool session in tmux using the configured layout strategy.
///
/// Layout strategies:
/// - `new_window` (default): Always create a new tmux window
/// - `split`: Split an existing window horizontally, up to 4 panes per window
/// - `per_project`: Same project shares a window with splits, different projects get new windows
///
/// Returns `(tmux_session, window_name, pane_id)` on success.
pub fn launch_in_tmux(
    project_path: &str,
    mode: LaunchMode,
    tool: CliTool,
) -> Result<(String, String, String), String> {
    launch_in_tmux_with_layout(project_path, mode, tool, "new_window", None)
}

/// Launch with explicit layout strategy and optional command override.
pub fn launch_in_tmux_with_layout(
    project_path: &str,
    mode: LaunchMode,
    tool: CliTool,
    layout: &str,
    command_override: Option<&str>,
) -> Result<(String, String, String), String> {
    // Validate project path
    if !Path::new(project_path).is_dir() {
        return Err(format!("Project path does not exist: {project_path}"));
    }

    // Ensure our dedicated tmux session exists
    let tmux_session = ensure_taurhaus_session()?;

    // Window name: last component of project path
    let window_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "claude".to_string());

    // Build the full command: use override if provided, otherwise fall back to defaults.
    let tool_cmd = match command_override {
        Some(cmd) if !cmd.is_empty() => cmd.to_string(),
        _ => build_launch_command(tool, mode),
    };
    let escaped_path = shell_escape(project_path);
    let inner_cmd = format!("cd {escaped_path} && {tool_cmd}; exec \"$SHELL\"");
    let shell_cmd = format!("exec \"$SHELL\" -ic {}", shell_escape(&inner_cmd));

    match layout {
        "split" => {
            // Try to split an existing window (max 4 panes per window)
            if let Some(target_pane) = find_window_with_space(&tmux_session, 4) {
                let pane_id = split_pane(&target_pane, &shell_cmd)?;
                return Ok((tmux_session, window_name, pane_id));
            }
            // No window with space — fall through to new window
        }
        "per_project" => {
            // Try to find an existing window named after this project
            if let Some(target_pane) = find_project_window(&tmux_session, &window_name) {
                let pane_id = split_pane(&target_pane, &shell_cmd)?;
                return Ok((tmux_session, window_name, pane_id));
            }
            // No matching window — fall through to new window
        }
        _ => {} // "new_window" or unknown — always create new window
    }

    // Default: create new tmux window
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
            &shell_cmd,
        ])
        .output()
        .map_err(|e| format!("Failed to create tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux new-window failed: {stderr}"));
    }

    let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok((tmux_session, window_name, pane_id))
}

/// Find a window in the session that has fewer than `max_panes` panes.
/// Returns the first pane ID in that window (as split target).
fn find_window_with_space(tmux_session: &str, max_panes: usize) -> Option<String> {
    let output = Command::new("tmux")
        .args([
            "list-windows",
            "-t",
            tmux_session,
            "-F",
            "#{window_index} #{window_panes}",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(pane_count) = parts[1].parse::<usize>() {
                if pane_count < max_panes {
                    // Return first pane in this window
                    let window_idx = parts[0];
                    let target = format!("{tmux_session}:{window_idx}.0");
                    return Some(target);
                }
            }
        }
    }
    None
}

/// Find a window named after the project in the session.
/// Returns the first pane ID in that window (as split target).
fn find_project_window(tmux_session: &str, window_name: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args([
            "list-windows",
            "-t",
            tmux_session,
            "-F",
            "#{window_index} #{window_name}",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() >= 2 && parts[1] == window_name {
            let window_idx = parts[0];
            let target = format!("{tmux_session}:{window_idx}.0");
            return Some(target);
        }
    }
    None
}

/// Split an existing pane horizontally and run a command in the new pane.
fn split_pane(target_pane: &str, shell_cmd: &str) -> Result<String, String> {
    let output = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-t",
            target_pane,
            "-P",
            "-F",
            "#{pane_id}",
            shell_cmd,
        ])
        .output()
        .map_err(|e| format!("Failed to split tmux pane: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux split-window failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Stop a CLI tool session by sending the exit signal to the tmux pane.
///
/// After the tool exits, polls for the process to terminate (pane returns
/// to shell), then kills the pane to clean up. If it's the last pane in the
/// window, tmux automatically closes the window too.
///
/// Exit strategies differ per tool:
/// - Claude & Gemini: `/exit` text command (typed + Enter)
/// - Codex: Ctrl+C (key signal, no text)
pub fn stop_session(tmux_pane: &str, tool: CliTool) -> Result<(), String> {
    match tool {
        CliTool::Codex => {
            // Codex exits on Ctrl+C, not a text command
            run_tmux_raw_key(tmux_pane, "C-c")?;
        }
        _ => {
            let config = cli_tool::config_for(tool);
            run_tmux_send_keys(tmux_pane, config.exit_command)?;
        }
    }

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
            LaunchMode::Continue => "codex --yolo".to_string(),
            LaunchMode::Fresh => "codex --yolo".to_string(),
            LaunchMode::Resume => "codex resume --last --yolo".to_string(),
        },
        CliTool::Gemini => match mode {
            LaunchMode::Continue => "gemini --yolo --resume".to_string(),
            LaunchMode::Fresh => "gemini --yolo".to_string(),
            LaunchMode::Resume => "gemini --yolo --resume".to_string(),
        },
    }
}

/// The tmux session name used by taurhaus for all CLI tool windows.
///
/// Using a dedicated named session avoids conflicts with the user's own
/// tmux sessions and ensures we always know where our tools are running.
pub const TMUX_SESSION_NAME: &str = "taurhaus";

/// Ensure the taurhaus tmux session exists, creating it if needed.
///
/// `tmux new-session` implicitly starts the server, so this also handles
/// the case where no tmux server is running yet.
///
/// After ensuring the session exists, propagates critical environment variables
/// (API keys, NODE_EXTRA_CA_CERTS) to the tmux global environment so all new
/// panes inherit them — even if the tmux server was started before the user's
/// shell profile set them.
fn ensure_taurhaus_session() -> Result<String, String> {
    // Check if session already exists
    let check = Command::new("tmux")
        .args(["has-session", "-t", TMUX_SESSION_NAME])
        .output()
        .map_err(|e| format!("tmux not available: {e}"))?;

    if !check.status.success() {
        // Create the session (detached — no client needed)
        let output = Command::new("tmux")
            .args(["new-session", "-d", "-s", TMUX_SESSION_NAME])
            .output()
            .map_err(|e| format!("Failed to create tmux session: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("tmux new-session failed: {stderr}"));
        }
    }

    // Propagate critical env vars to tmux global environment.
    // This ensures API keys and certs are available in all new panes,
    // even if the tmux server started before these were set in the shell.
    propagate_env_to_tmux();

    Ok(TMUX_SESSION_NAME.to_string())
}

/// Propagate important environment variables to the tmux global environment.
///
/// Runs `tmux set-environment -g KEY VALUE` for each var that's set in our
/// process environment. This is critical on macOS where the Tauri app inherits
/// the user's login shell env (via lib.rs startup), but the tmux server may
/// have been started earlier with a minimal env.
fn propagate_env_to_tmux() {
    const PROPAGATE_VARS: &[&str] = &[
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "NODE_EXTRA_CA_CERTS",
        "PATH",
    ];

    for var in PROPAGATE_VARS {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                let _ = Command::new("tmux")
                    .args(["set-environment", "-g", var, &val])
                    .output();
            }
        }
    }
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

/// Send a raw key sequence to a tmux pane (no Enter, no text escaping).
///
/// Used for control sequences like `C-c` (Ctrl+C) that aren't typed text.
fn run_tmux_raw_key(pane: &str, key: &str) -> Result<(), String> {
    let output = Command::new("tmux")
        .args(["send-keys", "-t", pane, key])
        .output()
        .map_err(|e| format!("Failed to send key to tmux pane {pane}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux send-keys failed: {stderr}"));
    }

    Ok(())
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
            "codex --yolo"
        );
    }

    #[test]
    fn build_codex_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Codex, LaunchMode::Resume),
            "codex resume --last --yolo"
        );
    }

    // -----------------------------------------------------------------------
    // Gemini command tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_gemini_fresh_command() {
        assert_eq!(
            build_launch_command(CliTool::Gemini, LaunchMode::Fresh),
            "gemini --yolo"
        );
    }

    #[test]
    fn build_gemini_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Gemini, LaunchMode::Resume),
            "gemini --yolo --resume"
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

    // -----------------------------------------------------------------------
    // Shell command wrapping test
    // -----------------------------------------------------------------------

    #[test]
    fn shell_cmd_wraps_in_interactive_shell() {
        // Verify the launch command is wrapped in $SHELL -ic for PATH access.
        // This ensures CLI tools installed via fnm/npm are found.
        let path = "/home/user/project";
        let tool_cmd = build_launch_command(CliTool::Claude, LaunchMode::Continue);
        let escaped_path = shell_escape(path);
        let inner_cmd = format!("cd {escaped_path} && {tool_cmd}; exec \"$SHELL\"");
        let shell_cmd = format!("exec \"$SHELL\" -ic {}", shell_escape(&inner_cmd));

        // Must start with exec "$SHELL" -ic
        assert!(shell_cmd.starts_with("exec \"$SHELL\" -ic "));
        // Inner command must contain the tool command
        assert!(shell_cmd.contains("claude --dangerously-skip-permissions --continue"));
        // Inner command must end with exec "$SHELL" for post-exit shell
        assert!(shell_cmd.contains("exec \"$SHELL\""));
        // Path must be shell-escaped inside the wrapper
        assert!(shell_cmd.contains("/home/user/project"));
    }
}
