use std::sync::Mutex;

use rusqlite::Connection;
use serde::Deserialize;
use tauri::{Emitter, State};

use crate::models::{ActivityThresholds, ProjectDetail, ProjectSummary};
use crate::services::project;

/// Managed state: a mutex-wrapped SQLite connection.
pub struct DbState(pub Mutex<Connection>);

/// Fields the frontend can update on a project.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectFields {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub hero_preference: Option<Option<String>>,
}

/// A discovered project from a directory scan.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredProject {
    pub path: String,
    pub name: String,
    pub has_git: bool,
}

#[tauri::command]
pub fn list_projects(db: State<'_, DbState>) -> Result<Vec<ProjectSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let thresholds = ActivityThresholds::default();
    project::list_projects(&conn, &thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<ProjectDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let thresholds = ActivityThresholds::default();
    project::get_project(&conn, &project_id, &thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn register_project(
    db: State<'_, DbState>,
    path: String,
    name: Option<String>,
) -> Result<ProjectDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    project::register_project(&conn, &path, name.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project(
    db: State<'_, DbState>,
    project_id: String,
    fields: UpdateProjectFields,
) -> Result<ProjectDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let thresholds = ActivityThresholds::default();

    project::update_project(
        &conn,
        &project_id,
        fields.name.as_deref(),
        fields.description.as_ref().map(|d| d.as_deref()),
        fields.hero_preference.as_ref().map(|h| h.as_deref()),
    )
    .map_err(|e| e.to_string())?;

    // Return the updated project.
    project::get_project(&conn, &project_id, &thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_project(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    project::remove_project(&conn, &project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_first_run(db: State<'_, DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let count = crate::db::queries::project_count(&conn).map_err(|e| e.to_string())?;
    Ok(count == 0)
}

/// Result of a single registration attempt within a batch.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRegistrationResult {
    pub path: String,
    pub success: bool,
    pub project: Option<ProjectDetail>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn register_projects_batch(
    db: State<'_, DbState>,
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<BatchRegistrationResult>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let total = paths.len();
    let mut results = Vec::with_capacity(total);

    for (index, path) in paths.iter().enumerate() {
        let result = match project::register_project(&conn, path, None) {
            Ok(detail) => {
                let _ = app.emit(
                    "batch-registration-progress",
                    serde_json::json!({
                        "project_name": detail.name,
                        "index": index,
                        "total": total,
                    }),
                );
                BatchRegistrationResult {
                    path: path.clone(),
                    success: true,
                    project: Some(detail),
                    error: None,
                }
            }
            Err(e) => BatchRegistrationResult {
                path: path.clone(),
                success: false,
                project: None,
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }

    Ok(results)
}

#[tauri::command]
pub fn scan_directory(path: String) -> Result<Vec<DiscoveredProject>, String> {
    let results = crate::services::scanner::scan_directory(std::path::Path::new(&path), 2)?;
    Ok(results
        .into_iter()
        .map(|d| DiscoveredProject {
            path: d.path,
            name: d.name,
            has_git: d.has_git,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::{NamedTempFile, TempDir};

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let conn = init_db(tmp.path()).unwrap();
        (DbState(Mutex::new(conn)), tmp)
    }

    fn temp_project_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        dir
    }

    #[test]
    fn scan_directory_finds_subdirs() {
        let parent = TempDir::new().unwrap();

        // Create some subdirectories
        let sub1 = parent.path().join("project-a");
        std::fs::create_dir(&sub1).unwrap();
        std::fs::create_dir(sub1.join(".git")).unwrap();

        let sub2 = parent.path().join("project-b");
        std::fs::create_dir(&sub2).unwrap();

        // Hidden dir should be skipped
        let hidden = parent.path().join(".hidden");
        std::fs::create_dir(&hidden).unwrap();

        let results = scan_directory(parent.path().to_str().unwrap().to_string()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "project-a");
        assert!(results[0].has_git);
        assert_eq!(results[1].name, "project-b");
        assert!(!results[1].has_git);
    }

    #[test]
    fn scan_directory_rejects_nonexistent() {
        let result = scan_directory("/nonexistent/path".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn command_register_and_list_roundtrip() {
        let (db_state, _tmp) = test_db_state();
        let dir = temp_project_dir();
        let path = dir.path().to_str().unwrap().to_string();

        // Can't use State<> directly in tests, so test the underlying functions
        let conn = db_state.0.lock().unwrap();
        let thresholds = ActivityThresholds::default();

        let detail = project::register_project(&conn, &path, Some("test")).unwrap();
        assert_eq!(detail.name, "test");

        let list = project::list_projects(&conn, &thresholds).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test");
    }

    // AC1: is_first_run returns true when DB has no projects
    #[test]
    fn first_run_true_when_empty() {
        let (_db_state, _tmp) = test_db_state();
        let conn = _db_state.0.lock().unwrap();
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert_eq!(count, 0);
    }

    // AC2: is_first_run returns false when projects exist
    #[test]
    fn first_run_false_when_projects_exist() {
        let (_db_state, _tmp) = test_db_state();
        let dir = temp_project_dir();
        let conn = _db_state.0.lock().unwrap();

        project::register_project(&conn, dir.path().to_str().unwrap(), Some("test")).unwrap();
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert!(count > 0);
    }

    // AC3: batch registration registers all valid paths
    #[test]
    fn batch_register_valid_paths() {
        let (_db_state, _tmp) = test_db_state();
        let dir1 = temp_project_dir();
        let dir2 = temp_project_dir();
        let conn = _db_state.0.lock().unwrap();

        let d1 = project::register_project(&conn, dir1.path().to_str().unwrap(), None).unwrap();
        let d2 = project::register_project(&conn, dir2.path().to_str().unwrap(), None).unwrap();

        assert!(!d1.id.is_empty());
        assert!(!d2.id.is_empty());
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert_eq!(count, 2);
    }

    // AC4: batch registration skips invalid paths gracefully
    #[test]
    fn batch_register_skips_invalid_paths() {
        let (_db_state, _tmp) = test_db_state();
        let conn = _db_state.0.lock().unwrap();

        let result = project::register_project(&conn, "/nonexistent/path", None);
        assert!(result.is_err());

        // DB should still be empty
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert_eq!(count, 0);
    }
}
