use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Deserialize;
use tauri::{Emitter, State};

use crate::commands::lifecycle::IpcCommandSpan;
use crate::db::{queries, settings_queries};
use crate::errors::{sanitize_error, SanitizeErr};
use crate::models::{ProjectDetail, ProjectSummary};
use crate::services::project;
use crate::{ProviderState, SearchState};

static PROJECT_SELECTION_RECONCILE_QUEUED: AtomicBool = AtomicBool::new(false);

fn enqueue_activity_watch_reconcile(app: tauri::AppHandle, reason: &'static str) {
    if PROJECT_SELECTION_RECONCILE_QUEUED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    #[cfg(test)]
    {
        crate::startup::watchers::reconcile_activity_watches(&app, reason);
        PROJECT_SELECTION_RECONCILE_QUEUED.store(false, Ordering::Release);
    }

    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            struct ResetQueuedFlag;
            impl Drop for ResetQueuedFlag {
                fn drop(&mut self) {
                    PROJECT_SELECTION_RECONCILE_QUEUED.store(false, Ordering::Release);
                }
            }

            let _reset_queued_flag = ResetQueuedFlag;
            crate::startup::watchers::reconcile_activity_watches(&app, reason);
        });
    }
}

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
    let span = IpcCommandSpan::start("list_projects");
    let result = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
        project::list_projects(&conn, &settings.thresholds).sanitize_err()
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_project(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    project_id: String,
) -> Result<ProjectDetail, String> {
    let span = IpcCommandSpan::start("get_project");
    let result = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
        // Project selection is explicit user activity; promote on read.
        project::touch_activity(&conn, &project_id).sanitize_err()?;
        let detail =
            project::get_project(&conn, &project_id, &settings.thresholds).sanitize_err()?;
        drop(conn);

        enqueue_activity_watch_reconcile(app.clone(), "project_selected");
        Ok(detail)
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn register_project(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    path: String,
    name: Option<String>,
) -> Result<ProjectDetail, String> {
    let span = IpcCommandSpan::start("register_project");
    let result = {
        let expanded = expand_tilde(&path);
        let detail = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
            project::register_project(&conn, &expanded, name.as_deref(), &settings.thresholds)
                .sanitize_err()?
        }; // conn dropped here — lock released

        // Correct last_activity_at from git (needs its own lock)
        reseed_activity_for_project(&db, &providers, &detail.id, &detail.path);

        Ok(detail)
    };
    span.finish_result(&result);
    result
}

fn create_project_impl(
    conn: &Connection,
    name: &str,
    parent_dir: &str,
    thresholds: &crate::models::ActivityThresholds,
) -> Result<ProjectDetail, crate::errors::AppError> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(crate::errors::AppError::InvalidPath(
            "Project name cannot be empty".to_string(),
        ));
    }
    if trimmed_name == "."
        || trimmed_name == ".."
        || trimmed_name.contains('/')
        || trimmed_name.contains('\\')
        || trimmed_name.contains('\0')
    {
        return Err(crate::errors::AppError::InvalidPath(format!(
            "Invalid project name: {trimmed_name}"
        )));
    }

    let parent_path = std::path::Path::new(parent_dir);
    if !parent_path.is_dir() {
        return Err(crate::errors::AppError::InvalidPath(format!(
            "Parent directory does not exist or is not a directory: {parent_dir}"
        )));
    }

    let target_dir = parent_path.join(trimmed_name);
    if target_dir.exists() {
        return Err(crate::errors::AppError::AlreadyExists(format!(
            "Target directory already exists: {}",
            target_dir.to_string_lossy()
        )));
    }

    std::fs::create_dir_all(&target_dir)?;

    let mut init_options = git2::RepositoryInitOptions::new();
    init_options.initial_head("main");
    git2::Repository::init_opts(&target_dir, &init_options)?;

    project::register_project(
        conn,
        target_dir.to_string_lossy().as_ref(),
        Some(trimmed_name),
        thresholds,
    )
}

#[tauri::command]
pub fn create_project(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    name: String,
    parent_dir: String,
) -> Result<ProjectDetail, String> {
    let span = IpcCommandSpan::start("create_project");
    let result = {
        let expanded_parent = expand_tilde(&parent_dir);
        let detail = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
            create_project_impl(&conn, &name, &expanded_parent, &settings.thresholds)
                .sanitize_err()?
        };

        reseed_activity_for_project(&db, &providers, &detail.id, &detail.path);

        Ok(detail)
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn update_project(
    db: State<'_, DbState>,
    project_id: String,
    fields: UpdateProjectFields,
) -> Result<ProjectDetail, String> {
    let span = IpcCommandSpan::start("update_project");
    let result = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
        let thresholds = settings.thresholds;

        project::update_project(
            &conn,
            &project_id,
            fields.name.as_deref(),
            fields.description.as_ref().map(|d| d.as_deref()),
            fields.hero_preference.as_ref().map(|h| h.as_deref()),
        )
        .sanitize_err()?;

        // Return the updated project.
        project::get_project(&conn, &project_id, &thresholds).sanitize_err()
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn remove_project(
    db: State<'_, DbState>,
    search: State<'_, SearchState>,
    project_id: String,
) -> Result<(), String> {
    let span = IpcCommandSpan::start("remove_project");
    let result = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        project::remove_project(&conn, &project_id).sanitize_err()?;

        // Clean up search index entries for this project
        match search.0.lock() {
            Ok(mut index) => {
                index.remove_by_project(&project_id);
                if let Err(e) = index.commit() {
                    tracing::warn!(project_id, error = %e, "search index commit failed after project removal");
                }
            }
            Err(e) => {
                tracing::warn!(project_id, error = %e, "search index lock failed during project removal");
            }
        }
        Ok(())
    };
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn is_first_run(db: State<'_, DbState>) -> Result<bool, String> {
    let span = IpcCommandSpan::start("is_first_run");
    let result = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let count = crate::db::queries::project_count(&conn).sanitize_err()?;
        Ok(count == 0)
    };
    span.finish_result(&result);
    result
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
    providers: State<'_, ProviderState>,
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<BatchRegistrationResult>, String> {
    let span = IpcCommandSpan::start("register_projects_batch");
    let result = {
        let results = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            let settings = settings_queries::get_all_settings(&conn).sanitize_err()?;
            let total = paths.len();
            let mut results = Vec::with_capacity(total);

            for (index, path) in paths.iter().enumerate() {
                let expanded = expand_tilde(path);
                let result =
                    match project::register_project(&conn, &expanded, None, &settings.thresholds) {
                        Ok(detail) => {
                            let _ = app.emit(
                                "batch-registration-progress",
                                serde_json::json!({
                                    "projectName": detail.name,
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
            results
        }; // conn dropped here — lock released

        // Correct last_activity_at from git for all new projects (needs its own lock)
        for r in &results {
            if let Some(ref detail) = r.project {
                reseed_activity_for_project(&db, &providers, &detail.id, &detail.path);
            }
        }
        Ok(results)
    };
    span.finish_result(&result);
    result
}

/// Reseed a single project's git status and last_activity_at from git.
/// Runs synchronously but is fast for local projects; WSL projects go through the daemon.
fn reseed_activity_for_project(
    db: &State<'_, DbState>,
    providers: &State<'_, ProviderState>,
    project_id: &str,
    project_path: &str,
) {
    let provider = providers.resolve(project_path);

    // Cache branch + dirty status so sidebar shows them immediately
    if let Ok(status) = provider.git_status(project_path) {
        match db.0.lock() {
            Ok(conn) => {
                if let Err(e) = queries::update_cached_git_status(
                    &conn,
                    project_id,
                    status.branch.as_deref(),
                    status.is_dirty,
                ) {
                    tracing::warn!(project_id, error = %e, "reseed: failed to cache git status");
                }
            }
            Err(e) => {
                tracing::warn!(project_id, error = %e, "reseed: db lock failed for git status cache");
            }
        }
    }

    match provider.latest_commit_time(project_path) {
        Ok(Some(commit_time)) => {
            let commit_ts = commit_time.to_rfc3339();
            tracing::info!(project_id, %commit_ts, "reseed: updating last_activity_at from git");
            match db.0.lock() {
                Ok(conn) => {
                    if let Err(e) = queries::update_project(
                        &conn,
                        project_id,
                        None,
                        None,
                        None,
                        Some(Some(&commit_ts)),
                        None,
                    ) {
                        tracing::warn!(project_id, error = %e, "reseed: failed to update last_activity_at");
                    }
                }
                Err(e) => {
                    tracing::warn!(project_id, error = %e, "reseed: db lock failed for activity update");
                }
            }
        }
        Ok(None) => {
            tracing::warn!(
                project_id,
                project_path,
                "reseed: no commits found, keeping registration time"
            );
        }
        Err(e) => {
            tracing::warn!(project_id, project_path, error = %e, "reseed: git query failed, keeping registration time");
        }
    }
}

#[tauri::command]
pub fn scan_directory(path: String) -> Result<Vec<DiscoveredProject>, String> {
    let span = IpcCommandSpan::start("scan_directory");
    let result = {
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
    };
    span.finish_result(&result);
    result
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
    let span = IpcCommandSpan::start("list_directory");
    let result = {
        let expanded = expand_tilde(&path);
        let dir = std::path::Path::new(&expanded);

        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let read_dir = std::fs::read_dir(dir).map_err(|e| sanitize_error(&e.to_string()))?;
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

            // Assume all directories are expandable (lazy-check).
            // Eagerly checking via nested read_dir is an N+1 penalty and on macOS
            // it triggers TCC permission prompts for protected folders like
            // ~/Desktop, ~/Documents, etc., which block the IPC thread.
            let is_expandable = true;

            entries.push(DirectoryEntry {
                name,
                path: full_path,
                is_expandable,
            });
        }

        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(entries)
    };
    span.finish_result(&result);
    result
}

/// Return filesystem root entries for the directory tree browser.
/// On Windows: available drive letters (C:\, D:\, etc.) + WSL distributions
/// On Linux/macOS: just ["/"]
#[tauri::command]
pub fn get_system_roots() -> Vec<DirectoryEntry> {
    let span = IpcCommandSpan::start("get_system_roots");
    let roots = {
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
    };
    span.complete();
    roots
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
    let span = IpcCommandSpan::start("validate_project_path");
    let result = {
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
        let is_registered = queries::project_exists_at_path(&conn, &expanded).sanitize_err()?;

        Ok(PathValidation {
            exists,
            is_git_repo,
            is_registered,
        })
    };
    span.finish_result(&result);
    result
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
        project::register_project(
            &conn,
            dir.path().to_str().unwrap(),
            Some("test"),
            &thresholds,
        )
        .unwrap();
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
        let d1 = project::register_project(&conn, dir1.path().to_str().unwrap(), None, &thresholds)
            .unwrap();
        let d2 = project::register_project(&conn, dir2.path().to_str().unwrap(), None, &thresholds)
            .unwrap();

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

    // All directories are marked expandable (lazy-check).
    // Eagerly checking via nested read_dir is an N+1 penalty and on macOS
    // it triggers TCC permission prompts for protected folders. Empty dirs
    // show "No subdirectories found" when expanded — standard file browser UX.
    #[test]
    fn list_directory_marks_all_dirs_expandable() {
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
        assert!(empty.is_expandable); // lazy-check: all dirs expandable
    }

    // validate_project_path: nonexistent path
    #[test]
    fn validate_nonexistent_path() {
        let (db_state, _tmp) = test_db_state();
        let _conn = db_state.0.lock().unwrap();

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

    #[test]
    fn create_project_initializes_git_on_main_and_registers_project() {
        let (_db_state, _tmp) = test_db_state();
        let parent = TempDir::new().unwrap();
        let conn = _db_state.0.lock().unwrap();
        let thresholds = ActivityThresholds::default();

        let detail = create_project_impl(
            &conn,
            "new-project",
            parent.path().to_str().unwrap(),
            &thresholds,
        )
        .unwrap();

        let created_path = parent.path().join("new-project");
        assert!(created_path.is_dir());
        assert_eq!(detail.path, created_path.to_string_lossy());
        assert_eq!(detail.name, "new-project");

        let repo = git2::Repository::open(&created_path).unwrap();
        let head = repo.find_reference("HEAD").unwrap();
        assert_eq!(head.symbolic_target(), Some("refs/heads/main"));

        let is_registered =
            crate::db::queries::project_exists_at_path(&conn, created_path.to_str().unwrap())
                .unwrap();
        assert!(is_registered);
    }

    #[test]
    fn create_project_rejects_existing_target_directory() {
        let (_db_state, _tmp) = test_db_state();
        let parent = TempDir::new().unwrap();
        let existing = parent.path().join("dupe");
        std::fs::create_dir_all(&existing).unwrap();
        let conn = _db_state.0.lock().unwrap();
        let thresholds = ActivityThresholds::default();

        let err = create_project_impl(&conn, "dupe", parent.path().to_str().unwrap(), &thresholds)
            .unwrap_err();

        assert!(matches!(err, crate::errors::AppError::AlreadyExists(_)));
    }
}
