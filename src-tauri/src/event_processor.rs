use std::collections::{HashMap, HashSet};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::bootstrap;
use crate::commands::projects::DbState;
use crate::sentinels::{CLAUDE_TASKS_PROJECT_ID, INTERNAL_PROJECT_ID_PREFIX};
use crate::{db, fs, search, services, ProviderState, SearchState};

/// Look up a project's path from the database, returning None on any error.
pub(crate) fn get_project_path(app: &AppHandle, project_id: &str) -> Option<String> {
    let db_state = app.state::<DbState>();
    let conn = match db_state.0.lock() {
        Ok(conn) => conn,
        Err(error) => {
            tracing::warn!(
                project_id,
                error = %error,
                "Failed to resolve project path: db lock poisoned"
            );
            return None;
        }
    };
    let project = match db::queries::get_project(&conn, project_id) {
        Ok(Some(project)) => project,
        Ok(None) => {
            tracing::warn!(
                project_id,
                "Failed to resolve project path: project not found"
            );
            return None;
        }
        Err(error) => {
            tracing::warn!(
                project_id,
                error = %error,
                "Failed to resolve project path: project query failed"
            );
            return None;
        }
    };
    Some(project.path)
}

/// Refresh one project's cached git status and emit sidebar update events.
///
/// Returns `Ok(true)` when branch/dirty changed from cached values.
pub(crate) fn refresh_project_git_status(
    app: &AppHandle,
    project_id: &str,
    emit_when_unchanged: bool,
) -> Result<bool, String> {
    let (project_path, cached_branch, cached_is_dirty) = {
        let db_state = app.state::<DbState>();
        let conn = db_state
            .0
            .lock()
            .map_err(|e| format!("db lock failed for project '{project_id}': {e}"))?;
        let project = db::queries::get_project(&conn, project_id)
            .map_err(|e| format!("project lookup failed for '{project_id}': {e}"))?
            .ok_or_else(|| format!("project '{project_id}' not found"))?;
        (project.path, project.cached_branch, project.cached_is_dirty)
    };

    let provider_state = app.state::<ProviderState>();
    let provider = provider_state.resolve(&project_path);
    let status = provider
        .git_status(&project_path)
        .map_err(|e| format!("git_status failed for '{project_id}' ({project_path}): {e}"))?;
    let changed = cached_branch != status.branch || cached_is_dirty != Some(status.is_dirty);

    {
        let db_state = app.state::<DbState>();
        let conn = db_state
            .0
            .lock()
            .map_err(|e| format!("db lock failed for project '{project_id}': {e}"))?;
        db::queries::update_cached_git_status(
            &conn,
            project_id,
            status.branch.as_deref(),
            status.is_dirty,
        )
        .map_err(|e| format!("cached git status update failed for '{project_id}': {e}"))?;
    }

    if emit_when_unchanged || changed {
        emit_frontend_event(
            app,
            "project-git-changed",
            serde_json::json!({
                "project_id": project_id,
                "branch": status.branch,
                "is_dirty": status.is_dirty,
            }),
        );
    }

    Ok(changed)
}

const MAX_PENDING_GIT_STATUS_RETRIES: usize = 32;
const GITIGNORE_REBUILD_MIN_INTERVAL: Duration = Duration::from_secs(10);

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

fn json_number_usize(value: usize) -> Value {
    Value::Number(serde_json::Number::from(value))
}

fn emit_watch_batch_flushed_event(
    batch_size: usize,
    file_projects: usize,
    git_projects: usize,
    session_files: usize,
    elapsed_ms: u64,
) {
    let mut fields = Map::new();
    fields.insert("batch_size".to_string(), json_number_usize(batch_size));
    fields.insert(
        "file_projects".to_string(),
        json_number_usize(file_projects),
    );
    fields.insert("git_projects".to_string(), json_number_usize(git_projects));
    fields.insert(
        "session_files".to_string(),
        json_number_usize(session_files),
    );
    fields.insert("elapsed_ms".to_string(), json_number_u64(elapsed_ms));
    crate::commands::logging::emit_global(
        "debug",
        "backend",
        "watch.batch.flushed",
        Some("Watch event batch flushed".to_string()),
        fields,
    );
}

fn emit_watch_git_status_refreshed_event(
    project_id: &str,
    retry_scheduled: bool,
    duration_ms: u64,
) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert("retry_scheduled".to_string(), Value::Bool(retry_scheduled));
    fields.insert("duration_ms".to_string(), json_number_u64(duration_ms));
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "watch.git_status.refreshed",
        Some("Git status refreshed from watcher event".to_string()),
        fields,
    );
}

fn emit_watch_git_status_refresh_failed_event(
    project_id: &str,
    retry_scheduled: bool,
    duration_ms: u64,
    error_message: &str,
) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert("retry_scheduled".to_string(), Value::Bool(retry_scheduled));
    fields.insert("duration_ms".to_string(), json_number_u64(duration_ms));
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    crate::commands::logging::emit_global(
        "warn",
        "backend",
        "watch.git_status.refresh_failed",
        Some("Git status refresh failed from watcher event".to_string()),
        fields,
    );
}

fn emit_search_file_index_updated_event(
    project_id: &str,
    docs_updated: usize,
    changed_path_count: usize,
    duration_ms: u64,
) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert("docs_updated".to_string(), json_number_usize(docs_updated));
    fields.insert(
        "changed_path_count".to_string(),
        json_number_usize(changed_path_count),
    );
    fields.insert("duration_ms".to_string(), json_number_u64(duration_ms));
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "search.file_index.updated",
        Some("Incremental file index update completed".to_string()),
        fields,
    );
}

fn emit_search_file_index_failed_event(
    project_id: &str,
    changed_path_count: usize,
    duration_ms: u64,
    error_message: &str,
) {
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert(
        "changed_path_count".to_string(),
        json_number_usize(changed_path_count),
    );
    fields.insert("duration_ms".to_string(), json_number_u64(duration_ms));
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    crate::commands::logging::emit_global(
        "warn",
        "backend",
        "search.file_index.failed",
        Some("Incremental file index update failed".to_string()),
        fields,
    );
}

fn emit_frontend_event(app: &AppHandle, event_name: &'static str, payload: Value) {
    if let Err(error) = app.emit(event_name, payload) {
        tracing::warn!(
            event_name,
            error = %error,
            "Failed to emit frontend event"
        );
    }
}

fn enqueue_task_trigger(
    tx: &std::sync::mpsc::Sender<crate::bootstrap::TaskScanTrigger>,
    trigger: crate::bootstrap::TaskScanTrigger,
    source: &'static str,
) {
    if let Err(error) = tx.send(trigger) {
        tracing::warn!(source, error = %error, "Failed to enqueue task scan trigger");
    }
}

fn pending_git_status_retries() -> &'static Mutex<HashSet<String>> {
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn last_gitignore_rebuilds() -> &'static Mutex<HashMap<String, Instant>> {
    static LAST: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reserve_git_status_retry(project_id: &str) -> bool {
    let pending = pending_git_status_retries();
    let Ok(mut pending) = pending.lock() else {
        tracing::warn!(
            project_id,
            "failed to reserve git status retry slot: pending retry lock poisoned"
        );
        return false;
    };

    if pending.contains(project_id) {
        return false;
    }

    if pending.len() >= MAX_PENDING_GIT_STATUS_RETRIES {
        tracing::warn!(
            project_id,
            pending = pending.len(),
            cap = MAX_PENDING_GIT_STATUS_RETRIES,
            "git status retry skipped due to pending retry backpressure cap"
        );
        return false;
    }

    pending.insert(project_id.to_string());
    true
}

fn release_git_status_retry(project_id: &str) {
    let pending = pending_git_status_retries();
    if let Ok(mut pending) = pending.lock() {
        pending.remove(project_id);
    }
}

#[cfg(test)]
fn clear_git_status_retry_reservations() {
    let pending = pending_git_status_retries();
    if let Ok(mut pending) = pending.lock() {
        pending.clear();
    }
}

fn should_run_gitignore_rebuild(project_id: &str) -> bool {
    let now = Instant::now();
    let last_rebuilds = last_gitignore_rebuilds();
    let Ok(mut last_rebuilds) = last_rebuilds.lock() else {
        tracing::warn!(
            project_id,
            "failed to evaluate gitignore rebuild rate limit: timestamp lock poisoned"
        );
        return true;
    };

    if let Some(last) = last_rebuilds.get(project_id) {
        if now.duration_since(*last) < GITIGNORE_REBUILD_MIN_INTERVAL {
            return false;
        }
    }

    last_rebuilds.insert(project_id.to_string(), now);
    true
}

#[cfg(test)]
fn clear_gitignore_rebuild_timestamps() {
    let last_rebuilds = last_gitignore_rebuilds();
    if let Ok(mut last_rebuilds) = last_rebuilds.lock() {
        last_rebuilds.clear();
    }
}

fn schedule_git_status_retry(app: AppHandle, project_id: String) {
    if !reserve_git_status_retry(&project_id) {
        return;
    }

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        if let Err(err) = refresh_project_git_status(&app, &project_id, true) {
            tracing::error!(
                project_id = project_id,
                error = %err,
                "git status retry failed after initial watcher refresh error"
            );
        }
        release_git_status_retry(&project_id);
    });
}

fn rebuild_project_index_for_gitignore_change(
    index: &mut search::indexer::SearchIndex,
    conn: &rusqlite::Connection,
    project_id: &str,
    project_root: &std::path::Path,
) -> Result<usize, crate::errors::AppError> {
    let (files, sessions, commits) =
        search::indexer::build_project_index(index, project_id, project_root, conn)?;
    Ok(files + sessions + commits)
}

/// One-shot git status reseed for daemon-watched (WSL) projects.
pub(crate) fn reseed_daemon_watched_git_status(app: &AppHandle) {
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(err) => {
                tracing::warn!(error = %err, "daemon reconnect reseed skipped: db lock failed");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(list) => list,
            Err(err) => {
                tracing::warn!(error = %err, "daemon reconnect reseed skipped: project list failed");
                return;
            }
        }
    };

    let mut attempted = 0usize;
    let mut changed = 0usize;
    let mut failed = 0usize;

    for project in projects
        .iter()
        .filter(|project| crate::provider::path::is_wsl_path(&project.path))
    {
        attempted += 1;
        match refresh_project_git_status(app, &project.id, false) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    project_id = project.id,
                    path = project.path,
                    error = %err,
                    "daemon reconnect git status reseed failed for project"
                );
            }
        }
    }

    tracing::info!(
        attempted = attempted,
        changed = changed,
        failed = failed,
        "daemon reconnect git status reseed finished"
    );
}

/// Process file watcher events on a background thread.
///
/// Uses **batch-and-flush** to coalesce the rapid event bursts that `notify`
/// produces for a single file edit (temp create → write → rename → cleanup).
///
/// Timing model:
/// - **Quiet window** (300 ms): if no new events arrive within this period
///   the batch is flushed. Resets with each incoming event.
/// - **Max-wait ceiling** (2 s): even if events keep arriving the batch is
///   flushed after this absolute deadline, preventing starvation.
///
/// The result: one `project-files-changed` Tauri event per edit operation
/// instead of 5–8, one search-index lock instead of 5–8, etc.
pub(crate) fn process_watch_events(
    rx: std::sync::mpsc::Receiver<fs::watcher::WatchEvent>,
    app: AppHandle,
) {
    use fs::watcher::WatchEvent;

    const QUIET_WINDOW: Duration = Duration::from_millis(300);
    const MAX_WAIT: Duration = Duration::from_millis(2000);

    // Spawn task scan thread with trailing-edge debounce (unchanged).
    let (task_trigger_tx, task_trigger_rx) =
        std::sync::mpsc::channel::<crate::bootstrap::TaskScanTrigger>();
    let app_for_tasks = app.clone();
    std::thread::spawn(move || {
        bootstrap::task_scan_loop(task_trigger_rx, app_for_tasks);
    });

    // Batch accumulators keyed by project_id. Cleared after each flush.
    struct Batch {
        file_paths: HashMap<String, Vec<std::path::PathBuf>>,
        git_projects: HashSet<String>,
        session_files: Vec<(String, std::path::PathBuf)>,
        gitignore_projects: HashSet<String>,
        activity_projects: HashSet<String>,
    }

    impl Batch {
        fn new() -> Self {
            Self {
                file_paths: HashMap::new(),
                git_projects: HashSet::new(),
                session_files: Vec::new(),
                gitignore_projects: HashSet::new(),
                activity_projects: HashSet::new(),
            }
        }

        fn is_empty(&self) -> bool {
            self.file_paths.is_empty()
                && self.git_projects.is_empty()
                && self.session_files.is_empty()
                && self.gitignore_projects.is_empty()
        }

        fn accumulate(&mut self, event: WatchEvent) {
            match event {
                WatchEvent::FileChanged { project_id, paths } => {
                    self.activity_projects.insert(project_id.clone());
                    self.file_paths.entry(project_id).or_default().extend(paths);
                }
                WatchEvent::GitChanged { project_id } => {
                    self.activity_projects.insert(project_id.clone());
                    self.git_projects.insert(project_id);
                }
                WatchEvent::SessionFileCreated { project_id, path } => {
                    self.activity_projects.insert(project_id.clone());
                    self.session_files.push((project_id, path));
                }
                WatchEvent::GitignoreChanged { project_id } => {
                    self.gitignore_projects.insert(project_id);
                }
            }
        }
    }

    // Main event loop. Block until the first event arrives or the channel closes.
    while let Ok(first) = rx.recv() {
        // Fast-path: internal watch events (task directory) bypass batching.
        if handle_internal_event(&task_trigger_tx, &first, "first_event") {
            continue;
        }

        let mut batch = Batch::new();
        batch.accumulate(first);

        // Collect more events within the batch window.
        let batch_start = Instant::now();
        loop {
            let elapsed = batch_start.elapsed();
            if elapsed >= MAX_WAIT {
                break; // ceiling reached — flush regardless
            }
            let timeout = (MAX_WAIT - elapsed).min(QUIET_WINDOW);
            match rx.recv_timeout(timeout) {
                Ok(event) => {
                    if handle_internal_event(&task_trigger_tx, &event, "batched_event") {
                    } else {
                        batch.accumulate(event);
                    }
                }
                Err(RecvTimeoutError::Timeout) => break, // quiet window or ceiling
                Err(RecvTimeoutError::Disconnected) => break, // channel closed
            }
        }

        if batch.is_empty() {
            continue;
        }

        let batch_size: usize = batch.file_paths.values().map(|v| v.len()).sum::<usize>()
            + batch.git_projects.len()
            + batch.session_files.len();
        tracing::debug!(
            batch_size,
            file_projects = batch.file_paths.len(),
            git_projects = batch.git_projects.len(),
            sessions = batch.session_files.len(),
            elapsed_ms = batch_start.elapsed().as_millis() as u64,
            "flushing watch event batch"
        );
        emit_watch_batch_flushed_event(
            batch_size,
            batch.file_paths.len(),
            batch.git_projects.len(),
            batch.session_files.len(),
            batch_start.elapsed().as_millis() as u64,
        );

        // Bump last_activity_at once per project.
        let db = app.state::<DbState>();
        match db.0.lock() {
            Ok(conn) => {
                for pid in &batch.activity_projects {
                    if let Err(e) = services::project::touch_activity(&conn, pid) {
                        tracing::warn!(project_id = pid.as_str(), error = %e, "failed to touch activity");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    projects = batch.activity_projects.len(),
                    "db lock failed for activity batch — timestamps not updated"
                );
            }
        }

        // Git events (one per project).
        for project_id in &batch.git_projects {
            let Some(project_path) = get_project_path(&app, project_id) else {
                tracing::warn!(
                    project_id = project_id.as_str(),
                    "Skipping git watcher refresh: project path lookup failed"
                );
                continue;
            };
            let path = std::path::Path::new(&project_path);

            let git_refresh_started = Instant::now();
            match refresh_project_git_status(&app, project_id, true) {
                Ok(_) => {
                    emit_watch_git_status_refreshed_event(
                        project_id,
                        false,
                        git_refresh_started.elapsed().as_millis() as u64,
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        project_id = project_id,
                        error = %err,
                        "git status refresh failed for watcher event; scheduling one retry"
                    );
                    schedule_git_status_retry(app.clone(), project_id.clone());
                    emit_watch_git_status_refresh_failed_event(
                        project_id,
                        true,
                        git_refresh_started.elapsed().as_millis() as u64,
                        &err,
                    );
                }
            }

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(project_id = project_id.as_str(), error = %e, "search index lock failed for git reindex");
                    continue;
                }
            };
            match search::indexer::reindex_commits(&mut index, project_id, path, 50) {
                Ok(count) if count > 0 => {
                    emit_frontend_event(
                        &app,
                        "search-index-updated",
                        serde_json::json!({
                            "project_id": project_id,
                            "reason": "git_changed",
                            "docs_updated": count,
                        }),
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to reindex commits on git change");
                }
                _ => {}
            }
        }

        // Session file events.
        for (project_id, path) in &batch.session_files {
            let db = app.state::<DbState>();
            let conn = match db.0.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            match services::session_import::import_handoff(&conn, project_id, path) {
                Ok(Some(session_id)) => {
                    emit_frontend_event(
                        &app,
                        "session-imported",
                        serde_json::json!({
                            "project_id": project_id,
                            "session_id": session_id,
                        }),
                    );
                    let ss = app.state::<SearchState>();
                    let mut index = match ss.0.lock() {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    match search::indexer::index_session(&mut index, project_id, &session_id, &conn)
                    {
                        Ok(true) => {
                            emit_frontend_event(
                                &app,
                                "search-index-updated",
                                serde_json::json!({
                                    "project_id": project_id,
                                    "reason": "session_imported",
                                    "docs_updated": 1,
                                }),
                            );
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to index imported session");
                        }
                        _ => {}
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to import session from watcher event"
                    );
                }
            }
        }

        // File change events (one per project, all paths merged).
        for (project_id, paths) in &batch.file_paths {
            let file_index_started = Instant::now();
            // Dedup paths within the batch (same file may appear in multiple events).
            let mut unique: Vec<&std::path::PathBuf> = paths.iter().collect();
            unique.sort();
            unique.dedup();
            let changed_path_count = unique.len();

            let path_strs: Vec<String> = unique
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            emit_frontend_event(
                &app,
                "project-files-changed",
                serde_json::json!({
                    "project_id": project_id,
                    "paths": path_strs,
                }),
            );

            // Incrementally update search index (one lock per project batch).
            let Some(project_path) = get_project_path(&app, project_id) else {
                tracing::warn!(
                    project_id = project_id.as_str(),
                    "Skipping file watcher indexing: project path lookup failed"
                );
                continue;
            };
            let project_root = std::path::Path::new(&project_path);
            let policy = {
                let db_state = app.state::<DbState>();
                let conn = match db_state.0.lock() {
                    Ok(conn) => conn,
                    Err(error) => {
                        tracing::warn!(
                            project_id = project_id.as_str(),
                            error = %error,
                            "Skipping file watcher indexing: db lock poisoned while loading scan/index policy"
                        );
                        emit_search_file_index_failed_event(
                            project_id,
                            changed_path_count,
                            file_index_started.elapsed().as_millis() as u64,
                            &error.to_string(),
                        );
                        continue;
                    }
                };
                match crate::services::scan_policy::ScanIndexPolicy::load(&conn) {
                    Ok(policy) => policy,
                    Err(error) => {
                        tracing::warn!(
                            project_id = project_id.as_str(),
                            error = %error,
                            "Skipping file watcher indexing: failed to load scan/index policy"
                        );
                        emit_search_file_index_failed_event(
                            project_id,
                            changed_path_count,
                            file_index_started.elapsed().as_millis() as u64,
                            &error.to_string(),
                        );
                        continue;
                    }
                }
            };

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(error) => {
                    tracing::warn!(
                        project_id = project_id.as_str(),
                        error = %error,
                        "Skipping file watcher indexing: search index lock poisoned"
                    );
                    emit_search_file_index_failed_event(
                        project_id,
                        changed_path_count,
                        file_index_started.elapsed().as_millis() as u64,
                        &error.to_string(),
                    );
                    continue;
                }
            };
            let mut updated = 0;
            let mut first_error: Option<String> = None;
            for path in &unique {
                match search::indexer::update_file_batched_with_scan_policy(
                    &mut index,
                    project_id,
                    project_root,
                    path,
                    &policy,
                ) {
                    Ok(true) => updated += 1,
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(e.to_string());
                        }
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to update search index for file"
                        );
                    }
                    _ => {}
                }
            }
            if updated > 0 {
                if let Err(e) = search::indexer::commit_batch(&mut index) {
                    tracing::warn!(
                        project_id = project_id.as_str(),
                        error = %e,
                        "Failed to commit batched search index update"
                    );
                    emit_search_file_index_failed_event(
                        project_id,
                        changed_path_count,
                        file_index_started.elapsed().as_millis() as u64,
                        &e.to_string(),
                    );
                    drop(index);
                    continue;
                }
            }
            drop(index);
            if let Some(error_message) = first_error {
                emit_search_file_index_failed_event(
                    project_id,
                    changed_path_count,
                    file_index_started.elapsed().as_millis() as u64,
                    &error_message,
                );
            }
            if updated > 0 {
                emit_frontend_event(
                    &app,
                    "search-index-updated",
                    serde_json::json!({
                        "project_id": project_id,
                        "reason": "file_changed",
                        "docs_updated": updated,
                    }),
                );
                emit_search_file_index_updated_event(
                    project_id,
                    updated,
                    changed_path_count,
                    file_index_started.elapsed().as_millis() as u64,
                );
            }
        }

        // Gitignore changes.
        for project_id in &batch.gitignore_projects {
            if !should_run_gitignore_rebuild(project_id) {
                tracing::debug!(
                    project_id = project_id.as_str(),
                    cooldown_secs = GITIGNORE_REBUILD_MIN_INTERVAL.as_secs(),
                    "Skipping gitignore reindex due to per-project cooldown"
                );
                continue;
            }

            let Some(project_path) = get_project_path(&app, project_id) else {
                tracing::warn!(
                    project_id = project_id.as_str(),
                    "Skipping gitignore reindex: project path lookup failed"
                );
                continue;
            };
            let project_root = std::path::Path::new(&project_path);

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(error) => {
                    tracing::warn!(
                        project_id = project_id.as_str(),
                        error = %error,
                        "Skipping gitignore reindex: search index lock poisoned"
                    );
                    continue;
                }
            };

            let db_state = app.state::<DbState>();
            let conn = match db_state.0.lock() {
                Ok(c) => c,
                Err(error) => {
                    tracing::warn!(
                        project_id = project_id.as_str(),
                        error = %error,
                        "Skipping gitignore reindex: db lock poisoned"
                    );
                    continue;
                }
            };

            match rebuild_project_index_for_gitignore_change(
                &mut index,
                &conn,
                project_id,
                project_root,
            ) {
                Ok(updated) => {
                    tracing::info!(
                        project_id = project_id.as_str(),
                        docs_updated = updated,
                        "gitignore changed — rebuilt project search index"
                    );
                    emit_frontend_event(
                        &app,
                        "search-index-updated",
                        serde_json::json!({
                            "project_id": project_id,
                            "reason": "gitignore_changed",
                            "docs_updated": updated,
                        }),
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        project_id = project_id.as_str(),
                        error = %error,
                        "Failed to rebuild search index on gitignore change"
                    );
                }
            }
        }
    }
}

fn internal_task_trigger(event: &fs::watcher::WatchEvent) -> crate::bootstrap::TaskScanTrigger {
    use fs::watcher::WatchEvent;
    match event {
        WatchEvent::FileChanged { project_id, paths } if project_id == CLAUDE_TASKS_PROJECT_ID => {
            crate::bootstrap::TaskScanTrigger::ClaudeTaskPaths(paths.clone())
        }
        _ => crate::bootstrap::TaskScanTrigger::Full,
    }
}

fn handle_internal_event(
    task_trigger_tx: &std::sync::mpsc::Sender<crate::bootstrap::TaskScanTrigger>,
    event: &fs::watcher::WatchEvent,
    source: &'static str,
) -> bool {
    use fs::watcher::WatchEvent;

    match event {
        WatchEvent::FileChanged { project_id, .. } if project_id == CLAUDE_TASKS_PROJECT_ID => {
            enqueue_task_trigger(task_trigger_tx, internal_task_trigger(event), source);
            true
        }
        WatchEvent::GitChanged { project_id }
            if project_id.starts_with(INTERNAL_PROJECT_ID_PREFIX) =>
        {
            enqueue_task_trigger(task_trigger_tx, internal_task_trigger(event), source);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use pretty_assertions::assert_eq;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn retry_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn setup_project_db(
        project_id: &str,
        project_root: &std::path::Path,
    ) -> (rusqlite::Connection, TempDir) {
        let db_dir = TempDir::new().expect("temp db dir");
        let db_path = db_dir.path().join("taurhaus.db");
        let conn = crate::db::init_db(&db_path).expect("init db");
        let project = crate::models::Project {
            id: project_id.to_string(),
            name: "test-project".to_string(),
            path: project_root.to_string_lossy().to_string(),
            description: None,
            last_activity_at: Some("2026-03-05T00:00:00Z".to_string()),
            hero_preference: None,
            created_at: "2026-03-05T00:00:00Z".to_string(),
            updated_at: "2026-03-05T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
            claude_account_id: None,
        };
        crate::db::queries::insert_project(&conn, &project).expect("insert project");
        (conn, db_dir)
    }

    fn wait_for_lines(path: &std::path::Path, expected: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.to_string())
                    .collect();
                if lines.len() >= expected {
                    return lines;
                }
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for event_processor log lines at {}",
                    path.display()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn gitignore_reindex_prunes_newly_ignored_files() {
        let project_id = "p1";
        let temp = TempDir::new().expect("temp project");
        let project_root = temp.path();

        std::fs::write(project_root.join("keep.md"), "visibletoken").expect("write keep file");
        std::fs::write(project_root.join("secret.md"), "hiddentoken").expect("write secret file");
        git2::Repository::init(project_root).expect("init git repo");

        let (conn, _db_dir) = setup_project_db(project_id, project_root);
        let mut index = search::indexer::SearchIndex::open_in_memory().expect("open index");

        let initial_docs =
            rebuild_project_index_for_gitignore_change(&mut index, &conn, project_id, project_root)
                .expect("initial index");
        assert_eq!(initial_docs, 2);
        assert_eq!(index.doc_count().expect("doc count"), 2);
        assert_eq!(
            index
                .search("hiddentoken", 10)
                .expect("search hidden")
                .len(),
            1
        );

        std::fs::write(project_root.join(".gitignore"), "secret.md\n").expect("write gitignore");

        let rebuilt_docs =
            rebuild_project_index_for_gitignore_change(&mut index, &conn, project_id, project_root)
                .expect("rebuild after gitignore");
        assert_eq!(rebuilt_docs, 1);
        assert_eq!(index.doc_count().expect("doc count"), 1);

        let hidden_results = index.search("hiddentoken", 10).expect("search hidden");
        assert!(
            hidden_results.is_empty(),
            "stale ignored file should be removed from index"
        );

        let visible_results = index.search("visibletoken", 10).expect("search visible");
        assert_eq!(visible_results.len(), 1);
        assert_eq!(visible_results[0].file_path, "keep.md");
    }

    #[test]
    fn git_status_retry_reservation_deduplicates_per_project() {
        let _guard = retry_test_lock().lock().expect("retry test lock");
        clear_git_status_retry_reservations();
        release_git_status_retry("retry-dedupe");
        assert!(reserve_git_status_retry("retry-dedupe"));
        assert!(!reserve_git_status_retry("retry-dedupe"));
        release_git_status_retry("retry-dedupe");
        assert!(reserve_git_status_retry("retry-dedupe"));
        release_git_status_retry("retry-dedupe");
        clear_git_status_retry_reservations();
    }

    #[test]
    fn git_status_retry_reservation_enforces_backpressure_cap() {
        let _guard = retry_test_lock().lock().expect("retry test lock");
        clear_git_status_retry_reservations();
        let mut ids = Vec::new();
        for idx in 0..MAX_PENDING_GIT_STATUS_RETRIES {
            let project_id = format!("retry-cap-{idx}");
            assert!(reserve_git_status_retry(&project_id));
            ids.push(project_id);
        }

        assert!(!reserve_git_status_retry("retry-cap-overflow"));

        for project_id in ids {
            release_git_status_retry(&project_id);
        }
        release_git_status_retry("retry-cap-overflow");
        clear_git_status_retry_reservations();
    }

    #[test]
    fn gitignore_rebuild_rate_limit_is_enforced_per_project() {
        clear_gitignore_rebuild_timestamps();
        assert!(should_run_gitignore_rebuild("p1"));
        assert!(!should_run_gitignore_rebuild("p1"));
        assert!(should_run_gitignore_rebuild("p2"));
        clear_gitignore_rebuild_timestamps();
    }

    #[test]
    fn emits_structured_watch_batch_and_git_status_events() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let log_dir = tempfile::TempDir::new().expect("temp log dir");
        let log_path = log_dir.path().join("watch-events.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        emit_watch_batch_flushed_event(7, 2, 1, 3, 250);
        emit_watch_git_status_refreshed_event("p1", false, 17);
        emit_watch_git_status_refresh_failed_event("p2", true, 42, "git status failed");

        let lines = wait_for_lines(&log_path, 3);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();

        let batch = events
            .iter()
            .find(|value| value["event"] == "watch.batch.flushed")
            .expect("watch.batch.flushed");
        assert_eq!(batch["batch_size"], 7);
        assert_eq!(batch["file_projects"], 2);
        assert_eq!(batch["git_projects"], 1);

        let refreshed = events
            .iter()
            .find(|value| value["event"] == "watch.git_status.refreshed")
            .expect("watch.git_status.refreshed");
        assert_eq!(refreshed["project_id"], "p1");
        assert_eq!(refreshed["retry_scheduled"], false);

        let failed = events
            .iter()
            .find(|value| value["event"] == "watch.git_status.refresh_failed")
            .expect("watch.git_status.refresh_failed");
        assert_eq!(failed["project_id"], "p2");
        assert_eq!(failed["retry_scheduled"], true);
        assert_eq!(failed["error.message"], "git status failed");
    }

    #[test]
    fn emits_structured_search_file_index_events() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let log_dir = tempfile::TempDir::new().expect("temp log dir");
        let log_path = log_dir.path().join("search-file-index-events.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        emit_search_file_index_updated_event("p-search", 4, 9, 120);
        emit_search_file_index_failed_event("p-search", 9, 140, "index lock poisoned");

        let lines = wait_for_lines(&log_path, 2);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();

        let updated = events
            .iter()
            .find(|value| value["event"] == "search.file_index.updated")
            .expect("search.file_index.updated");
        assert_eq!(updated["project_id"], "p-search");
        assert_eq!(updated["docs_updated"], 4);
        assert_eq!(updated["changed_path_count"], 9);

        let failed = events
            .iter()
            .find(|value| value["event"] == "search.file_index.failed")
            .expect("search.file_index.failed");
        assert_eq!(failed["project_id"], "p-search");
        assert_eq!(failed["changed_path_count"], 9);
        assert_eq!(failed["error.message"], "index lock poisoned");
    }
}
