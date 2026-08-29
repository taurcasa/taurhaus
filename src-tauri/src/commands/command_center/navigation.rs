use crate::commands::logging::LogFileState;
use crate::commands::terminal_settings::load_terminal_settings;
use serde_json::{Map, Value};

use super::*;

pub(super) fn stop_cli_session_impl(
    log_file: &LogFileState,
    provider: &ProviderState,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> Result<(), String> {
    let tool = cli_tool.unwrap_or_default();

    // Stopping a managed member is the moment the operator's own saved effort
    // default has to come back: mesh's `/effort` rewrote it for the assignment,
    // and nothing else would put it right. Runs before the pane goes away, so
    // the member that owns it can still be identified.
    crate::coordination::effort_default::restore_effort_default_for_pane(
        &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        &tmux_pane,
    );

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "stop-session";
            let daemon_method = protocol::method::STOP_SESSION;
            let request = protocol::DaemonRequest::new(
                id,
                daemon_method,
                protocol::StopSessionParams {
                    tmux_pane: tmux_pane.clone(),
                    cli_tool: tool,
                },
            );
            let mut request_fields = Map::new();
            request_fields.insert(
                "caller".to_string(),
                Value::String("command_center.stop".to_string()),
            );
            request_fields.insert(
                "daemon_request_id".to_string(),
                Value::String(id.to_string()),
            );
            request_fields.insert(
                "daemon_method".to_string(),
                Value::String(daemon_method.to_string()),
            );
            request_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
            request_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
            log_file.emit(
                "info",
                "command_center",
                "command_center.stop.daemon_request",
                Some("Submitting stop request to daemon".to_string()),
                request_fields,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let mut success_fields = Map::new();
                    success_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.stop".to_string()),
                    );
                    success_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    success_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    success_fields
                        .insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    success_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    log_file.emit(
                        "info",
                        "command_center",
                        "command_center.stop.daemon_success",
                        Some("Stop succeeded via daemon".to_string()),
                        success_fields,
                    );
                    return Ok(());
                }
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    let mut fail_fields = Map::new();
                    fail_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.stop".to_string()),
                    );
                    fail_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    fail_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    fail_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    fail_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    fail_fields.insert("error".to_string(), Value::String(msg.clone()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.stop.daemon_failed",
                        Some("Stop failed via daemon".to_string()),
                        fail_fields,
                    );
                    return Err(format!("Failed to stop session: {msg}"));
                }
                Err(e) => {
                    let mut unreachable_fields = Map::new();
                    unreachable_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.stop".to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    unreachable_fields
                        .insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    unreachable_fields
                        .insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    unreachable_fields.insert("error".to_string(), Value::String(e.to_string()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.stop.daemon_unreachable",
                        Some("Daemon unreachable during stop".to_string()),
                        unreachable_fields,
                    );
                    tracing::warn!(error = %e, "Daemon unreachable for stop");
                }
            }
        }
    }

    let mut fallback_fields = Map::new();
    fallback_fields.insert(
        "caller".to_string(),
        Value::String("command_center.stop".to_string()),
    );
    fallback_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
    fallback_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
    log_file.emit(
        "info",
        "command_center",
        "command_center.stop.local_fallback",
        Some("Falling back to local tmux stop".to_string()),
        fallback_fields,
    );
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
            let daemon_method = protocol::method::NAVIGATE_TO_SESSION;
            let request = protocol::DaemonRequest::new(
                id,
                daemon_method,
                protocol::NavigateToSessionParams {
                    tmux_session: tmux_session.clone(),
                    tmux_window: tmux_window.clone(),
                    tmux_pane: tmux_pane.clone(),
                },
            );
            let mut daemon_request_fields = Map::new();
            daemon_request_fields.insert(
                "caller".to_string(),
                Value::String("command_center.navigate".to_string()),
            );
            daemon_request_fields.insert(
                "daemon_request_id".to_string(),
                Value::String(id.to_string()),
            );
            daemon_request_fields.insert(
                "daemon_method".to_string(),
                Value::String(daemon_method.to_string()),
            );
            daemon_request_fields.insert(
                "tmux_session".to_string(),
                Value::String(tmux_session.clone()),
            );
            daemon_request_fields.insert(
                "tmux_window".to_string(),
                Value::String(tmux_window.clone()),
            );
            daemon_request_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
            daemon_request_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
            log_file.emit(
                "info",
                "command_center",
                "command_center.navigate.daemon_request",
                Some("Submitting navigation request to daemon".to_string()),
                daemon_request_fields,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let mut success_fields = Map::new();
                    success_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.navigate".to_string()),
                    );
                    success_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    success_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    success_fields.insert(
                        "tmux_session".to_string(),
                        Value::String(tmux_session.clone()),
                    );
                    success_fields.insert(
                        "tmux_window".to_string(),
                        Value::String(tmux_window.clone()),
                    );
                    success_fields
                        .insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    success_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
                    log_file.emit(
                        "info",
                        "command_center",
                        "command_center.navigate.daemon_success",
                        Some("Navigation succeeded via daemon".to_string()),
                        success_fields,
                    );
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
                    let mut fail_fields = Map::new();
                    fail_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.navigate".to_string()),
                    );
                    fail_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    fail_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    fail_fields.insert(
                        "tmux_session".to_string(),
                        Value::String(tmux_session.clone()),
                    );
                    fail_fields.insert(
                        "tmux_window".to_string(),
                        Value::String(tmux_window.clone()),
                    );
                    fail_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    fail_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
                    fail_fields.insert("error".to_string(), Value::String(msg.clone()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.navigate.daemon_failed",
                        Some("Navigation failed via daemon".to_string()),
                        fail_fields,
                    );
                    return Err(format!("Failed to navigate: {msg}"));
                }
                Err(e) => {
                    let mut unreachable_fields = Map::new();
                    unreachable_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.navigate".to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    unreachable_fields.insert(
                        "tmux_session".to_string(),
                        Value::String(tmux_session.clone()),
                    );
                    unreachable_fields.insert(
                        "tmux_window".to_string(),
                        Value::String(tmux_window.clone()),
                    );
                    unreachable_fields
                        .insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
                    unreachable_fields
                        .insert("open_terminal".to_string(), Value::Bool(should_open));
                    unreachable_fields.insert("error".to_string(), Value::String(e.to_string()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.navigate.daemon_unreachable",
                        Some("Daemon unreachable during navigation".to_string()),
                        unreachable_fields,
                    );
                    tracing::warn!(error = %e, "Daemon unreachable for navigate");
                }
            }
        }
    }

    let mut fallback_fields = Map::new();
    fallback_fields.insert(
        "caller".to_string(),
        Value::String("command_center.navigate".to_string()),
    );
    fallback_fields.insert(
        "tmux_session".to_string(),
        Value::String(tmux_session.clone()),
    );
    fallback_fields.insert(
        "tmux_window".to_string(),
        Value::String(tmux_window.clone()),
    );
    fallback_fields.insert("tmux_pane".to_string(), Value::String(tmux_pane.clone()));
    fallback_fields.insert("open_terminal".to_string(), Value::Bool(should_open));
    log_file.emit(
        "info",
        "command_center",
        "command_center.navigate.local_fallback",
        Some("Falling back to local tmux navigation".to_string()),
        fallback_fields,
    );
    crate::session_scanner::control::navigate_to_pane(&tmux_session, &tmux_window, &tmux_pane)
        .map_err(|e| format!("Failed to navigate: {e}"))
}
