use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

use crate::commands::projects::DbState;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::services::task_sync::TaskScanGenerationState;
use crate::session_scanner::cli_tool::CliTool;
use crate::ProviderState;

/// Get tasks from all CLI tools for a project.
///
/// Pure DB read — returns persisted tasks from SQLite.
pub fn get_project_tasks(
    db: &DbState,
    project_path: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let db_tasks = {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        crate::db::task_queries::get_tasks_for_project(&conn, &normalized_path).sanitize_err()?
    };

    let tasks: Vec<crate::task_scanner::UnifiedTask> =
        db_tasks.into_iter().map(persisted_to_unified).collect();

    Ok(crate::task_scanner::TaskResult {
        tasks,
        errors: vec![],
        source_outcomes: vec![],
    })
}

pub fn get_or_refresh_project_tasks(
    db: &DbState,
    providers: &ProviderState,
    generation_state: &TaskScanGenerationState,
    project_path: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    let _ = providers;
    let _ = generation_state;
    get_project_tasks(db, project_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchivedSessionCacheStatus {
    Fresh,
    Missing,
    Stale,
}

pub struct ArchivedSessionsQueryResult {
    pub result: crate::task_scanner::ArchivedSessionsResult,
    pub cache_status: ArchivedSessionCacheStatus,
}

fn archived_session_key(session_id: Option<&str>) -> String {
    crate::db::task_queries::archived_session_key(session_id)
}

/// Get enriched detail for a single task: full data + session info + commits + files changed.
pub fn get_task_detail(
    db: &DbState,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());

    let db_task = {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        match crate::db::task_queries::get_task_for_project_by_identity(
            &conn,
            &normalized_path,
            &source,
            &source_key,
            &task_id,
        )
        .sanitize_err()?
        {
            Some(task) => task,
            None => find_archived_task_by_identity(
                &conn,
                &normalized_path,
                &source,
                &source_key,
                &task_id,
            )?
            .ok_or_else(|| format!("Task not found: {source}/{source_key}/{task_id}"))?,
        }
    };

    let session_id_for_enrich = db_task.session_id.clone();
    let task = persisted_to_unified(db_task);

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

/// Get archived sessions for the session history timeline.
///
/// Returns completed tasks grouped by session, enriched with commit and file counts.
/// Sorted reverse-chronological (newest session first).
pub fn get_archived_sessions(
    db: &DbState,
    _providers: &ProviderState,
    project_path: String,
) -> IpcResult<ArchivedSessionsQueryResult> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());
    let (db_tasks, cached_summaries) = {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        (
            crate::db::task_queries::get_archived_tasks_for_project(&conn, &normalized_path)
                .sanitize_err()?,
            crate::db::task_queries::get_archived_session_summaries_for_project(
                &conn,
                &normalized_path,
            )
            .sanitize_err()?,
        )
    };

    if db_tasks.is_empty() {
        return Ok(ArchivedSessionsQueryResult {
            result: crate::task_scanner::ArchivedSessionsResult {
                sessions: vec![],
                errors: vec![],
            },
            cache_status: ArchivedSessionCacheStatus::Fresh,
        });
    }

    let mut groups: std::collections::BTreeMap<
        String,
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::BTreeMap::new();
    for t in db_tasks {
        let session_key = archived_session_key(t.session_id.as_deref());
        groups.entry(session_key).or_default().push(t);
    }

    let summaries_by_key: HashMap<
        String,
        crate::db::task_queries::PersistedArchivedSessionSummary,
    > = cached_summaries
        .into_iter()
        .map(|summary| (summary.session_key.clone(), summary))
        .collect();
    let cache_status = archived_session_cache_status(&groups, &summaries_by_key);

    let mut sessions: Vec<crate::task_scanner::ArchivedSession> = groups
        .iter()
        .map(|(session_key, raw)| {
            build_cached_archived_session(session_key, raw, summaries_by_key.get(session_key))
        })
        .collect();

    for session in &mut sessions {
        let Some(session_id) = session.session_id.as_deref() else {
            continue;
        };
        session.account_label = session.sources.iter().find_map(|source| {
            source.parse::<CliTool>().ok().and_then(|tool| {
                crate::session_scanner::accounts::account_label_for_session(
                    tool,
                    &normalized_path,
                    session_id,
                )
            })
        });
    }

    sessions.sort_by(|a, b| match (&b.started_at, &a.started_at) {
        (Some(b_start), Some(a_start)) => b_start.cmp(a_start),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(ArchivedSessionsQueryResult {
        result: crate::task_scanner::ArchivedSessionsResult {
            sessions,
            errors: vec![],
        },
        cache_status,
    })
}

pub fn rebuild_archived_session_summaries(
    db: &DbState,
    providers: &ProviderState,
    project_path: String,
) -> IpcResult<usize> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());
    let db_tasks = {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        crate::db::task_queries::get_archived_tasks_for_project(&conn, &normalized_path)
            .sanitize_err()?
    };

    let provider = providers.resolve(&project_path);
    let claude_team_names = known_claude_team_names();

    let mut groups: std::collections::BTreeMap<
        String,
        Vec<crate::db::task_queries::PersistedTask>,
    > = std::collections::BTreeMap::new();
    for task in db_tasks {
        let session_key = archived_session_key(task.session_id.as_deref());
        groups.entry(session_key).or_default().push(task);
    }

    let updated_at = Utc::now().to_rfc3339();
    let summaries: Vec<crate::db::task_queries::PersistedArchivedSessionSummary> = groups
        .iter()
        .map(|(session_key, raw_tasks)| {
            build_archived_session_summary(
                session_key,
                raw_tasks,
                provider,
                &project_path,
                &claude_team_names,
                &updated_at,
            )
        })
        .collect();

    {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        crate::db::task_queries::replace_archived_session_summaries_for_project(
            &conn,
            &normalized_path,
            &summaries,
        )
        .sanitize_err()?;
    }

    Ok(summaries.len())
}

/// Get files changed by a specific commit.
pub fn get_commit_files(
    providers: &ProviderState,
    project_path: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    let provider = providers.resolve(&project_path);
    provider.commit_files(&project_path, &hash).ipc()
}

/// Get diff hunks for a specific file in a specific commit.
pub fn get_commit_diff(
    providers: &ProviderState,
    project_path: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    let provider = providers.resolve(&project_path);
    provider.commit_diff(&project_path, &hash, &file_path).ipc()
}

/// Get commits and files changed in a time range.
pub fn get_commits_in_range(
    providers: &ProviderState,
    project_path: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    let provider = providers.resolve(&project_path);
    let range = provider
        .commits_in_range(
            &project_path,
            &after,
            &before,
            Some(crate::git::commits::DEFAULT_RANGE_QUERY_COMMIT_CAP),
        )
        .ipc()?;
    Ok(crate::models::GitRangeResult {
        commits: range.commits,
        files: range.files,
        truncated: range.truncated,
        total_count: range.total_count,
    })
}

pub(crate) fn persisted_to_unified(
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
            "stale" => crate::task_scanner::TaskStatus::Stale,
            _ => crate::task_scanner::TaskStatus::Pending,
        },
        source: CliTool::from_persisted(&t.source),
        blocks: t.blocks,
        blocked_by: t.blocked_by,
        owner: t.owner,
        session_id: t.session_id,
        state_changed_at: t.state_changed_at,
        updated_at: Some(t.updated_at),
        archived_at: t.archived_at,
        last_status: t.last_status,
        archived_reason: t.archived_reason,
        effort: t.effort,
        effort_why: t.effort_why,
        deadline_minutes: t.deadline_minutes,
        // SQLite does not persist ledger metadata. This rehydrated path is
        // presentation-only; terminal telemetry consumes the fresh scanner
        // result before persistence in daemon/handlers.rs.
        has_review_ruling: false,
    }
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

fn build_archived_session_summary(
    session_key: &str,
    raw_tasks: &[crate::db::task_queries::PersistedTask],
    provider: &dyn crate::provider::ProjectProvider,
    project_path: &str,
    claude_team_names: &HashSet<String>,
    updated_at: &str,
) -> crate::db::task_queries::PersistedArchivedSessionSummary {
    let tasks: Vec<crate::task_scanner::UnifiedTask> = raw_tasks
        .iter()
        .cloned()
        .map(persisted_to_unified)
        .collect();
    let session_id = raw_tasks.iter().find_map(|task| task.session_id.clone());
    let sources = unique_sources(&tasks);
    let mut enrichment_warnings = Vec::new();
    let (started_at, ended_at, duration_ms) = derive_archive_time_range(
        &session_id,
        raw_tasks,
        &sources,
        project_path,
        claude_team_names,
        &mut enrichment_warnings,
    );

    let (commit_count, file_count) = match (&started_at, &ended_at) {
        (Some(started_at), Some(ended_at)) => provider
            .commits_in_range(project_path, started_at, ended_at, None)
            .map(|result| (result.commits.len(), result.files.len()))
            .unwrap_or_else(|err| {
                enrichment_warnings.push(format!(
                    "Failed to enrich git counts for session {} in [{started_at}..{ended_at}]: {err}",
                    session_id.as_deref().unwrap_or("<ungrouped>")
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

    crate::db::task_queries::PersistedArchivedSessionSummary {
        project_path: crate::provider::path::normalize_project_path(project_path),
        session_key: session_key.to_string(),
        session_id,
        started_at,
        ended_at,
        duration_ms,
        commit_count,
        file_count,
        sources,
        last_archived_at,
        enrichment_warnings,
        updated_at: updated_at.to_string(),
    }
}

fn archived_session_cache_status(
    groups: &std::collections::BTreeMap<String, Vec<crate::db::task_queries::PersistedTask>>,
    summaries_by_key: &HashMap<String, crate::db::task_queries::PersistedArchivedSessionSummary>,
) -> ArchivedSessionCacheStatus {
    if groups.is_empty() {
        return ArchivedSessionCacheStatus::Fresh;
    }
    if summaries_by_key.is_empty() {
        return ArchivedSessionCacheStatus::Missing;
    }
    if summaries_by_key.len() != groups.len() {
        return ArchivedSessionCacheStatus::Stale;
    }

    for (session_key, raw_tasks) in groups {
        let Some(summary) = summaries_by_key.get(session_key) else {
            return ArchivedSessionCacheStatus::Stale;
        };
        let expected_last_archived_at = raw_tasks
            .iter()
            .filter_map(|task| task.archived_at.as_deref())
            .max()
            .map(String::from);
        if summary.last_archived_at != expected_last_archived_at {
            return ArchivedSessionCacheStatus::Stale;
        }
    }

    ArchivedSessionCacheStatus::Fresh
}

fn build_cached_archived_session(
    session_key: &str,
    raw_tasks: &[crate::db::task_queries::PersistedTask],
    summary: Option<&crate::db::task_queries::PersistedArchivedSessionSummary>,
) -> crate::task_scanner::ArchivedSession {
    let tasks: Vec<crate::task_scanner::UnifiedTask> = raw_tasks
        .iter()
        .cloned()
        .map(persisted_to_unified)
        .collect();

    if let Some(summary) = summary {
        return crate::task_scanner::ArchivedSession {
            session_id: summary.session_id.clone(),
            account_label: None,
            started_at: summary.started_at.clone(),
            ended_at: summary.ended_at.clone(),
            duration_ms: summary.duration_ms,
            tasks,
            commit_count: summary.commit_count,
            file_count: summary.file_count,
            sources: summary.sources.clone(),
            last_archived_at: summary.last_archived_at.clone(),
            enrichment_warnings: summary.enrichment_warnings.clone(),
        };
    }

    let fallback_sources = unique_sources(&tasks);
    let (started_at, ended_at, duration_ms) = time_range_from_tasks(raw_tasks);
    let last_archived_at = raw_tasks
        .iter()
        .filter_map(|task| task.archived_at.clone())
        .max();

    crate::task_scanner::ArchivedSession {
        session_id: if session_key == "<ungrouped>" {
            None
        } else {
            raw_tasks.iter().find_map(|task| task.session_id.clone())
        },
        account_label: None,
        started_at,
        ended_at,
        duration_ms,
        tasks,
        commit_count: 0,
        file_count: 0,
        sources: fallback_sources,
        last_archived_at,
        enrichment_warnings: vec!["History enrichment pending background refresh.".to_string()],
    }
}

pub(crate) fn derive_archive_time_range(
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

fn unique_sources(tasks: &[crate::task_scanner::UnifiedTask]) -> Vec<String> {
    let mut s: Vec<String> = tasks.iter().map(|t| t.source.to_string()).collect();
    s.sort();
    s.dedup();
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

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
            effort: None,
            effort_why: None,
            deadline_minutes: None,
        }
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
    fn retired_database_tool_loads_as_unknown() {
        // Regression: commit 4cd067a removed the old wire value while task rows
        // still persisted it, causing upgrades to relabel those rows as Claude.
        let task = make_archived_task(
            "gemini",
            "legacy-session",
            Some("legacy-session"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        );

        assert_eq!(persisted_to_unified(task).source, CliTool::Unknown);
    }

    // Regression: 1bb8668e introduced the deadline pass's `stale` status, but
    // the DB-to-board adapter normalized that closed token back to `pending`.
    #[test]
    fn stale_database_task_stays_closed_in_the_unified_model() {
        let mut task = make_archived_task(
            "claude",
            "deadline-team",
            Some("deadline-team"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        );
        task.status = "stale".to_string();

        assert_eq!(
            persisted_to_unified(task).status,
            crate::task_scanner::TaskStatus::Stale
        );
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

        let _ = derive_archive_time_range(
            &Some("taurhaus-team".to_string()),
            &tasks,
            &sources,
            "/projects/does-not-exist",
            &claude_team_names,
            &mut warnings,
        );

        assert!(warnings.is_empty());
    }

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");
        (DbState(Mutex::new(conn)), tmp)
    }

    fn insert_project(db: &DbState, project_id: &str, project_path: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let project = crate::models::Project {
            id: project_id.to_string(),
            name: format!("project-{project_id}"),
            path: project_path.to_string(),
            description: None,
            last_activity_at: None,
            hero_preference: None,
            created_at: now.clone(),
            updated_at: now,
            cached_branch: None,
            cached_is_dirty: None,
            account_memory: Default::default(),
        };
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::insert_project(&conn, &project).expect("insert project");
    }

    fn local_provider_state() -> ProviderState {
        ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: None,
            wsl_distro: None,
        }
    }

    #[test]
    fn the_board_reads_the_assignment_effort_off_the_persisted_task() {
        // The lead's per-assignment effort is a different number from the one
        // the session launched with, so the card has to carry it explicitly.
        let mut persisted = make_archived_task(
            "claude",
            "session-1",
            Some("session-1"),
            "2026-08-29T09:00:00Z",
            "2026-08-29T09:30:00Z",
        );
        persisted.effort = Some("high".to_string());
        persisted.effort_why = Some("the migration is irreversible".to_string());

        let unified = persisted_to_unified(persisted);

        assert_eq!(unified.effort.as_deref(), Some("high"));
        assert_eq!(
            unified.effort_why.as_deref(),
            Some("the migration is irreversible")
        );
    }

    fn insert_task(
        db: &DbState,
        project_path: &str,
        source_key: &str,
        task_id: &str,
        status: &str,
        updated_at: &str,
    ) {
        let task = crate::db::task_queries::PersistedTask {
            project_path: project_path.to_string(),
            source: "claude".to_string(),
            source_key: source_key.to_string(),
            source_task_id: task_id.to_string(),
            subject: format!("Task {task_id}"),
            description: Some("detail".to_string()),
            active_form: Some("Doing the work".to_string()),
            status: status.to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: Some("developer3".to_string()),
            session_id: Some(source_key.to_string()),
            first_seen_at: updated_at.to_string(),
            state_changed_at: Some(updated_at.to_string()),
            updated_at: updated_at.to_string(),
            archived_at: None,
            last_status: Some(status.to_string()),
            archived_reason: None,
            effort: None,
            effort_why: None,
            deadline_minutes: None,
        };

        let conn = db.0.lock().expect("db lock");
        crate::db::task_queries::upsert_task(&conn, &task).expect("insert task");
    }

    #[test]
    fn get_or_refresh_project_tasks_returns_persisted_rows_only() {
        let (db, _tmp) = test_db_state();
        insert_project(
            &db,
            "proj-taurhaus",
            r"\\wsl.localhost\Ubuntu\home\user\projects\taurhaus",
        );
        insert_task(
            &db,
            "/home/user/projects/taurhaus",
            "taurhaus-team",
            "914",
            "completed",
            "2026-03-08T05:14:54.154825060+00:00",
        );

        let providers = local_provider_state();
        let generation_state = TaskScanGenerationState::default();

        let result = get_or_refresh_project_tasks(
            &db,
            &providers,
            &generation_state,
            r"\\wsl.localhost\Ubuntu\home\user\projects\taurhaus".to_string(),
        )
        .expect("project task query");

        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].id, "914");
    }

    #[test]
    fn get_archived_sessions_reports_missing_cache_and_fallback_summary() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "proj-archive", "/projects/archive");
        let mut task = make_archived_task(
            "claude",
            "session-a",
            Some("session-a"),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        );
        task.project_path = "/projects/archive".to_string();
        {
            let conn = db.0.lock().expect("db lock");
            crate::db::task_queries::upsert_task(&conn, &task).expect("insert archived task");
            crate::db::task_queries::archive_or_delete_stale_tasks(
                &conn,
                "/projects/archive",
                "claude",
                "session-a",
                &[],
            )
            .expect("archive");
        }

        let query = get_archived_sessions(
            &db,
            &local_provider_state(),
            "/projects/archive".to_string(),
        )
        .expect("archived sessions");

        assert_eq!(query.cache_status, ArchivedSessionCacheStatus::Missing);
        assert_eq!(query.result.sessions.len(), 1);
        assert_eq!(query.result.sessions[0].commit_count, 0);
        assert_eq!(query.result.sessions[0].file_count, 0);
        assert_eq!(
            query.result.sessions[0].enrichment_warnings,
            vec!["History enrichment pending background refresh.".to_string()]
        );
    }

    #[test]
    fn archived_session_payload_includes_the_transcript_account_label() {
        // Regression: 179a767 added `session.account_label` markup without a
        // producer, so every archived-session row omitted its account.
        let project_path = "/projects/archive-label";
        let session_id = "session-label";
        let config = tempfile::tempdir().unwrap();
        let account_dir = config.path().join(".claude-second");
        let transcript = account_dir
            .join("projects")
            .join(crate::session_scanner::idle::path_to_slug(project_path))
            .join(format!("{session_id}.jsonl"));
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, "{}\n").unwrap();
        let _accounts = crate::session_scanner::accounts::install_detection_override(
            CliTool::Claude,
            crate::session_scanner::accounts::AccountScan {
                config_dirs: vec![account_dir.clone()],
                accounts: vec![crate::session_scanner::accounts::Account {
                    tool: CliTool::Claude,
                    id: "account-label".to_string(),
                    dir: account_dir,
                    identity: crate::session_scanner::accounts::AccountIdentity {
                        id: "account-label".to_string(),
                        label: "label@example.com".to_string(),
                        display_name: Some("Labelled account".to_string()),
                        organization: None,
                        plan: None,
                        logged_in: true,
                        usage_capable: true,
                        credential_expires_at: None,
                    },
                    is_default: false,
                    is_process_default: false,
                    usage: None,
                }],
            },
        );
        let (db, _tmp) = test_db_state();
        insert_project(&db, "proj-archive-label", project_path);
        let mut task = make_archived_task(
            "claude",
            session_id,
            Some(session_id),
            "2026-03-01T10:00:00Z",
            "2026-03-01T11:00:00Z",
        );
        task.project_path = project_path.to_string();
        {
            let conn = db.0.lock().expect("db lock");
            crate::db::task_queries::upsert_task(&conn, &task).expect("insert archived task");
            crate::db::task_queries::archive_or_delete_stale_tasks(
                &conn,
                project_path,
                "claude",
                session_id,
                &[],
            )
            .expect("archive");
        }

        let query = get_archived_sessions(&db, &local_provider_state(), project_path.to_string())
            .expect("archived sessions");

        assert_eq!(
            query.result.sessions[0].account_label.as_deref(),
            Some("Labelled account")
        );
    }
}
