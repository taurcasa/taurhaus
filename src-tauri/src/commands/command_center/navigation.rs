use crate::commands::logging::LogFileState;
use crate::commands::terminal_settings::load_terminal_settings;
use serde_json::{Map, Value};

use super::*;

pub(super) fn stop_cli_session_impl(
    provider: &ProviderState,
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

pub(super) fn navigate_to_session_impl(
    db: &DbState,
    provider: &ProviderState,
    log_file: &LogFileState,
    tmux_session: String,
    tmux_window: String,
    tmux_pane: String,
    open_terminal: Option<bool>,
) -> Result<(), String> {
    let should_open = open_terminal.unwrap_or(false);

    let mut navigation_fields = Map::new();
    navigation_fields.insert(
        "tmux_session".to_string(),
        Value::String(tmux_session.clone()),
    );
    navigation_fields.insert(
        "tmux_window".to_string(),
        Value::String(tmux_window.clone()),
    );
    navigation_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
    navigation_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
    log_file.emit(
        "info",
        "command_center",
        "command_center.navigate",
        Some("Navigate to tmux session".to_string()),
        navigation_fields,
    );
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
                    let ts = load_terminal_settings(db);
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
