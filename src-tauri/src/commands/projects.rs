use std::sync::Mutex;

use rusqlite::Connection;
use serde::Deserialize;
use tauri::{Emitter, State};

use crate::db::{queries, settings_queries};
use crate::models::{ProjectDetail, ProjectSummary};
use crate::services::project;
use crate::SearchState;

/// Expand `~` or `~/` at the start of a path to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

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
    let settings = settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())?;
    project::list_projects(&conn, &settings.thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<ProjectDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())?;
    project::get_project(&conn, &project_id, &settings.thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn register_project(
    db: State<'_, DbState>,
    path: String,
    name: Option<String>,
) -> Result<ProjectDetail, String> {
    let expanded = expand_tilde(&path);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())?;
    project::register_project(&conn, &expanded, name.as_deref(), &settings.thresholds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_project(
    db: State<'_, DbState>,
    project_id: String,
    fields: UpdateProjectFields,
) -> Result<ProjectDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())?;
    let thresholds = settings.thresholds;

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
    search: State<'_, SearchState>,
    project_id: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    project::remove_project(&conn, &project_id).map_err(|e| e.to_string())?;

    // Clean up search index entries for this project
    if let Ok(mut index) = search.0.lock() {
        index.remove_by_project(&project_id);
        let _ = index.commit();
    }
    Ok(())
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
    let settings = settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())?;
    let total = paths.len();
    let mut results = Vec::with_capacity(total);

    for (index, path) in paths.iter().enumerate() {
        let expanded = expand_tilde(path);
        let result = match project::register_project(&conn, &expanded, None, &settings.thresholds) {
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
    let expanded = expand_tilde(&path);
    let results = crate::services::scanner::scan_directory(std::path::Path::new(&expanded), 2)?;
    Ok(results
        .into_iter()
        .map(|d| DiscoveredProject {
            path: d.path,
            name: d.name,
            has_git: d.has_git,
        })
        .collect())
}

/// A directory entry returned by list_directory.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_expandable: bool,
}

/// List subdirectories at a given path (directories only, no files).
/// Used by the directory tree browser for manual path selection.
#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirectoryEntry>, String> {
    let expanded = expand_tilde(&path);
    let dir = std::path::Path::new(&expanded);

    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let read_dir = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut entries: Vec<DirectoryEntry> = Vec::new();

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden directories
        if name.starts_with('.') {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if !file_type.is_dir() {
            continue;
        }

        let full_path = entry.path().to_string_lossy().to_string();

        // Check if this directory has subdirectories (for expand chevron)
        let is_expandable = std::fs::read_dir(entry.path())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| {
                        e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                            && !e.file_name().to_string_lossy().starts_with('.')
                    })
            })
            .unwrap_or(false);

        entries.push(DirectoryEntry {
            name,
            path: full_path,
            is_expandable,
        });
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

/// Return filesystem root entries for the directory tree browser.
/// On Windows: available drive letters (C:\, D:\, etc.) + WSL distributions
/// On Linux/macOS: just ["/"]
#[tauri::command]
pub fn get_system_roots() -> Vec<DirectoryEntry> {
    #[cfg(target_os = "windows")]
    {
        let mut roots = Vec::new();

        // Check drives A-Z for existence
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = std::path::Path::new(&drive);
            if path.exists() {
                let is_expandable = std::fs::read_dir(path)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .any(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    })
                    .unwrap_or(false);
                roots.push(DirectoryEntry {
                    name: drive.clone(),
                    path: drive,
                    is_expandable,
                });
            }
        }

        // Discover WSL distributions via `wsl --list --quiet`.
        // The \\wsl$\ UNC root can't be listed with read_dir, but individual
        // distro paths like \\wsl$\Ubuntu\ work fine.
        if let Ok(output) = std::process::Command::new("wsl")
            .args(["--list", "--quiet"])
            .output()
        {
            // wsl.exe outputs UTF-16LE; decode and parse distro names
            let text = String::from_utf16_lossy(
                &output
                    .stdout
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            );
            for line in text.lines() {
                let distro = line.trim();
                if distro.is_empty() {
                    continue;
                }
                let wsl_path = format!("\\\\wsl$\\{}", distro);
                if std::path::Path::new(&wsl_path).is_dir() {
                    roots.push(DirectoryEntry {
                        name: format!("WSL: {}", distro),
                        path: wsl_path,
                        is_expandable: true,
                    });
                }
            }
        }

        roots
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![DirectoryEntry {
            name: "/".to_string(),
            path: "/".to_string(),
            is_expandable: true,
        }]
    }
}

/// Result of validating a project path.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathValidation {
    pub exists: bool,
    pub is_git_repo: bool,
    pub is_registered: bool,
}

/// Validate whether a path is a valid project directory.
/// Checks: exists, is a git repo, already registered.
#[tauri::command]
pub fn validate_project_path(
    db: State<'_, DbState>,
    path: String,
) -> Result<PathValidation, String> {
    let expanded = expand_tilde(&path);
    let dir = std::path::Path::new(&expanded);

    let exists = dir.is_dir();
    if !exists {
        return Ok(PathValidation {
            exists: false,
            is_git_repo: false,
            is_registered: false,
        });
    }

    let is_git_repo = git2::Repository::open(dir).is_ok();

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let is_registered =
        queries::project_exists_at_path(&conn, &expanded).map_err(|e| e.to_string())?;

    Ok(PathValidation {
        exists,
        is_git_repo,
        is_registered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::models::ActivityThresholds;
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

        let detail = project::register_project(&conn, &path, Some("test"), &thresholds).unwrap();
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

        let thresholds = ActivityThresholds::default();
        project::register_project(&conn, dir.path().to_str().unwrap(), Some("test"), &thresholds).unwrap();
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

        let thresholds = ActivityThresholds::default();
        let d1 = project::register_project(&conn, dir1.path().to_str().unwrap(), None, &thresholds).unwrap();
        let d2 = project::register_project(&conn, dir2.path().to_str().unwrap(), None, &thresholds).unwrap();

        assert!(!d1.id.is_empty());
        assert!(!d2.id.is_empty());
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert_eq!(count, 2);
    }

    // list_directory returns only directories, sorted alphabetically
    #[test]
    fn list_directory_returns_dirs_only() {
        let parent = TempDir::new().unwrap();

        // Create directories
        std::fs::create_dir(parent.path().join("alpha")).unwrap();
        std::fs::create_dir(parent.path().join("beta")).unwrap();
        // Create a file — should NOT appear
        std::fs::write(parent.path().join("readme.txt"), "hello").unwrap();
        // Create hidden dir — should NOT appear
        std::fs::create_dir(parent.path().join(".hidden")).unwrap();

        let results = list_directory(parent.path().to_str().unwrap().to_string()).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "alpha");
        assert_eq!(results[1].name, "beta");
    }

    // list_directory returns empty vec for nonexistent path
    #[test]
    fn list_directory_nonexistent_returns_empty() {
        let results = list_directory("/nonexistent/path/abc".to_string()).unwrap();
        assert!(results.is_empty());
    }

    // list_directory detects expandable (has subdirectories)
    #[test]
    fn list_directory_detects_expandable() {
        let parent = TempDir::new().unwrap();
        let child = parent.path().join("has-children");
        std::fs::create_dir(&child).unwrap();
        std::fs::create_dir(child.join("grandchild")).unwrap();

        let empty_child = parent.path().join("empty");
        std::fs::create_dir(&empty_child).unwrap();

        let results = list_directory(parent.path().to_str().unwrap().to_string()).unwrap();
        assert_eq!(results.len(), 2);

        let expandable = results.iter().find(|e| e.name == "has-children").unwrap();
        assert!(expandable.is_expandable);

        let empty = results.iter().find(|e| e.name == "empty").unwrap();
        assert!(!empty.is_expandable);
    }

    // validate_project_path: nonexistent path
    #[test]
    fn validate_nonexistent_path() {
        let (db_state, _tmp) = test_db_state();
        let conn = db_state.0.lock().unwrap();

        let dir = std::path::Path::new("/nonexistent/validate/path");
        let result = PathValidation {
            exists: dir.is_dir(),
            is_git_repo: false,
            is_registered: false,
        };

        assert!(!result.exists);
        assert!(!result.is_git_repo);
        assert!(!result.is_registered);
    }

    // validate_project_path: existing dir, not a git repo
    #[test]
    fn validate_non_git_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path();

        let exists = path.is_dir();
        let is_git_repo = git2::Repository::open(path).is_ok();

        assert!(exists);
        assert!(!is_git_repo);
    }

    // validate_project_path: existing git repo, not registered
    #[test]
    fn validate_git_repo_not_registered() {
        let (db_state, _tmp) = test_db_state();
        let dir = temp_project_dir();
        let path_str = dir.path().to_str().unwrap();

        let exists = dir.path().is_dir();
        let is_git_repo = dir.path().join(".git").is_dir();
        let conn = db_state.0.lock().unwrap();
        let is_registered = crate::db::queries::project_exists_at_path(&conn, path_str).unwrap();

        assert!(exists);
        assert!(is_git_repo);
        assert!(!is_registered);
    }

    // validate_project_path: registered project
    #[test]
    fn validate_registered_project() {
        let (db_state, _tmp) = test_db_state();
        let dir = temp_project_dir();
        let path_str = dir.path().to_str().unwrap();

        let conn = db_state.0.lock().unwrap();
        let thresholds = ActivityThresholds::default();
        project::register_project(&conn, path_str, None, &thresholds).unwrap();

        let is_registered = crate::db::queries::project_exists_at_path(&conn, path_str).unwrap();
        assert!(is_registered);
    }

    // AC4: batch registration skips invalid paths gracefully
    #[test]
    fn batch_register_skips_invalid_paths() {
        let (_db_state, _tmp) = test_db_state();
        let conn = _db_state.0.lock().unwrap();

        let thresholds = ActivityThresholds::default();
        let result = project::register_project(&conn, "/nonexistent/path", None, &thresholds);
        assert!(result.is_err());

        // DB should still be empty
        let count = crate::db::queries::project_count(&conn).unwrap();
        assert_eq!(count, 0);
    }
}
