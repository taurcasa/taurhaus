use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::commands::projects::DbState;
use crate::{db, provider, search, services, ProviderState, SearchState};

/// On startup, re-seed last_activity_at from each project's latest git commit.
/// This corrects projects whose activity timestamp was incorrectly set to
/// registration time instead of actual last-commit time.
///
/// IMPORTANT: The DB lock is released between projects so frontend IPC commands
/// are not blocked during slow git operations (especially over the daemon).
pub(crate) fn startup_reseed_activity(app: &AppHandle) {
    // Snapshot the project list, then release the DB lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for activity reseed: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for activity reseed: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let provider_state = app.state::<ProviderState>();
    let db_state = app.state::<DbState>();

    let mut updated = 0;
    for project in &projects {
        let provider = provider_state.resolve(&project.path);

        // Do git I/O WITHOUT holding the DB lock
        let git_status = provider.git_status(&project.path).ok();
        let commit_time = provider.latest_commit_time(&project.path).ok().flatten();

        // Brief DB lock per project to write results
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(status) = git_status {
            let _ = db::queries::update_cached_git_status(
                &conn,
                &project.id,
                status.branch.as_deref(),
                status.is_dirty,
            );
        }

        if let Some(commit_time) = commit_time {
            let commit_ts = commit_time.to_rfc3339();
            if project.last_activity_at.as_deref() != Some(&commit_ts) {
                let _ = db::queries::update_project(
                    &conn,
                    &project.id,
                    None,
                    None,
                    None,
                    Some(Some(&commit_ts)),
                    None,
                );
                updated += 1;
            }
        }
        // conn lock dropped here — frontend can interleave
    }

    if updated > 0 {
        tracing::info!(updated, "Re-seeded activity timestamps from git");
    }

    // Notify frontend that cached git data is now fresh — it may have loaded
    // the project list before the reseed completed (race on first launch).
    let _ = app.emit("projects-reseed-complete", ());
}

/// On startup, build the search index if it's empty.
///
/// Only holds locks briefly: checks doc count with search lock, then acquires
/// both locks for the rebuild if needed. The rebuild is a one-time operation
/// (subsequent startups skip it), so the longer hold is acceptable.
pub(crate) fn startup_search_index(app: &AppHandle) {
    // Check if index is already populated — brief lock
    {
        let search_state = app.state::<SearchState>();
        let index = match search_state.0.lock() {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("Failed to lock search index for startup build: {e}");
                return;
            }
        };

        let doc_count = index.doc_count().unwrap_or(0);
        if doc_count > 0 {
            tracing::info!(
                doc_count,
                "Search index already populated, skipping rebuild"
            );
            return;
        }
        // search lock dropped here
    }

    // Index is empty — need to rebuild. This holds both locks but only happens
    // on first run (or after index wipe), so the brief block is acceptable.
    let search_state = app.state::<SearchState>();
    let mut index = match search_state.0.lock() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("Failed to lock search index for rebuild: {e}");
            return;
        }
    };

    let db_state = app.state::<DbState>();
    let conn = match db_state.0.lock() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to lock DB for startup index build: {e}");
            return;
        }
    };

    match search::indexer::rebuild_all(&mut index, &conn) {
        Ok(total) => {
            tracing::info!(total, "Built initial search index");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build initial search index");
        }
    }
}

/// On startup, scan all registered projects for unimported session handoffs.
///
/// IMPORTANT: DB lock released between projects to avoid blocking frontend IPC.
pub(crate) fn startup_session_scan(app: &AppHandle) {
    // Snapshot project list, release lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for startup session scan: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for startup scan: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let db_state = app.state::<DbState>();

    for project in &projects {
        let project_root = std::path::Path::new(&project.path);

        // Brief lock per project for the import operation
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => continue,
        };

        match services::session_import::scan_and_import_sessions(&conn, &project.id, project_root) {
            Ok(imported) if !imported.is_empty() => {
                tracing::info!(
                    project = project.name,
                    count = imported.len(),
                    "Imported sessions on startup"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    project = project.name,
                    error = %e,
                    "Failed to scan sessions on startup"
                );
            }
        }
        // conn lock dropped here — frontend can interleave
    }
}

/// On startup, scan all registered projects' tasks and seed the SQLite database.
///
/// This ensures the first frontend read has data. Subsequent updates are
/// event-driven (daemon watches `~/.claude/tasks/`).
pub(crate) fn startup_task_scan(app: &AppHandle) {
    sync_all_project_tasks(app);
}

/// Background thread that handles task re-scanning with trailing-edge debounce.
///
/// Waits for a trigger signal (from file watcher events), then drains additional
/// signals for 2 seconds. After the debounce window closes, scans all projects'
/// tasks and persists to SQLite. This ensures rapid task file changes (e.g.,
/// Claude creating 4 tasks at once) result in only one scan.
pub(crate) fn task_scan_loop(rx: std::sync::mpsc::Receiver<()>, app: AppHandle) {
    use std::time::{Duration, Instant};
    const DEBOUNCE: Duration = Duration::from_secs(2);

    loop {
        // Wait for first trigger
        if rx.recv().is_err() {
            break;
        }

        // Trailing-edge debounce: drain for 2 seconds
        let deadline = Instant::now() + DEBOUNCE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(()) => {} // More triggers, keep draining
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }

        // Scan all projects' tasks
        sync_all_project_tasks(&app);
    }
}

/// Scan tasks for all registered projects, persist to SQLite, and notify frontend.
///
/// Called from both the startup seed and the event-driven scan loop.
pub(crate) fn sync_all_project_tasks(app: &AppHandle) {
    let db_state = app.state::<DbState>();
    let provider_state = app.state::<ProviderState>();

    // Snapshot project list (brief DB lock)
    let projects = {
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        db::queries::list_projects(&conn).unwrap_or_default()
    };

    let mut total_tasks = 0;
    for project in &projects {
        // Scan tasks from files (daemon or local)
        let scan_result = commands::tasks::scan_tasks_from_files(&provider_state, &project.path);

        if scan_result.tasks.is_empty() {
            continue;
        }

        // Normalize path for DB storage
        let normalized_path =
            provider::path::to_linux(&project.path).unwrap_or_else(|| project.path.clone());

        // Persist to SQLite (brief DB lock per project)
        {
            let conn = match db_state.0.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            commands::tasks::persist_task_scan(&conn, &normalized_path, &scan_result);
        }

        total_tasks += scan_result.tasks.len();

        // Emit per-project event to frontend
        let _ = app.emit(
            "project-tasks-changed",
            serde_json::json!({
                "project_id": project.id,
                "task_count": scan_result.tasks.len(),
            }),
        );
    }

    if total_tasks > 0 {
        tracing::debug!(total_tasks, projects = projects.len(), "Task sync complete");
    }
}
