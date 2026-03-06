use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::relationship_queries;
use crate::errors::SanitizeErr;
use crate::models::Relationship;

#[tauri::command]
pub fn get_relationships(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<Relationship>, String> {
    get_relationships_with_span(db.inner(), project_id)
}

fn get_relationships_with_span(
    db: &DbState,
    project_id: String,
) -> Result<Vec<Relationship>, String> {
    let span = IpcCommandSpan::start("get_relationships");
    let result = get_relationships_impl(db, project_id);
    span.finish_result(&result);
    result
}

fn get_relationships_impl(db: &DbState, project_id: String) -> Result<Vec<Relationship>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::list_relationships(&conn, &project_id).sanitize_err()
}

#[tauri::command]
pub fn dismiss_relationship(db: State<'_, DbState>, relationship_id: String) -> Result<(), String> {
    dismiss_relationship_with_span(db.inner(), relationship_id)
}

fn dismiss_relationship_with_span(db: &DbState, relationship_id: String) -> Result<(), String> {
    let span = IpcCommandSpan::start("dismiss_relationship");
    let result = dismiss_relationship_impl(db, relationship_id);
    span.finish_result(&result);
    result
}

fn dismiss_relationship_impl(db: &DbState, relationship_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::dismiss_relationship(&conn, &relationship_id).sanitize_err()?;
    Ok(())
}

#[tauri::command]
pub fn create_relationship(
    db: State<'_, DbState>,
    source_id: String,
    target_id: String,
    relationship_type: String,
) -> Result<Relationship, String> {
    create_relationship_with_span(db.inner(), source_id, target_id, relationship_type)
}

fn create_relationship_with_span(
    db: &DbState,
    source_id: String,
    target_id: String,
    relationship_type: String,
) -> Result<Relationship, String> {
    let span = IpcCommandSpan::start("create_relationship");
    let result = create_relationship_impl(db, source_id, target_id, relationship_type);
    span.finish_result(&result);
    result
}

fn create_relationship_impl(
    db: &DbState,
    source_id: String,
    target_id: String,
    relationship_type: String,
) -> Result<Relationship, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let rel = Relationship {
        id: uuid::Uuid::new_v4().to_string(),
        source_project_id: source_id,
        target_project_id: target_id,
        relationship_type,
        detection_source: "manual".to_string(),
        dismissed: false,
        first_detected_at: now.clone(),
        last_seen_at: now,
    };

    relationship_queries::insert_relationship(&conn, &rel).sanitize_err()?;
    Ok(rel)
}

#[tauri::command]
pub fn remove_relationship(db: State<'_, DbState>, relationship_id: String) -> Result<(), String> {
    remove_relationship_with_span(db.inner(), relationship_id)
}

fn remove_relationship_with_span(db: &DbState, relationship_id: String) -> Result<(), String> {
    let span = IpcCommandSpan::start("remove_relationship");
    let result = remove_relationship_impl(db, relationship_id);
    span.finish_result(&result);
    result
}

fn remove_relationship_impl(db: &DbState, relationship_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::remove_relationship(&conn, &relationship_id).sanitize_err()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use serde_json::Value;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");
        (DbState(Mutex::new(conn)), tmp)
    }

    fn insert_project(db: &DbState, id: &str, path: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let project = crate::models::Project {
            id: id.to_string(),
            name: format!("project-{id}"),
            path: path.to_string(),
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

    #[test]
    fn relationship_commands_crud_round_trip() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "p1", "/tmp/project-1");
        insert_project(&db, "p2", "/tmp/project-2");

        let created = create_relationship_impl(
            &db,
            "p1".to_string(),
            "p2".to_string(),
            "depends_on".to_string(),
        )
        .expect("create relationship");

        let listed = get_relationships_impl(&db, "p1".to_string()).expect("list relationships");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].relationship_type, "depends_on");

        dismiss_relationship_impl(&db, created.id.clone()).expect("dismiss relationship");
        let listed_after_dismiss = get_relationships_impl(&db, "p1".to_string())
            .expect("list relationships after dismiss");
        assert!(listed_after_dismiss.is_empty());

        remove_relationship_impl(&db, created.id.clone()).expect("remove relationship");
        remove_relationship_impl(&db, created.id).expect("remove missing relationship is ok");
    }

    #[test]
    fn create_relationship_reports_invalid_project_error() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "p1", "/tmp/project-1");

        let err = create_relationship_impl(
            &db,
            "p1".to_string(),
            "missing-project".to_string(),
            "depends_on".to_string(),
        )
        .expect_err("invalid project should fail");

        assert!(
            err.to_lowercase().contains("foreign key"),
            "expected foreign-key error, got: {err}"
        );
    }

    #[test]
    fn get_relationships_reports_db_lock_failure() {
        let db = DbState(Mutex::new(
            rusqlite::Connection::open_in_memory().expect("open memory db"),
        ));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = db.0.lock().expect("lock");
            panic!("poison lock");
        }));

        let err =
            get_relationships_impl(&db, "p1".to_string()).expect_err("poisoned lock should fail");
        assert!(err.to_lowercase().contains("poison"));
    }

    fn wait_for_lines(path: &std::path::Path, expected: usize) -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.to_string())
                    .collect();
                if lines.len() >= expected {
                    return lines;
                }
            }
            if std::time::Instant::now() >= deadline {
                panic!("timed out waiting for log lines in {}", path.display());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn get_relationships_emits_lifecycle_events() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "p1", "/tmp/project-1");

        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("relationships-lifecycle.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let _ = get_relationships_with_span(&db, "p1".to_string()).expect("get relationships");

        let lines = wait_for_lines(&log_path, 2);
        let received: Value = serde_json::from_str(&lines[0]).expect("received json");
        let completed: Value = serde_json::from_str(&lines[1]).expect("completed json");

        assert_eq!(received["event"], "ipc.command.received");
        assert_eq!(received["command"], "get_relationships");
        assert_eq!(completed["event"], "ipc.command.completed");
        assert_eq!(completed["command"], "get_relationships");
        assert_eq!(completed["status"], "ok");
    }
}
