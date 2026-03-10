use chrono::{DateTime, Utc};
use std::collections::HashSet;

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
    let initial = get_project_tasks(db, project_path.clone())?;
    if !initial.tasks.is_empty() {
        return Ok(initial);
    }

    tracing::info!(
        project_path,
        "Task query returned no persisted tasks; running on-demand recovery scan"
    );

    let scan_result = crate::services::task_sync::scan_tasks_from_files(
        providers,
        &project_path,
        None,
        None,
        None,
    );
    let normalized_path = crate::provider::path::normalize_project_path(&project_path);
    let recovery_generation = chrono::Utc::now().timestamp_millis().max(0) as u64;

    {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        crate::services::task_sync::persist_task_scan_with_generation(
            &conn,
            &normalized_path,
            &scan_result,
            generation_state,
            recovery_generation,
        );
    }

    get_project_tasks(db, project_path)
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
    providers: &ProviderState,
    project_path: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    let normalized_path =
        crate::provider::path::to_linux(&project_path).unwrap_or_else(|| project_path.clone());
    let provider = providers.resolve(&project_path);

    let db_tasks = {
        let conn = db.0.lock().map_err(|e| format!("{e}"))?;
        crate::db::task_queries::get_archived_tasks_for_project(&conn, &normalized_path)
            .sanitize_err()?
    };

    if db_tasks.is_empty() {
        return Ok(crate::task_scanner::ArchivedSessionsResult {
            sessions: vec![],
            errors: vec![],
        });
    }

    let claude_team_names = known_claude_team_names();

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

    let (commit_count, file_count) = match (&started_at, &ended_at) {
        (Some(s), Some(e)) => provider
            .commits_in_range(project_path, s, e, None)
            .map(|result| (result.commits.len(), result.files.len()))
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
    use std::fs;
    use std::io::Write;
    use std::sync::{LazyLock, Mutex};
    use tempfile::{NamedTempFile, TempDir};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

    fn write_file(path: &std::path::Path, contents: &str) {
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(contents.as_bytes()).expect("write file");
        file.sync_all().expect("sync file");
    }

    #[test]
    fn get_or_refresh_project_tasks_recovers_from_empty_db_for_windows_unc_project() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = TempDir::new().expect("tempdir");
        let claude_dir = temp.path().join("claude");
        let tasks_dir = claude_dir.join("tasks").join("taurhaus-team");
        let teams_dir = claude_dir.join("teams").join("taurhaus-team");
        fs::create_dir_all(&tasks_dir).expect("create tasks dir");
        fs::create_dir_all(&teams_dir).expect("create teams dir");

        write_file(
            &tasks_dir.join("909.json"),
            r#"{
                "id":"909",
                "subject":"Investigate task tracking",
                "activeForm":"Investigating task tracking",
                "status":"in_progress",
                "blocks":[],
                "blockedBy":[],
                "owner":"developer3"
            }"#,
        );
        write_file(
            &teams_dir.join("config.json"),
            r#"{
                "members":[
                    {
                        "projectPath":"/home/mstie/projects/taurhaus"
                    }
                ]
            }"#,
        );

        std::env::set_var("TAURHAUS_CLAUDE_DIR", &claude_dir);

        let (db, _tmp) = test_db_state();
        insert_project(
            &db,
            "proj-taurhaus",
            r"\\wsl.localhost\Ubuntu\home\mstie\projects\taurhaus",
        );
        let providers = local_provider_state();
        let generation_state = TaskScanGenerationState::default();

        let result = get_or_refresh_project_tasks(
            &db,
            &providers,
            &generation_state,
            r"\\wsl.localhost\Ubuntu\home\mstie\projects\taurhaus".to_string(),
        )
        .expect("recovered task query");

        std::env::remove_var("TAURHAUS_CLAUDE_DIR");

        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].id, "909");
        assert_eq!(result.tasks[0].source_key, "taurhaus-team");

        let persisted = get_project_tasks(
            &db,
            r"\\wsl.localhost\Ubuntu\home\mstie\projects\taurhaus".to_string(),
        )
        .expect("persisted task query");
        assert_eq!(persisted.tasks.len(), 1);
        assert_eq!(persisted.tasks[0].id, "909");
    }
}
