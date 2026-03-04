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
            last_status, archived_reason
        )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, NULL, ?16, NULL)
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
            updated_at = excluded.updated_at,
            archived_at = NULL,
            last_status = excluded.status,
            archived_reason = NULL",
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
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason
         FROM tasks
         WHERE project_path = ?1 AND archived_at IS NULL
         ORDER BY source, source_key, source_task_id",
    )?;

    let tasks = stmt
        .query_map([project_path], |row| {
            let blocks_str: String = row.get(8)?;
            let blocked_by_str: String = row.get(9)?;
            Ok(PersistedTask {
                project_path: row.get(0)?,
                source: row.get(1)?,
                source_key: row.get(2)?,
                source_task_id: row.get(3)?,
                subject: row.get(4)?,
                description: row.get(5)?,
                active_form: row.get(6)?,
                status: row.get(7)?,
                blocks: serde_json::from_str(&blocks_str).unwrap_or_default(),
                blocked_by: serde_json::from_str(&blocked_by_str).unwrap_or_default(),
                owner: row.get(10)?,
                session_id: row.get(11)?,
                first_seen_at: row.get(12)?,
                state_changed_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
                last_status: row.get(16)?,
                archived_reason: row.get(17)?,
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
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason
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
    Ok(Some(PersistedTask {
        project_path: row.get(0)?,
        source: row.get(1)?,
        source_key: row.get(2)?,
        source_task_id: row.get(3)?,
        subject: row.get(4)?,
        description: row.get(5)?,
        active_form: row.get(6)?,
        status: row.get(7)?,
        blocks: serde_json::from_str(&blocks_str).unwrap_or_default(),
        blocked_by: serde_json::from_str(&blocked_by_str).unwrap_or_default(),
        owner: row.get(10)?,
        session_id: row.get(11)?,
        first_seen_at: row.get(12)?,
        state_changed_at: row.get(13)?,
        updated_at: row.get(14)?,
        archived_at: row.get(15)?,
        last_status: row.get(16)?,
        archived_reason: row.get(17)?,
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
        "SELECT project_path, source, source_key, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, state_changed_at, updated_at, archived_at, last_status, archived_reason
         FROM tasks
         WHERE project_path = ?1 AND archived_at IS NOT NULL
         ORDER BY session_id, source, source_key, source_task_id",
    )?;

    let tasks = stmt
        .query_map([project_path], |row| {
            let blocks_str: String = row.get(8)?;
            let blocked_by_str: String = row.get(9)?;
            Ok(PersistedTask {
                project_path: row.get(0)?,
                source: row.get(1)?,
                source_key: row.get(2)?,
                source_task_id: row.get(3)?,
                subject: row.get(4)?,
                description: row.get(5)?,
                active_form: row.get(6)?,
                status: row.get(7)?,
                blocks: serde_json::from_str(&blocks_str).unwrap_or_default(),
                blocked_by: serde_json::from_str(&blocked_by_str).unwrap_or_default(),
                owner: row.get(10)?,
                session_id: row.get(11)?,
                first_seen_at: row.get(12)?,
                state_changed_at: row.get(13)?,
                updated_at: row.get(14)?,
                archived_at: row.get(15)?,
                last_status: row.get(16)?,
                archived_reason: row.get(17)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
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
        }
    }

    fn default_source_key(source: &str) -> String {
        format!("{source}-default")
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
            &make_task("gemini", "todo-1", "Gemini task", "completed"),
        )
        .unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].source, "claude");
        assert_eq!(tasks[1].source, "codex");
        assert_eq!(tasks[2].source, "gemini");
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
        let result =
            archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["1"]).unwrap();
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
        let result =
            archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["1"]).unwrap();
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
        let result =
            archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["1"]).unwrap();
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

        let result =
            archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["1"]).unwrap();
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
        let result =
            archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &empty).unwrap();
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
        assert_eq!(tasks[0].state_changed_at.as_deref(), Some("2026-02-22T10:00:00Z"));

        // Status transition; state_changed_at should update to current updated_at.
        task.status = "completed".to_string();
        task.updated_at = "2026-02-22T10:10:00Z".to_string();
        upsert_task(&conn, &task).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks[0].state_changed_at.as_deref(), Some("2026-02-22T10:10:00Z"));
        assert_eq!(tasks[0].last_status.as_deref(), Some("completed"));
    }

    #[test]
    fn upsert_preserves_archived_row_and_inserts_new_active_row() {
        let (conn, _tmp) = test_db();

        // Create and archive a completed task
        upsert_task(&conn, &make_task("claude", "1", "Done task", "completed")).unwrap();
        upsert_task(&conn, &make_task("claude", "2", "Active", "in_progress")).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["2"]).unwrap();

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
        let r1 = archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["2"]).unwrap();
        assert_eq!(r1.archived, 1);

        // Second call — task 1 is already archived, should not re-archive
        let r2 = archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["2"]).unwrap();
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
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["1"]).unwrap();

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

        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["99"]).unwrap();

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

        // Task with no session_id (e.g., Gemini/Codex source)
        upsert_task(
            &conn,
            &make_task("gemini", "todo-1", "Gemini task", "completed"),
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
            &make_task("gemini", "todo-99", "Active gemini", "in_progress"),
        )
        .unwrap();

        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["99"]).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "gemini", &default_source_key("gemini"), &["todo-99"]).unwrap();

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

        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["99"]).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/bar", "claude", &default_source_key("claude"), &["99"]).unwrap();

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
            &make_task("gemini", "todo-1", "Gemini done", "completed"),
        )
        .unwrap();

        // Keep one active per source
        upsert_task(&conn, &make_task("claude", "99", "Active", "in_progress")).unwrap();
        upsert_task(
            &conn,
            &make_task("codex", "codex-99", "Active", "in_progress"),
        )
        .unwrap();
        upsert_task(
            &conn,
            &make_task("gemini", "todo-99", "Active", "in_progress"),
        )
        .unwrap();

        archive_or_delete_stale_tasks(&conn, "/projects/foo", "claude", &default_source_key("claude"), &["99"]).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "codex", &default_source_key("codex"), &["codex-99"]).unwrap();
        archive_or_delete_stale_tasks(&conn, "/projects/foo", "gemini", &default_source_key("gemini"), &["todo-99"]).unwrap();

        let archived = get_archived_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(archived.len(), 3);

        let sources: Vec<&str> = archived.iter().map(|t| t.source.as_str()).collect();
        assert!(sources.contains(&"claude"));
        assert!(sources.contains(&"codex"));
        assert!(sources.contains(&"gemini"));
    }
}
