use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{SessionDetail, SessionSummary};

/// Insert a new session into the database.
pub fn insert_session(conn: &Connection, session: &SessionDetail) -> Result<(), rusqlite::Error> {
    let next_steps_json = serde_json::to_string(&session.next_steps).unwrap_or_default();
    let open_questions_json = serde_json::to_string(&session.open_questions).unwrap_or_default();
    let metadata_json = serde_json::to_string(&session.metadata).unwrap_or_default();

    conn.execute(
        "INSERT INTO sessions (id, project_id, date, summary, next_steps, open_questions, metadata, file_path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            session.id,
            session.project_id,
            session.date,
            session.summary,
            next_steps_json,
            open_questions_json,
            metadata_json,
            session.file_path,
            session.created_at,
        ],
    )?;
    Ok(())
}

/// Retrieve a session by ID, returning full detail.
pub fn get_session(conn: &Connection, id: &str) -> Result<Option<SessionDetail>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, project_id, date, summary, next_steps, open_questions, metadata, file_path, created_at
         FROM sessions WHERE id = ?1",
        [id],
        row_to_session_detail,
    )
    .optional()
}

/// Retrieve the most recent session for a project.
pub fn get_latest_session(
    conn: &Connection,
    project_id: &str,
) -> Result<Option<SessionDetail>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, project_id, date, summary, next_steps, open_questions, metadata, file_path, created_at
         FROM sessions WHERE project_id = ?1
         ORDER BY date DESC LIMIT 1",
        [project_id],
        row_to_session_detail,
    )
    .optional()
}

/// List sessions for a project with pagination, ordered by date DESC.
pub fn list_sessions(
    conn: &Connection,
    project_id: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<SessionSummary>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, date, summary
         FROM sessions WHERE project_id = ?1
         ORDER BY date DESC
         LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(params![project_id, limit, offset], |row| {
        Ok(SessionSummary {
            id: row.get(0)?,
            project_id: row.get(1)?,
            date: row.get(2)?,
            summary: row.get(3)?,
        })
    })?;

    rows.collect()
}

/// Check if a session with the given file_path already exists (dedup on import).
pub fn session_exists_by_file_path(
    conn: &Connection,
    file_path: &str,
) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE file_path = ?1)",
        [file_path],
        |row| row.get(0),
    )
}

/// Map a row to SessionDetail, deserializing JSON arrays.
fn row_to_session_detail(row: &rusqlite::Row<'_>) -> Result<SessionDetail, rusqlite::Error> {
    let next_steps_raw: Option<String> = row.get(4)?;
    let open_questions_raw: Option<String> = row.get(5)?;
    let metadata_raw: Option<String> = row.get(6)?;

    let next_steps: Vec<String> = next_steps_raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let open_questions: Vec<String> = open_questions_raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let metadata: serde_json::Value = metadata_raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    Ok(SessionDetail {
        id: row.get(0)?,
        project_id: row.get(1)?,
        date: row.get(2)?,
        summary: row.get(3)?,
        next_steps,
        open_questions,
        metadata,
        file_path: row.get(7)?,
        created_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::db::queries::insert_project;
    use crate::models::Project;
    use pretty_assertions::assert_eq;
    use tempfile::NamedTempFile;

    fn test_db() -> (Connection, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (conn, tmp)
    }

    fn seed_project(conn: &Connection, id: &str) {
        let project = Project {
            id: id.to_string(),
            name: "test-project".to_string(),
            path: format!("/projects/{id}"),
            description: None,
            last_activity_at: Some("2026-01-01T00:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
        };
        insert_project(conn, &project).unwrap();
    }

    fn make_session(id: &str, project_id: &str, date: &str) -> SessionDetail {
        SessionDetail {
            id: id.to_string(),
            project_id: project_id.to_string(),
            date: date.to_string(),
            summary: format!("Session {id}"),
            next_steps: vec!["step 1".to_string(), "step 2".to_string()],
            open_questions: vec!["question 1".to_string()],
            metadata: serde_json::json!({"branch": "main"}),
            file_path: format!("/sessions/{id}.md"),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // AC1: insert_session stores all fields correctly
    #[test]
    fn insert_and_get_session() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        let session = make_session("s1", "p1", "2026-02-17");
        insert_session(&conn, &session).unwrap();

        let fetched = get_session(&conn, "s1")
            .unwrap()
            .expect("session should exist");
        assert_eq!(fetched.id, "s1");
        assert_eq!(fetched.project_id, "p1");
        assert_eq!(fetched.date, "2026-02-17");
        assert_eq!(fetched.summary, "Session s1");
        assert_eq!(fetched.next_steps, vec!["step 1", "step 2"]);
        assert_eq!(fetched.open_questions, vec!["question 1"]);
        assert_eq!(fetched.metadata["branch"], "main");
        assert_eq!(fetched.file_path, "/sessions/s1.md");
    }

    // AC2: get_session returns full SessionDetail with deserialized arrays
    #[test]
    fn get_session_deserializes_json_arrays() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        let session = SessionDetail {
            id: "s2".to_string(),
            project_id: "p1".to_string(),
            date: "2026-02-17".to_string(),
            summary: "Test".to_string(),
            next_steps: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            open_questions: vec![],
            metadata: serde_json::Value::Null,
            file_path: "/s2.md".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        insert_session(&conn, &session).unwrap();

        let fetched = get_session(&conn, "s2").unwrap().unwrap();
        assert_eq!(fetched.next_steps.len(), 3);
        assert!(fetched.open_questions.is_empty());
    }

    // AC3: get_latest_session returns most recent by date
    #[test]
    fn get_latest_session_returns_newest() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        insert_session(&conn, &make_session("s1", "p1", "2026-02-10")).unwrap();
        insert_session(&conn, &make_session("s2", "p1", "2026-02-17")).unwrap();
        insert_session(&conn, &make_session("s3", "p1", "2026-02-14")).unwrap();

        let latest = get_latest_session(&conn, "p1").unwrap().unwrap();
        assert_eq!(latest.id, "s2");
        assert_eq!(latest.date, "2026-02-17");
    }

    // AC3b: get_latest_session returns None when no sessions
    #[test]
    fn get_latest_session_none_when_empty() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        let latest = get_latest_session(&conn, "p1").unwrap();
        assert!(latest.is_none());
    }

    // AC4: list_sessions respects limit/offset
    #[test]
    fn list_sessions_with_pagination() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        for i in 1..=5 {
            insert_session(
                &conn,
                &make_session(&format!("s{i}"), "p1", &format!("2026-02-{i:02}")),
            )
            .unwrap();
        }

        // Newest first
        let page1 = list_sessions(&conn, "p1", 2, 0).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].date, "2026-02-05");
        assert_eq!(page1[1].date, "2026-02-04");

        let page2 = list_sessions(&conn, "p1", 2, 2).unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].date, "2026-02-03");

        let page3 = list_sessions(&conn, "p1", 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    // AC5: session_exists_by_file_path prevents duplicate imports
    #[test]
    fn session_dedup_by_file_path() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        assert!(!session_exists_by_file_path(&conn, "/sessions/s1.md").unwrap());

        insert_session(&conn, &make_session("s1", "p1", "2026-02-17")).unwrap();

        assert!(session_exists_by_file_path(&conn, "/sessions/s1.md").unwrap());
        assert!(!session_exists_by_file_path(&conn, "/sessions/other.md").unwrap());
    }

    // AC6: Sessions cascade-delete when project is removed
    #[test]
    fn sessions_cascade_on_project_delete() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");

        insert_session(&conn, &make_session("s1", "p1", "2026-02-17")).unwrap();
        insert_session(&conn, &make_session("s2", "p1", "2026-02-18")).unwrap();

        // Verify sessions exist
        let sessions = list_sessions(&conn, "p1", 10, 0).unwrap();
        assert_eq!(sessions.len(), 2);

        // Delete the project
        crate::db::queries::delete_project(&conn, "p1").unwrap();

        // Sessions should be gone
        let sessions = list_sessions(&conn, "p1", 10, 0).unwrap();
        assert!(sessions.is_empty());
    }

    // list_sessions only returns sessions for the specified project
    #[test]
    fn list_sessions_scoped_to_project() {
        let (conn, _tmp) = test_db();
        seed_project(&conn, "p1");
        seed_project(&conn, "p2");

        insert_session(&conn, &make_session("s1", "p1", "2026-02-17")).unwrap();
        insert_session(&conn, &make_session("s2", "p2", "2026-02-17")).unwrap();

        let p1_sessions = list_sessions(&conn, "p1", 10, 0).unwrap();
        assert_eq!(p1_sessions.len(), 1);
        assert_eq!(p1_sessions[0].id, "s1");
    }

    // get_session for nonexistent ID
    #[test]
    fn get_nonexistent_session() {
        let (conn, _tmp) = test_db();
        let result = get_session(&conn, "no-such").unwrap();
        assert!(result.is_none());
    }
}
