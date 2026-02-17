use std::sync::Mutex;

use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

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
pub fn scan_directory(path: String) -> Result<Vec<DiscoveredProject>, String> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }

    let mut discovered = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let has_git = entry_path.join(".git").is_dir();
            let name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip hidden directories
            if name.starts_with('.') {
                continue;
            }

            discovered.push(DiscoveredProject {
                path: entry_path.to_string_lossy().to_string(),
                name,
                has_git,
            });
        }
    }

    discovered.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(discovered)
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
}
