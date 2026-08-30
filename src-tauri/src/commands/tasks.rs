//! Thin Tauri command handlers for task workflows.
//!
//! Business logic lives in `services::task_query` and `services::task_sync`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::queries;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::services::task_query;
use crate::services::task_sync::TaskScanGenerationState;
use crate::ProviderState;

const TASK_REFRESH_COOLDOWN: Duration = Duration::from_secs(3);
const ARCHIVED_REFRESH_COOLDOWN: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy)]
struct RefreshSlot {
    in_progress: bool,
    last_started_at: Instant,
}

#[derive(Default)]
pub struct TaskQueryRefreshState {
    task_refreshes: Mutex<HashMap<String, RefreshSlot>>,
    archived_refreshes: Mutex<HashMap<String, RefreshSlot>>,
}

impl TaskQueryRefreshState {
    fn try_begin_task_refresh(&self, project_key: &str) -> bool {
        try_begin_refresh(&self.task_refreshes, project_key, TASK_REFRESH_COOLDOWN)
    }

    fn finish_task_refresh(&self, project_key: &str) {
        finish_refresh(&self.task_refreshes, project_key);
    }

    fn try_begin_archived_refresh(&self, project_key: &str) -> bool {
        try_begin_refresh(
            &self.archived_refreshes,
            project_key,
            ARCHIVED_REFRESH_COOLDOWN,
        )
    }

    fn finish_archived_refresh(&self, project_key: &str) {
        finish_refresh(&self.archived_refreshes, project_key);
    }
}

#[tauri::command]
pub fn get_project_tasks(
    app: AppHandle,
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    generation_state: State<'_, TaskScanGenerationState>,
    refresh_state: State<'_, TaskQueryRefreshState>,
    project_id: String,
) -> IpcResult<crate::task_scanner::TaskResult> {
    let span = IpcCommandSpan::start("get_project_tasks");
    let project_path =
        resolve_project_path(db.inner(), &project_id, Some(&span)).ipc_cmd("get_project_tasks")?;
    let refresh_project_path = project_path.clone();
    let result = get_project_tasks_impl(
        db.inner(),
        providers.inner(),
        generation_state.inner(),
        project_path,
    );
    span.finish_result(&result);
    if result.is_ok() {
        schedule_project_task_refresh(
            &app,
            refresh_state.inner(),
            project_id,
            refresh_project_path,
        );
    }
    result
}

#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn get_archived_sessions(
    app: AppHandle,
    db: State<'_, DbState>,
    providers: State<'_, ProviderState>,
    refresh_state: State<'_, TaskQueryRefreshState>,
    project_id: String,
) -> IpcResult<crate::task_scanner::ArchivedSessionsResult> {
    let span = IpcCommandSpan::start("get_archived_sessions");
    let project_path = resolve_project_path(db.inner(), &project_id, Some(&span))
        .ipc_cmd("get_archived_sessions")?;
    let result = get_archived_sessions_impl(db.inner(), providers.inner(), project_path.clone());
    let response = result.map(|query| {
        if query.cache_status != task_query::ArchivedSessionCacheStatus::Fresh {
            schedule_archived_session_refresh(
                &app,
                refresh_state.inner(),
                project_id.clone(),
                project_path.clone(),
            );
        }
        query.result
    });
    span.finish_result(&response);
    response
}

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
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
) -> IpcResult<task_query::ArchivedSessionsQueryResult> {
    task_query::get_archived_sessions(db, providers, project_path).ipc_cmd("get_archived_sessions")
}

fn try_begin_refresh(
    refreshes: &Mutex<HashMap<String, RefreshSlot>>,
    project_key: &str,
    cooldown: Duration,
) -> bool {
    let mut guard = refreshes.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(slot) = guard.get(project_key) {
        if slot.in_progress || slot.last_started_at.elapsed() < cooldown {
            return false;
        }
    }
    guard.insert(
        project_key.to_string(),
        RefreshSlot {
            in_progress: true,
            last_started_at: Instant::now(),
        },
    );
    true
}

fn finish_refresh(refreshes: &Mutex<HashMap<String, RefreshSlot>>, project_key: &str) {
    let mut guard = refreshes.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(slot) = guard.get_mut(project_key) {
        slot.in_progress = false;
        slot.last_started_at = Instant::now();
    }
}

fn schedule_project_task_refresh(
    app: &AppHandle,
    refresh_state: &TaskQueryRefreshState,
    project_id: String,
    project_path: String,
) {
    let project_key = crate::provider::path::normalize_project_path(&project_path);
    if !refresh_state.try_begin_task_refresh(&project_key) {
        return;
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let refresh_state = app_handle.state::<TaskQueryRefreshState>();
        let providers = app_handle.state::<ProviderState>();
        let generation_state = app_handle.state::<TaskScanGenerationState>();
        let db = app_handle.state::<DbState>();
        let project_key = crate::provider::path::normalize_project_path(&project_path);

        let refresh_result = (|| -> Result<Option<usize>, String> {
            let scan_generation = crate::bootstrap::next_task_scan_cycle_id();
            let scan_result = crate::services::task_sync::scan_tasks_from_files(
                providers.inner(),
                &project_path,
                Some(scan_generation),
                None,
                None,
            );

            let (before_sig, after_sig) = {
                let conn = db.0.lock().map_err(|e| format!("{e}"))?;
                let before = crate::db::task_queries::get_tasks_for_project(&conn, &project_key)
                    .sanitize_err()?;
                crate::services::task_sync::persist_task_scan_with_generation(
                    &conn,
                    &project_key,
                    &scan_result,
                    generation_state.inner(),
                    scan_generation,
                );
                let after = crate::db::task_queries::get_tasks_for_project(&conn, &project_key)
                    .sanitize_err()?;
                let after_len = after.len();
                (task_signature(&before), (task_signature(&after), after_len))
            };

            if before_sig != after_sig.0 {
                Ok(Some(after_sig.1))
            } else {
                Ok(None)
            }
        })();

        match refresh_result {
            Ok(Some(task_count)) => {
                let _ = app_handle.emit(
                    "project-tasks-changed",
                    serde_json::json!({
                        "project_id": project_id,
                        "task_count": task_count,
                    }),
                );
                crate::commands::coordination::apply_task_effort_after_task_change(
                    &app_handle,
                    &project_key,
                );
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    project_id = %project_id,
                    project_path = %project_path,
                    error = %error,
                    "background project task refresh failed"
                );
            }
        }

        refresh_state.finish_task_refresh(&project_key);
    });
}

fn schedule_archived_session_refresh(
    app: &AppHandle,
    refresh_state: &TaskQueryRefreshState,
    project_id: String,
    project_path: String,
) {
    let project_key = crate::provider::path::normalize_project_path(&project_path);
    if !refresh_state.try_begin_archived_refresh(&project_key) {
        return;
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let refresh_state = app_handle.state::<TaskQueryRefreshState>();
        let db = app_handle.state::<DbState>();
        let providers = app_handle.state::<ProviderState>();
        let project_key = crate::provider::path::normalize_project_path(&project_path);

        let refresh_result = (|| -> Result<bool, String> {
            let before_signature = {
                let conn = db.0.lock().map_err(|e| format!("{e}"))?;
                let summaries =
                    crate::db::task_queries::get_archived_session_summaries_for_project(
                        &conn,
                        &project_key,
                    )
                    .sanitize_err()?;
                archived_session_signature(&summaries)
            };

            crate::services::task_query::rebuild_archived_session_summaries(
                db.inner(),
                providers.inner(),
                project_path.clone(),
            )
            .map_err(|error| error.to_string())?;

            let after_signature = {
                let conn = db.0.lock().map_err(|e| format!("{e}"))?;
                let summaries =
                    crate::db::task_queries::get_archived_session_summaries_for_project(
                        &conn,
                        &project_key,
                    )
                    .sanitize_err()?;
                archived_session_signature(&summaries)
            };

            Ok(before_signature != after_signature)
        })();

        match refresh_result {
            Ok(true) => {
                let _ = app_handle.emit(
                    "project-task-history-changed",
                    serde_json::json!({
                        "project_id": project_id,
                    }),
                );
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    project_id = %project_id,
                    project_path = %project_path,
                    error = %error,
                    "background archived session summary refresh failed"
                );
            }
        }

        refresh_state.finish_archived_refresh(&project_key);
    });
}

fn task_signature(tasks: &[crate::db::task_queries::PersistedTask]) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for task in tasks {
        task.source.hash(&mut hasher);
        task.source_key.hash(&mut hasher);
        task.source_task_id.hash(&mut hasher);
        task.status.hash(&mut hasher);
        task.updated_at.hash(&mut hasher);
        task.archived_at.hash(&mut hasher);
    }
    hasher.finish()
}

fn archived_session_signature(
    summaries: &[crate::db::task_queries::PersistedArchivedSessionSummary],
) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for summary in summaries {
        summary.session_key.hash(&mut hasher);
        summary.session_id.hash(&mut hasher);
        summary.started_at.hash(&mut hasher);
        summary.ended_at.hash(&mut hasher);
        summary.duration_ms.hash(&mut hasher);
        summary.commit_count.hash(&mut hasher);
        summary.file_count.hash(&mut hasher);
        summary.sources.hash(&mut hasher);
        summary.last_archived_at.hash(&mut hasher);
        summary.enrichment_warnings.hash(&mut hasher);
    }
    hasher.finish()
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
            effort: None,
            effort_why: None,
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
            account_memory: Default::default(),
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
