use tauri::State;

use crate::commands::projects::DbState;
use crate::db::queries;
use crate::git::{commits, status};
use crate::models::{Commit, GitStatus};

#[tauri::command]
pub fn get_recent_commits(
    db: State<'_, DbState>,
    project_id: String,
    limit: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let repo_path = std::path::Path::new(&project.path);
    commits::get_recent_commits(repo_path, limit.unwrap_or(10).min(500)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_commits(
    db: State<'_, DbState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let repo_path = std::path::Path::new(&project.path);
    commits::get_all_commits(repo_path, limit.unwrap_or(50).min(500), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_git_status(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<GitStatus, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let repo_path = std::path::Path::new(&project.path);
    status::get_status(repo_path).map_err(|e| e.to_string())
}
