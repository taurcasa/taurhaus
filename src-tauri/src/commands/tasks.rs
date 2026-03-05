//! Thin Tauri command handlers for task workflows.
//!
//! Business logic lives in `services::task_query` and `services::task_sync`.

use tauri::State;

use crate::commands::projects::DbState;
use crate::errors::IpcResult;
use crate::services::task_query;
use crate::ProviderState;

#[tauri::command]
pub fn get_project_tasks(
    db: State<'_, DbState>,
    project_path: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    task_query::get_project_tasks(db.inner(), project_path)
}

#[tauri::command]
pub fn get_task_detail(
    db: State<'_, DbState>,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    task_query::get_task_detail(db.inner(), project_path, task_id, source, source_key)
}

#[tauri::command]
pub fn get_archived_sessions(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_path: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    task_query::get_archived_sessions(db.inner(), providers.inner(), project_path)
}

#[tauri::command]
pub fn get_commit_files(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    task_query::get_commit_files(providers.inner(), project_path, hash)
}

#[tauri::command]
pub fn get_commit_diff(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    task_query::get_commit_diff(providers.inner(), project_path, hash, file_path)
}

#[tauri::command]
pub fn get_commits_in_range(
    providers: State<'_, ProviderState>,
    project_path: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    task_query::get_commits_in_range(providers.inner(), project_path, after, before)
}
