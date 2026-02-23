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
        .map(persisted_to_unified)
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

    let session_id_for_enrich = db_task.session_id.clone();
    let task = persisted_to_unified(db_task);

    // Try to enrich with session context (commits + files changed)
    let (session, commits, files_changed) = match session_id_for_enrich {
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

/// Get archived sessions for the session history timeline.
///
/// Returns completed tasks grouped by session, enriched with commit and file counts.
/// Sorted reverse-chronological (newest session first).
#[tauri::command]
pub fn get_archived_sessions(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_path: String,
) -> Result<crate::task_scanner::ArchivedSessionsResult, String> {
    let normalized_path = crate::provider::path::to_linux(&project_path)
        .unwrap_or_else(|| project_path.clone());
    let provider = providers.resolve(&project_path);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks =
        crate::db::task_queries::get_archived_tasks_for_project(&conn, &normalized_path)
            .map_err(|e| e.to_string())?;

    if db_tasks.is_empty() {
        return Ok(crate::task_scanner::ArchivedSessionsResult {
            sessions: vec![],
            errors: vec![],
        });
    }

    // Group raw persisted tasks by session_id (None → "ungrouped").
    // We keep PersistedTask (not UnifiedTask) so we can derive time ranges
    // from first_seen_at / updated_at timestamps in the DB rows.
    let mut groups: std::collections::BTreeMap<
        String,
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::BTreeMap::new();
    for t in db_tasks {
        let session_key = t.session_id.clone().unwrap_or_else(|| "ungrouped".to_string());
        groups.entry(session_key).or_default().push(t);
    }

    let mut sessions = Vec::new();
    let errors: Vec<String> = Vec::new();

    for (session_key, raw_tasks) in &groups {
        let tasks: Vec<crate::task_scanner::UnifiedTask> =
            raw_tasks.iter().cloned().map(persisted_to_unified).collect();

        let sources: Vec<String> = {
            let mut s: Vec<String> = tasks.iter().map(|t| t.source.to_string()).collect();
            s.sort();
            s.dedup();
            s
        };

        // Derive session time range from DB timestamps (earliest first_seen_at,
        // latest updated_at). This works cross-platform — no JSONL file access needed.
        let started_at = raw_tasks
            .iter()
            .map(|t| t.first_seen_at.as_str())
            .min()
            .map(String::from);
        let ended_at = raw_tasks
            .iter()
            .map(|t| t.updated_at.as_str())
            .max()
            .map(String::from);

        let duration_ms = started_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .and_then(|start| {
                ended_at
                    .as_deref()
                    .and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())
                    .map(|end| (end - start).num_milliseconds())
            });

        // Query git for commits and files changed during the session time range.
        // Uses the provider (daemon for WSL paths, local for Windows paths).
        let (commit_count, file_count) = match (&started_at, &ended_at) {
            (Some(s), Some(e)) => {
                match provider.commits_in_range(&project_path, s, e) {
                    Ok((commits, files)) => (commits.len(), files.len()),
                    Err(_) => (0, 0),
                }
            }
            _ => (0, 0),
        };

        if session_key == "ungrouped" && started_at.is_none() {
            sessions.push(crate::task_scanner::ArchivedSession {
                session_id: "ungrouped".to_string(),
                started_at: None,
                ended_at: None,
                duration_ms: None,
                tasks,
                commit_count: 0,
                file_count: 0,
                sources,
            });
        } else {
            sessions.push(crate::task_scanner::ArchivedSession {
                session_id: session_key.clone(),
                started_at,
                ended_at,
                duration_ms,
                tasks,
                commit_count,
                file_count,
                sources,
            });
        }
    }

    // Sort reverse-chronological: sessions with started_at first (newest first),
    // then ungrouped/unresolved at the end
    sessions.sort_by(|a, b| {
        match (&b.started_at, &a.started_at) {
            (Some(b_start), Some(a_start)) => b_start.cmp(a_start),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(crate::task_scanner::ArchivedSessionsResult { sessions, errors })
}

/// Get files changed by a specific commit.
///
/// Used by the Git tab to show commit detail (file list with status).
#[tauri::command]
pub fn get_commit_files(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
) -> Result<Vec<crate::models::CommitFile>, String> {
    let provider = providers.resolve(&project_path);
    provider
        .commit_files(&project_path, &hash)
        .map_err(|e| e.to_string())
}

/// Get diff hunks for a specific file in a specific commit.
///
/// Used by the Git tab for inline diff view.
#[tauri::command]
pub fn get_commit_diff(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
    file_path: String,
) -> Result<Vec<crate::models::DiffHunk>, String> {
    let provider = providers.resolve(&project_path);
    provider
        .commit_diff(&project_path, &hash, &file_path)
        .map_err(|e| e.to_string())
}

/// Get commits and files changed in a time range.
///
/// Used by the Git tab for range-filtered views and Session History enrichment.
#[tauri::command]
pub fn get_commits_in_range(
    providers: State<'_, ProviderState>,
    project_path: String,
    after: String,
    before: String,
) -> Result<crate::daemon::protocol::GitCommitsInRangeResult, String> {
    let provider = providers.resolve(&project_path);
    let (commits, files) = provider
        .commits_in_range(&project_path, &after, &before)
        .map_err(|e| e.to_string())?;
    Ok(crate::daemon::protocol::GitCommitsInRangeResult { commits, files })
}

/// Convert a persisted DB task row to a UnifiedTask.
fn persisted_to_unified(t: crate::db::task_queries::PersistedTask) -> crate::task_scanner::UnifiedTask {
    crate::task_scanner::UnifiedTask {
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
    }
}

/// Persist scanned tasks into SQLite (upsert + prune stale entries).
///
/// After upserting the current scan results, removes DB entries for tasks that
/// no longer appear in the scan (e.g., deleted from disk or status changed to
/// "deleted"). Only prunes sources that contributed at least one task — if a
/// source returned 0 tasks, its existing DB entries are preserved (the scanner
/// may not have been able to reach the data).
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
            archived_at: None,
        })
        .collect();
    if let Err(e) = crate::db::task_queries::upsert_tasks(conn, &persisted) {
        tracing::warn!(error = %e, "Failed to persist scanned tasks");
    }

    // Prune stale tasks: for each source that contributed ≥1 task,
    // remove DB entries that are no longer in the scan result.
    let mut sources: std::collections::HashMap<String, Vec<&str>> =
        std::collections::HashMap::new();
    for task in &scan_result.tasks {
        sources
            .entry(task.source.to_string())
            .or_default()
            .push(&task.id);
    }
    for (source, active_ids) in &sources {
        match crate::db::task_queries::archive_or_delete_stale_tasks(
            conn,
            normalized_path,
            source,
            active_ids,
        ) {
            Ok(result) => {
                if result.archived > 0 || result.deleted > 0 {
                    tracing::info!(
                        source = %source,
                        archived = result.archived,
                        deleted = result.deleted,
                        "Pruned stale tasks"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, source = %source, "Failed to prune stale tasks");
            }
        }
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
