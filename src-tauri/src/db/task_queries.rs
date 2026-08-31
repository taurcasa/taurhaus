use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A persisted task row, matching the `tasks` table schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedTask {
    pub project_path: String,
    pub source: String,
    pub source_key: String,
    pub source_task_id: String,
    pub subject: String,
    pub description: Option<String>,
    pub active_form: Option<String>,
    pub status: String,
    pub blocks: Vec<String>,
    pub blocked_by: Vec<String>,
    pub owner: Option<String>,
    pub session_id: Option<String>,
    pub first_seen_at: String,
    pub state_changed_at: Option<String>,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub last_status: Option<String>,
    pub archived_reason: Option<String>,
    /// Reasoning effort the lead attached when assigning this task.
    pub effort: Option<String>,
    /// Why the lead chose that level.
    pub effort_why: Option<String>,
    /// Deadline in minutes from the mesh assignment metadata.
    pub deadline_minutes: Option<u32>,
}

/// A persisted archived session summary row, used to keep History loads off the
/// transcript/git enrichment path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedArchivedSessionSummary {
    pub project_path: String,
    pub session_key: String,
    pub session_id: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub commit_count: usize,
    pub file_count: usize,
    pub sources: Vec<String>,
    pub last_archived_at: Option<String>,
    pub enrichment_warnings: Vec<String>,
    pub updated_at: String,
}

fn decode_json_string_list(
    raw: &str,
    field: &str,
    project_path: &str,
    source: &str,
    source_key: &str,
    source_task_id: &str,
) -> Vec<String> {
    match serde_json::from_str(raw) {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(
                field,
                project_path,
                source,
                source_key,
                source_task_id,
                error = %error,
                "Failed to decode task JSON column; using empty list fallback"
            );
            Vec::new()
        }
    }
}

fn encode_json_string_list(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

/// Upsert a task — insert or update if the composite key already exists.
///
/// Updates subject, description, active_form, status, blocks, blocked_by,
/// owner, session_id, and updated_at on conflict.
pub fn upsert_task(conn: &Connection, task: &PersistedTask) -> Result<(), rusqlite::Error> {
    let blocks_json = serde_json::to_string(&task.blocks).unwrap_or_else(|_| "[]".to_string());
    let blocked_by_json =
        serde_json::to_string(&task.blocked_by).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO tasks (
            project_path, source, source_key, source_task_id, subject, description, active_form, status,
            blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at,
            last_status, archived_reason, effort, effort_why, deadline_minutes
        )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, NULL, ?16, NULL, ?17, ?18, ?19)
         ON CONFLICT (project_path, source, source_key, source_task_id) WHERE archived_at IS NULL DO UPDATE SET
            subject = excluded.subject,
            description = excluded.description,
            active_form = excluded.active_form,
            status = excluded.status,
            blocks = excluded.blocks,
            blocked_by = excluded.blocked_by,
            owner = excluded.owner,
            session_id = excluded.session_id,
            state_changed_at = CASE
                WHEN tasks.status != excluded.status THEN excluded.updated_at
                ELSE tasks.state_changed_at
            END,
            updated_at = CASE
                WHEN tasks.subject != excluded.subject
                  OR tasks.description IS NOT excluded.description
                  OR tasks.active_form IS NOT excluded.active_form
                  OR tasks.status != excluded.status
                  OR tasks.blocks != excluded.blocks
                  OR tasks.blocked_by != excluded.blocked_by
                  OR tasks.owner IS NOT excluded.owner
                  OR tasks.session_id IS NOT excluded.session_id
                  OR tasks.effort IS NOT excluded.effort
                  OR tasks.effort_why IS NOT excluded.effort_why
                  OR tasks.deadline_minutes IS NOT excluded.deadline_minutes
                THEN excluded.updated_at
                ELSE tasks.updated_at
            END,
            archived_at = NULL,
            last_status = excluded.status,
            archived_reason = NULL,
            effort = excluded.effort,
            effort_why = excluded.effort_why,
            deadline_minutes = excluded.deadline_minutes",
        params![
            task.project_path,
            task.source,
            task.source_key,
            task.source_task_id,
            task.subject,
            task.description,
            task.active_form,
            task.status,
            blocks_json,
            blocked_by_json,
            task.owner,
            task.session_id,
            task.first_seen_at,
            task.state_changed_at,
            task.updated_at,
            task.last_status,
            task.effort,
            task.effort_why,
            task.deadline_minutes,
        ],
    )?;
    Ok(())
}

/// Upsert multiple tasks in a single transaction.
pub fn upsert_tasks(conn: &Connection, tasks: &[PersistedTask]) -> Result<(), rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;
    for task in tasks {
        upsert_task(&tx, task)?;
    }
    tx.commit()
}

/// Get all active tasks for a project, ordered by source/source_key/source_task_id.
pub fn get_tasks_for_project(
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<PersistedTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason, effort, effort_why, deadline_minutes
         FROM tasks
         WHERE project_path = ?1 AND archived_at IS NULL
         ORDER BY source, source_key, source_task_id",
    )?;

    let tasks = stmt
        .query_map([project_path], |row| {
            let blocks_str: String = row.get(8)?;
            let blocked_by_str: String = row.get(9)?;
            let project_path: String = row.get(0)?;
            let source: String = row.get(1)?;
            let source_key: String = row.get(2)?;
            let source_task_id: String = row.get(3)?;
            Ok(PersistedTask {
                project_path: project_path.clone(),
                source: source.clone(),
                source_key: source_key.clone(),
                source_task_id: source_task_id.clone(),
                subject: row.get(4)?,
                description: row.get(5)?,
                active_form: row.get(6)?,
                status: row.get(7)?,
                blocks: decode_json_string_list(
                    &blocks_str,
                    "blocks",
                    &project_path,
                    &source,
                    &source_key,
                    &source_task_id,
                ),
                blocked_by: decode_json_string_list(
                    &blocked_by_str,
                    "blocked_by",
                    &project_path,
                    &source,
                    &source_key,
                    &source_task_id,
                ),
                owner: row.get(10)?,
                session_id: row.get(11)?,
                first_seen_at: row.get(12)?,
                state_changed_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
                last_status: row.get(16)?,
                archived_reason: row.get(17)?,
                effort: row.get(18)?,
                effort_why: row.get(19)?,
                deadline_minutes: row.get(20)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
}

/// Get all distinct active source keys for a project/source pair.
pub fn get_active_source_keys_for_project_source(
    conn: &Connection,
    project_path: &str,
    source: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT source_key
         FROM tasks
         WHERE project_path = ?1 AND source = ?2 AND archived_at IS NULL
         ORDER BY source_key",
    )?;

    let rows = stmt.query_map(params![project_path, source], |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>()
}

/// Get one active task by its full identity key.
pub fn get_task_for_project_by_identity(
    conn: &Connection,
    project_path: &str,
    source: &str,
    source_key: &str,
    source_task_id: &str,
) -> Result<Option<PersistedTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason, effort, effort_why, deadline_minutes
         FROM tasks
         WHERE project_path = ?1 AND source = ?2 AND source_key = ?3 AND source_task_id = ?4 AND archived_at IS NULL
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![project_path, source, source_key, source_task_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let blocks_str: String = row.get(8)?;
    let blocked_by_str: String = row.get(9)?;
    let project_path_value: String = row.get(0)?;
    let source_value: String = row.get(1)?;
    let source_key_value: String = row.get(2)?;
    let source_task_id_value: String = row.get(3)?;
    Ok(Some(PersistedTask {
        project_path: project_path_value.clone(),
        source: source_value.clone(),
        source_key: source_key_value.clone(),
        source_task_id: source_task_id_value.clone(),
        subject: row.get(4)?,
        description: row.get(5)?,
        active_form: row.get(6)?,
        status: row.get(7)?,
        blocks: decode_json_string_list(
            &blocks_str,
            "blocks",
            &project_path_value,
            &source_value,
            &source_key_value,
            &source_task_id_value,
        ),
        blocked_by: decode_json_string_list(
            &blocked_by_str,
            "blocked_by",
            &project_path_value,
            &source_value,
            &source_key_value,
            &source_task_id_value,
        ),
        owner: row.get(10)?,
        session_id: row.get(11)?,
        first_seen_at: row.get(12)?,
        state_changed_at: row.get(13)?,
        updated_at: row.get(14)?,
        archived_at: row.get(15)?,
        last_status: row.get(16)?,
        archived_reason: row.get(17)?,
        effort: row.get(18)?,
        effort_why: row.get(19)?,
        deadline_minutes: row.get(20)?,
    }))
}

/// Get most-recent archived task by full identity key.
pub fn get_archived_task_for_project_by_identity(
    conn: &Connection,
    project_path: &str,
    source: &str,
    source_key: &str,
    source_task_id: &str,
) -> Result<Option<PersistedTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason, effort, effort_why, deadline_minutes
         FROM tasks
         WHERE project_path = ?1 AND source = ?2 AND source_key = ?3 AND source_task_id = ?4 AND archived_at IS NOT NULL
         ORDER BY archived_at DESC
         LIMIT 1",
    )?;

    let mut rows = stmt.query(params![project_path, source, source_key, source_task_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let blocks_str: String = row.get(8)?;
    let blocked_by_str: String = row.get(9)?;
    let project_path_value: String = row.get(0)?;
    let source_value: String = row.get(1)?;
    let source_key_value: String = row.get(2)?;
    let source_task_id_value: String = row.get(3)?;
    Ok(Some(PersistedTask {
        project_path: project_path_value.clone(),
        source: source_value.clone(),
        source_key: source_key_value.clone(),
        source_task_id: source_task_id_value.clone(),
        subject: row.get(4)?,
        description: row.get(5)?,
        active_form: row.get(6)?,
        status: row.get(7)?,
        blocks: decode_json_string_list(
            &blocks_str,
            "blocks",
            &project_path_value,
            &source_value,
            &source_key_value,
            &source_task_id_value,
        ),
        blocked_by: decode_json_string_list(
            &blocked_by_str,
            "blocked_by",
            &project_path_value,
            &source_value,
            &source_key_value,
            &source_task_id_value,
        ),
        owner: row.get(10)?,
        session_id: row.get(11)?,
        first_seen_at: row.get(12)?,
        state_changed_at: row.get(13)?,
        updated_at: row.get(14)?,
        archived_at: row.get(15)?,
        last_status: row.get(16)?,
        archived_reason: row.get(17)?,
        effort: row.get(18)?,
        effort_why: row.get(19)?,
        deadline_minutes: row.get(20)?,
    }))
}

/// Handle stale tasks for a project+source+source_key that are NOT in the given set of IDs.
///
/// Used after scanning: if the scanner returned tasks `{1, 3, 5}` for a specific
/// source key, any rows in DB with IDs not in that set are stale (deleted from
/// disk or status changed to "deleted").
///
/// - **Completed** stale tasks are archived (`archived_at` set to now) — they
///   represent finished work and should be preserved for history.
/// - **Non-completed** stale tasks are hard-deleted — they represent abandoned
///   or superseded work.
pub fn archive_or_delete_stale_tasks(
    conn: &Connection,
    project_path: &str,
    source: &str,
    source_key: &str,
    active_ids: &[&str],
) -> Result<StaleTaskResult, rusqlite::Error> {
    let stale_filter = if active_ids.is_empty() {
        String::new()
    } else {
        let placeholders: Vec<String> = (0..active_ids.len())
            .map(|i| format!("?{}", i + 4))
            .collect();
        format!("AND source_task_id NOT IN ({})", placeholders.join(", "))
    };

    let mut sql_params: Vec<String> = vec![
        project_path.to_string(),
        source.to_string(),
        source_key.to_string(),
    ];
    sql_params.extend(active_ids.iter().map(|id| id.to_string()));

    // Archive completed stale tasks
    let now = chrono::Utc::now().to_rfc3339();
    let archive_sql = format!(
        "UPDATE tasks SET archived_at = '{now}', last_status = status, archived_reason = 'completed_and_removed' \
         WHERE project_path = ?1 AND source = ?2 AND source_key = ?3 \
         {stale_filter} \
         AND status = 'completed' AND archived_at IS NULL"
    );
    let archived = conn.execute(&archive_sql, rusqlite::params_from_iter(sql_params.iter()))?;

    // Delete non-completed stale tasks
    let delete_sql = format!(
        "DELETE FROM tasks \
         WHERE project_path = ?1 AND source = ?2 AND source_key = ?3 \
         {stale_filter} \
         AND status != 'completed' AND archived_at IS NULL"
    );
    let deleted = conn.execute(&delete_sql, rusqlite::params_from_iter(sql_params.iter()))?;

    Ok(StaleTaskResult { archived, deleted })
}

/// Result of stale task handling — how many were archived vs deleted.
#[derive(Debug, PartialEq)]
pub struct StaleTaskResult {
    pub archived: usize,
    pub deleted: usize,
}

/// Get archived tasks for a project, ordered by session/source/source_key/task.
///
/// Returns tasks where `archived_at IS NOT NULL`. The caller groups these by
/// `session_id` to build the session history timeline.
pub fn get_archived_tasks_for_project(
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<PersistedTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason, effort, effort_why, deadline_minutes
         FROM tasks
         WHERE project_path = ?1 AND archived_at IS NOT NULL
         ORDER BY session_id, source, source_key, source_task_id",
    )?;

    let tasks = stmt
        .query_map([project_path], |row| {
            let blocks_str: String = row.get(8)?;
            let blocked_by_str: String = row.get(9)?;
            let project_path: String = row.get(0)?;
            let source: String = row.get(1)?;
            let source_key: String = row.get(2)?;
            let source_task_id: String = row.get(3)?;
            Ok(PersistedTask {
                project_path: project_path.clone(),
                source: source.clone(),
                source_key: source_key.clone(),
                source_task_id: source_task_id.clone(),
                subject: row.get(4)?,
                description: row.get(5)?,
                active_form: row.get(6)?,
                status: row.get(7)?,
                blocks: decode_json_string_list(
                    &blocks_str,
                    "blocks",
                    &project_path,
                    &source,
                    &source_key,
                    &source_task_id,
                ),
                blocked_by: decode_json_string_list(
                    &blocked_by_str,
                    "blocked_by",
                    &project_path,
                    &source,
                    &source_key,
                    &source_task_id,
                ),
                owner: row.get(10)?,
                session_id: row.get(11)?,
                first_seen_at: row.get(12)?,
                state_changed_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
                last_status: row.get(16)?,
                archived_reason: row.get(17)?,
                effort: row.get(18)?,
                effort_why: row.get(19)?,
                deadline_minutes: row.get(20)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
}

pub fn archived_session_key(session_id: Option<&str>) -> String {
    session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<ungrouped>")
        .to_string()
}

pub fn get_archived_session_summaries_for_project(
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<PersistedArchivedSessionSummary>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, session_key, session_id, started_at, ended_at, duration_ms,
                commit_count, file_count, sources_json, last_archived_at, enrichment_warnings, updated_at
         FROM archived_task_session_summaries
         WHERE project_path = ?1
         ORDER BY last_archived_at DESC, ended_at DESC, session_key",
    )?;

    let rows = stmt.query_map([project_path], |row| {
        let project_path: String = row.get(0)?;
        let session_key: String = row.get(1)?;
        let sources_json: String = row.get(8)?;
        let warnings_json: String = row.get(10)?;
        Ok(PersistedArchivedSessionSummary {
            project_path: project_path.clone(),
            session_key: session_key.clone(),
            session_id: row.get(2)?,
            started_at: row.get(3)?,
            ended_at: row.get(4)?,
            duration_ms: row.get(5)?,
            commit_count: row.get(6)?,
            file_count: row.get(7)?,
            sources: decode_json_string_list(
                &sources_json,
                "sources_json",
                &project_path,
                "history_cache",
                &session_key,
                &session_key,
            ),
            last_archived_at: row.get(9)?,
            enrichment_warnings: decode_json_string_list(
                &warnings_json,
                "enrichment_warnings",
                &project_path,
                "history_cache",
                &session_key,
                &session_key,
            ),
            updated_at: row.get(11)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub fn replace_archived_session_summaries_for_project(
    conn: &Connection,
    project_path: &str,
    summaries: &[PersistedArchivedSessionSummary],
) -> Result<(), rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM archived_task_session_summaries WHERE project_path = ?1",
        [project_path],
    )?;

    if !summaries.is_empty() {
        let mut stmt = tx.prepare(
            "INSERT INTO archived_task_session_summaries (
                project_path, session_key, session_id, started_at, ended_at, duration_ms,
                commit_count, file_count, sources_json, last_archived_at, enrichment_warnings, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;

        for summary in summaries {
            stmt.execute(params![
                summary.project_path,
                summary.session_key,
                summary.session_id,
                summary.started_at,
                summary.ended_at,
                summary.duration_ms,
                summary.commit_count,
                summary.file_count,
                encode_json_string_list(&summary.sources),
                summary.last_archived_at,
                encode_json_string_list(&summary.enrichment_warnings),
                summary.updated_at,
            ])?;
        }
    }

    tx.commit()
}

/// Delete all tasks for a project from a specific source.
/// Useful when re-importing from a source that replaces all tasks (e.g., Codex update_plan).
pub fn delete_tasks_for_source(
    conn: &Connection,
    project_path: &str,
    source: &str,
) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM tasks WHERE project_path = ?1 AND source = ?2",
        params![project_path, source],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn make_task(source: &str, id: &str, subject: &str, status: &str) -> PersistedTask {
        PersistedTask {
            project_path: "/projects/foo".to_string(),
            source: source.to_string(),
            source_key: default_source_key(source),
            source_task_id: id.to_string(),
            subject: subject.to_string(),
            description: None,
            active_form: None,
            status: status.to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            session_id: None,
            first_seen_at: "2026-02-22T10:00:00Z".to_string(),
            state_changed_at: Some("2026-02-22T10:00:00Z".to_string()),
            updated_at: "2026-02-22T10:00:00Z".to_string(),
            archived_at: None,
            last_status: Some(status.to_string()),
            archived_reason: None,
            effort: None,
            effort_why: None,
            deadline_minutes: None,
        }
    }

    #[test]
    fn an_assignment_effort_survives_the_round_trip_and_a_later_update() {
        let (conn, _tmp) = test_db();
        let mut task = make_task("claude", "7", "Migrate the account store", "pending");
        task.effort = Some("high".to_string());
        task.effort_why = Some("the migration is irreversible".to_string());
        task.deadline_minutes = Some(20);
        upsert_task(&conn, &task).unwrap();

        let stored = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(stored[0].effort.as_deref(), Some("high"));
        assert_eq!(
            stored[0].effort_why.as_deref(),
            Some("the migration is irreversible")
        );
        assert_eq!(stored[0].deadline_minutes, Some(20));

        let mut reassigned = task.clone();
        reassigned.status = "in_progress".to_string();
        reassigned.effort = Some("medium".to_string());
        reassigned.effort_why = Some("the risky half is done".to_string());
        upsert_task(&conn, &reassigned).unwrap();

        let stored = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].effort.as_deref(), Some("medium"));
        assert_eq!(
            stored[0].effort_why.as_deref(),
            Some("the risky half is done")
        );
    }

    #[test]
    fn a_task_from_a_source_without_assignments_stores_no_effort() {
        let (conn, _tmp) = test_db();
        upsert_task(&conn, &make_task("codex", "1", "Local todo", "pending")).unwrap();

        let stored = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(stored[0].effort, None);
        assert_eq!(stored[0].effort_why, None);
    }

    fn default_source_key(source: &str) -> String {
        format!("{source}-default")
    }

    fn make_summary(
        session_key: &str,
        session_id: Option<&str>,
    ) -> PersistedArchivedSessionSummary {
        PersistedArchivedSessionSummary {
            project_path: "/projects/foo".to_string(),
            session_key: session_key.to_string(),
            session_id: session_id.map(ToString::to_string),
            started_at: Some("2026-03-01T10:00:00Z".to_string()),
            ended_at: Some("2026-03-01T11:00:00Z".to_string()),
            duration_ms: Some(3_600_000),
            commit_count: 2,
            file_count: 5,
            sources: vec!["claude".to_string(), "codex".to_string()],
            last_archived_at: Some("2026-03-01T11:00:00Z".to_string()),
            enrichment_warnings: vec!["fallback".to_string()],
            updated_at: "2026-03-01T11:05:00Z".to_string(),
        }
    }

    fn explain_plan_details(conn: &Connection, sql: &str, project_path: &str) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut stmt = conn.prepare(&explain_sql).unwrap();
        let rows = stmt
            .query_map([project_path], |row| row.get::<_, String>(3))
            .unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    }

    #[test]
    fn insert_and_retrieve_round_trip() {
        let (conn, _tmp) = test_db();
        let task = make_task("claude", "1", "Implement feature", "in_progress");
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source, "claude");
        assert_eq!(tasks[0].source_task_id, "1");
        assert_eq!(tasks[0].subject, "Implement feature");
        assert_eq!(tasks[0].status, "in_progress");
    }

    #[test]
    fn upsert_updates_existing_task() {
        let (conn, _tmp) = test_db();

        let mut task = make_task("claude", "1", "Original subject", "pending");
        upsert_task(&conn, &task).unwrap();

        task.subject = "Updated subject".to_string();
        task.status = "completed".to_string();
        task.updated_at = "2026-02-22T12:00:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Updated subject");
        assert_eq!(tasks[0].status, "completed");
        // first_seen_at should be preserved (not overwritten)
        assert_eq!(tasks[0].first_seen_at, "2026-02-22T10:00:00Z");
    }

    #[test]
    fn multiple_sources_per_project() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Claude task", "pending")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-0", "Codex task", "in_progress"),
        )
        .unwrap();
        upsert_task(
            &conn,
            &make_task("agy", "todo-1", "Antigravity task", "completed"),
        )
        .unwrap();

        // Rows come back ORDER BY source, so the expected sequence is alphabetical:
        // "agy" sorts ahead of "claude"/"codex" where the retired tool sorted last.
        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].source, "agy");
        assert_eq!(tasks[1].source, "claude");
        assert_eq!(tasks[2].source, "codex");
    }

    #[test]
    fn same_source_task_id_in_different_source_keys_do_not_collide() {
        let (conn, _tmp) = test_db();

        let mut task_a = make_task("claude", "1", "Session task", "pending");
        task_a.source_key = "session-aaa".to_string();
        upsert_task(&conn, &task_a).unwrap();

        let mut task_b = make_task("claude", "1", "Team task", "in_progress");
        task_b.source_key = "team-ops".to_string();
        upsert_task(&conn, &task_b).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].source_key, "session-aaa");
        assert_eq!(tasks[0].subject, "Session task");
        assert_eq!(tasks[1].source_key, "team-ops");
        assert_eq!(tasks[1].subject, "Team task");
    }

    #[test]
    fn different_projects_isolated() {
        let (conn, _tmp) = test_db();

        let mut task_a = make_task("claude", "1", "Task A", "pending");
        task_a.project_path = "/projects/alpha".to_string();
        upsert_task(&conn, &task_a).unwrap();

        let mut task_b = make_task("claude", "1", "Task B", "pending");
        task_b.project_path = "/projects/beta".to_string();
        upsert_task(&conn, &task_b).unwrap();

        let alpha = get_tasks_for_project(&conn, "/projects/alpha").unwrap();
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].subject, "Task A");

        let beta = get_tasks_for_project(&conn, "/projects/beta").unwrap();
        assert_eq!(beta.len(), 1);
        assert_eq!(beta[0].subject, "Task B");
    }

    #[test]
    fn get_archived_task_for_project_by_identity_returns_latest_match() {
        let (conn, _tmp) = test_db();

        let mut first = make_task("claude", "1", "First archive", "completed");
        first.source_key = "session-aaa".to_string();
        upsert_task(&conn, &first).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", "session-aaa", &[])
            .unwrap();

        let mut second = make_task("claude", "1", "Second archive", "completed");
        second.source_key = "session-aaa".to_string();
        second.updated_at = "2026-02-22T12:00:00Z".to_string();
        upsert_task(&conn, &second).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", "session-aaa", &[])
            .unwrap();

        let archived = get_archived_task_for_project_by_identity(
            &conn,
            "/projects/foo",
            "claude",
            "session-aaa",
            "1",
        )
        .unwrap()
        .expect("expected archived task");

        assert_eq!(archived.subject, "Second archive");
        assert!(archived.archived_at.is_some());
    }

    #[test]
    fn get_archived_task_for_project_by_identity_returns_none_when_missing() {
        let (conn, _tmp) = test_db();
        let archived = get_archived_task_for_project_by_identity(
            &conn,
            "/projects/foo",
            "claude",
            "missing",
            "1",
        )
        .unwrap();
        assert_eq!(archived, None);
    }

    #[test]
    fn archived_session_summary_round_trips() {
        let (conn, _tmp) = test_db();
        let summary = make_summary("session-1", Some("session-1"));

        replace_archived_session_summaries_for_project(
            &conn,
            "/projects/foo",
            std::slice::from_ref(&summary),
        )
        .unwrap();

        let loaded = get_archived_session_summaries_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(loaded, vec![summary]);
    }

    #[test]
    fn replacing_archived_session_summaries_clears_old_rows() {
        let (conn, _tmp) = test_db();
        let first = make_summary("session-1", Some("session-1"));
        let second = make_summary("session-2", Some("session-2"));

        replace_archived_session_summaries_for_project(
            &conn,
            "/projects/foo",
            &[first.clone(), second.clone()],
        )
        .unwrap();
        replace_archived_session_summaries_for_project(
            &conn,
            "/projects/foo",
            std::slice::from_ref(&second),
        )
        .unwrap();

        let loaded = get_archived_session_summaries_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(loaded, vec![second]);
    }

    #[test]
    fn upsert_batch() {
        let (conn, _tmp) = test_db();

        let tasks = vec![
            make_task("claude", "1", "Task 1", "pending"),
            make_task("claude", "2", "Task 2", "in_progress"),
            make_task("claude", "3", "Task 3", "completed"),
        ];
        upsert_tasks(&conn, &tasks).unwrap();

        let result = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn delete_tasks_for_source_only_affects_target() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Claude task", "pending")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-0", "Codex task", "pending"),
        )
        .unwrap();

        let deleted = delete_tasks_for_source(&conn, "/projects/foo", "codex").unwrap();
        assert_eq!(deleted, 1);

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source, "claude");
    }

    #[test]
    fn blocks_and_blocked_by_round_trip() {
        let (conn, _tmp) = test_db();

        let mut task = make_task("claude", "2", "Blocked task", "pending");
        task.blocks = vec!["3".to_string(), "4".to_string()];
        task.blocked_by = vec!["1".to_string()];
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks[0].blocks, vec!["3", "4"]);
        assert_eq!(tasks[0].blocked_by, vec!["1"]);
    }

    #[test]
    fn optional_fields_round_trip() {
        let (conn, _tmp) = test_db();

        let mut task = make_task("claude", "1", "Full task", "in_progress");
        task.description = Some("A detailed description".to_string());
        task.active_form = Some("Implementing...".to_string());
        task.owner = Some("agent-1".to_string());
        task.session_id = Some("sess-abc-123".to_string());
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(
            tasks[0].description.as_deref(),
            Some("A detailed description")
        );
        assert_eq!(tasks[0].active_form.as_deref(), Some("Implementing..."));
        assert_eq!(tasks[0].owner.as_deref(), Some("agent-1"));
        assert_eq!(tasks[0].session_id.as_deref(), Some("sess-abc-123"));
    }

    #[test]
    fn empty_project_returns_empty_vec() {
        let (conn, _tmp) = test_db();
        let tasks = get_tasks_for_project(&conn, "/projects/nonexistent").unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn stale_pending_task_is_deleted() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Task 1", "pending")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Task 2", "in_progress")).unwrap();

        // Scan returns only task 1 — task 2 (in_progress) should be deleted
        let result = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["1"],
        )
        .unwrap();
        assert_eq!(result.deleted, 1);
        assert_eq!(result.archived, 0);

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_task_id, "1");
    }

    #[test]
    fn stale_completed_task_is_archived_not_deleted() {
        let (conn, _tmp) = test_db();

        upsert_task(
            &conn,
            &make_task("claude", "1", "Active task", "in_progress"),
        )
        .unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Done task", "completed")).unwrap();

        // Scan returns only task 1 — task 2 (completed) should be archived
        let result = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["1"],
        )
        .unwrap();
        assert_eq!(result.archived, 1);
        assert_eq!(result.deleted, 0);

        // Active query should only show task 1 (archived tasks are filtered out)
        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_task_id, "1");

        // But the archived row still exists in the DB
        let all_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE project_path = '/projects/foo'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(all_count, 2);
    }

    #[test]
    fn stale_mix_archives_completed_deletes_others() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Active", "in_progress")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Done", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "3", "Abandoned", "pending")).unwrap();

        // Scan returns only task 1 — task 2 archived, task 3 deleted
        let result = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["1"],
        )
        .unwrap();
        assert_eq!(result.archived, 1);
        assert_eq!(result.deleted, 1);

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_task_id, "1");

        // Total rows: 1 active + 1 archived = 2
        let all_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE project_path = '/projects/foo'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(all_count, 2);
    }

    #[test]
    fn stale_does_not_affect_other_sources() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Claude task", "pending")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-0", "Codex task", "pending"),
        )
        .unwrap();

        let result = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["1"],
        )
        .unwrap();
        assert_eq!(result.deleted, 0);
        assert_eq!(result.archived, 0);

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn stale_with_empty_active_ids_archives_and_deletes_all_for_source() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Task 1", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Task 2", "pending")).unwrap();

        let empty: Vec<&str> = vec![];
        let result = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &empty,
        )
        .unwrap();
        assert_eq!(result.archived, 1);
        assert_eq!(result.deleted, 1);

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert!(tasks.is_empty());

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(
            archived[0].archived_reason.as_deref(),
            Some("completed_and_removed")
        );
    }

    #[test]
    fn upsert_sets_state_changed_only_on_status_transition() {
        let (conn, _tmp) = test_db();

        let mut task = make_task("claude", "1", "Task", "pending");
        task.updated_at = "2026-02-22T10:00:00Z".to_string();
        task.state_changed_at = Some("2026-02-22T10:00:00Z".to_string());
        upsert_task(&conn, &task).unwrap();

        // Same status; state_changed_at should be preserved.
        task.subject = "Task renamed".to_string();
        task.updated_at = "2026-02-22T10:05:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(
            tasks[0].state_changed_at.as_deref(),
            Some("2026-02-22T10:00:00Z")
        );

        // Status transition; state_changed_at should update to current updated_at.
        task.status = "completed".to_string();
        task.updated_at = "2026-02-22T10:10:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(
            tasks[0].state_changed_at.as_deref(),
            Some("2026-02-22T10:10:00Z")
        );
        assert_eq!(tasks[0].last_status.as_deref(), Some("completed"));
    }

    #[test]
    fn upsert_preserves_updated_at_when_material_fields_do_not_change() {
        let (conn, _tmp) = test_db();

        let mut task = make_task("claude", "1", "Task", "pending");
        task.updated_at = "2026-02-22T10:00:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        task.updated_at = "2026-02-22T10:05:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks[0].updated_at, "2026-02-22T10:00:00Z");

        task.subject = "Task renamed".to_string();
        task.updated_at = "2026-02-22T10:10:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks[0].updated_at, "2026-02-22T10:10:00Z");
    }

    #[test]
    fn upsert_preserves_archived_row_and_inserts_new_active_row() {
        let (conn, _tmp) = test_db();

        // Create and archive a completed task
        upsert_task(&conn, &make_task("claude", "1", "Done task", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Active", "in_progress")).unwrap();
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["2"],
        )
        .unwrap();

        // Task 1 should be archived (not visible)
        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 1);

        // Now task 1 reappears in a scan.
        upsert_task(&conn, &make_task("claude", "1", "Done task", "completed")).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 2);

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].source_task_id, "1");

        let all_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE project_path = '/projects/foo'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(all_count, 3);
    }

    #[test]
    fn already_archived_task_is_not_double_archived() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Done", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Active", "in_progress")).unwrap();

        // First archive
        let r1 = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["2"],
        )
        .unwrap();
        assert_eq!(r1.archived, 1);

        // Second call — task 1 is already archived, should not re-archive
        let r2 = archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["2"],
        )
        .unwrap();
        assert_eq!(r2.archived, 0);
    }

    // --- get_archived_tasks_for_project tests ---

    #[test]
    fn archived_query_returns_only_archived_tasks() {
        let (conn, _tmp) = test_db();

        // Create 3 tasks: one active, two will become stale (one completed → archived, one pending → deleted)
        upsert_task(&conn, &make_task("claude", "1", "Active", "in_progress")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Done A", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "3", "Abandoned", "pending")).unwrap();

        // Archive task 2, delete task 3
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["1"],
        )
        .unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].source_task_id, "2");
        assert_eq!(archived[0].subject, "Done A");
        assert!(archived[0].archived_at.is_some());
    }

    #[test]
    fn archived_query_returns_empty_when_no_archived() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Active", "in_progress")).unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert!(archived.is_empty());
    }

    #[test]
    fn archived_query_returns_empty_for_nonexistent_project() {
        let (conn, _tmp) = test_db();
        let archived = get_archived_tasks_for_project(&conn, "/projects/ghost").unwrap();
        assert!(archived.is_empty());
    }

    #[test]
    fn archived_query_groups_by_session_id() {
        let (conn, _tmp) = test_db();

        // Create tasks with different session_ids
        let mut t1 = make_task("claude", "1", "Session A task 1", "completed");
        t1.session_id = Some("sess-aaa".to_string());
        upsert_task(&conn, &t1).unwrap();

        let mut t2 = make_task("claude", "2", "Session A task 2", "completed");
        t2.session_id = Some("sess-aaa".to_string());
        upsert_task(&conn, &t2).unwrap();

        let mut t3 = make_task("claude", "3", "Session B task 1", "completed");
        t3.session_id = Some("sess-bbb".to_string());
        upsert_task(&conn, &t3).unwrap();

        // Keep one active so we can archive the others
        upsert_task(&conn, &make_task("claude", "99", "Active", "in_progress")).unwrap();

        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["99"],
        )
        .unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 3);

        // Ordered by session_id: sess-aaa (2 tasks) then sess-bbb (1 task)
        assert_eq!(archived[0].session_id.as_deref(), Some("sess-aaa"));
        assert_eq!(archived[1].session_id.as_deref(), Some("sess-aaa"));
        assert_eq!(archived[2].session_id.as_deref(), Some("sess-bbb"));
    }

    #[test]
    fn archived_query_includes_tasks_without_session_id() {
        let (conn, _tmp) = test_db();

        // Task with no session_id (e.g., Agy/Codex source)
        upsert_task(
            &conn,
            &make_task("agy", "todo-1", "Antigravity task", "completed"),
        )
        .unwrap();

        // Task with session_id
        let mut t2 = make_task("claude", "1", "Claude task", "completed");
        t2.session_id = Some("sess-123".to_string());
        upsert_task(&conn, &t2).unwrap();

        // Keep one active
        upsert_task(&conn, &make_task("claude", "99", "Active", "in_progress")).unwrap();
        upsert_task(
            &conn,
            &make_task("agy", "todo-99", "Active agy", "in_progress"),
        )
        .unwrap();

        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["99"],
        )
        .unwrap();
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "agy",
            &default_source_key("agy"),
            &["todo-99"],
        )
        .unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 2);

        // NULL session_id sorts first in SQLite ORDER BY
        let session_ids: Vec<Option<&str>> =
            archived.iter().map(|t| t.session_id.as_deref()).collect();
        assert!(session_ids.contains(&None));
        assert!(session_ids.contains(&Some("sess-123")));
    }

    #[test]
    fn archived_query_isolates_projects() {
        let (conn, _tmp) = test_db();

        let mut t1 = make_task("claude", "1", "Foo task", "completed");
        t1.project_path = "/projects/foo".to_string();
        upsert_task(&conn, &t1).unwrap();

        let mut t2 = make_task("claude", "1", "Bar task", "completed");
        t2.project_path = "/projects/bar".to_string();
        upsert_task(&conn, &t2).unwrap();

        // Archive both by adding active tasks then pruning
        upsert_task(&conn, &{
            let mut t = make_task("claude", "99", "Active foo", "in_progress");
            t.project_path = "/projects/foo".to_string();
            t
        })
        .unwrap();
        upsert_task(&conn, &{
            let mut t = make_task("claude", "99", "Active bar", "in_progress");
            t.project_path = "/projects/bar".to_string();
            t
        })
        .unwrap();

        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["99"],
        )
        .unwrap();
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/bar",
            "claude",
            &default_source_key("claude"),
            &["99"],
        )
        .unwrap();

        let foo_archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(foo_archived.len(), 1);
        assert_eq!(foo_archived[0].subject, "Foo task");

        let bar_archived = get_archived_tasks_for_project(&conn, "/projects/bar").unwrap();
        assert_eq!(bar_archived.len(), 1);
        assert_eq!(bar_archived[0].subject, "Bar task");
    }

    #[test]
    fn archived_query_spans_multiple_sources() {
        let (conn, _tmp) = test_db();

        upsert_task(&conn, &make_task("claude", "1", "Claude done", "completed")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-0", "Codex done", "completed"),
        )
        .unwrap();
        upsert_task(
            &conn,
            &make_task("agy", "todo-1", "Antigravity done", "completed"),
        )
        .unwrap();

        // Keep one active per source
        upsert_task(&conn, &make_task("claude", "99", "Active", "in_progress")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-99", "Active", "in_progress"),
        )
        .unwrap();
        upsert_task(&conn, &make_task("agy", "todo-99", "Active", "in_progress")).unwrap();

        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "claude",
            &default_source_key("claude"),
            &["99"],
        )
        .unwrap();
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "codex",
            &default_source_key("codex"),
            &["codex-99"],
        )
        .unwrap();
        archive_or_delete_stale_tasks(
            &conn,
            "/projects/foo",
            "agy",
            &default_source_key("agy"),
            &["todo-99"],
        )
        .unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 3);

        let sources: Vec<&str> = archived.iter().map(|t| t.source.as_str()).collect();
        assert!(sources.contains(&"claude"));
        assert!(sources.contains(&"codex"));
        assert!(sources.contains(&"agy"));
    }

    #[test]
    fn archived_query_plan_avoids_temp_btree_sort() {
        let (conn, _tmp) = test_db();
        let details = explain_plan_details(
            &conn,
            "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason
             FROM tasks
             WHERE project_path = ?1 AND archived_at IS NOT NULL
             ORDER BY session_id, source, source_key, source_task_id",
            "/projects/foo",
        );

        assert!(
            details.iter().all(|detail| !detail
                .to_ascii_uppercase()
                .contains("USE TEMP B-TREE FOR ORDER BY")),
            "query plan should avoid temp sort btree, got: {details:?}"
        );
    }
}
