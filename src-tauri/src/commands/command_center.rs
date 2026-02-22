//! Command Center — Tauri IPC commands for Claude Code session management.
//!
//! These commands are called by the frontend to list, launch, stop, and
//! navigate to Claude Code sessions running in tmux.

use std::io::Write;

use tauri::State;

use crate::commands::logging::LogFileState;
use crate::commands::projects::DbState;
use crate::daemon::protocol::{self, LaunchMode};
use crate::session_scanner::cli_tool::CliTool;
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

                    // Convert Linux project paths to Windows paths so the frontend
                    // can match sessions to projects (stored as Windows paths).
                    // - /mnt/d/foo → D:\foo (Windows-native projects)
                    // - /home/user/foo → \\wsl.localhost\distro\... (WSL projects)
                    if let Some(ref distro) = provider.wsl_distro {
                        for session in &mut sessions {
                            if session.project_path.starts_with('/') {
                                session.project_path =
                                    crate::provider::path::to_windows(
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
    cli_tool: Option<CliTool>,
) -> Result<protocol::LaunchSessionResult, String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    if let Ok(mut f) = log_file.0.lock() {
        let _ = writeln!(f, "[cmd-center] launch_claude_session: project_id={project_id} mode={mode:?} tool={tool:?}");
    }

    // Resolve project path from DB
    let project_path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let project = crate::db::queries::get_project(&conn, &project_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Project not found: {project_id}"))?;
        project.path
    };

    // Convert Windows path to Linux path if needed (WSL UNC or drive path)
    let linux_path = crate::provider::path::to_linux(&project_path)
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
                    cli_tool: tool,
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
        crate::session_scanner::control::launch_in_tmux(&linux_path, mode, tool)
            .map_err(|e| format!("Failed to launch session: {e}"))?;

    Ok(protocol::LaunchSessionResult {
        tmux_window: window,
        tmux_pane: pane,
    })
}

/// Stop a running CLI tool session by sending the exit command to its tmux pane.
#[tauri::command]
pub fn stop_claude_session(
    provider: State<'_, ProviderState>,
    tmux_pane: String,
    cli_tool: Option<CliTool>,
) -> Result<(), String> {
    let tool = cli_tool.unwrap_or(CliTool::Claude);

    // Try daemon first
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

    // Fall back to direct stop (Linux dev)
    crate::session_scanner::control::stop_session(&tmux_pane, tool)
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

/// Record a completed CLI session's activity stats for historical tracking.
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

/// Get aggregated activity stats for a project path.
#[tauri::command]
pub fn get_project_activity(
    db: State<'_, DbState>,
    project_path: String,
) -> Result<crate::db::activity_queries::ProjectActivityStats, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::db::activity_queries::get_project_activity(&conn, &project_path)
        .map_err(|e| e.to_string())
}

/// Get tasks from all CLI tools for a project.
///
/// Pure DB read — returns persisted tasks from SQLite.
/// Task scanning and persistence happen in the background via the event-driven
/// task sync pipeline (daemon watches `~/.claude/tasks/`, triggers scan + persist).
#[tauri::command]
pub fn get_project_tasks(
    db: State<'_, DbState>,
    project_path: String,
) -> Result<crate::task_scanner::TaskResult, String> {
    let normalized_path = crate::provider::path::to_linux(&project_path)
        .unwrap_or_else(|| project_path.clone());

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks = crate::db::task_queries::get_tasks_for_project(&conn, &normalized_path)
        .map_err(|e| e.to_string())?;

    let tasks: Vec<crate::task_scanner::UnifiedTask> = db_tasks
        .into_iter()
        .map(|t| crate::task_scanner::UnifiedTask {
            id: t.source_task_id,
            subject: t.subject,
            description: t.description,
            active_form: t.active_form,
            status: match t.status.as_str() {
                "in_progress" => crate::task_scanner::TaskStatus::InProgress,
                "completed" => crate::task_scanner::TaskStatus::Completed,
                _ => crate::task_scanner::TaskStatus::Pending,
            },
            source: match t.source.as_str() {
                "codex" => CliTool::Codex,
                "gemini" => CliTool::Gemini,
                _ => CliTool::Claude,
            },
            blocks: t.blocks,
            blocked_by: t.blocked_by,
            owner: t.owner,
            session_id: t.session_id,
        })
        .collect();

    Ok(crate::task_scanner::TaskResult {
        tasks,
        errors: vec![],
    })
}

/// Get enriched detail for a single task: full data + session info + commits + files changed.
#[tauri::command]
pub fn get_task_detail(
    db: State<'_, DbState>,
    project_path: String,
    task_id: String,
    source: String,
) -> Result<crate::task_scanner::TaskDetail, String> {
    let normalized_path = crate::provider::path::to_linux(&project_path)
        .unwrap_or_else(|| project_path.clone());

    // Find the task in DB
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks = crate::db::task_queries::get_tasks_for_project(&conn, &normalized_path)
        .map_err(|e| e.to_string())?;

    let db_task = db_tasks
        .into_iter()
        .find(|t| t.source_task_id == task_id && t.source == source)
        .ok_or_else(|| format!("Task not found: {source}/{task_id}"))?;

    let task = crate::task_scanner::UnifiedTask {
        id: db_task.source_task_id,
        subject: db_task.subject,
        description: db_task.description,
        active_form: db_task.active_form,
        status: match db_task.status.as_str() {
            "in_progress" => crate::task_scanner::TaskStatus::InProgress,
            "completed" => crate::task_scanner::TaskStatus::Completed,
            _ => crate::task_scanner::TaskStatus::Pending,
        },
        source: match db_task.source.as_str() {
            "codex" => CliTool::Codex,
            "gemini" => CliTool::Gemini,
            _ => CliTool::Claude,
        },
        blocks: db_task.blocks,
        blocked_by: db_task.blocked_by,
        owner: db_task.owner,
        session_id: db_task.session_id.clone(),
    };

    // Try to enrich with session context (commits + files changed)
    let (session, commits, files_changed) = match db_task.session_id {
        Some(ref session_id) => enrich_from_session(&normalized_path, session_id),
        None => (None, vec![], vec![]),
    };

    Ok(crate::task_scanner::TaskDetail {
        task,
        session,
        commits,
        files_changed,
    })
}

/// Look up session time range and find commits/files changed during it.
fn enrich_from_session(
    project_path: &str,
    session_id: &str,
) -> (
    Option<crate::task_scanner::SessionInfo>,
    Vec<crate::models::Commit>,
    Vec<String>,
) {
    let path = std::path::Path::new(project_path);

    let time_range = crate::claude_code::resolver::session_time_range(path, session_id);

    match time_range {
        Some((start, end)) => {
            let session_info = crate::task_scanner::SessionInfo {
                id: session_id.to_string(),
                started_at: start.to_rfc3339(),
                ended_at: end.to_rfc3339(),
            };

            let commits = crate::git::commits::get_commits_in_range(path, start, end)
                .unwrap_or_default();

            let files = crate::git::commits::get_files_changed_in_range(path, start, end)
                .unwrap_or_default();

            (Some(session_info), commits, files)
        }
        None => (None, vec![], vec![]),
    }
}

/// Persist scanned tasks into SQLite (upsert — never lose history).
pub(crate) fn persist_task_scan(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
) {
    if scan_result.tasks.is_empty() {
        return;
    }
    let now = chrono::Utc::now().to_rfc3339();
    let persisted: Vec<crate::db::task_queries::PersistedTask> = scan_result
        .tasks
        .iter()
        .map(|t| crate::db::task_queries::PersistedTask {
            project_path: normalized_path.to_string(),
            source: t.source.to_string(),
            source_task_id: t.id.clone(),
            subject: t.subject.clone(),
            description: t.description.clone(),
            active_form: t.active_form.clone(),
            status: t.status.to_string(),
            blocks: t.blocks.clone(),
            blocked_by: t.blocked_by.clone(),
            owner: t.owner.clone(),
            session_id: t.session_id.clone(),
            first_seen_at: now.clone(),
            updated_at: now.clone(),
        })
        .collect();
    if let Err(e) = crate::db::task_queries::upsert_tasks(conn, &persisted) {
        tracing::warn!(error = %e, "Failed to persist scanned tasks");
    }
}

/// Scan task files from live sources (daemon or local).
pub(crate) fn scan_tasks_from_files(
    provider: &ProviderState,
    project_path: &str,
) -> crate::task_scanner::TaskResult {
    // Try daemon first — required on Windows where task files live in WSL
    if let Some(ref daemon) = provider.daemon {
        if !daemon.is_connected() {
            daemon.try_reconnect();
        }

        if daemon.is_connected() {
            let linux_path = crate::provider::path::to_linux(project_path)
                .unwrap_or_else(|| project_path.to_string());

            let id = "scan-project-tasks";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::GET_PROJECT_TASKS,
                protocol::PathParams { path: linux_path },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    if let Some(result) = response
                        .result
                        .and_then(|v| serde_json::from_value(v).ok())
                    {
                        return result;
                    }
                }
                Ok(response) => {
                    tracing::warn!(error = ?response.error, "Daemon task scan failed");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Daemon request failed for task scan");
                }
            }
        }
    }

    // Local fallback (Linux, or daemon unavailable)
    let all_sessions = crate::session_scanner::scan_sessions();
    let project_sessions: Vec<ClaudeSession> = all_sessions
        .into_iter()
        .filter(|s| s.project_path == project_path)
        .collect();

    crate::task_scanner::get_tasks_for_project(project_path, &project_sessions)
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
