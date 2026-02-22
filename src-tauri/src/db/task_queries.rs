use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A persisted task row, matching the `tasks` table schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedTask {
    pub project_path: String,
    pub source: String,
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
    pub updated_at: String,
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
        "INSERT INTO tasks (project_path, source, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT (project_path, source, source_task_id) DO UPDATE SET
            subject = excluded.subject,
            description = excluded.description,
            active_form = excluded.active_form,
            status = excluded.status,
            blocks = excluded.blocks,
            blocked_by = excluded.blocked_by,
            owner = excluded.owner,
            session_id = excluded.session_id,
            updated_at = excluded.updated_at",
        params![
            task.project_path,
            task.source,
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
            task.updated_at,
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

/// Get all tasks for a project, ordered by source then source_task_id.
pub fn get_tasks_for_project(
    conn: &Connection,
    project_path: &str,
) -> Result<Vec<PersistedTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT project_path, source, source_task_id, subject, description, active_form, status, blocks, blocked_by, owner, session_id, first_seen_at, updated_at
         FROM tasks
         WHERE project_path = ?1
         ORDER BY source, source_task_id",
    )?;

    let tasks = stmt
        .query_map([project_path], |row| {
            let blocks_str: String = row.get(7)?;
            let blocked_by_str: String = row.get(8)?;
            Ok(PersistedTask {
                project_path: row.get(0)?,
                source: row.get(1)?,
                source_task_id: row.get(2)?,
                subject: row.get(3)?,
                description: row.get(4)?,
                active_form: row.get(5)?,
                status: row.get(6)?,
                blocks: serde_json::from_str(&blocks_str).unwrap_or_default(),
                blocked_by: serde_json::from_str(&blocked_by_str).unwrap_or_default(),
                owner: row.get(9)?,
                session_id: row.get(10)?,
                first_seen_at: row.get(11)?,
                updated_at: row.get(12)?,
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
            updated_at: "2026-02-22T10:00:00Z".to_string(),
        }
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
        upsert_task(&conn, &make_task("codex", "codex-0", "Codex task", "in_progress")).unwrap();
        upsert_task(&conn, &make_task("gemini", "todo-1", "Gemini task", "completed")).unwrap();

        let tasks = get_tasks_for_project(&conn, "/projects/foo").unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].source, "claude");
        assert_eq!(tasks[1].source, "codex");
        assert_eq!(tasks[2].source, "gemini");
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
        upsert_task(&conn, &make_task("codex", "codex-0", "Codex task", "pending")).unwrap();

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
        assert_eq!(tasks[0].description.as_deref(), Some("A detailed description"));
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
}
