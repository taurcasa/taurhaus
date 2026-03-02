use std::io::Write;

use tauri::State;

use crate::commands::logging::LogFileState;
use crate::commands::projects::DbState;
use crate::commands::terminal_settings::load_terminal_settings;
use crate::daemon::protocol::{self, LaunchMode};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::control::{resolve_configured_tool_command, TMUX_SESSION_NAME};
use crate::session_scanner::ClaudeSession;
use crate::ProviderState;

#[tauri::command]
pub fn list_claude_sessions(
    provider: State<'_, ProviderState>,
) -> Result<Vec<ClaudeSession>, String> {
    if let Some(ref daemon) = provider.daemon {
        if !daemon.is_connected() {
            daemon.try_reconnect();
        }

        if daemon.is_connected() {
            let id = "list-sessions";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LIST_CLAUDE_SESSIONS,
                serde_json::Value::Null,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let mut sessions = decode_daemon_session_list(response.result)?;

                    if !crate::daemon::launcher::is_native_daemon() {
                        if let Some(ref distro) = provider.wsl_distro {
                            for session in &mut sessions {
                                if session.project_path.starts_with('/') {
                                    session.project_path = crate::provider::path::to_windows(
                                        &session.project_path,
                                        distro,
                                    );
                                }
                            }
                        }
                    }

                    return Ok(sessions);
                }
                Ok(response) => {
                    tracing::warn!(error = ?response.error, "Daemon returned error for session listing");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to reach daemon for session listing");
                }
            }
        }
    }

    let fallback = crate::session_scanner::scan_sessions();
    tracing::debug!(
        count = fallback.len(),
        "list_claude_sessions: fallback scan"
    );
    Ok(fallback)
}

#[tauri::command]
pub fn launch_claude_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
) -> Result<protocol::LaunchSessionResult, String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch_claude_session: project_id={project_id} mode={mode:?} tool={tool:?}");
    }

    let project_path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let project = crate::db::queries::get_project(&conn, &project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {project_id}"))?;
        project.path
    };

    let linux_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(
            f,
            "[cmd-center] launch: db_path={project_path} linux_path={linux_path}"
        );
    }

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "launch-session";
            let ts = load_terminal_settings(&db);
            let tool_cmd = resolve_configured_tool_command(&ts.cli_commands, tool, mode);
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LAUNCH_SESSION,
                protocol::LaunchSessionParams {
                    project_path: linux_path.clone(),
                    mode,
                    cli_tool: tool,
                    tmux_layout: ts.tmux_layout.clone(),
                    command_override: Some(tool_cmd),
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let result = decode_daemon_launch_result(response.result)?;

                    if let Ok(mut f) = log_file.0.lock() {
                        let _ = writeln!(
                            f,
                            "[cmd-center] launch SUCCESS via daemon: window={} pane={}",
                            result.tmux_window, result.tmux_pane
                        );
                    }

                    let tmux_session = result.tmux_session.as_deref().unwrap_or(TMUX_SESSION_NAME);
                    let ts = load_terminal_settings(&db);
                    let _ = crate::terminal::handle_terminal(
                        crate::terminal::TerminalIntent::EnsureOpen {
                            distro: provider.wsl_distro.clone(),
                            tmux_session: tmux_session.to_string(),
                            emulator: ts.emulator,
                            custom_command: ts.custom_command,
                        },
                    );
                    return Ok(result);
                }
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    if let Ok(mut f) = log_file.0.lock() {
                        let _ = writeln!(f, "[cmd-center] launch FAILED via daemon: {msg}");
                    }
                    return Err(format!("Failed to launch session: {msg}"));
                }
                Err(e) => {
                    if let Ok(mut f) = log_file.0.lock() {
                        let _ = writeln!(f, "[cmd-center] launch: daemon unreachable: {e}");
                    }
                    tracing::warn!(error = %e, "Daemon unreachable for launch");
                }
            }
        }
    }

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch: falling back to direct tmux");
    }
    let ts = load_terminal_settings(&db);
    let tool_cmd = resolve_configured_tool_command(&ts.cli_commands, tool, mode);
    let (session, window, pane) = crate::session_scanner::control::launch_in_tmux_with_layout(
        &linux_path,
        mode,
        tool,
        &ts.tmux_layout,
        Some(&tool_cmd),
    )
    .map_err(|e| format!("Failed to launch session: {e}"))?;

    #[cfg(target_os = "macos")]
    {
        let _ = crate::terminal::handle_terminal(crate::terminal::TerminalIntent::EnsureOpen {
            distro: None,
            tmux_session: session.clone(),
            emulator: ts.emulator,
            custom_command: ts.custom_command,
        });
    }

    Ok(protocol::LaunchSessionResult {
        tmux_session: Some(session),
        tmux_window: window,
        tmux_pane: pane,
    })
}

#[tauri::command]
pub fn stop_claude_session(
    provider: State<'_, ProviderState>,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> Result<(), String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "stop-session";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::STOP_SESSION,
                protocol::StopSessionParams {
                    tmux_pane: tmux_pane.clone(),
                    cli_tool: tool,
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => return Ok(()),
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(format!("Failed to stop session: {msg}"));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Daemon unreachable for stop");
                }
            }
        }
    }

    crate::session_scanner::control::stop_session(&tmux_pane, tool)
        .map_err(|e| format!("Failed to stop session: {e}"))
}

#[tauri::command]
pub fn navigate_to_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    tmux_session: String,
    tmux_window: String,
    tmux_pane: String,
    open_terminal: Option<bool>,
) -> Result<(), String> {
    let should_open = open_terminal.unwrap_or(false);

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] navigate_to_session: session={tmux_session} window={tmux_window} pane={tmux_pane} open={should_open}");
    }
    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "navigate-session";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::NAVIGATE_TO_SESSION,
                protocol::NavigateToSessionParams {
                    tmux_session: tmux_session.clone(),
                    tmux_window: tmux_window.clone(),
                    tmux_pane: tmux_pane.clone(),
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let ts = load_terminal_settings(&db);
                    let intent = if should_open || cfg!(target_os = "macos") {
                        crate::terminal::TerminalIntent::EnsureOpen {
                            distro: provider.wsl_distro.clone(),
                            tmux_session: tmux_session.clone(),
                            emulator: ts.emulator,
                            custom_command: ts.custom_command,
                        }
                    } else {
                        crate::terminal::TerminalIntent::FocusOnly {
                            emulator: ts.emulator,
                        }
                    };
                    let _ = crate::terminal::handle_terminal(intent);
                    return Ok(());
                }
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(format!("Failed to navigate: {msg}"));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Daemon unreachable for navigate");
                }
            }
        }
    }

    crate::session_scanner::control::navigate_to_pane(&tmux_session, &tmux_window, &tmux_pane)
        .map_err(|e| format!("Failed to navigate: {e}"))
}

#[tauri::command]
pub fn record_session_activity(
    db: State<'_, DbState>,
    project_path: String,
    cli_tool: String,
    started_at: String,
    ended_at: String,
    active_duration_ms: i64,
    total_duration_ms: i64,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::db::activity_queries::insert_session_activity(
        &conn,
        &project_path,
        &cli_tool,
        &started_at,
        &ended_at,
        active_duration_ms,
        total_duration_ms,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project_activity(
    db: State<'_, DbState>,
    project_path: String,
) -> Result<crate::db::activity_queries::ProjectActivityStats, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::db::activity_queries::get_project_activity(&conn, &project_path)
        .map_err(|e| e.to_string())
}

fn decode_daemon_session_list(
    payload: Option<serde_json::Value>,
) -> Result<Vec<ClaudeSession>, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|e| {
            tracing::warn!(error = %e, "Failed to deserialize session list from daemon");
            format!("Session list decode error: {e}")
        }),
        None => Ok(Vec::new()),
    }
}

fn decode_daemon_launch_result(
    payload: Option<serde_json::Value>,
) -> Result<protocol::LaunchSessionResult, String> {
    let value = payload.ok_or_else(|| "Invalid launch result from daemon".to_string())?;
    serde_json::from_value(value).map_err(|e| {
        tracing::warn!(error = %e, "Failed to deserialize launch result from daemon");
        format!("Invalid launch result from daemon: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn launch_mode_deserializes_valid_values_and_rejects_invalid() {
        for (raw, expected) in [
            ("\"continue\"", LaunchMode::Continue),
            ("\"fresh\"", LaunchMode::Fresh),
            ("\"resume\"", LaunchMode::Resume),
        ] {
            let mode: LaunchMode = serde_json::from_str(raw).unwrap();
            assert_eq!(mode, expected);
        }
        assert!(serde_json::from_str::<LaunchMode>("\"invalid\"").is_err());
    }

    #[test]
    fn daemon_session_decode_handles_missing_invalid_and_valid_payloads() {
        assert!(decode_daemon_session_list(None).unwrap().is_empty());
        assert!(
            decode_daemon_session_list(Some(serde_json::json!({"not": "a session list"}))).is_err()
        );
        assert!(decode_daemon_session_list(Some(serde_json::json!([])))
            .unwrap()
            .is_empty());

        let payload = Some(serde_json::json!([
            {"pid": 1234, "project_path": "/tmp/project-a", "tty": "/dev/pts/1", "args": "claude --continue", "cli_tool": "claude", "tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%1", "tmux_window_name": "a", "state": "active", "session_id": "sess-a", "jsonl_path": "/tmp/a.jsonl"},
            {"pid": 5678, "project_path": "/tmp/project-b", "tty": "/dev/pts/2", "args": "codex --yolo", "cli_tool": "codex", "tmux_session": null, "tmux_window": null, "tmux_pane": null, "tmux_window_name": null, "state": "idle", "session_id": null, "jsonl_path": null}
        ]));
        let sessions = decode_daemon_session_list(payload).expect("valid session payload");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].project_path, "/tmp/project-a");
        assert_eq!(sessions[1].project_path, "/tmp/project-b");
    }

    #[test]
    fn daemon_launch_decode_handles_missing_invalid_and_valid_payloads() {
        let payload = Some(
            serde_json::json!({"tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%2"}),
        );
        let result = decode_daemon_launch_result(payload).expect("valid launch payload");
        assert_eq!(result.tmux_session.as_deref(), Some("taurhaus"));
        assert_eq!(result.tmux_window, "1");
        assert_eq!(result.tmux_pane, "%2");

        let err = decode_daemon_launch_result(Some(serde_json::json!({"unexpected": "shape"})))
            .expect_err("invalid payload should error");
        assert!(err.contains("Invalid launch result from daemon"));
        assert_eq!(
            decode_daemon_launch_result(None).expect_err("missing payload should error"),
            "Invalid launch result from daemon"
        );
    }

    #[test]
    fn resolve_tool_command_defaults_are_non_empty_and_match_expected_values() {
        let cmds = crate::models::CliCommandSettings::default();
        for tool in [CliTool::Claude, CliTool::Codex, CliTool::Gemini] {
            for mode in [LaunchMode::Continue, LaunchMode::Fresh, LaunchMode::Resume] {
                let command = resolve_configured_tool_command(&cmds, tool, mode);
                assert!(
                    !command.trim().is_empty(),
                    "command must be non-empty for {tool:?}/{mode:?}"
                );
            }
        }
        for (tool, mode, expected) in [
            (
                CliTool::Claude,
                LaunchMode::Continue,
                "claude --dangerously-skip-permissions --continue",
            ),
            (
                CliTool::Claude,
                LaunchMode::Fresh,
                "claude --dangerously-skip-permissions",
            ),
            (
                CliTool::Claude,
                LaunchMode::Resume,
                "claude --dangerously-skip-permissions --resume",
            ),
            (CliTool::Codex, LaunchMode::Continue, "codex --yolo"),
            (CliTool::Codex, LaunchMode::Fresh, "codex --yolo"),
            (
                CliTool::Codex,
                LaunchMode::Resume,
                "codex resume --last --yolo",
            ),
            (
                CliTool::Gemini,
                LaunchMode::Continue,
                "gemini --yolo --resume",
            ),
            (CliTool::Gemini, LaunchMode::Fresh, "gemini --yolo"),
            (
                CliTool::Gemini,
                LaunchMode::Resume,
                "gemini --yolo --resume",
            ),
        ] {
            assert_eq!(
                resolve_configured_tool_command(&cmds, tool, mode),
                expected
            );
        }
    }

    #[test]
    fn load_terminal_settings_returns_default_on_query_and_lock_errors() {
        let db = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        assert_eq!(
            load_terminal_settings(&db),
            crate::models::TerminalSettings::default()
        );

        let poisoned = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned.0.lock().unwrap();
            panic!("intentional poison");
        }));
        assert_eq!(
            load_terminal_settings(&poisoned),
            crate::models::TerminalSettings::default()
        );
    }
}
