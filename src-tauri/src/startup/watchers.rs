use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

use crate::commands::projects::DbState;
use crate::db::settings_queries;
use crate::models::ActivityThresholds;
use crate::sentinels::{
    CLAUDE_TASKS_PROJECT_ID, INTERNAL_PROJECT_ID_PREFIX, TMUX_FOCUS_PROJECT_ID,
};
use crate::{
    daemon_lifecycle, db, event_processor, platform, provider, watch_targets, WatcherState,
};

use super::SetupContext;

static RECONCILE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct ReconcileInProgressGuard;

impl Drop for ReconcileInProgressGuard {
    fn drop(&mut self) {
        RECONCILE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

fn should_watch_locally(project_path: &str, has_daemon: bool, defer_wsl_to_daemon: bool) -> bool {
    if provider::path::is_wsl_path(project_path) {
        return !has_daemon && !defer_wsl_to_daemon;
    }

    true
}

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
            let thresholds = settings_queries::get_all_settings(&db_projects_guard)
                .map(|settings| settings.thresholds)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        error = %error,
                        "Failed to load activity thresholds for daemon watch bootstrap; using defaults"
                    );
                    ActivityThresholds::default()
                });

            std::thread::spawn(move || {
                daemon_lifecycle::start_daemon_watches(
                    daemon_addr,
                    event_tx_clone,
                    distro,
                    db_projects,
                    thresholds,
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

    let startup_reconcile_handle = app.handle().clone();
    std::thread::spawn(move || {
        reconcile_activity_watches(&startup_reconcile_handle, "startup");
    });
    ensure_task_directory_watch(app, context.daemon_connected_at_startup);
    ensure_tmux_focus_watch(app, &context.data_dir);

    let periodic_handle = app.handle().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        reconcile_activity_watches(&periodic_handle, "periodic");
    });

    Ok(())
}

pub(crate) fn reconcile_activity_watches(app: &tauri::AppHandle, reason: &str) {
    if RECONCILE_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        tracing::debug!(
            reason,
            "Skipping activity watch reconcile because another run is in progress"
        );
        return;
    }
    let _in_progress_guard = ReconcileInProgressGuard;

    let (projects, thresholds, has_daemon, defer_wsl_to_daemon) = {
        let db_state = app.state::<DbState>();
        let db_guard = match db_state.0.lock() {
            Ok(guard) => guard,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reason,
                    "DB lock poisoned while reconciling activity watches; recovering"
                );
                error.into_inner()
            }
        };
        let projects = db::queries::list_projects(&db_guard).unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Failed to list projects while reconciling activity watches"
            );
            Vec::new()
        });
        let thresholds = settings_queries::get_all_settings(&db_guard)
            .map(|settings| settings.thresholds)
            .unwrap_or_else(|error| {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Failed to load activity thresholds while reconciling activity watches; using defaults"
                );
                ActivityThresholds::default()
            });

        let provider_state = app.state::<crate::ProviderState>();
        let has_daemon = provider_state
            .daemon
            .as_ref()
            .is_some_and(|daemon| daemon.is_connected());
        let defer_wsl_to_daemon =
            provider_state.wsl_distro.is_some() && !crate::daemon::launcher::is_native_daemon();
        (projects, thresholds, has_daemon, defer_wsl_to_daemon)
    };

    let (watched, unwatched, watch_limit_hit) = {
        let watcher_state = app.state::<WatcherState>();
        let mut watcher_guard = match watcher_state.0.lock() {
            Ok(guard) => guard,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Watcher lock poisoned while reconciling activity watches; recovering"
                );
                error.into_inner()
            }
        };
        let watched_ids: HashSet<String> = watcher_guard.watched_projects().into_iter().collect();

        let planned_targets = watch_targets::plan_activity_watch_targets(&projects, &thresholds);
        let mut by_id: HashMap<String, watch_targets::ActivityWatchTarget> = HashMap::new();
        for target in planned_targets {
            by_id.insert(target.project_id.clone(), target);
        }

        let mut watched = 0usize;
        let mut unwatched = 0usize;
        let mut watch_limit_hit = false;

        for (project_id, target) in &by_id {
            if !should_watch_locally(&target.project_path, has_daemon, defer_wsl_to_daemon) {
                continue;
            }

            let should_watch = target.should_watch;
            let is_watched = watched_ids.contains(project_id);
            let path = std::path::Path::new(&target.project_path);
            let can_watch_path = path.is_dir();

            if should_watch && can_watch_path && !is_watched {
                match watcher_guard.watch_project(project_id.clone(), path.to_path_buf()) {
                    Ok(()) => watched += 1,
                    Err(error) => {
                        let msg = error.to_string();
                        if platform::is_watch_limit_error(&msg) {
                            tracing::warn!(
                                project = target.project_name,
                                error = %error,
                                reason,
                                "Watch limit reached — skipping project"
                            );
                            watch_limit_hit = true;
                        } else {
                            tracing::debug!(
                                project = target.project_name,
                                error = %error,
                                reason,
                                "Could not watch project directory (local)"
                            );
                        }
                    }
                }
                continue;
            }

            if is_watched && (!should_watch || !can_watch_path) {
                watcher_guard.unwatch_project(project_id);
                unwatched += 1;
            }
        }

        for watched_id in watched_ids {
            if watched_id.starts_with(INTERNAL_PROJECT_ID_PREFIX) {
                continue;
            }
            if by_id.contains_key(&watched_id) {
                continue;
            }
            watcher_guard.unwatch_project(&watched_id);
            unwatched += 1;
        }

        (watched, unwatched, watch_limit_hit)
    };

    let logical_watch_subscriptions = {
        let watcher_state = app.state::<WatcherState>();
        let watcher_guard = match watcher_state.0.lock() {
            Ok(guard) => guard,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Watcher lock poisoned while collecting inotify telemetry; recovering"
                );
                error.into_inner()
            }
        };
        watcher_guard.watched_projects().len()
    };
    crate::inotify_diagnostics::emit_app_telemetry(reason, logical_watch_subscriptions);

    if watched > 0 || unwatched > 0 {
        tracing::info!(
            watched,
            unwatched,
            reason,
            "Reconciled local project watches from activity state"
        );
    }
    daemon_lifecycle::reconcile_daemon_activity_watches(app, &projects, &thresholds, reason);

    if watch_limit_hit {
        tracing::warn!(
            "Some projects could not be watched — watch limit reached. \
             File changes in those projects won't be detected. {}",
            platform::watch_limit_help()
        );
    }
}

fn ensure_task_directory_watch(app: &tauri::App, has_daemon: bool) {
    let watcher_state = app.state::<WatcherState>();
    let mut watcher_guard = watcher_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Watcher lock poisoned while bootstrapping task directory watch; recovering"
        );
        error.into_inner()
    });
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

fn ensure_tmux_focus_watch(app: &tauri::App, data_dir: &std::path::Path) {
    let watcher_state = app.state::<WatcherState>();
    let mut watcher_guard = watcher_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Watcher lock poisoned while bootstrapping tmux focus watch; recovering"
        );
        error.into_inner()
    });
    let focus_path = crate::session_scanner::tmux::focus_file_path(data_dir);
    if !focus_path.exists() {
        if let Err(error) = crate::session_scanner::tmux::write_focus_state(
            &focus_path,
            &crate::session_scanner::tmux::TmuxFocusState::detached(),
        ) {
            tracing::warn!(
                error = %error,
                path = %focus_path.display(),
                "Failed to initialize tmux focus file before watch registration"
            );
            return;
        }
    }
    crate::session_scanner::control::ensure_tmux_focus_hooks_for_path(&focus_path);
    if let Err(error) =
        watcher_guard.watch_file(TMUX_FOCUS_PROJECT_ID.to_string(), focus_path.clone())
    {
        tracing::debug!(
            error = %error,
            path = %focus_path.display(),
            "Could not watch tmux focus file"
        );
    } else {
        tracing::info!(
            path = %focus_path.display(),
            "Watching tmux focus file"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::should_watch_locally;

    #[test]
    fn local_watch_skips_wsl_paths_when_daemon_is_connected() {
        assert!(!should_watch_locally(
            r"\\wsl$\Ubuntu\home\mstie\projects\taurhaus",
            true,
            true
        ));
    }

    #[test]
    fn local_watch_skips_wsl_paths_while_waiting_for_wsl_daemon() {
        assert!(!should_watch_locally(
            r"\\wsl$\Ubuntu\home\mstie\projects\taurhaus",
            false,
            true
        ));
    }

    #[test]
    fn local_watch_allows_wsl_paths_only_when_no_daemon_path_exists() {
        assert!(should_watch_locally(
            r"\\wsl$\Ubuntu\home\mstie\projects\taurhaus",
            false,
            false
        ));
    }

    #[test]
    fn local_watch_allows_normal_local_projects() {
        assert!(should_watch_locally(
            r"C:\Users\mstie\projects\taurhaus",
            false,
            true
        ));
    }
}
