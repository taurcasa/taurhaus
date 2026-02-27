use tauri::State;

use crate::commands::projects::DbState;
use crate::db::queries;
use crate::models::{Commit, GitStatus};
use crate::ProviderState;

/// Look up a project's path from the DB, releasing the lock immediately.
fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
    // conn (MutexGuard) drops here — lock released before any git work
}

#[tauri::command]
pub fn get_recent_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .recent_commits(&path, limit.unwrap_or(10).min(500))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_commits(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Commit>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .all_commits(&path, limit.unwrap_or(50).min(500), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_git_status(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<GitStatus, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider.git_status(&path).map_err(|e| e.to_string())
}
