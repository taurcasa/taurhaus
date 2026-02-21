//! Command Center — Tauri IPC commands for Claude Code session management.
//!
//! These commands are called by the frontend to list, launch, stop, and
//! navigate to Claude Code sessions running in tmux.

use std::io::Write;

use tauri::State;

use crate::commands::logging::LogFileState;
use crate::commands::projects::DbState;
use crate::daemon::protocol::{self, LaunchMode};
use crate::session_scanner::ClaudeSession;
use crate::ProviderState;

/// List all running Claude Code sessions.
///
/// Tries the daemon first (for Windows → WSL scenarios), falls back to
/// direct local scanning on Linux. Returns empty vec if neither is available.
#[tauri::command]
pub fn list_claude_sessions(
    provider: State<'_, ProviderState>,
) -> Result<Vec<ClaudeSession>, String> {
    // Try daemon first (required on Windows where we can't run ps/tmux directly)

    if let Some(ref daemon) = provider.daemon {
        // If disconnected, try a rate-limited inline reconnect (max once per 5s)
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
                    let mut sessions: Vec<ClaudeSession> = response
                        .result
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();

                    // Convert Linux project paths to WSL UNC paths so the frontend
                    // can match sessions to projects (which are stored as UNC paths).
                    if let Some(ref distro) = provider.wsl_distro {
                        for session in &mut sessions {
                            if session.project_path.starts_with('/') {
                                session.project_path =
                                    crate::provider::path::linux_to_wsl_unc(
                                        &session.project_path,
                                        distro,
                                    );
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

    // Fall back to direct scan (works on Linux where ps/tmux are available)
    let fallback = crate::session_scanner::scan_sessions();
    tracing::debug!(count = fallback.len(), "list_claude_sessions: fallback scan");
    Ok(fallback)
}

/// Launch a new Claude Code session for a project.
///
/// Resolves the project path from the database, then creates a tmux window
/// and starts Claude Code in the specified mode.
#[tauri::command]
pub fn launch_claude_session(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    project_id: String,
    mode: LaunchMode,
) -> Result<protocol::LaunchSessionResult, String> {
    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch_claude_session: project_id={project_id} mode={mode:?}");
    }

    // Resolve project path from DB
    let project_path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let project = crate::db::queries::get_project(&conn, &project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {project_id}"))?;
        project.path
    };

    // Convert WSL UNC path to Linux path if needed
    let linux_path = crate::provider::path::wsl_unc_to_linux(&project_path)
        .unwrap_or_else(|| project_path.clone());

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch: db_path={project_path} linux_path={linux_path}");
    }

    // Try daemon first
    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "launch-session";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LAUNCH_SESSION,
                protocol::LaunchSessionParams {
                    project_path: linux_path.clone(),
                    mode,
                },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let result: protocol::LaunchSessionResult = response
                        .result
                        .and_then(|v| serde_json::from_value(v).ok())
                        .ok_or("Invalid launch result from daemon")?;

                    if let Ok(mut f) = log_file.0.lock() {
                        let _ = writeln!(f, "[cmd-center] launch SUCCESS via daemon: window={} pane={}", result.tmux_window, result.tmux_pane);
                    }

                    // Focus Windows Terminal after successful launch
                    let _ = crate::terminal::focus_windows_terminal();
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

    // Fall back to direct launch (Linux dev)
    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch: falling back to direct tmux");
    }
    let (window, pane) =
        crate::session_scanner::control::launch_in_tmux(&linux_path, mode)
            .map_err(|e| format!("Failed to launch session: {e}"))?;

    Ok(protocol::LaunchSessionResult {
        tmux_window: window,
        tmux_pane: pane,
    })
}

/// Stop a running Claude Code session by sending /exit to its tmux pane.
#[tauri::command]
pub fn stop_claude_session(
    provider: State<'_, ProviderState>,
    tmux_pane: String,
) -> Result<(), String> {
    // Try daemon first
    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "stop-session";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::STOP_SESSION,
                protocol::StopSessionParams {
                    tmux_pane: tmux_pane.clone(),
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

    // Fall back to direct stop (Linux dev)
    crate::session_scanner::control::stop_session(&tmux_pane)
        .map_err(|e| format!("Failed to stop session: {e}"))
}

/// Navigate to a Claude Code session's tmux pane and focus the terminal.
#[tauri::command]
pub fn navigate_to_session(
    provider: State<'_, ProviderState>,
    log_file: State<'_, LogFileState>,
    tmux_session: String,
    tmux_window: String,
    tmux_pane: String,
) -> Result<(), String> {
    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] navigate_to_session: session={tmux_session} window={tmux_window} pane={tmux_pane}");
    }
    // Try daemon first
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
                    let _ = crate::terminal::focus_windows_terminal();
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

    // Fall back to direct navigate (Linux dev)
    crate::session_scanner::control::navigate_to_pane(
        &tmux_session,
        &tmux_window,
        &tmux_pane,
    )
    .map_err(|e| format!("Failed to navigate: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_mode_deserializes_from_frontend_string() {
        // Frontend sends mode as a lowercase string like "continue"
        let mode: LaunchMode = serde_json::from_str("\"continue\"").unwrap();
        assert_eq!(mode, LaunchMode::Continue);

        let mode: LaunchMode = serde_json::from_str("\"fresh\"").unwrap();
        assert_eq!(mode, LaunchMode::Fresh);

        let mode: LaunchMode = serde_json::from_str("\"resume\"").unwrap();
        assert_eq!(mode, LaunchMode::Resume);
    }

    #[test]
    fn launch_mode_rejects_invalid_string() {
        let result: Result<LaunchMode, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
    }
}
