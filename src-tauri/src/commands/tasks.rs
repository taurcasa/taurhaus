//! Task-related commands and helpers.
//!
//! Extracted from `command_center.rs` to keep session-management and task
//! workflows separated.

use tauri::State;

use crate::commands::projects::DbState;
use crate::errors::sanitize_error;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;
use crate::ProviderState;

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
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks = crate::db::task_queries::get_tasks_for_project(&conn, &normalized_path)
        .map_err(|e| e.to_string())?;

    let tasks: Vec<crate::task_scanner::UnifiedTask> =
        db_tasks.into_iter().map(persisted_to_unified).collect();

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
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

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

            let commits =
                crate::git::commits::get_commits_in_range(path, start, end).unwrap_or_default();

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
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());
    let provider = providers.resolve(&project_path);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks = crate::db::task_queries::get_archived_tasks_for_project(&conn, &normalized_path)
        .map_err(|e| e.to_string())?;

    if db_tasks.is_empty() {
        return Ok(crate::task_scanner::ArchivedSessionsResult {
            sessions: vec![],
            errors: vec![],
        });
    }

    // Group raw persisted tasks by session_id (None -> "ungrouped").
    let mut groups: std::collections::BTreeMap<
        String,
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::BTreeMap::new();
    for t in db_tasks {
        let session_key = t
            .session_id
            .clone()
            .unwrap_or_else(|| "ungrouped".to_string());
        groups.entry(session_key).or_default().push(t);
    }

    let mut sessions: Vec<crate::task_scanner::ArchivedSession> = groups
        .iter()
        .map(|(key, raw)| build_archived_session(key, raw, provider, &project_path))
        .collect();

    // Sort reverse-chronological: sessions with started_at first (newest first),
    // then ungrouped/unresolved at the end
    sessions.sort_by(|a, b| match (&b.started_at, &a.started_at) {
        (Some(b_start), Some(a_start)) => b_start.cmp(a_start),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(crate::task_scanner::ArchivedSessionsResult {
        sessions,
        errors: vec![],
    })
}

/// Build one `ArchivedSession` from a group of persisted tasks.
fn build_archived_session(
    session_key: &str,
    raw_tasks: &[crate::db::task_queries::PersistedTask],
    provider: &dyn crate::provider::ProjectProvider,
    project_path: &str,
) -> crate::task_scanner::ArchivedSession {
    let tasks: Vec<crate::task_scanner::UnifiedTask> = raw_tasks
        .iter()
        .cloned()
        .map(persisted_to_unified)
        .collect();

    let sources = unique_sources(&tasks);
    let (started_at, ended_at, duration_ms) = time_range_from_tasks(raw_tasks);

    // Query git for commits and files changed during the session time range.
    let (commit_count, file_count) = match (&started_at, &ended_at) {
        (Some(s), Some(e)) => provider
            .commits_in_range(project_path, s, e)
            .map(|(c, f)| (c.len(), f.len()))
            .unwrap_or((0, 0)),
        _ => (0, 0),
    };

    let last_archived_at = raw_tasks
        .iter()
        .filter_map(|t| t.archived_at.as_deref())
        .max()
        .map(String::from);

    // Ungrouped sessions with no timestamps get zeroed git counts.
    if session_key == "ungrouped" && started_at.is_none() {
        return crate::task_scanner::ArchivedSession {
            session_id: "ungrouped".to_string(),
            started_at: None,
            ended_at: None,
            duration_ms: None,
            tasks,
            commit_count: 0,
            file_count: 0,
            sources,
            last_archived_at,
        };
    }

    crate::task_scanner::ArchivedSession {
        session_id: session_key.to_string(),
        started_at,
        ended_at,
        duration_ms,
        tasks,
        commit_count,
        file_count,
        sources,
        last_archived_at,
    }
}

/// Derive session time boundaries from the earliest/latest timestamps
/// in a set of persisted tasks.
fn time_range_from_tasks(
    tasks: &[crate::db::task_queries::PersistedTask],
) -> (Option<String>, Option<String>, Option<i64>) {
    let started_at = tasks
        .iter()
        .map(|t| t.first_seen_at.as_str())
        .min()
        .map(String::from);
    let ended_at = tasks
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

    (started_at, ended_at, duration_ms)
}

/// Collect deduplicated, sorted tool sources from a set of unified tasks.
fn unique_sources(tasks: &[crate::task_scanner::UnifiedTask]) -> Vec<String> {
    let mut s: Vec<String> = tasks.iter().map(|t| t.source.to_string()).collect();
    s.sort();
    s.dedup();
    s
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
        .map_err(|e| sanitize_error(&e.to_string()))
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
        .map_err(|e| sanitize_error(&e.to_string()))
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
        .map_err(|e| sanitize_error(&e.to_string()))?;
    Ok(crate::daemon::protocol::GitCommitsInRangeResult { commits, files })
}

/// Convert a persisted DB task row to a UnifiedTask.
fn persisted_to_unified(
    t: crate::db::task_queries::PersistedTask,
) -> crate::task_scanner::UnifiedTask {
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

    prune_stale_tasks(conn, normalized_path, scan_result);
}

/// Archive or delete tasks that no longer appear in a scan result.
///
/// Groups scan results by source, then for each source that contributed >=1 task,
/// removes DB entries not present in the current scan.
fn prune_stale_tasks(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
) {
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
            let request = crate::daemon::protocol::DaemonRequest::new(
                id,
                crate::daemon::protocol::method::GET_PROJECT_TASKS,
                crate::daemon::protocol::PathParams { path: linux_path },
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    if let Some(result_payload) = response.result {
                        match serde_json::from_value(result_payload) {
                            Ok(result) => return result,
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "Failed to deserialize task scan from daemon"
                                );
                            }
                        }
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
