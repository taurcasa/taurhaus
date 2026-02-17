use std::path::Path;

use base64::Engine as _;
use tauri::State;

use crate::commands::projects::DbState;
use crate::db::queries;
use crate::fs::{reader, readme, tree};
use crate::models::{FileContent, FileTreeNode};

#[tauri::command]
pub fn get_file_tree(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<FileTreeNode>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let root = std::path::Path::new(&project.path);
    tree::build_file_tree(root).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_file(
    db: State<'_, DbState>,
    project_id: String,
    relative_path: String,
) -> Result<FileContent, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let root = std::path::Path::new(&project.path);
    reader::read_file(root, &relative_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_readme(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Option<FileContent>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let root = std::path::Path::new(&project.path);
    readme::find_readme(root).map_err(|e| e.to_string())
}

/// Read a binary file from a project directory and return it as a base64 data URI.
/// Used for rendering images embedded in markdown READMEs.
#[tauri::command]
pub fn read_project_asset(
    db: State<'_, DbState>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let project = queries::get_project(&conn, &project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let root = Path::new(&project.path);
    let full_path = root.join(&relative_path);

    // Security: ensure resolved path is within the project directory
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_file = full_path.canonicalize().map_err(|e| {
        format!("Asset not found: {relative_path} ({e})")
    })?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err("Access denied: path traversal detected".to_string());
    }

    let bytes = std::fs::read(&canonical_file).map_err(|e| {
        format!("Failed to read asset: {relative_path} ({e})")
    })?;

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
