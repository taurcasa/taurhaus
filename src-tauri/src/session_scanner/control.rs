//! tmux control — launch, stop, and navigate to CLI tool sessions.

use std::path::Path;
use std::process::Command;

use crate::daemon::protocol::LaunchMode;
use crate::models::CliCommandSettings;
use crate::platform::apply_background_command_settings;
use crate::session_scanner::cli_tool::{self, CliTool};
use crate::session_scanner::launch::{base_command, shell_escape, LaunchSpec, ModelSpec};
use crate::tmux_layout::{
    derive_window_name, parse_pane_records, parse_window_records, resolve_layout_allocation,
    resolve_split_target_pane, wait_for_tmux_session_ready, TmuxLayoutAllocation, TmuxLayoutPolicy,
    DEFAULT_SPLIT_MAX_PANES, LIST_PANES_FORMAT, LIST_WINDOWS_FORMAT,
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
        TmuxLayoutAllocation::SplitExisting { window_index, .. } => {
            let target_pane = resolve_split_target_pane_for_window(&tmux_session, &window_index)?;
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

fn list_tmux_window_panes(
    tmux_session: &str,
    window_index: &str,
) -> Result<Vec<crate::tmux_layout::TmuxPaneRecord>, String> {
    let target = format!("{tmux_session}:{window_index}");
    let output = tmux_command()
        .args(["list-panes", "-t", &target, "-F", LIST_PANES_FORMAT])
        .output()
        .map_err(|e| format!("Failed to inspect tmux panes for {target}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux list-panes failed for {target}: {stderr}"));
    }

    Ok(parse_pane_records(&String::from_utf8_lossy(&output.stdout)))
}

fn resolve_split_target_pane_for_window(
    tmux_session: &str,
    window_index: &str,
) -> Result<String, String> {
    let panes = list_tmux_window_panes(tmux_session, window_index)?;
    let target = resolve_split_target_pane(tmux_session, window_index, &panes)?;

    let validation = tmux_command()
        .args(["display-message", "-p", "-t", &target, "#{pane_id}"])
        .output()
        .map_err(|e| format!("Failed to validate tmux pane target {target}: {e}"))?;
    if validation.status.success() {
        return Ok(target);
    }

    let pane_ids = panes
        .iter()
        .map(|pane| pane.pane_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "tmux window '{tmux_session}:{window_index}' resolved pane target '{target}' is not addressable; pane_ids=[{pane_ids}]"
    ))
}

fn verify_tmux_session_ready(tmux_session: &str) -> Result<(), String> {
    let windows = list_tmux_windows(tmux_session)?;
    let first_window = windows
        .first()
        .ok_or_else(|| format!("tmux session '{tmux_session}' has no windows yet"))?;
    let _ = resolve_split_target_pane_for_window(tmux_session, &first_window.index)?;
    Ok(())
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
/// - Claude & Antigravity: `/exit` text command (typed + Enter)
/// - Grok: `/quit` text command, confirmed by its registry row disappearing
/// - Codex: Ctrl+C (key signal, no text)
pub fn stop_session(tmux_pane: &str, tool: CliTool) -> Result<(), String> {
    let config = cli_tool::spec(tool);
    let presence_lock = stop_presence_lock(tmux_pane, config);
    let registry_release = stop_registry_release(tmux_pane, config);
    match config.stop_strategy {
        cli_tool::StopStrategy::Interrupt => {
            // Codex exits on Ctrl+C, not a text command
            run_tmux_raw_key(tmux_pane, "C-c")?;
        }
        cli_tool::StopStrategy::SlashExit => {
            run_tmux_send_keys(tmux_pane, config.exit_command)?;
        }
    }

    // Poll for exit, then kill the pane. Background thread so we don't block IPC.
    let pane = tmux_pane.to_string();
    // The budget is the harness's own: grok runs its `SessionEnd` hooks inside a
    // documented ten-second exit budget, so the shared five seconds would kill a
    // shutdown that is going exactly to plan.
    let timeout_ms = config.stop_timeout.as_millis() as u64;
    std::thread::spawn(move || {
        const POLL_MS: u64 = 200;
        let mut elapsed = 0u64;

        tracing::info!(pane = %pane, "stop_session: polling for exit");

        // Poll until the pane's command becomes a shell (Claude exited)
        // or the pane disappears, or we hit the timeout.
        while elapsed < timeout_ms {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
            elapsed += POLL_MS;

            match pane_current_command(&pane) {
                Some(cmd)
                    if stop_has_completed(
                        Some(&cmd),
                        presence_lock.as_deref(),
                        registry_release.as_ref(),
                    ) =>
                {
                    tracing::info!(pane = %pane, cmd = %cmd, elapsed_ms = elapsed, "stop_session: graceful exit confirmed, killing pane");
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

        if elapsed >= timeout_ms {
            tracing::warn!(pane = %pane, "stop_session: timeout, killing pane anyway");
        }

        // Kill the pane (noop if already gone)
        let result = tmux_command().args(["kill-pane", "-t", &pane]).output();
        crate::session_scanner::notify_tmux_changed();
        tracing::info!(pane = %pane, success = ?result.as_ref().map(|o| o.status.success()), "stop_session: kill-pane result");
    });

    Ok(())
}

fn stop_presence_lock(
    pane: &str,
    config: &crate::session_scanner::cli_tool::CliToolSpec,
) -> Option<std::path::PathBuf> {
    let presence_dir = config.stop_presence_dir?;
    let pid = pane_process_id(pane)?;
    let cwd = crate::platform::process_cwd(pid)?;
    let cwd = cwd.to_string_lossy();
    let resolved = config.session_source().resolve(&cwd, pid, Some(pane));
    let transcript = std::path::Path::new(resolved.jsonl_path.as_deref()?);
    let stem = transcript.file_stem()?.to_str()?;
    let app_data = transcript.parent()?.parent()?;
    let path = app_data.join(presence_dir).join(format!("{stem}.lock"));
    crate::session_scanner::idle::presence_lock_is_held(&path).then_some(path)
}

/// The grok home and pid whose registry row proves the session is still up.
///
/// grok removes the row on `/quit` and leaves it behind on a kill, so its
/// disappearance is a positive clean-stop signal rather than an inference from
/// what the pane is running.
struct StopRegistryRelease {
    home: std::path::PathBuf,
    pid: u32,
}

fn stop_registry_release(
    pane: &str,
    config: &crate::session_scanner::cli_tool::CliToolSpec,
) -> Option<StopRegistryRelease> {
    if !config.stop_registry_release {
        return None;
    }
    let pane_pid = pane_process_id(pane)?;
    let candidates = registry_pid_candidates(
        config.tool,
        pane_pid,
        pane_tty(pane).as_deref(),
        &crate::session_scanner::process::scan_processes().processes,
    );
    resolve_registry_release_with(&candidates, &|pid| {
        crate::platform::process_env_var(pid, "GROK_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::provider::platform_paths::PlatformPaths::grok_dir)
    })
}

/// The pids that could own this pane's registry row, harness child first.
///
/// Every launch runs inside `exec "$SHELL" -ic '…; exec "$SHELL"'`, so tmux's
/// `#{pane_pid}` is the shell while the registry row — and any inline
/// `GROK_HOME=` assignment — belongs to the harness it started. The harness is
/// the tool process sharing the pane's tty. The pane pid stays as the fallback
/// for a launch that never got the shell wrapper.
fn registry_pid_candidates(
    tool: CliTool,
    pane_pid: u32,
    pane_tty: Option<&str>,
    processes: &[crate::session_scanner::process::ProcessInfo],
) -> Vec<u32> {
    let mut candidates: Vec<u32> = pane_tty
        .map(|tty| {
            processes
                .iter()
                .filter(|process| process.cli_tool == tool && process.tty == tty)
                .map(|process| process.pid)
                .collect()
        })
        .unwrap_or_default();
    if !candidates.contains(&pane_pid) {
        candidates.push(pane_pid);
    }
    candidates
}

fn resolve_registry_release_with(
    candidates: &[u32],
    home_for: &dyn Fn(u32) -> std::path::PathBuf,
) -> Option<StopRegistryRelease> {
    candidates.iter().find_map(|&pid| {
        let home = home_for(pid);
        matches!(
            crate::session_scanner::idle::grok_session_registry_residence(&home, pid),
            crate::session_scanner::idle::GrokRegistryResidence::Held
        )
        .then_some(StopRegistryRelease { home, pid })
    })
}

/// Whether the harness has actually finished, given what proof this stop has.
///
/// The proofs are ranked, not combined: a harness that publishes its own
/// clean-stop signal is believed over the pane, because every launch runs
/// `exec "$SHELL" -ic '…; exec "$SHELL"'` and a shell can be back on top of the
/// pane while the harness child is still working. Silence from an authoritative
/// proof keeps the poll going until the deadline, where the tmux floor takes
/// over and kills the pane anyway.
fn stop_has_completed(
    current_command: Option<&str>,
    presence_lock: Option<&std::path::Path>,
    registry_release: Option<&StopRegistryRelease>,
) -> bool {
    // Only a registry that was read and no longer names the pid proves the
    // stop; an unreadable one is silence, not release.
    if let Some(release) = registry_release {
        return crate::session_scanner::idle::grok_session_registry_residence(
            &release.home,
            release.pid,
        ) == crate::session_scanner::idle::GrokRegistryResidence::Released;
    }
    if let Some(path) = presence_lock {
        return !crate::session_scanner::idle::presence_lock_is_held(path);
    }
    current_command.is_some_and(is_shell)
}

fn pane_process_id(pane: &str) -> Option<u32> {
    pane_field(pane, "#{pane_pid}").and_then(|pid| pid.parse().ok())
}

fn pane_tty(pane: &str) -> Option<String> {
    pane_field(pane, "#{pane_tty}")
}

fn pane_field(pane: &str, format: &str) -> Option<String> {
    tmux_command()
        .args(["display-message", "-p", "-t", pane, format])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

/// Validate a user-supplied command override.
///
/// The override is whatever the user typed into Settings → CLI commands and is
/// executed as that same user through their interactive shell
/// (`exec "$SHELL" -ic ...`). It is deliberately free-form so aliases,
/// alternate binaries (`claude2`), environment prefixes
/// (`CLAUDE_CONFIG_DIR=~/.x claude`), and ordinary shell syntax all work the
/// way they would in a terminal. The daemon only accepts launch requests over
/// a token-authenticated localhost connection, and any such request already
/// implies local code execution, so a tool-name allowlist adds no meaningful
/// protection — it only blocks legitimate configurations.
///
/// The single invariant enforced here is that the command is a non-empty,
/// single-line string: empty commands produce confusing "shell exited"
/// windows and embedded line breaks can split the tmux launch command.
pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
    if cmd.trim().is_empty() {
        return Err("Command override is empty".to_string());
    }
    if let Some(c) = cmd.chars().find(|c| matches!(c, '\n' | '\r' | '\0')) {
        return Err(format!(
            "Command override must be a single line without control characters, found: {c:?}"
        ));
    }
    Ok(())
}

/// Build the launch command string for a given tool and launch mode.
///
/// This is the daemon-only fallback for old callers that omit `command_override`.
/// The daemon has no settings database; every app-side launch sends a command
/// rendered from the loaded `CliCommandSettings` instead.
pub fn build_launch_command(tool: CliTool, mode: LaunchMode) -> String {
    let commands = CliCommandSettings::default();
    LaunchSpec {
        tool,
        mode,
        base: base_command(&commands, tool, mode),
        model: ModelSpec::default(),
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
        account_dir: None,
        selector: None,
        team: None,
    }
    .render()
    .command
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
    let mut created_session = false;

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

        created_session = true;
    }

    if created_session {
        wait_for_tmux_session_ready(TMUX_SESSION_NAME, verify_tmux_session_ready)?;
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
        "NODE_EXTRA_CA_CERTS",
        "PATH",
        "TAURHAUS_DATA_DIR",
    ];

    for var in PROPAGATE_VARS {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                if *var == "PATH" {
                    sync_tmux_path_environment(&val);
                    continue;
                }
                // The tmux server lives in WSL; a Windows-form data dir must
                // cross in its /mnt form, exactly as the daemon launcher
                // converts it (daemon_data_dir_env_value_for_launch).
                let val = if *var == "TAURHAUS_DATA_DIR" {
                    crate::provider::path::to_linux(&val).unwrap_or(val)
                } else {
                    val
                };
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

fn build_tmux_shell_command(project_path: &str, command: &str) -> String {
    let escaped_path = shell_escape(project_path);
    let inner_cmd = format!("cd {escaped_path} && {command}; exec \"$SHELL\"");
    format!("exec \"$SHELL\" -ic {}", shell_escape(&inner_cmd))
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

    use crate::session_scanner::process::ProcessInfo;

    #[test]
    fn agy_stop_waits_for_presence_lock_release() {
        // Regression: commit 9a66d1c treated slash-exit tools as stopped only
        // when tmux returned to a shell; agy exposes a stronger clean-shutdown
        // signal through its conversation presence lock.
        use fs2::FileExt;

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("conversation.lock");
        let lock = std::fs::File::create(&path).unwrap();
        lock.lock_exclusive().unwrap();
        assert!(!stop_has_completed(Some("agy"), Some(&path), None));
        FileExt::unlock(&lock).unwrap();
        assert!(stop_has_completed(Some("agy"), Some(&path), None));
    }

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
    // Antigravity command tests
    // -----------------------------------------------------------------------

    #[test]
    fn grok_stop_completes_when_its_registry_row_is_released() {
        // Regression: commit 16de5ec stopped every SlashExit harness on the
        // tmux floor alone, so `/quit` was only ever confirmed by the pane
        // returning to a shell — a pane still showing `grok` after a clean exit
        // was killed on the timeout instead of recognised as finished.
        let temp = tempfile::TempDir::new().unwrap();
        let home = temp.path().to_path_buf();
        std::fs::write(
            home.join("active_sessions.json"),
            r#"[{"session_id":"01a04585-2d53-7123-8000-9a0f4d0b21ce","pid":4242,"cwd":"/home/user/projects/grok","opened_at":"2026-08-27T23:22:06.993848110Z"}]"#,
        )
        .unwrap();
        let release = StopRegistryRelease {
            home: home.clone(),
            pid: 4242,
        };

        assert!(!stop_has_completed(Some("grok"), None, Some(&release)));

        std::fs::write(home.join("active_sessions.json"), "[]").unwrap();
        assert!(stop_has_completed(Some("grok"), None, Some(&release)));
    }

    #[test]
    fn the_registry_proof_outranks_the_pane_returning_to_a_shell() {
        // Regression: commit 358a7c9 ORed grok's registry proof with the tmux
        // floor, so the poll settled on the first shell the pane reported.
        // Every launch runs `exec "$SHELL" -ic '…; exec "$SHELL"'`, so a shell
        // can sit on top of the pane while the grok child still holds its
        // `active_sessions.json` row — and an unreadable registry passed as a
        // stop for the same reason. Both killed the pane while the session was
        // still resident.
        let temp = tempfile::TempDir::new().unwrap();
        let home = temp.path().to_path_buf();
        let registry = home.join("active_sessions.json");
        std::fs::write(
            &registry,
            r#"[{"session_id":"01a04585-2d53-7123-8000-9a0f4d0b21ce","pid":4242,"cwd":"/home/user/projects/grok","opened_at":"2026-08-27T23:22:06.993848110Z"}]"#,
        )
        .unwrap();
        let release = StopRegistryRelease {
            home: home.clone(),
            pid: 4242,
        };

        assert!(
            !stop_has_completed(Some("bash"), None, Some(&release)),
            "a pane back at a shell does not release grok's registry row"
        );

        std::fs::write(
            &registry,
            r#"[{"session_id":"01a04585-2d53-7123-8000-9a0f4d0b21ce","pid":4242,"cwd":"/p","#,
        )
        .unwrap();
        assert!(
            !stop_has_completed(Some("bash"), None, Some(&release)),
            "an unreadable registry is silence, not a stop"
        );

        std::fs::write(&registry, "[]").unwrap();
        assert!(
            stop_has_completed(Some("bash"), None, Some(&release)),
            "the released row is the proof, whatever the pane runs"
        );

        // A harness with no registry proof keeps the tmux floor.
        assert!(stop_has_completed(Some("bash"), None, None));
        assert!(!stop_has_completed(Some("grok"), None, None));
    }

    #[test]
    fn grok_declares_quit_as_its_exit_command() {
        assert_eq!(
            build_launch_command(CliTool::Grok, LaunchMode::Fresh),
            "grok --always-approve"
        );
        assert_eq!(cli_tool::spec(CliTool::Grok).exit_command, "/quit");
        assert!(cli_tool::spec(CliTool::Grok).stop_registry_release);
    }

    #[test]
    fn the_registry_probe_follows_the_pane_shell_to_the_harness_child() {
        // Regression: commit 358a7c9 probed `active_sessions.json` with tmux's
        // `#{pane_pid}`. Every launch runs inside `exec "$SHELL" -ic '…'`, so
        // that pid is the shell: the registry row belongs to its grok child,
        // and an inline `GROK_HOME=` assignment is only in the child's
        // environment. The probe found nothing and every stop fell back to the
        // timeout kill.
        let pane_tty = "/dev/pts/9";
        let processes = vec![
            ProcessInfo {
                pid: 4242,
                project_path: "/home/user/projects/grok".to_string(),
                tty: pane_tty.to_string(),
                args: "grok --always-approve".to_string(),
                cli_tool: CliTool::Grok,
            },
            ProcessInfo {
                pid: 5151,
                project_path: "/home/user/projects/other".to_string(),
                tty: "/dev/pts/3".to_string(),
                args: "grok --always-approve".to_string(),
                cli_tool: CliTool::Grok,
            },
        ];

        assert_eq!(
            registry_pid_candidates(CliTool::Grok, 1111, Some(pane_tty), &processes),
            vec![4242, 1111],
            "the harness child on this pane's tty is tried before the pane shell"
        );
        assert_eq!(
            registry_pid_candidates(CliTool::Grok, 1111, None, &processes),
            vec![1111]
        );
    }

    #[test]
    fn a_prompted_grok_session_still_offers_its_registry_pid() {
        // Regression: commit 54c9103 classified the joined command line, so a
        // TUI started as `grok "help me"` never entered the process inventory
        // at all: no tool process shared the pane's tty, the registry row that
        // proves a clean `/quit` was never probed, and the stop fell back to
        // the tmux floor.
        let argv = ["grok".to_string(), "help me".to_string()];
        let tool = crate::session_scanner::process::detect_cli_tool_argv(&argv)
            .expect("a prompted grok TUI is a session");
        let pane_tty = "/dev/pts/9";
        let processes = vec![ProcessInfo {
            pid: 4242,
            project_path: "/home/user/projects/grok".to_string(),
            tty: pane_tty.to_string(),
            args: argv.join(" "),
            cli_tool: tool,
        }];

        assert_eq!(
            registry_pid_candidates(CliTool::Grok, 1111, Some(pane_tty), &processes),
            vec![4242, 1111],
            "the prompted grok child is probed before the pane shell"
        );
    }

    #[test]
    fn the_registry_release_binds_the_home_of_the_pid_that_holds_a_row() {
        // Regression: commit 358a7c9 read the registry of the default grok home
        // for the pane's shell pid, so a session started under a second
        // GROK_HOME had no clean-stop proof at all.
        let tmp = tempfile::TempDir::new().unwrap();
        let default_home = tmp.path().join(".grok");
        let work_home = tmp.path().join(".grok-work");
        std::fs::create_dir_all(&default_home).unwrap();
        std::fs::create_dir_all(&work_home).unwrap();
        std::fs::write(default_home.join("active_sessions.json"), "[]").unwrap();
        std::fs::write(
            work_home.join("active_sessions.json"),
            r#"[{"session_id":"01a04585-2d53-7123-8000-9a0f4d0b21ce","pid":4242,"cwd":"/home/user/projects/grok","opened_at":"2026-08-27T23:22:06.993848110Z"}]"#,
        )
        .unwrap();
        let home_for = |pid: u32| {
            if pid == 4242 {
                work_home.clone()
            } else {
                default_home.clone()
            }
        };

        let release = resolve_registry_release_with(&[1111, 4242], &home_for)
            .expect("the grok child holds the row");
        assert_eq!(release.pid, 4242);
        assert_eq!(release.home, work_home);
        assert!(resolve_registry_release_with(&[1111], &home_for).is_none());
    }

    #[test]
    fn a_half_written_registry_is_not_a_clean_stop() {
        // Regression: commit 358a7c9 read every failed or unparsable registry
        // read as an empty registry, so a truncated file caught mid-rewrite
        // looked exactly like the row grok removes on `/quit` and the pane was
        // killed in the middle of a turn.
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let registry = home.join("active_sessions.json");
        std::fs::write(
            &registry,
            r#"[{"session_id":"01a04585-2d53-7123-8000-9a0f4d0b21ce","pid":4242,"cwd":"/p","#,
        )
        .unwrap();
        let release = StopRegistryRelease {
            home: home.clone(),
            pid: 4242,
        };

        assert!(
            !stop_has_completed(Some("grok"), None, Some(&release)),
            "an unreadable registry proves nothing"
        );

        std::fs::write(&registry, "[]").unwrap();
        assert!(stop_has_completed(Some("grok"), None, Some(&release)));
    }

    #[test]
    fn grok_gets_its_documented_shutdown_budget_before_the_pane_is_killed() {
        // Regression: commit 358a7c9 gave every harness the shared five-second
        // stop timeout. grok runs its `SessionEnd` hooks inside a documented
        // ten-second exit budget, so a correct shutdown was force-killed.
        assert!(
            cli_tool::spec(CliTool::Grok).stop_timeout >= std::time::Duration::from_secs(10),
            "grok's SessionEnd hooks own a ten-second exit budget"
        );
        assert_eq!(
            cli_tool::spec(CliTool::Claude).stop_timeout,
            std::time::Duration::from_secs(5)
        );
    }

    #[test]
    fn build_agy_fresh_command() {
        assert_eq!(
            build_launch_command(CliTool::Agy, LaunchMode::Fresh),
            "agy --dangerously-skip-permissions"
        );
    }

    #[test]
    fn build_agy_resume_command() {
        assert_eq!(
            build_launch_command(CliTool::Agy, LaunchMode::Resume),
            "agy --dangerously-skip-permissions --conversation {session_id}"
        );
    }

    // -----------------------------------------------------------------------
    // Command override validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_command_override_accepts_tool_commands() {
        assert!(validate_command_override("claude --dangerously-skip-permissions").is_ok());
        assert!(validate_command_override("codex --yolo").is_ok());
        assert!(validate_command_override("agy --conversation session-id").is_ok());
        assert!(validate_command_override("/usr/local/bin/claude --flag").is_ok());
    }

    // Regression: configuring a CLI command such as `claude2` (a second
    // Claude install/alias) in Settings made every launch fail with
    // "Could not start Claude" because the validator required the basename
    // to be an exact built-in harness name. The Settings UI offers a free-text
    // field, so whatever works in the user's terminal must work here too:
    // aliases, alternate binaries, env-var prefixes, wrappers, shell syntax.
    #[test]
    fn validate_command_override_accepts_free_form_shell_commands() {
        assert!(validate_command_override("claude2 --dangerously-skip-permissions").is_ok());
        assert!(validate_command_override("CLAUDE_CONFIG_DIR=~/.claude-account2 claude").is_ok());
        assert!(validate_command_override("env FOO=bar claude --model opus").is_ok());
        assert!(validate_command_override("npx @anthropic-ai/claude-code --resume").is_ok());
        assert!(validate_command_override("~/bin/my-claude-wrapper.sh --flag").is_ok());
        assert!(validate_command_override("claude --add-dir \"$HOME/notes\"").is_ok());
        assert!(validate_command_override("source ~/.profile && claude").is_ok());
        assert!(validate_command_override("claude 2>&1 | tee ~/claude.log").is_ok());
    }

    #[test]
    fn validate_command_override_rejects_empty_and_multiline() {
        assert!(validate_command_override("").is_err());
        assert!(validate_command_override("   ").is_err());
        assert!(validate_command_override("claude\nrm -rf /").is_err());
        assert!(validate_command_override("claude\r").is_err());
        assert!(validate_command_override("claude\0").is_err());
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
    fn tmux_path_looks_windows_style_rejects_semicolon_lists() {
        assert!(tmux_path_looks_windows_style(
            r"C:\Windows\system32;C:\Users\user\.bun\bin"
        ));
        assert!(!tmux_path_looks_windows_style(
            "/usr/local/bin:/usr/bin:/bin"
        ));
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
