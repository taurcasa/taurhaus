use std::sync::Mutex;

use tauri::Manager;

use crate::commands::projects::DbState;
use crate::sentinels::CLAUDE_TASKS_PROJECT_ID;
use crate::{daemon_lifecycle, db, event_processor, platform, provider, WatcherState};

use super::SetupContext;

pub(crate) fn initialize(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let (watcher, rx) = crate::fs::watcher::ProjectWatcher::new();
    let event_tx = watcher.event_sender();
    app.manage(WatcherState(Mutex::new(watcher)));

    if context.daemon_connected_at_startup {
        if let Some(daemon_addr) = context.daemon_addr.clone() {
            let distro = context.wsl_distro.clone();
            let event_tx_clone = event_tx.clone();
            let db_state = app.state::<DbState>();
            let db_projects_guard = db_state.0.lock().unwrap_or_else(|error| {
                tracing::warn!(
                    error = %error,
                    "DB lock poisoned while collecting projects for daemon watch bootstrap; recovering"
                );
                error.into_inner()
            });
            let db_projects =
                db::queries::list_projects(&db_projects_guard).unwrap_or_else(|error| {
                    tracing::warn!(
                        error = %error,
                        "Failed to list projects for daemon watch bootstrap"
                    );
                    Vec::new()
                });

            std::thread::spawn(move || {
                daemon_lifecycle::start_daemon_watches(
                    daemon_addr,
                    event_tx_clone,
                    distro,
                    db_projects,
                );
            });
        } else {
            tracing::warn!(
                "Daemon reported as connected at startup but daemon address was missing; skipping daemon watch bootstrap"
            );
        }
    }

    let handle = app.handle().clone();
    std::thread::spawn(move || {
        event_processor::process_watch_events(rx, handle);
    });

    register_local_watches(app, context.daemon_connected_at_startup);
    Ok(())
}

fn register_local_watches(app: &mut tauri::App, has_daemon: bool) {
    let db_state = app.state::<DbState>();
    let db_guard = db_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "DB lock poisoned while collecting projects for local watch bootstrap; recovering"
        );
        error.into_inner()
    });
    let projects = db::queries::list_projects(&db_guard).unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Failed to list projects for local watch bootstrap"
        );
        Vec::new()
    });

    let watcher_state = app.state::<WatcherState>();
    let mut watcher_guard = watcher_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Watcher lock poisoned while bootstrapping local watches; recovering"
        );
        error.into_inner()
    });

    let mut count = 0;
    let mut watch_limit_hit = false;
    for project in &projects {
        if has_daemon && provider::path::is_wsl_path(&project.path) {
            continue;
        }

        let path = std::path::Path::new(&project.path);
        if path.is_dir() {
            match watcher_guard.watch_project(project.id.clone(), path.to_path_buf()) {
                Ok(()) => count += 1,
                Err(error) => {
                    let msg = error.to_string();
                    if platform::is_watch_limit_error(&msg) {
                        tracing::warn!(
                            project = project.name,
                            error = %error,
                            "Watch limit reached — skipping project"
                        );
                        watch_limit_hit = true;
                    } else {
                        tracing::debug!(
                            project = project.name,
                            error = %error,
                            "Could not watch project directory (local)"
                        );
                    }
                }
            }
        }
    }

    if count > 0 {
        tracing::info!(count, "Watching project directories (local)");
    }
    if watch_limit_hit {
        tracing::warn!(
            "Some projects could not be watched — watch limit reached. \
             File changes in those projects won't be detected. {}",
            platform::watch_limit_help()
        );
    }

    if !has_daemon || crate::daemon::launcher::is_native_daemon() {
        if let Some(tasks_dir) = super::resolve_claude_tasks_dir() {
            if tasks_dir.is_dir() {
                if let Err(error) = watcher_guard
                    .watch_project(CLAUDE_TASKS_PROJECT_ID.to_string(), tasks_dir.clone())
                {
                    tracing::debug!(
                        error = %error,
                        path = %tasks_dir.display(),
                        "Could not watch Claude tasks directory"
                    );
                } else {
                    tracing::info!(
                        path = %tasks_dir.display(),
                        "Watching Claude tasks directory (local)"
                    );
                }
            }
        }
    }
}
