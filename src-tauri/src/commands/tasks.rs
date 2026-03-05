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
    get_project_tasks_impl(db.inner(), project_path)
}

#[tauri::command]
pub fn get_task_detail(
    db: State<'_, DbState>,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    get_task_detail_impl(db.inner(), project_path, task_id, source, source_key)
}

#[tauri::command]
pub fn get_archived_sessions(
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    project_path: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    get_archived_sessions_impl(db.inner(), providers.inner(), project_path)
}

#[tauri::command]
pub fn get_commit_files(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    get_commit_files_impl(providers.inner(), project_path, hash)
}

#[tauri::command]
pub fn get_commit_diff(
    providers: State<'_, ProviderState>,
    project_path: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    get_commit_diff_impl(providers.inner(), project_path, hash, file_path)
}

#[tauri::command]
pub fn get_commits_in_range(
    providers: State<'_, ProviderState>,
    project_path: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    get_commits_in_range_impl(providers.inner(), project_path, after, before)
}

fn get_project_tasks_impl(
    db: &DbState,
    project_path: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    task_query::get_project_tasks(db, project_path)
}

fn get_task_detail_impl(
    db: &DbState,
    project_path: String,
    task_id: String,
    source: String,
    source_key: String,
) -> IpcResult<crate::task_scanner::TaskDetail> {
    task_query::get_task_detail(db, project_path, task_id, source, source_key)
}

fn get_archived_sessions_impl(
    db: &DbState,
    providers: &ProviderState,
    project_path: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    task_query::get_archived_sessions(db, providers, project_path)
}

fn get_commit_files_impl(
    providers: &ProviderState,
    project_path: String,
    hash: String,
) -> IpcResult<Vec<crate::models::CommitFile>> {
    task_query::get_commit_files(providers, project_path, hash)
}

fn get_commit_diff_impl(
    providers: &ProviderState,
    project_path: String,
    hash: String,
    file_path: String,
) -> IpcResult<Vec<crate::models::DiffHunk>> {
    task_query::get_commit_diff(providers, project_path, hash, file_path)
}

fn get_commits_in_range_impl(
    providers: &ProviderState,
    project_path: String,
    after: String,
    before: String,
) -> IpcResult<crate::models::GitRangeResult> {
    task_query::get_commits_in_range(providers, project_path, after, before)
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
        insert_task(&db, "/projects/demo", "session-a", "1");
        insert_task(&db, "/projects/demo", "session-b", "2");

        let result =
            get_project_tasks_impl(&db, "/projects/demo".to_string()).expect("project tasks");
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
        let err = get_task_detail_impl(
            &db,
            "/projects/demo".to_string(),
            "missing".to_string(),
            "claude".to_string(),
            "session-x".to_string(),
        )
        .expect_err("missing task should fail");

        assert_eq!(err.code, IpcErrorCode::NotFound);
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

        let err = get_project_tasks_impl(&db, "/projects/demo".to_string())
            .expect_err("poisoned lock should fail");
        assert_eq!(err.code, IpcErrorCode::InternalError);
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
}
