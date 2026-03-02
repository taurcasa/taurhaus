use std::path::Path;

use base64::Engine as _;
use tauri::State;

use crate::commands::projects::DbState;
use crate::db::queries;
use crate::errors::sanitize_error;
use crate::models::{FileContent, FileTreeNode};
use crate::ProviderState;

/// Look up a project's path from the DB, releasing the lock immediately.
fn resolve_project_path(db: &DbState, project_id: &str) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
}

#[tauri::command]
pub fn get_file_tree(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<Vec<FileTreeNode>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let provider = providers.resolve(&path);
    provider
        .file_tree(&path)
        .map_err(|e| sanitize_error(&e.to_string()))
}

#[tauri::command]
pub fn read_file(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    relative_path: String,
) -> Result<FileContent, String> {
    let path = resolve_project_path(&db, &project_id)?;
    // Normalize backslashes — search index on Windows may store paths with
    // backslashes (e.g. "tests\test_integration.py") that the Linux daemon
    // can't resolve. Belt-and-suspenders with the indexer normalization.
    let relative_path = relative_path.replace('\\', "/");
    let provider = providers.resolve(&path);
    provider
        .read_file(&path, &relative_path)
        .map_err(|e| sanitize_error(&e.to_string()))
}

#[tauri::command]
pub fn get_readme(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> Result<Option<FileContent>, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let is_wsl = crate::provider::path::is_wsl_path(&path);
    let has_daemon = providers.daemon.as_ref().is_some_and(|d| d.is_connected());
    let using_daemon = is_wsl && has_daemon;
    tracing::debug!(
        project_id,
        path,
        is_wsl,
        has_daemon,
        using_daemon,
        "get_readme: resolving provider"
    );
    let provider = providers.resolve(&path);
    let result = provider
        .read_readme(&path)
        .map_err(|e| sanitize_error(&e.to_string()))?;
    if let Some(ref content) = result {
        tracing::debug!(
            project_id,
            readme_path = content.path,
            content_len = content.content.len(),
            content_preview = &content.content[..content.content.len().min(80)],
            "get_readme: returning content"
        );
    } else {
        tracing::debug!(project_id, "get_readme: no README found");
    }
    Ok(result)
}

/// Read a binary file from a project directory and return it as a base64 data URI.
/// Used for rendering images embedded in markdown READMEs.
#[tauri::command]
pub fn read_project_asset(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let path = resolve_project_path(&db, &project_id)?;
    let relative_path = relative_path.replace('\\', "/");
    let provider = providers.resolve(&path);
    let bytes = provider
        .read_asset(&path, &relative_path)
        .map_err(|e| sanitize_error(&e.to_string()))?;

    let mime = mime_from_extension(&relative_path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

fn mime_from_extension(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}
