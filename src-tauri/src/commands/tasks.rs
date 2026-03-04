//! Task-related commands and helpers.
//!
//! Extracted from `command_center.rs` to keep session-management and task
//! workflows separated.

use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use tauri::State;

use crate::commands::projects::DbState;
use crate::errors::{sanitize_error, SanitizeErr};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;
use crate::ProviderState;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScanGenerationKey {
    project_path: String,
    source: String,
    source_key: String,
}

static APPLIED_SCAN_GENERATIONS: LazyLock<Mutex<HashMap<ScanGenerationKey, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const SCAN_GENERATION_RETENTION_WINDOW: u64 = 100;

#[cfg(test)]
static FALLBACK_SCAN_GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
fn next_fallback_scan_generation() -> u64 {
    FALLBACK_SCAN_GENERATION.fetch_add(1, Ordering::Relaxed)
}

fn should_apply_scan_generation(
    project_path: &str,
    source: &str,
    source_key: &str,
    generation: u64,
) -> bool {
    if cfg!(test) {
        return true;
    }

    let key = ScanGenerationKey {
        project_path: project_path.to_string(),
        source: source.to_string(),
        source_key: source_key.to_string(),
    };

    let mut applied = APPLIED_SCAN_GENERATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match applied.get(&key) {
        Some(existing) if generation < *existing => false,
        _ => {
            applied.insert(key, generation);
            true
        }
    }
}

fn cleanup_applied_scan_generations(current_generation: u64) {
    let mut applied = APPLIED_SCAN_GENERATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    prune_generation_map(
        &mut applied,
        current_generation,
        SCAN_GENERATION_RETENTION_WINDOW,
    );
}

fn prune_generation_map(
    applied: &mut HashMap<ScanGenerationKey, u64>,
    current_generation: u64,
    retention_window: u64,
) {
    let min_generation = current_generation.saturating_sub(retention_window);
    applied.retain(|_, generation| *generation >= min_generation);
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
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_tasks =
        crate::db::task_queries::get_tasks_for_project(&conn, &normalized_path).sanitize_err()?;

    let tasks: Vec<crate::task_scanner::UnifiedTask> =
        db_tasks.into_iter().map(persisted_to_unified).collect();

    Ok(crate::task_scanner::TaskResult {
        tasks,
        errors: vec![],
        source_outcomes: vec![],
    })
}

/// Get enriched detail for a single task: full data + session info + commits + files changed.
#[tauri::command]
pub fn get_task_detail(
    db: State<'_, DbState>,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> Result<crate::task_scanner::TaskDetail, String> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let db_task = match crate::db::task_queries::get_task_for_project_by_identity(
        &conn,
        &normalized_path,
        &source,
        &source_key,
        &task_id,
    )
    .sanitize_err()?
    {
        Some(task) => task,
        None => {
            find_archived_task_by_identity(&conn, &normalized_path, &source, &source_key, &task_id)?
                .ok_or_else(|| format!("Task not found: {source}/{source_key}/{task_id}"))?
        }
    };

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

fn find_archived_task_by_identity(
    conn: &rusqlite::Connection,
    project_path: &str,
    source: &str,
    source_key: &str,
    source_task_id: &str,
) -> Result<Option<crate::db::task_queries::PersistedTask>, String> {
    crate::db::task_queries::get_archived_task_for_project_by_identity(
        conn,
        project_path,
        source,
        source_key,
        source_task_id,
    )
    .sanitize_err()
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
        .sanitize_err()?;

    if db_tasks.is_empty() {
        return Ok(crate::task_scanner::ArchivedSessionsResult {
            sessions: vec![],
            errors: vec![],
        });
    }

    let claude_team_names = known_claude_team_names();

    // Group raw persisted tasks by nullable session id.
    let mut groups: std::collections::BTreeMap<
        Option<String>,
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::BTreeMap::new();
    for t in db_tasks {
        let session_key = t.session_id.clone();
        groups.entry(session_key).or_default().push(t);
    }

    let mut sessions: Vec<crate::task_scanner::ArchivedSession> = groups
        .iter()
        .map(|(key, raw)| {
            build_archived_session(key, raw, provider, &project_path, &claude_team_names)
        })
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
    session_key: &Option<String>,
    raw_tasks: &[crate::db::task_queries::PersistedTask],
    provider: &dyn crate::provider::ProjectProvider,
    project_path: &str,
    claude_team_names: &HashSet<String>,
) -> crate::task_scanner::ArchivedSession {
    let tasks: Vec<crate::task_scanner::UnifiedTask> = raw_tasks
        .iter()
        .cloned()
        .map(persisted_to_unified)
        .collect();

    let sources = unique_sources(&tasks);
    let mut enrichment_warnings = Vec::new();
    let (started_at, ended_at, duration_ms) = derive_archive_time_range(
        session_key,
        raw_tasks,
        &sources,
        project_path,
        claude_team_names,
        &mut enrichment_warnings,
    );

    // Query git for commits and files changed during the session time range.
    let (commit_count, file_count) = match (&started_at, &ended_at) {
        (Some(s), Some(e)) => provider
            .commits_in_range(project_path, s, e)
            .map(|(c, f)| (c.len(), f.len()))
            .unwrap_or_else(|err| {
                enrichment_warnings.push(format!(
                    "Failed to enrich git counts for session {} in [{s}..{e}]: {err}",
                    session_key.as_deref().unwrap_or("ungrouped")
                ));
                (0, 0)
            }),
        _ => (0, 0),
    };

    let last_archived_at = raw_tasks
        .iter()
        .filter_map(|t| t.archived_at.as_deref())
        .max()
        .map(String::from);

    crate::task_scanner::ArchivedSession {
        session_id: session_key.clone(),
        started_at,
        ended_at,
        duration_ms,
        tasks,
        commit_count,
        file_count,
        sources,
        last_archived_at,
        enrichment_warnings,
    }
}

fn derive_archive_time_range(
    session_key: &Option<String>,
    tasks: &[crate::db::task_queries::PersistedTask],
    sources: &[String],
    project_path: &str,
    claude_team_names: &HashSet<String>,
    warnings: &mut Vec<String>,
) -> (Option<String>, Option<String>, Option<i64>) {
    let fallback = time_range_from_tasks(tasks);
    let Some(session_id) = session_key.as_deref() else {
        return fallback;
    };

    if is_team_scoped_claude_group(session_id, tasks, claude_team_names) {
        return fallback;
    }

    if let Some((start, end)) = transcript_time_range(project_path, session_id, sources) {
        return to_iso_range(start, end);
    }

    warnings.push(format!(
        "Could not resolve transcript time range for session {session_id}; using task timestamp fallback."
    ));
    fallback
}

fn is_team_scoped_claude_group(
    session_id: &str,
    tasks: &[crate::db::task_queries::PersistedTask],
    claude_team_names: &HashSet<String>,
) -> bool {
    if !claude_team_names.contains(session_id) {
        return false;
    }

    tasks
        .iter()
        .any(|task| task.source == "claude" && task.source_key == session_id)
}

fn known_claude_team_names() -> HashSet<String> {
    crate::task_scanner::claude_index::build_claude_source_index()
        .teams
        .into_keys()
        .collect()
}

fn transcript_time_range(
    project_path: &str,
    session_id: &str,
    sources: &[String],
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let path = std::path::Path::new(project_path);
    let mut ranges: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();

    for source in sources {
        let range = match source.as_str() {
            "claude" => crate::claude_code::resolver::session_time_range(path, session_id),
            "codex" => crate::task_scanner::codex::session_time_range(path, session_id),
            "gemini" => crate::task_scanner::gemini::session_time_range(path, session_id),
            _ => None,
        };
        if let Some(r) = range {
            ranges.push(r);
        }
    }

    if ranges.is_empty() {
        return None;
    }

    let start = ranges.iter().map(|(s, _)| *s).min()?;
    let mut end = ranges.iter().map(|(_, e)| *e).max()?;
    if end < start {
        end = start;
    }
    Some((start, end))
}

fn to_iso_range(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> (Option<String>, Option<String>, Option<i64>) {
    (
        Some(start.to_rfc3339()),
        Some(end.to_rfc3339()),
        Some((end - start).num_milliseconds()),
    )
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
        source_key: t.source_key,
        subject: t.subject,
        description: t.description,
        active_form: t.active_form,
        status: match t.status.as_str() {
            "in_progress" => crate::task_scanner::TaskStatus::InProgress,
            "completed" => crate::task_scanner::TaskStatus::Completed,
            _ => crate::task_scanner::TaskStatus::Pending,
        },
        source: t.source.parse::<CliTool>().unwrap_or(CliTool::Claude),
        blocks: t.blocks,
        blocked_by: t.blocked_by,
        owner: t.owner,
        session_id: t.session_id,
        state_changed_at: t.state_changed_at,
        updated_at: Some(t.updated_at),
        archived_at: t.archived_at,
        last_status: t.last_status,
        archived_reason: t.archived_reason,
    }
}

/// Persist scanned tasks into SQLite (upsert + prune stale entries).
///
/// After upserting the current scan results, removes DB entries for tasks that
/// no longer appear in the scan (e.g., deleted from disk or status changed to
/// "deleted"). Reconciliation runs even on empty scans, but only for sources
/// that were successfully scanned in this cycle.
#[cfg(test)]
pub(crate) fn persist_task_scan(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
) {
    let scan_generation = next_fallback_scan_generation();
    persist_task_scan_with_generation(conn, normalized_path, scan_result, scan_generation);
}

pub(crate) fn persist_task_scan_with_generation(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    scan_result: &crate::task_scanner::TaskResult,
    scan_generation: u64,
) {
    cleanup_applied_scan_generations(scan_generation);
    let source_outcomes = normalized_source_outcomes(scan_result);
    let now = chrono::Utc::now().to_rfc3339();
    let mut persisted_by_key: std::collections::HashMap<
        (String, String),
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::HashMap::new();

    for source_outcome in &source_outcomes {
        if let crate::task_scanner::ScanOutcome::Data(tasks) = &source_outcome.outcome {
            for t in tasks {
                persisted_by_key
                    .entry((source_outcome.source.clone(), t.source_key.clone()))
                    .or_default()
                    .push(crate::db::task_queries::PersistedTask {
                        project_path: normalized_path.to_string(),
                        source: source_outcome.source.clone(),
                        source_key: t.source_key.clone(),
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
                        state_changed_at: Some(now.clone()),
                        updated_at: now.clone(),
                        archived_at: None,
                        last_status: Some(t.status.to_string()),
                        archived_reason: None,
                    });
            }
        }
    }

    for ((source, source_key), persisted) in persisted_by_key {
        if !should_apply_scan_generation(normalized_path, &source, &source_key, scan_generation) {
            tracing::debug!(
                source = %source,
                source_key = %source_key,
                generation = scan_generation,
                "Skipping stale task upsert generation"
            );
            continue;
        }

        if let Err(e) = crate::db::task_queries::upsert_tasks(conn, &persisted) {
            tracing::warn!(
                error = %e,
                source = %source,
                source_key = %source_key,
                "Failed to persist scanned tasks"
            );
        }
    }

    prune_stale_tasks(conn, normalized_path, &source_outcomes, scan_generation);
}

/// Archive or delete tasks that no longer appear in a scan result.
///
/// Groups scan results by source, then reconciles only sources that were
/// successfully scanned in this cycle. A successfully scanned source with zero
/// tasks means all previous tasks from that source are stale.
fn prune_stale_tasks(
    conn: &rusqlite::Connection,
    normalized_path: &str,
    source_outcomes: &[crate::task_scanner::SourceScanOutcome],
    scan_generation: u64,
) {
    let mut active_by_source_key: std::collections::HashMap<(String, String), Vec<String>> =
        std::collections::HashMap::new();
    for source_outcome in source_outcomes {
        let source = source_outcome.source.clone();
        match &source_outcome.outcome {
            crate::task_scanner::ScanOutcome::Unavailable(reason) => {
                tracing::info!(
                    source = %source,
                    reason = %reason,
                    "Skipping stale prune for unavailable source"
                );
                continue;
            }
            crate::task_scanner::ScanOutcome::Data(tasks) => {
                for task in tasks {
                    active_by_source_key
                        .entry((source.clone(), task.source_key.clone()))
                        .or_default()
                        .push(task.id.clone());
                }
            }
            crate::task_scanner::ScanOutcome::DefinitivelyEmpty => {}
        }

        let db_keys = match crate::db::task_queries::get_active_source_keys_for_project_source(
            conn,
            normalized_path,
            &source,
        ) {
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    source = %source,
                    "Failed to load source keys for stale task pruning"
                );
                continue;
            }
            Ok(keys) => keys,
        };

        let mut source_keys: std::collections::BTreeSet<String> = db_keys.into_iter().collect();
        source_keys.extend(
            active_by_source_key
                .keys()
                .filter(|(s, _)| s == &source)
                .map(|(_, key)| key.clone()),
        );

        for source_key in source_keys {
            if !should_apply_scan_generation(normalized_path, &source, &source_key, scan_generation)
            {
                tracing::debug!(
                    source = %source,
                    source_key = %source_key,
                    generation = scan_generation,
                    "Skipping stale prune for stale generation"
                );
                continue;
            }

            let active_ids_storage = active_by_source_key
                .get(&(source.clone(), source_key.clone()))
                .cloned()
                .unwrap_or_default();
            let active_ids: Vec<&str> = active_ids_storage.iter().map(String::as_str).collect();

            match crate::db::task_queries::archive_or_delete_stale_tasks(
                conn,
                normalized_path,
                &source,
                &source_key,
                &active_ids,
            ) {
                Ok(result) => {
                    if result.archived > 0 || result.deleted > 0 {
                        tracing::info!(
                            source = %source,
                            source_key = %source_key,
                            archived = result.archived,
                            deleted = result.deleted,
                            "Pruned stale tasks"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        source = %source,
                        source_key = %source_key,
                        "Failed to prune stale tasks"
                    );
                }
            }
        }
    }
}

fn normalized_source_outcomes(
    scan_result: &crate::task_scanner::TaskResult,
) -> Vec<crate::task_scanner::SourceScanOutcome> {
    if !scan_result.source_outcomes.is_empty() {
        return scan_result.source_outcomes.clone();
    }

    let failures: std::collections::HashMap<String, String> = scan_result
        .errors
        .iter()
        .map(|(source, reason)| (source.clone(), reason.clone()))
        .collect();

    crate::session_scanner::cli_tool::all_tools()
        .iter()
        .map(|tool| {
            let source = tool.tool.to_string();
            let outcome = if let Some(reason) = failures.get(&source) {
                crate::task_scanner::ScanOutcome::Unavailable(reason.clone())
            } else {
                let tasks: Vec<crate::task_scanner::UnifiedTask> = scan_result
                    .tasks
                    .iter()
                    .filter(|t| t.source.to_string() == source)
                    .cloned()
                    .collect();
                if tasks.is_empty() {
                    crate::task_scanner::ScanOutcome::DefinitivelyEmpty
                } else {
                    crate::task_scanner::ScanOutcome::Data(tasks)
                }
            };
            crate::task_scanner::SourceScanOutcome { source, outcome }
        })
        .collect()
}

/// Scan task files from live sources (daemon or local).
pub(crate) fn scan_tasks_from_files(
    provider: &ProviderState,
    project_path: &str,
    scan_cycle_id: Option<u64>,
    cached_sessions: Option<&[ClaudeSession]>,
    cached_claude_index: Option<&crate::task_scanner::claude_index::ClaudeSourceIndex>,
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
                crate::daemon::protocol::ProjectTasksParams {
                    path: linux_path,
                    scan_cycle_id,
                },
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
    let all_sessions: Vec<ClaudeSession> = cached_sessions
        .map(|s| s.to_vec())
        .unwrap_or_else(crate::session_scanner::scan_sessions);
    let project_sessions: Vec<ClaudeSession> = all_sessions
        .into_iter()
        .filter(|s| s.project_path == project_path)
        .collect();

    crate::task_scanner::get_tasks_for_project_with_index(
        project_path,
        &project_sessions,
        cached_claude_index,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::task_scanner::{TaskResult, TaskStatus, UnifiedTask};
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    fn test_db() -> (rusqlite::Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn make_task_result(tasks: Vec<UnifiedTask>, errors: Vec<(&str, &str)>) -> TaskResult {
        let errors_vec: Vec<(String, String)> = errors
            .iter()
            .map(|(source, error)| ((*source).to_string(), (*error).to_string()))
            .collect();
        let error_map: std::collections::HashMap<&str, &str> = errors.into_iter().collect();
        let source_outcomes = crate::session_scanner::cli_tool::all_tools()
            .iter()
            .map(|tool| {
                let source = tool.tool.to_string();
                let outcome = if let Some(reason) = error_map.get(source.as_str()) {
                    crate::task_scanner::ScanOutcome::Unavailable((*reason).to_string())
                } else {
                    let source_tasks: Vec<UnifiedTask> = tasks
                        .iter()
                        .filter(|t| t.source.to_string() == source)
                        .cloned()
                        .collect();
                    if source_tasks.is_empty() {
                        crate::task_scanner::ScanOutcome::DefinitivelyEmpty
                    } else {
                        crate::task_scanner::ScanOutcome::Data(source_tasks)
                    }
                };
                crate::task_scanner::SourceScanOutcome { source, outcome }
            })
            .collect();

        TaskResult {
            tasks,
            errors: errors_vec,
            source_outcomes,
        }
    }

    fn make_unified_task(source: CliTool, id: &str, status: TaskStatus) -> UnifiedTask {
        UnifiedTask {
            id: id.to_string(),
            source_key: format!("{source}-default"),
            subject: format!("Task {id}"),
            description: None,
            active_form: None,
            status,
            source,
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            session_id: None,
            state_changed_at: None,
            updated_at: None,
            archived_at: None,
            last_status: None,
            archived_reason: None,
        }
    }

    fn make_archived_task(
        source: &str,
        source_key: &str,
        session_id: Option<&str>,
        first_seen_at: &str,
        updated_at: &str,
    ) -> crate::db::task_queries::PersistedTask {
        crate::db::task_queries::PersistedTask {
            project_path: "/projects/foo".to_string(),
            source: source.to_string(),
            source_key: source_key.to_string(),
            source_task_id: "1".to_string(),
            subject: "Done".to_string(),
            description: None,
            active_form: None,
            status: "completed".to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            session_id: session_id.map(ToString::to_string),
            first_seen_at: first_seen_at.to_string(),
            state_changed_at: Some(first_seen_at.to_string()),
            updated_at: updated_at.to_string(),
            archived_at: Some(updated_at.to_string()),
            last_status: Some("completed".to_string()),
            archived_reason: Some("completed_and_removed".to_string()),
        }
    }

    #[test]
    fn empty_scan_archives_completed_and_deletes_non_completed_for_scanned_source() {
        let (conn, _tmp) = test_db();
        let now = chrono::Utc::now().to_rfc3339();
        crate::db::task_queries::upsert_tasks(
            &conn,
            &[
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: "claude-default".to_string(),
                    source_task_id: "1".to_string(),
                    subject: "Done".to_string(),
                    description: None,
                    active_form: None,
                    status: "completed".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: None,
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now.clone(),
                    archived_at: None,
                    last_status: Some("completed".to_string()),
                    archived_reason: None,
                },
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: "claude-default".to_string(),
                    source_task_id: "2".to_string(),
                    subject: "Pending".to_string(),
                    description: None,
                    active_form: None,
                    status: "pending".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: None,
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now,
                    archived_at: None,
                    last_status: Some("pending".to_string()),
                    archived_reason: None,
                },
            ],
        )
        .unwrap();

        // Successful empty scan for Claude. Codex/Gemini failed, so only Claude is reconciled.
        let scan_result = make_task_result(vec![], vec![("codex", "failed"), ("gemini", "failed")]);
        persist_task_scan(&conn, "/projects/foo", &scan_result);

        let active =
            crate::db::task_queries::get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert!(active.is_empty());

        let archived =
            crate::db::task_queries::get_archived_tasks_for_project(&conn, "/projects/foo")
                .unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].source, "claude");
        assert_eq!(archived[0].source_task_id, "1");
        assert_eq!(archived[0].last_status.as_deref(), Some("completed"));
        assert_eq!(
            archived[0].archived_reason.as_deref(),
            Some("completed_and_removed")
        );
    }

    #[test]
    fn partial_scan_failure_does_not_prune_unscanned_sources() {
        let (conn, _tmp) = test_db();
        let now = chrono::Utc::now().to_rfc3339();
        crate::db::task_queries::upsert_tasks(
            &conn,
            &[
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: "claude-default".to_string(),
                    source_task_id: "1".to_string(),
                    subject: "Done".to_string(),
                    description: None,
                    active_form: None,
                    status: "completed".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: None,
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now.clone(),
                    archived_at: None,
                    last_status: Some("completed".to_string()),
                    archived_reason: None,
                },
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "codex".to_string(),
                    source_key: "codex-default".to_string(),
                    source_task_id: "codex-1".to_string(),
                    subject: "Codex pending".to_string(),
                    description: None,
                    active_form: None,
                    status: "pending".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: None,
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now,
                    archived_at: None,
                    last_status: Some("pending".to_string()),
                    archived_reason: None,
                },
            ],
        )
        .unwrap();

        // Claude scan succeeded with no tasks; Codex/Gemini failed this cycle.
        let scan_result =
            make_task_result(vec![], vec![("codex", "timeout"), ("gemini", "timeout")]);
        persist_task_scan(&conn, "/projects/foo", &scan_result);

        let active =
            crate::db::task_queries::get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source, "codex");
        assert_eq!(active[0].source_task_id, "codex-1");
    }

    #[test]
    fn unavailable_source_does_not_prune_that_source() {
        let (conn, _tmp) = test_db();
        let now = chrono::Utc::now().to_rfc3339();
        crate::db::task_queries::upsert_tasks(
            &conn,
            &[crate::db::task_queries::PersistedTask {
                project_path: "/projects/foo".to_string(),
                source: "claude".to_string(),
                source_key: "claude-default".to_string(),
                source_task_id: "1".to_string(),
                subject: "Claude task".to_string(),
                description: None,
                active_form: None,
                status: "pending".to_string(),
                blocks: vec![],
                blocked_by: vec![],
                owner: None,
                session_id: None,
                first_seen_at: now.clone(),
                state_changed_at: Some(now.clone()),
                updated_at: now,
                archived_at: None,
                last_status: Some("pending".to_string()),
                archived_reason: None,
            }],
        )
        .unwrap();

        let scan_result = TaskResult {
            tasks: vec![],
            errors: vec![("claude".to_string(), "degraded I/O".to_string())],
            source_outcomes: vec![
                crate::task_scanner::SourceScanOutcome {
                    source: "claude".to_string(),
                    outcome: crate::task_scanner::ScanOutcome::Unavailable(
                        "degraded I/O".to_string(),
                    ),
                },
                crate::task_scanner::SourceScanOutcome {
                    source: "codex".to_string(),
                    outcome: crate::task_scanner::ScanOutcome::DefinitivelyEmpty,
                },
                crate::task_scanner::SourceScanOutcome {
                    source: "gemini".to_string(),
                    outcome: crate::task_scanner::ScanOutcome::DefinitivelyEmpty,
                },
            ],
        };
        persist_task_scan(&conn, "/projects/foo", &scan_result);

        let active =
            crate::db::task_queries::get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source, "claude");
        assert_eq!(active[0].source_task_id, "1");
    }

    #[test]
    fn pending_to_completed_to_removed_is_archived() {
        let (conn, _tmp) = test_db();

        let pending_scan = make_task_result(
            vec![make_unified_task(CliTool::Claude, "1", TaskStatus::Pending)],
            vec![("codex", "not-run"), ("gemini", "not-run")],
        );
        persist_task_scan(&conn, "/projects/foo", &pending_scan);

        let completed_scan = make_task_result(
            vec![make_unified_task(
                CliTool::Claude,
                "1",
                TaskStatus::Completed,
            )],
            vec![("codex", "not-run"), ("gemini", "not-run")],
        );
        persist_task_scan(&conn, "/projects/foo", &completed_scan);

        let removed_scan =
            make_task_result(vec![], vec![("codex", "not-run"), ("gemini", "not-run")]);
        persist_task_scan(&conn, "/projects/foo", &removed_scan);

        let active =
            crate::db::task_queries::get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert!(active.is_empty());

        let archived =
            crate::db::task_queries::get_archived_tasks_for_project(&conn, "/projects/foo")
                .unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].source, "claude");
        assert_eq!(archived[0].source_task_id, "1");
        assert_eq!(archived[0].status, "completed");
    }

    #[test]
    fn prune_stale_tasks_is_scoped_by_source_key() {
        let (conn, _tmp) = test_db();
        let now = chrono::Utc::now().to_rfc3339();

        crate::db::task_queries::upsert_tasks(
            &conn,
            &[
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: "session-aaa".to_string(),
                    source_task_id: "1".to_string(),
                    subject: "Session AAA task".to_string(),
                    description: None,
                    active_form: None,
                    status: "pending".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: Some("session-aaa".to_string()),
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now.clone(),
                    archived_at: None,
                    last_status: Some("pending".to_string()),
                    archived_reason: None,
                },
                crate::db::task_queries::PersistedTask {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: "team-ops".to_string(),
                    source_task_id: "1".to_string(),
                    subject: "Team task".to_string(),
                    description: None,
                    active_form: None,
                    status: "pending".to_string(),
                    blocks: vec![],
                    blocked_by: vec![],
                    owner: None,
                    session_id: Some("team-ops".to_string()),
                    first_seen_at: now.clone(),
                    state_changed_at: Some(now.clone()),
                    updated_at: now.clone(),
                    archived_at: None,
                    last_status: Some("pending".to_string()),
                    archived_reason: None,
                },
            ],
        )
        .unwrap();

        let scan_result = make_task_result(
            vec![UnifiedTask {
                id: "1".to_string(),
                source_key: "session-aaa".to_string(),
                subject: "Session AAA task".to_string(),
                description: None,
                active_form: None,
                status: TaskStatus::Pending,
                source: CliTool::Claude,
                blocks: vec![],
                blocked_by: vec![],
                owner: None,
                session_id: Some("session-aaa".to_string()),
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
            }],
            vec![("codex", "not-run"), ("gemini", "not-run")],
        );
        persist_task_scan(&conn, "/projects/foo", &scan_result);

        let active =
            crate::db::task_queries::get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source, "claude");
        assert_eq!(active[0].source_key, "session-aaa");
        assert_eq!(active[0].source_task_id, "1");
    }

    #[test]
    fn transcript_resolution_failure_adds_enrichment_warning() {
        let tasks = vec![make_archived_task(
            "codex",
            "missing-session",
            Some("missing-session"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        )];
        let sources = vec!["codex".to_string()];
        let mut warnings = Vec::new();

        let (started_at, ended_at, duration_ms) = derive_archive_time_range(
            &Some("missing-session".to_string()),
            &tasks,
            &sources,
            "/projects/does-not-exist",
            &HashSet::new(),
            &mut warnings,
        );

        assert_eq!(started_at.as_deref(), Some("2026-03-01T10:00:00Z"));
        assert_eq!(ended_at.as_deref(), Some("2026-03-01T11:00:00Z"));
        assert_eq!(duration_ms, Some(3_600_000));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("missing-session"));
    }

    #[test]
    fn team_scoped_claude_session_skips_transcript_warning() {
        let tasks = vec![make_archived_task(
            "claude",
            "taurhaus-team",
            Some("taurhaus-team"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        )];
        let sources = vec!["claude".to_string()];
        let mut warnings = Vec::new();
        let claude_team_names = HashSet::from(["taurhaus-team".to_string()]);

        let (started_at, ended_at, duration_ms) = derive_archive_time_range(
            &Some("taurhaus-team".to_string()),
            &tasks,
            &sources,
            "/projects/does-not-exist",
            &claude_team_names,
            &mut warnings,
        );

        assert_eq!(started_at.as_deref(), Some("2026-03-01T10:00:00Z"));
        assert_eq!(ended_at.as_deref(), Some("2026-03-01T11:00:00Z"));
        assert_eq!(duration_ms, Some(3_600_000));
        assert!(
            warnings.is_empty(),
            "team-scoped Claude groups should use fallback timestamps silently"
        );
    }

    #[test]
    fn non_claude_group_named_like_team_keeps_warning_behavior() {
        let tasks = vec![make_archived_task(
            "codex",
            "different-source-key",
            Some("taurhaus-team"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        )];
        let sources = vec!["codex".to_string()];
        let mut warnings = Vec::new();
        let claude_team_names = HashSet::from(["taurhaus-team".to_string()]);

        let (started_at, ended_at, duration_ms) = derive_archive_time_range(
            &Some("taurhaus-team".to_string()),
            &tasks,
            &sources,
            "/projects/does-not-exist",
            &claude_team_names,
            &mut warnings,
        );

        assert_eq!(started_at.as_deref(), Some("2026-03-01T10:00:00Z"));
        assert_eq!(ended_at.as_deref(), Some("2026-03-01T11:00:00Z"));
        assert_eq!(duration_ms, Some(3_600_000));
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn generation_map_pruning_keeps_recent_window() {
        let mut map = std::collections::HashMap::new();
        for i in 1..=300_u64 {
            map.insert(
                ScanGenerationKey {
                    project_path: "/projects/foo".to_string(),
                    source: "claude".to_string(),
                    source_key: format!("session-{i}"),
                },
                i,
            );
        }

        prune_generation_map(&mut map, 300, 100);
        assert!(map.len() <= 101);
        assert!(map.values().all(|generation| *generation >= 200));
    }
}
