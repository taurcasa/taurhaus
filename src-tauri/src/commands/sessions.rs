use tauri::State;

use crate::commands::projects::DbState;
use crate::db::session_queries;
use crate::errors::SanitizeErr;
use crate::models::{SessionDetail, SessionSummary};

#[tauri::command]
pub fn get_latest_session(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Option<SessionDetail>, String> {
    get_latest_session_impl(db.inner(), project_id)
}

fn get_latest_session_impl(
    db: &DbState,
    project_id: String,
) -> Result<Option<SessionDetail>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::get_latest_session(&conn, &project_id).sanitize_err()
}

#[tauri::command]
pub fn list_sessions(
    db: State<'_, DbState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SessionSummary>, String> {
    list_sessions_impl(db.inner(), project_id, limit, offset)
}

fn list_sessions_impl(
    db: &DbState,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SessionSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::list_sessions(
        &conn,
        &project_id,
        limit.unwrap_or(20).min(100),
        offset.unwrap_or(0),
    )
    .sanitize_err()
}

#[tauri::command]
pub fn get_session(db: State<'_, DbState>, session_id: String) -> Result<SessionDetail, String> {
    get_session_impl(db.inner(), session_id)
}

fn get_session_impl(db: &DbState, session_id: String) -> Result<SessionDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::get_session(&conn, &session_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Session not found: {session_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");
        (DbState(Mutex::new(conn)), tmp)
    }

    fn insert_project(db: &DbState, id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let project = crate::models::Project {
            id: id.to_string(),
            name: format!("project-{id}"),
            path: format!("/tmp/{id}"),
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

    fn insert_session(db: &DbState, id: &str, project_id: &str, date: &str) {
        let session = SessionDetail {
            id: id.to_string(),
            project_id: project_id.to_string(),
            date: date.to_string(),
            summary: format!("Session {id}"),
            next_steps: vec!["next".to_string()],
            open_questions: vec!["question".to_string()],
            metadata: serde_json::json!({"branch": "main"}),
            file_path: format!("/sessions/{id}.md"),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let conn = db.0.lock().expect("db lock");
        crate::db::session_queries::insert_session(&conn, &session).expect("insert session");
    }

    #[test]
    fn session_commands_list_get_and_latest_round_trip() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "p1");
        insert_session(&db, "s1", "p1", "2026-02-01");
        insert_session(&db, "s2", "p1", "2026-02-02");

        let listed =
            list_sessions_impl(&db, "p1".to_string(), Some(20), Some(0)).expect("list sessions");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, "s2");

        let latest = get_latest_session_impl(&db, "p1".to_string())
            .expect("latest session")
            .expect("latest should exist");
        assert_eq!(latest.id, "s2");

        let session = get_session_impl(&db, "s1".to_string()).expect("get session");
        assert_eq!(session.id, "s1");
        assert_eq!(session.project_id, "p1");
    }

    #[test]
    fn get_session_returns_not_found_error() {
        let (db, _tmp) = test_db_state();
        let err =
            get_session_impl(&db, "missing-session".to_string()).expect_err("missing session");
        assert_eq!(err, "Session not found: missing-session");
    }

    #[test]
    fn session_commands_report_db_lock_failure() {
        let db = DbState(Mutex::new(
            rusqlite::Connection::open_in_memory().expect("open memory db"),
        ));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = db.0.lock().expect("lock");
            panic!("poison lock");
        }));

        let err =
            get_latest_session_impl(&db, "p1".to_string()).expect_err("poisoned lock should fail");
        assert!(err.to_lowercase().contains("poison"));
    }
}
