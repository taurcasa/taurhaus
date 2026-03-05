use std::collections::{HashMap, HashSet};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::bootstrap;
use crate::commands::projects::DbState;
use crate::{db, fs, search, services, ProviderState, SearchState};

/// Look up a project's path from the database, returning None on any error.
pub(crate) fn get_project_path(app: &AppHandle, project_id: &str) -> Option<String> {
    let db_state = app.state::<DbState>();
    let conn = db_state.0.lock().ok()?;
    let project = db::queries::get_project(&conn, project_id).ok()??;
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
        let _ = app.emit(
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

fn schedule_git_status_retry(app: AppHandle, project_id: String) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        if let Err(err) = refresh_project_git_status(&app, &project_id, true) {
            tracing::error!(
                project_id = project_id,
                error = %err,
                "git status retry failed after initial watcher refresh error"
            );
        }
    });
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

    // Main event loop.
    loop {
        // Block until the first event arrives.
        let Ok(first) = rx.recv() else {
            break; // channel closed
        };

        // Fast-path: internal watch events (task directory) bypass batching.
        if is_internal_event(&first) {
            let _ = task_trigger_tx.send(internal_task_trigger(&first));
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
                    if is_internal_event(&event) {
                        let _ = task_trigger_tx.send(internal_task_trigger(&event));
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
                continue;
            };
            let path = std::path::Path::new(&project_path);

            if let Err(err) = refresh_project_git_status(&app, project_id, true) {
                tracing::warn!(
                    project_id = project_id,
                    error = %err,
                    "git status refresh failed for watcher event; scheduling one retry"
                );
                schedule_git_status_retry(app.clone(), project_id.clone());
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
                    let _ = app.emit(
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
                    let _ = app.emit(
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
                            let _ = app.emit(
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
            // Dedup paths within the batch (same file may appear in multiple events).
            let mut unique: Vec<&std::path::PathBuf> = paths.iter().collect();
            unique.sort();
            unique.dedup();

            let path_strs: Vec<String> = unique
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            let _ = app.emit(
                "project-files-changed",
                serde_json::json!({
                    "project_id": project_id,
                    "paths": path_strs,
                }),
            );

            // Incrementally update search index (one lock per project batch).
            let Some(project_path) = get_project_path(&app, project_id) else {
                continue;
            };
            let project_root = std::path::Path::new(&project_path);

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(_) => continue,
            };
            let mut updated = 0;
            for path in &unique {
                match search::indexer::update_file_batched(
                    &mut index,
                    project_id,
                    project_root,
                    path,
                ) {
                    Ok(true) => updated += 1,
                    Err(e) => {
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
                    drop(index);
                    continue;
                }
            }
            drop(index);
            if updated > 0 {
                let _ = app.emit(
                    "search-index-updated",
                    serde_json::json!({
                        "project_id": project_id,
                        "reason": "file_changed",
                        "docs_updated": updated,
                    }),
                );
            }
        }

        // Gitignore changes.
        for project_id in &batch.gitignore_projects {
            tracing::info!(
                project_id,
                "gitignore changed — watch rebuild not yet implemented"
            );
        }
    }
}

fn internal_task_trigger(event: &fs::watcher::WatchEvent) -> crate::bootstrap::TaskScanTrigger {
    use fs::watcher::WatchEvent;
    match event {
        WatchEvent::FileChanged { project_id, paths } if project_id == "__claude_tasks__" => {
            crate::bootstrap::TaskScanTrigger::ClaudeTaskPaths(paths.clone())
        }
        _ => crate::bootstrap::TaskScanTrigger::Full,
    }
}

/// Check if a watch event is an internal event (task directory etc.) that
/// should bypass batching and trigger the task scan thread instead.
pub(crate) fn is_internal_event(event: &fs::watcher::WatchEvent) -> bool {
    use fs::watcher::WatchEvent;
    matches!(
        event,
        WatchEvent::FileChanged { project_id, .. }
        | WatchEvent::GitChanged { project_id }
            if project_id.starts_with("__")
    )
}
