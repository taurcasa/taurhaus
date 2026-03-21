//! tmux control — launch, stop, and navigate to CLI tool sessions.

use std::path::Path;
use std::process::Command;

use crate::daemon::protocol::LaunchMode;
use crate::models::CliCommandSettings;
use crate::platform::apply_background_command_settings;
use crate::session_scanner::cli_tool::{self, CliTool};
use crate::tmux_layout::{
    derive_window_name, parse_window_records, resolve_layout_allocation, TmuxLayoutAllocation,
    TmuxLayoutPolicy, DEFAULT_SPLIT_MAX_PANES, LIST_WINDOWS_FORMAT,
};

#[cfg(any(target_os = "windows", test))]
fn wsl_exec_args(program: &str) -> [String; 2] {
    ["-e".to_string(), program.to_string()]
}

#[cfg(target_os = "windows")]
fn wsl_exec_command(program: &str) -> Command {
    let mut cmd = crate::daemon::launcher::wsl_command();
    // Use `-e` for direct exec semantics. `wsl -- <command>` can route the
    // remainder through shell-style parsing, which breaks tmux format strings
    // like `#{pane_id}` on Windows before tmux ever receives them.
    cmd.stdin(std::process::Stdio::null());
    cmd.args(wsl_exec_args(program));
    cmd
}

fn tmux_command() -> Command {
    #[cfg(target_os = "windows")]
    let mut cmd = { wsl_exec_command("tmux") };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = Command::new("tmux");

    apply_background_command_settings(&mut cmd);
    cmd
}

fn project_path_exists_for_tmux(project_path: &str) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        let output = wsl_exec_command("test")
            .args(["-d", project_path])
            .output()
            .map_err(|e| format!("Failed to validate WSL project path: {e}"))?;

        if output.status.success() {
            return Ok(true);
        }

        if output.status.code() == Some(1) {
            return Ok(false);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr
        };
        return Err(format!("Failed to validate WSL project path: {detail}"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(Path::new(project_path).is_dir())
    }
}

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
    let tool_cmd = match command_override {
        Some(cmd) if !cmd.is_empty() => {
            validate_command_override(cmd)?;
            cmd.to_string()
        }
        _ => build_launch_command(tool, mode),
    };
    launch_command_in_tmux_with_layout(project_path, layout, &tool_cmd)
}

/// Launch an explicit command in tmux using the configured layout strategy.
///
/// The command is run as part of pane creation rather than injected later with
/// `send-keys`, which avoids the shell-readiness race for fresh launches.
pub fn launch_command_in_tmux_with_layout(
    project_path: &str,
    layout: &str,
    command: &str,
) -> Result<(String, String, String), String> {
    if !project_path_exists_for_tmux(project_path)? {
        return Err(format!("Project path does not exist: {project_path}"));
    }

    let tmux_session = ensure_taurhaus_session()?;
    let window_name = derive_window_name(project_path, "claude");
    let policy = TmuxLayoutPolicy::from_setting(layout, DEFAULT_SPLIT_MAX_PANES);
    let shell_cmd = build_tmux_shell_command(project_path, command);
    let windows = match policy {
        TmuxLayoutPolicy::NewWindow => Vec::new(),
        _ => list_tmux_windows(&tmux_session).unwrap_or_default(),
    };

    let pane_id = match resolve_layout_allocation(&policy, &tmux_session, &window_name, &windows) {
        TmuxLayoutAllocation::NewWindow { .. } => {
            create_new_window_pane(&tmux_session, &window_name, &shell_cmd)?
        }
        TmuxLayoutAllocation::SplitExisting { target_pane, .. } => {
            split_pane(&target_pane, &shell_cmd)?
        }
    };
    crate::session_scanner::notify_tmux_changed();

    Ok((tmux_session, window_name, pane_id))
}

/// Split a known tmux pane target and launch a command in the new pane.
///
/// This is the deterministic primitive coordination uses for per-project
/// batch launches: once the first pane exists, subsequent members should split
/// that same window rather than rediscovering it from tmux state each time.
pub fn split_command_in_tmux_target_pane(
    project_path: &str,
    target_pane: &str,
    command: &str,
) -> Result<String, String> {
    if !project_path_exists_for_tmux(project_path)? {
        return Err(format!("Project path does not exist: {project_path}"));
    }

    let shell_cmd = build_tmux_shell_command(project_path, command);
    let pane_id = split_pane(target_pane, &shell_cmd)?;
    crate::session_scanner::notify_tmux_changed();
    Ok(pane_id)
}

fn list_tmux_windows(
    tmux_session: &str,
) -> Result<Vec<crate::tmux_layout::TmuxWindowRecord>, String> {
    let output = tmux_command()
        .args([
            "list-windows",
            "-t",
            tmux_session,
            "-F",
            LIST_WINDOWS_FORMAT,
        ])
        .output()
        .map_err(|e| format!("Failed to inspect tmux windows: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux list-windows failed: {stderr}"));
    }

    Ok(parse_window_records(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn create_new_window_pane(
    tmux_session: &str,
    window_name: &str,
    shell_cmd: &str,
) -> Result<String, String> {
    let target = format!("{tmux_session}:");
    let output = tmux_command()
        .args([
            "new-window",
            "-n",
            window_name,
            "-t",
            &target,
            "-P",
            "-F",
            "#{pane_id}",
            shell_cmd,
        ])
        .output()
        .map_err(|e| format!("Failed to create tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux new-window failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Split an existing pane horizontally and run a command in the new pane.
fn split_pane(target_pane: &str, shell_cmd: &str) -> Result<String, String> {
    let output = tmux_command()
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
                    crate::session_scanner::notify_tmux_changed();
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
        let result = tmux_command().args(["kill-pane", "-t", &pane]).output();
        crate::session_scanner::notify_tmux_changed();
        tracing::info!(pane = %pane, success = ?result.as_ref().map(|o| o.status.success()), "stop_session: kill-pane result");
    });

    Ok(())
}

/// Get the current command running in a tmux pane.
fn pane_current_command(pane: &str) -> Option<String> {
    tmux_command()
        .args([
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_current_command}",
        ])
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
    let output = tmux_command()
        .args(["select-window", "-t", &target])
        .output()
        .map_err(|e| format!("Failed to select tmux window: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux select-window failed: {stderr}"));
    }

    // Select the pane
    let output = tmux_command()
        .args(["select-pane", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("Failed to select tmux pane: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux select-pane failed: {stderr}"));
    }

    Ok(())
}

/// Validate a user-supplied command override for safety.
///
/// Ensures the command starts with a known CLI tool name and contains no
/// shell metacharacters that could enable command injection.
pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
    let first_token = cmd.split_whitespace().next().unwrap_or("");
    let base_name = first_token.rsplit('/').next().unwrap_or(first_token);

    const ALLOWED_TOOLS: &[&str] = &["claude", "codex", "gemini"];
    if !ALLOWED_TOOLS.contains(&base_name) {
        return Err(format!(
            "Command override must start with claude/codex/gemini, got: {base_name}"
        ));
    }

    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '$', '`', '(', ')', '{', '}', '<', '>', '!', '\\', '\n', '\r',
    ];
    if let Some(c) = cmd.chars().find(|c| FORBIDDEN.contains(c)) {
        return Err(format!(
            "Command override contains forbidden character: {c:?}"
        ));
    }

    Ok(())
}

/// Resolve a launch command using configured per-tool/per-mode settings.
pub fn resolve_configured_tool_command(
    cmds: &CliCommandSettings,
    tool: CliTool,
    mode: LaunchMode,
) -> String {
    let tool_cmds = match tool {
        CliTool::Claude => &cmds.claude,
        CliTool::Codex => &cmds.codex,
        CliTool::Gemini => &cmds.gemini,
    };
    let cmd = match mode {
        LaunchMode::Continue => &tool_cmds.continue_cmd,
        LaunchMode::Fresh => &tool_cmds.fresh,
        LaunchMode::Resume => &tool_cmds.resume,
    };
    cmd.clone()
}

fn codex_command_has_model_arg(command: &str) -> bool {
    let mut tokens = command.split_whitespace();
    while let Some(token) = tokens.next() {
        if token == "-m" {
            return true;
        }
        if token.starts_with("--model") {
            return true;
        }
        // consume next token for "-m <value>" cases already handled above
        if token == "--model" {
            let _ = tokens.next();
            return true;
        }
    }
    false
}

fn normalize_codex_model(model: &str) -> String {
    let trimmed = model.trim();
    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.eq_ignore_ascii_case("gpt-5.4")
        || compact.eq_ignore_ascii_case("gpt-5.4 high")
        || compact.eq_ignore_ascii_case("gpt-5.4 medium")
        || compact.eq_ignore_ascii_case("gpt-5.4 low")
        || trimmed.eq_ignore_ascii_case("gpt-5.4-high")
        || trimmed.eq_ignore_ascii_case("gpt-5.4-medium")
        || trimmed.eq_ignore_ascii_case("gpt-5.4-low")
    {
        return "gpt-5.4".to_string();
    }
    if trimmed.eq_ignore_ascii_case("gpt-5.3") {
        return "gpt-5.3-codex".to_string();
    }
    compact
}

/// Build the command used for team-agent launch (fresh mode + optional model).
pub fn build_team_launch_command(cmds: &CliCommandSettings, tool: CliTool, model: &str) -> String {
    let base = resolve_configured_tool_command(cmds, tool, LaunchMode::Fresh);
    if tool != CliTool::Codex {
        return base;
    }

    let model = model.trim();
    if model.is_empty() || codex_command_has_model_arg(&base) {
        return base;
    }

    let normalized_model = normalize_codex_model(model);
    format!("{base} -m {}", shell_escape(&normalized_model))
}

/// Build the launch command string for a given tool and launch mode.
pub fn build_launch_command(tool: CliTool, mode: LaunchMode) -> String {
    resolve_configured_tool_command(&CliCommandSettings::default(), tool, mode)
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
    let check = tmux_command()
        .args(["has-session", "-t", TMUX_SESSION_NAME])
        .output()
        .map_err(|e| format!("tmux not available: {e}"))?;

    if !check.status.success() {
        // Create the session (detached — no client needed)
        let output = tmux_command()
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
    remove_legacy_tmux_focus_hooks();
    install_tmux_focus_hooks();

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
                if *var == "PATH" {
                    sync_tmux_path_environment(&val);
                    continue;
                }
                let _ = tmux_command()
                    .args(["set-environment", "-t", TMUX_SESSION_NAME, var, &val])
                    .output();
            }
        }
    }
}

fn sync_tmux_path_environment(path_value: &str) {
    if tmux_path_looks_windows_style(path_value) {
        let _ = tmux_command()
            .args(["set-environment", "-r", "-t", TMUX_SESSION_NAME, "PATH"])
            .output();
        tracing::debug!(
            "Skipping tmux PATH propagation because the app PATH is Windows-style and would break tmux hooks"
        );
        return;
    }

    let _ = tmux_command()
        .args([
            "set-environment",
            "-t",
            TMUX_SESSION_NAME,
            "PATH",
            path_value,
        ])
        .output();
}

fn tmux_path_looks_windows_style(path_value: &str) -> bool {
    path_value.contains(';')
}

fn install_tmux_focus_hooks() {
    let Some(focus_path) = default_tmux_focus_path() else {
        tracing::debug!(
            "Skipping tmux focus hook installation; tmux focus path could not be resolved"
        );
        return;
    };

    ensure_tmux_focus_hooks_for_path(&focus_path);
}

pub(crate) fn ensure_tmux_focus_hooks_for_path(focus_path: &Path) {
    ensure_tmux_focus_file_exists(focus_path);
    tracing::info!(
        path = %focus_path.display(),
        "Ensuring tmux focus hooks for focus file"
    );

    let attached_hook = build_tmux_focus_hook_command(focus_path);
    let detached_hook = build_tmux_focus_detached_hook_command(focus_path);
    for (hook_name, hook_command) in [
        ("after-select-window", attached_hook.as_str()),
        ("session-window-changed", attached_hook.as_str()),
        ("client-session-changed", attached_hook.as_str()),
        ("client-detached", detached_hook.as_str()),
    ] {
        match tmux_command()
            .args(["set-hook", "-g", hook_name, hook_command])
            .output()
        {
            Ok(output) if output.status.success() => {
                tracing::debug!(
                    hook = hook_name,
                    path = %focus_path.display(),
                    "Installed tmux focus hook"
                );
            }
            Ok(output) => {
                tracing::warn!(
                    hook = hook_name,
                    path = %focus_path.display(),
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "Failed to install tmux focus hook"
                );
            }
            Err(error) => {
                tracing::warn!(
                    hook = hook_name,
                    path = %focus_path.display(),
                    error = %error,
                    "Failed to execute tmux focus hook installation"
                );
            }
        }
    }
}

pub(crate) fn remove_legacy_tmux_focus_hooks() {
    let focus_path = default_tmux_focus_path();
    let output = match tmux_command().args(["show-hooks", "-g"]).output() {
        Ok(output) => output,
        Err(error) => {
            tracing::debug!(
                error = %error,
                "Skipping tmux focus hook cleanup because hook inspection failed"
            );
            return;
        }
    };

    if !output.status.success() {
        tracing::debug!(
            stderr = %String::from_utf8_lossy(&output.stderr),
            "Skipping tmux focus hook cleanup because tmux show-hooks failed"
        );
        return;
    }

    let hooks = String::from_utf8_lossy(&output.stdout);
    for hook_name in legacy_tmux_focus_hook_names(&hooks, focus_path.as_deref()) {
        match tmux_command()
            .args(["set-hook", "-gu", &hook_name])
            .output()
        {
            Ok(result) if result.status.success() => {
                tracing::info!(
                    hook = %hook_name,
                    "Removed legacy Taurhaus tmux focus hook"
                );
            }
            Ok(result) => {
                tracing::warn!(
                    hook = %hook_name,
                    stderr = %String::from_utf8_lossy(&result.stderr),
                    "Failed to remove legacy Taurhaus tmux focus hook"
                );
            }
            Err(error) => {
                tracing::warn!(
                    hook = %hook_name,
                    error = %error,
                    "Failed to execute legacy Taurhaus tmux focus hook cleanup"
                );
            }
        }
    }
}

fn ensure_tmux_focus_file_exists(focus_path: &Path) {
    if !focus_path.exists() {
        let _ = crate::session_scanner::tmux::write_focus_state(
            focus_path,
            &crate::session_scanner::tmux::TmuxFocusState::detached(),
        );
    }
}

fn default_tmux_focus_path() -> Option<std::path::PathBuf> {
    std::env::var_os("TAURHAUS_DATA_DIR")
        .map(std::path::PathBuf::from)
        .map(|data_dir| crate::session_scanner::tmux::focus_file_path(&data_dir))
}

fn legacy_tmux_focus_hook_names(show_hooks_output: &str, focus_path: Option<&Path>) -> Vec<String> {
    show_hooks_output
        .lines()
        .filter_map(|line| {
            if !line.contains("tmux-focus.json") {
                return None;
            }

            let mut parts = line.splitn(2, char::is_whitespace);
            let hook_name = parts.next()?.to_string();
            let hook_command = parts.next().map(str::trim).unwrap_or_default();

            if is_current_tmux_focus_hook(hook_name.as_str(), hook_command, focus_path) {
                return None;
            }

            Some(hook_name)
        })
        .collect()
}

fn is_current_tmux_focus_hook(
    hook_name: &str,
    hook_command: &str,
    focus_path: Option<&Path>,
) -> bool {
    let Some(focus_path) = focus_path else {
        return false;
    };

    let expected = if hook_name.starts_with("client-detached") {
        build_tmux_focus_detached_hook_command(focus_path)
    } else if hook_name.starts_with("after-select-window")
        || hook_name.starts_with("session-window-changed")
        || hook_name.starts_with("client-session-changed")
    {
        build_tmux_focus_hook_command(focus_path)
    } else {
        return false;
    };

    hook_command == expected
}

fn build_tmux_focus_hook_command(focus_path: &Path) -> String {
    let file = shell_escape(&tmux_shell_path(focus_path));
    let dir = shell_escape(&tmux_shell_parent_path(focus_path));
    let payload = format!(
        "PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH; mkdir -p {dir} && printf '%s\\n' '{{\\\"session\\\":\\\"#{{session_name}}\\\",\\\"window\\\":\\\"#{{window_index}}\\\",\\\"timestamp\\\":#{{window_activity}}}}' > {file}"
    );
    format!("run-shell -b \"/bin/sh -c {}\"", shell_escape(&payload))
}

fn build_tmux_focus_detached_hook_command(focus_path: &Path) -> String {
    let file = shell_escape(&tmux_shell_path(focus_path));
    let dir = shell_escape(&tmux_shell_parent_path(focus_path));
    let payload = format!(
        "PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH; mkdir -p {dir} && printf '%s\\n' '{{\\\"session\\\":null,\\\"window\\\":null,\\\"timestamp\\\":null}}' > {file}"
    );
    format!("run-shell -b \"/bin/sh -c {}\"", shell_escape(&payload))
}

fn tmux_shell_parent_path(focus_path: &Path) -> String {
    focus_path
        .parent()
        .map(tmux_shell_path)
        .unwrap_or_else(|| ".".to_string())
}

fn tmux_shell_path(path: &Path) -> String {
    let raw = path.display().to_string();
    crate::provider::path::to_linux(&raw).unwrap_or(raw)
}

fn build_tmux_shell_command(project_path: &str, command: &str) -> String {
    let escaped_path = shell_escape(project_path);
    let inner_cmd = format!("cd {escaped_path} && {command}; exec \"$SHELL\"");
    format!("exec \"$SHELL\" -ic {}", shell_escape(&inner_cmd))
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
    let output = tmux_command()
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
    let output = tmux_command()
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
    let output = tmux_command()
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
    use tempfile::TempDir;

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

    // -----------------------------------------------------------------------
    // Command override validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_command_override_accepts_tool_commands() {
        assert!(validate_command_override("claude --dangerously-skip-permissions").is_ok());
        assert!(validate_command_override("codex --yolo").is_ok());
        assert!(validate_command_override("gemini --yolo --resume").is_ok());
    }

    #[test]
    fn validate_command_override_accepts_absolute_paths() {
        assert!(validate_command_override("/usr/local/bin/claude --flag").is_ok());
        assert!(validate_command_override("/home/user/.local/bin/codex --yolo").is_ok());
    }

    #[test]
    fn validate_command_override_rejects_unknown_tools() {
        assert!(validate_command_override("bash -c 'evil'").is_err());
        assert!(validate_command_override("python3 script.py").is_err());
        assert!(validate_command_override("rm -rf /").is_err());
    }

    #[test]
    fn validate_command_override_rejects_shell_injection() {
        assert!(validate_command_override("claude; rm -rf /").is_err());
        assert!(validate_command_override("claude && evil").is_err());
        assert!(validate_command_override("claude | cat /etc/passwd").is_err());
        assert!(validate_command_override("claude $(whoami)").is_err());
        assert!(validate_command_override("claude `id`").is_err());
    }

    #[test]
    fn resolve_configured_tool_command_uses_settings_values() {
        let mut cmds = crate::models::CliCommandSettings::default();
        cmds.codex.fresh = "codex --yolo --sandbox workspace-write".to_string();
        assert_eq!(
            resolve_configured_tool_command(&cmds, CliTool::Codex, LaunchMode::Fresh),
            "codex --yolo --sandbox workspace-write"
        );
    }

    #[test]
    fn build_team_launch_command_for_codex_appends_model_when_missing() {
        let cmds = crate::models::CliCommandSettings::default();
        assert_eq!(
            build_team_launch_command(&cmds, CliTool::Codex, "gpt-5.4"),
            "codex --yolo -m 'gpt-5.4'"
        );
    }

    #[test]
    fn build_team_launch_command_for_codex_normalizes_legacy_hyphenated_high_model() {
        let cmds = crate::models::CliCommandSettings::default();
        assert_eq!(
            build_team_launch_command(&cmds, CliTool::Codex, "gpt-5.4-high"),
            "codex --yolo -m 'gpt-5.4'"
        );
    }

    #[test]
    fn build_team_launch_command_for_codex_strips_embedded_reasoning_suffix() {
        let cmds = crate::models::CliCommandSettings::default();
        assert_eq!(
            build_team_launch_command(&cmds, CliTool::Codex, "gpt-5.4 high"),
            "codex --yolo -m 'gpt-5.4'"
        );
    }

    #[test]
    fn build_team_launch_command_for_codex_preserves_codex_model_suffix() {
        let cmds = crate::models::CliCommandSettings::default();
        assert_eq!(
            build_team_launch_command(&cmds, CliTool::Codex, "gpt-5.3-codex"),
            "codex --yolo -m 'gpt-5.3-codex'"
        );
    }

    #[test]
    fn build_team_launch_command_for_codex_keeps_existing_model_flag() {
        let mut cmds = crate::models::CliCommandSettings::default();
        cmds.codex.fresh = "codex --yolo --model gpt-6".to_string();
        assert_eq!(
            build_team_launch_command(&cmds, CliTool::Codex, "gpt-5.4"),
            "codex --yolo --model gpt-6"
        );
    }

    #[test]
    fn launch_rejects_nonexistent_path() {
        let result = launch_in_tmux(
            "/nonexistent/path/12345",
            LaunchMode::Continue,
            CliTool::Claude,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn project_path_exists_for_tmux_accepts_real_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        assert!(project_path_exists_for_tmux(tempdir.path().to_string_lossy().as_ref()).unwrap());
    }

    #[test]
    fn project_path_exists_for_tmux_rejects_missing_directory() {
        assert!(!project_path_exists_for_tmux("/nonexistent/path/12345").unwrap());
    }

    #[test]
    fn wsl_exec_args_use_direct_exec_semantics() {
        assert_eq!(
            wsl_exec_args("tmux"),
            ["-e".to_string(), "tmux".to_string()]
        );
        assert_eq!(
            wsl_exec_args("test"),
            ["-e".to_string(), "test".to_string()]
        );
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

    #[test]
    fn legacy_tmux_focus_hook_names_match_only_taurhaus_hooks() {
        // Regression: commit ea3b44f installed global tmux hooks that mutated
        // the user's session manager and surfaced `run-shell ... returned 127`
        // on window changes. We only want to clean up Taurhaus-owned hook entries.
        let hooks = r##"
after-select-window[0] run-shell -b "mkdir -p '/mnt/c/Users/me/AppData/Roaming/com.taurhaus.dev' && printf '%s\n' '{"session":"#{session_name}"}' > '/mnt/c/Users/me/AppData/Roaming/com.taurhaus.dev/tmux-focus.json'"
after-new-window[0] run-shell -b "echo keep-me"
client-detached[0] run-shell -b "printf '%s\n' '{"session":null}' > '/tmp/tmux-focus.json'"
client-session-changed[0] run-shell -b "printf '%s\n' '{"session":"#{session_name}"}' > '/mnt/c/Users/me/AppData/Roaming/com.taurhaus.dev/tmux-focus.json'"
        "##;

        assert_eq!(
            legacy_tmux_focus_hook_names(hooks, None),
            vec![
                "after-select-window[0]".to_string(),
                "client-detached[0]".to_string(),
                "client-session-changed[0]".to_string(),
            ]
        );
    }

    #[test]
    fn legacy_tmux_focus_hook_names_ignore_unrelated_hooks() {
        // Regression: cleanup must not remove unrelated user-defined tmux hooks.
        let hooks = r#"
after-select-window[0] run-shell -b "echo hello"
after-new-window[0] display-message "hi"
"#;

        assert!(legacy_tmux_focus_hook_names(hooks, None).is_empty());
    }

    #[test]
    fn default_tmux_focus_path_uses_canonical_env_override() {
        let temp = TempDir::new().expect("temp dir");
        let original = std::env::var_os("TAURHAUS_DATA_DIR");
        std::env::set_var("TAURHAUS_DATA_DIR", temp.path());

        let path = default_tmux_focus_path().expect("default tmux focus path");
        assert_eq!(path, temp.path().join("tmux-focus.json"));

        match original {
            Some(value) => std::env::set_var("TAURHAUS_DATA_DIR", value),
            None => std::env::remove_var("TAURHAUS_DATA_DIR"),
        }
    }

    #[test]
    fn default_tmux_focus_path_requires_canonical_env_override() {
        let original = std::env::var_os("TAURHAUS_DATA_DIR");
        std::env::remove_var("TAURHAUS_DATA_DIR");

        let path = default_tmux_focus_path();
        assert_eq!(path, None);

        match original {
            Some(value) => std::env::set_var("TAURHAUS_DATA_DIR", value),
            None => std::env::remove_var("TAURHAUS_DATA_DIR"),
        }
    }

    #[test]
    fn ensure_tmux_focus_file_exists_creates_focus_file() {
        let temp = TempDir::new().expect("temp dir");
        let focus_path = temp.path().join("tmux-focus.json");
        assert!(!focus_path.exists());

        ensure_tmux_focus_file_exists(&focus_path);

        let focus = crate::session_scanner::tmux::read_focus_state_from_path(&focus_path)
            .expect("focus state should be created");
        assert_eq!(
            focus,
            crate::session_scanner::tmux::TmuxFocusState::detached()
        );
    }

    #[test]
    fn build_tmux_focus_hook_command_targets_focus_file() {
        let focus_path = Path::new("/tmp/taurhaus data/tmux-focus.json");
        let command = build_tmux_focus_hook_command(focus_path);

        assert!(command.contains("run-shell -b"));
        assert!(command.contains("/bin/sh -c"));
        assert!(command.contains("PATH=/usr/bin:/bin:/usr/sbin:/sbin"));
        assert!(command.contains("/tmp/taurhaus data/tmux-focus.json"));
        assert!(command.contains("#{session_name}"));
        assert!(command.contains("#{window_index}"));
        assert!(command.contains("#{window_activity}"));
    }

    #[test]
    fn build_tmux_focus_detached_hook_command_writes_null_focus_state() {
        let focus_path = Path::new("/tmp/taurhaus-data/tmux-focus.json");
        let command = build_tmux_focus_detached_hook_command(focus_path);

        assert!(command.contains("/bin/sh -c"));
        assert!(command.contains("PATH=/usr/bin:/bin:/usr/sbin:/sbin"));
        assert!(command.contains("/tmp/taurhaus-data/tmux-focus.json"));
        assert!(command.contains("\\\"session\\\":null"));
        assert!(command.contains("\\\"window\\\":null"));
        assert!(command.contains("\\\"timestamp\\\":null"));
    }

    #[test]
    fn tmux_path_looks_windows_style_rejects_semicolon_lists() {
        assert!(tmux_path_looks_windows_style(
            r"C:\Windows\system32;C:\Users\mstie\.bun\bin"
        ));
        assert!(!tmux_path_looks_windows_style(
            "/usr/local/bin:/usr/bin:/bin"
        ));
    }

    #[test]
    fn legacy_tmux_focus_hook_names_preserve_current_hooks() {
        let focus_path = Path::new("/tmp/taurhaus-data/tmux-focus.json");
        let attached = build_tmux_focus_hook_command(focus_path);
        let detached = build_tmux_focus_detached_hook_command(focus_path);
        let hooks = format!(
            "after-select-window[0] {attached}\n\
session-window-changed[0] {attached}\n\
client-session-changed[0] {attached}\n\
client-detached[0] {detached}\n"
        );

        assert!(
            legacy_tmux_focus_hook_names(&hooks, Some(focus_path)).is_empty(),
            "current focus hooks should not be treated as legacy"
        );
    }

    #[test]
    fn legacy_tmux_focus_hook_names_remove_mismatched_focus_hooks() {
        let focus_path = Path::new("/tmp/taurhaus-data/tmux-focus.json");
        let hooks = "after-select-window[0] run-shell -b \"printf '%s\\n' '{\\\"session\\\":\\\"legacy\\\"}' > '/tmp/taurhaus-data/tmux-focus.json'\"\n";

        assert_eq!(
            legacy_tmux_focus_hook_names(hooks, Some(focus_path)),
            vec!["after-select-window[0]".to_string()]
        );
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
        assert_eq!(shell_escape("/tmp/foo$(rm -rf /)"), "'/tmp/foo$(rm -rf /)'");
        assert_eq!(shell_escape("/tmp/foo`id`bar"), "'/tmp/foo`id`bar'");
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
