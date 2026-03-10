//! Thin Tauri command handlers for task workflows.
//!
//! Business logic lives in `services::task_query` and `services::task_sync`.

use std::time::Instant;

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::queries;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::services::task_query;
use crate::services::task_sync::TaskScanGenerationState;
use crate::ProviderState;

#[tauri::command]
pub fn get_project_tasks(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    generation_state: State<'_, TaskScanGenerationState>,
    project_id: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    let span = IpcCommandSpan::start("get_project_tasks");
    let project_path =
        resolve_project_path(db.inner(), &project_id, Some(&span)).ipc_cmd("get_project_tasks")?;
    let result = get_project_tasks_impl(
        db.inner(),
        providers.inner(),
        generation_state.inner(),
        project_path,
    );
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_task_detail(
    db: State<'_, DbState>,
    project_id: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    let span = IpcCommandSpan::start("get_task_detail");
    let project_path =
        resolve_project_path(db.inner(), &project_id, Some(&span)).ipc_cmd("get_task_detail")?;
    let result = get_task_detail_impl(db.inner(), project_path, task_id, source, source_key);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_archived_sessions(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    let span = IpcCommandSpan::start("get_archived_sessions");
    let project_path = resolve_project_path(db.inner(), &project_id, Some(&span))
        .ipc_cmd("get_archived_sessions")?;
    let result = get_archived_sessions_impl(db.inner(), providers.inner(), project_path);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_commit_files(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    let span = IpcCommandSpan::start("get_commit_files");
    let project_path =
        resolve_project_path(db.inner(), &project_id, Some(&span)).ipc_cmd("get_commit_files")?;
    let result = get_commit_files_impl(providers.inner(), project_path, hash);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_commit_diff(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    let span = IpcCommandSpan::start("get_commit_diff");
    let project_path =
        resolve_project_path(db.inner(), &project_id, Some(&span)).ipc_cmd("get_commit_diff")?;
    let result = get_commit_diff_impl(providers.inner(), project_path, hash, file_path);
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_commits_in_range(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_id: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    let span = IpcCommandSpan::start("get_commits_in_range");
    let project_path = resolve_project_path(db.inner(), &project_id, Some(&span))
        .ipc_cmd("get_commits_in_range")?;
    let result = get_commits_in_range_impl(providers.inner(), project_path, after, before);
    span.finish_result(&result);
    result
}

fn resolve_project_path(
    db: &DbState,
    project_id: &str,
    span: Option<&IpcCommandSpan>,
) -> Result<String, String> {
    let lock_started = Instant::now();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if let Some(span) = span {
        span.emit_lock_wait("db", lock_started.elapsed().as_millis() as u64);
    }
    let project = queries::get_project(&conn, project_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    Ok(project.path)
}

fn get_project_tasks_impl(
    db: &DbState,
    providers: &ProviderState,
    generation_state: &TaskScanGenerationState,
    project_path: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    task_query::get_or_refresh_project_tasks(db, providers, generation_state, project_path)
        .ipc_cmd("get_project_tasks")
}

fn get_task_detail_impl(
    db: &DbState,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    task_query::get_task_detail(db, project_path, task_id, source, source_key)
        .ipc_cmd("get_task_detail")
}

fn get_archived_sessions_impl(
    db: &DbState,
    providers: &ProviderState,
    project_path: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    task_query::get_archived_sessions(db, providers, project_path).ipc_cmd("get_archived_sessions")
}

fn get_commit_files_impl(
    providers: &ProviderState,
    project_path: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    task_query::get_commit_files(providers, project_path, hash).ipc_cmd("get_commit_files")
}

fn get_commit_diff_impl(
    providers: &ProviderState,
    project_path: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    task_query::get_commit_diff(providers, project_path, hash, file_path).ipc_cmd("get_commit_diff")
}

fn get_commits_in_range_impl(
    providers: &ProviderState,
    project_path: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    task_query::get_commits_in_range(providers, project_path, after, before)
        .ipc_cmd("get_commits_in_range")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::IpcErrorCode;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");
        (DbState(Mutex::new(conn)), tmp)
    }

    fn insert_task(db: &DbState, project_path: &str, source_key: &str, task_id: &str) {
        let task = crate::db::task_queries::PersistedTask {
            project_path: project_path.to_string(),
            source: "claude".to_string(),
            source_key: source_key.to_string(),
            source_task_id: task_id.to_string(),
            subject: format!("Task {task_id}"),
            description: Some("detail".to_string()),
            active_form: None,
            status: "in_progress".to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            session_id: None,
            first_seen_at: "2026-03-01T10:00:00Z".to_string(),
            state_changed_at: Some("2026-03-01T10:00:00Z".to_string()),
            updated_at: "2026-03-01T10:00:00Z".to_string(),
            archived_at: None,
            last_status: Some("in_progress".to_string()),
            archived_reason: None,
        };

        let conn = db.0.lock().expect("db lock");
        crate::db::task_queries::upsert_task(&conn, &task).expect("insert task");
    }

    fn insert_project(db: &DbState, project_id: &str, project_path: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let project = crate::models::Project {
            id: project_id.to_string(),
            name: format!("project-{project_id}"),
            path: project_path.to_string(),
            description: None,
            last_activity_at: None,
            hero_preference: None,
            created_at: now.clone(),
            updated_at: now,
            cached_branch: None,
            cached_is_dirty: None,
        };
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::insert_project(&conn, &project).expect("insert project");
    }

    fn local_provider_state() -> ProviderState {
        ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: None,
            wsl_distro: None,
        }
    }

    #[test]
    fn project_tasks_and_detail_commands_map_identity_args() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "proj-demo", "/projects/demo");
        insert_task(&db, "/projects/demo", "session-a", "1");
        insert_task(&db, "/projects/demo", "session-b", "2");
        let providers = local_provider_state();
        let generation_state = TaskScanGenerationState::default();

        let result = get_project_tasks_impl(
            &db,
            &providers,
            &generation_state,
            "/projects/demo".to_string(),
        )
        .expect("project tasks");
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.tasks[0].source_key, "session-a");
        assert_eq!(result.tasks[1].source_key, "session-b");

        let detail = get_task_detail_impl(
            &db,
            "/projects/demo".to_string(),
            "2".to_string(),
            "claude".to_string(),
            "session-b".to_string(),
        )
        .expect("task detail");
        assert_eq!(detail.task.id, "2");
        assert_eq!(detail.task.source_key, "session-b");
        assert_eq!(detail.task.subject, "Task 2");
    }

    #[test]
    fn task_detail_command_propagates_not_found_error() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "proj-demo", "/projects/demo");
        let err = get_task_detail_impl(
            &db,
            "/projects/demo".to_string(),
            "missing".to_string(),
            "claude".to_string(),
            "session-x".to_string(),
        )
        .expect_err("missing task should fail");

        assert_eq!(err.code, IpcErrorCode::NotFound);
        assert_eq!(err.command.as_deref(), Some("get_task_detail"));
        assert!(err.message.contains("Task not found"));
    }

    #[test]
    fn project_tasks_command_reports_db_lock_poison_error() {
        let db = DbState(Mutex::new(
            rusqlite::Connection::open_in_memory().expect("open memory db"),
        ));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = db.0.lock().expect("lock");
            panic!("poison lock");
        }));
        let providers = local_provider_state();
        let generation_state = TaskScanGenerationState::default();

        let err = get_project_tasks_impl(
            &db,
            &providers,
            &generation_state,
            "/projects/demo".to_string(),
        )
        .expect_err("poisoned lock should fail");
        assert_eq!(err.code, IpcErrorCode::InternalError);
        assert_eq!(err.command.as_deref(), Some("get_project_tasks"));
        assert!(err.message.to_lowercase().contains("poison"));
    }

    #[test]
    fn commit_range_command_propagates_provider_errors() {
        let providers = local_provider_state();
        let err = get_commits_in_range_impl(
            &providers,
            "/path/that/does/not/exist".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
            "2026-01-02T00:00:00Z".to_string(),
        )
        .expect_err("invalid path should fail");

        assert!(!err.message.trim().is_empty());
    }

    #[test]
    fn resolve_project_path_returns_registered_path() {
        let (db, _tmp) = test_db_state();
        insert_project(&db, "proj-1", "/projects/demo");

        let path = resolve_project_path(&db, "proj-1", None).expect("project path");
        assert_eq!(path, "/projects/demo");
    }

    #[test]
    fn resolve_project_path_returns_error_for_missing_project() {
        let (db, _tmp) = test_db_state();
        let err = resolve_project_path(&db, "missing", None).expect_err("missing project");
        assert!(err.contains("Project not found"));
    }
}
