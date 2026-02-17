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
