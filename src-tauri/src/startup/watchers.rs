use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Map, Value};
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

fn should_watch_claude_tasks_locally(
    has_daemon: bool,
    native_daemon: bool,
    prefer_local_when_daemon_connected: bool,
    allow_disconnected_windows_fallback: bool,
) -> bool {
    native_daemon
        || prefer_local_when_daemon_connected
        || (!has_daemon && allow_disconnected_windows_fallback)
}

pub(crate) fn initialize(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let (watcher, rx) = crate::fs::watcher::ProjectWatcher::new();
    app.manage(WatcherState(Mutex::new(watcher)));

    let handle = app.handle().clone();
    std::thread::spawn(move || {
        event_processor::process_watch_events(rx, handle);
    });

    let startup_reconcile_handle = app.handle().clone();
    std::thread::spawn(move || {
        reconcile_activity_watches(&startup_reconcile_handle, "startup");
    });
    spawn_auxiliary_watch_bootstrap(
        app.handle().clone(),
        context.data_dir.clone(),
        context.daemon_connected_at_startup,
        context.wsl_distro.is_none() || crate::daemon::launcher::is_native_daemon(),
    );

    let periodic_handle = app.handle().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        reconcile_activity_watches(&periodic_handle, "periodic");
    });

    Ok(())
}

fn spawn_auxiliary_watch_bootstrap(
    app: tauri::AppHandle,
    data_dir: std::path::PathBuf,
    has_daemon: bool,
    allow_disconnected_windows_fallback: bool,
) {
    spawn_auxiliary_watch_bootstrap_task(move || {
        refresh_auxiliary_watches(
            &app,
            &data_dir,
            has_daemon,
            allow_disconnected_windows_fallback,
            "startup",
        );
    });
}

pub(crate) fn refresh_auxiliary_watches(
    app: &tauri::AppHandle,
    data_dir: &std::path::Path,
    has_daemon: bool,
    allow_disconnected_windows_fallback: bool,
    reason: &'static str,
) {
    let started_at = std::time::Instant::now();
    emit_watch_bootstrap_event(
        "info",
        "startup.watchers.bootstrap.started",
        "Startup auxiliary watcher bootstrap started",
        {
            let mut fields = Map::new();
            fields.insert("reason".to_string(), Value::String(reason.to_string()));
            fields
        },
    );
    ensure_task_directory_watch(app, has_daemon, allow_disconnected_windows_fallback);
    ensure_tmux_focus_watch(app, data_dir);

    let mut fields = Map::new();
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(
            started_at.elapsed().as_millis() as u64
        )),
    );
    emit_watch_bootstrap_event(
        "info",
        "startup.watchers.bootstrap.completed",
        "Startup auxiliary watcher bootstrap completed",
        fields,
    );
}

fn spawn_auxiliary_watch_bootstrap_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    std::thread::spawn(task);
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

fn ensure_task_directory_watch<T, R>(
    app: &T,
    has_daemon: bool,
    allow_disconnected_windows_fallback: bool,
) where
    R: tauri::Runtime,
    T: Manager<R>,
{
    let watcher_state = app.state::<WatcherState>();
    let mut watcher_guard = watcher_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Watcher lock poisoned while bootstrapping task directory watch; recovering"
        );
        error.into_inner()
    });
    if let Some(tasks_dir) = super::resolve_claude_tasks_dir() {
        let prefer_local_when_daemon_connected = cfg!(target_os = "windows") && tasks_dir.is_dir();
        if tasks_dir.is_dir()
            && should_watch_claude_tasks_locally(
                has_daemon,
                crate::daemon::launcher::is_native_daemon(),
                prefer_local_when_daemon_connected,
                allow_disconnected_windows_fallback,
            )
        {
            if let Err(error) =
                watcher_guard.watch_project(CLAUDE_TASKS_PROJECT_ID.to_string(), tasks_dir.clone())
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
        } else {
            watcher_guard.unwatch_project(CLAUDE_TASKS_PROJECT_ID);
        }
    }
}

fn ensure_tmux_focus_watch<T, R>(app: &T, data_dir: &std::path::Path)
where
    R: tauri::Runtime,
    T: Manager<R>,
{
    let watcher_state = app.state::<WatcherState>();
    let mut watcher_guard = watcher_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Watcher lock poisoned while bootstrapping tmux focus watch; recovering"
        );
        error.into_inner()
    });
    let focus_path = crate::session_scanner::tmux::focus_file_path(data_dir);
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
    crate::session_scanner::control::remove_legacy_tmux_focus_hooks();
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

fn emit_watch_bootstrap_event(
    level: &str,
    event: &str,
    message: &'static str,
    fields: Map<String, Value>,
) {
    crate::commands::logging::emit_global(
        level,
        "backend",
        event,
        Some(message.to_string()),
        fields,
    );
}

#[cfg(test)]
mod tests {
    use super::should_watch_claude_tasks_locally;
    use super::should_watch_locally;
    use super::spawn_auxiliary_watch_bootstrap_task;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

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

    #[test]
    fn claude_tasks_local_watch_prefers_windows_accessible_path_even_with_daemon() {
        assert!(should_watch_claude_tasks_locally(true, false, true, false));
    }

    #[test]
    fn claude_tasks_local_watch_skips_non_native_daemon_when_no_windows_fallback() {
        assert!(!should_watch_claude_tasks_locally(
            true, false, false, false
        ));
    }

    #[test]
    fn claude_tasks_local_watch_skips_startup_windows_fallback_while_daemon_recovers() {
        assert!(!should_watch_claude_tasks_locally(
            false, false, false, false
        ));
    }

    #[test]
    fn claude_tasks_local_watch_allows_explicit_windows_fallback_after_daemon_failure() {
        assert!(should_watch_claude_tasks_locally(false, false, false, true));
    }

    #[test]
    fn auxiliary_watch_bootstrap_spawns_asynchronously() {
        // Regression: startup used to perform auxiliary watch registration inline,
        // which could delay setup completion and leave the splash window waiting.
        let completed = Arc::new(AtomicBool::new(false));
        let completed_in_thread = completed.clone();

        spawn_auxiliary_watch_bootstrap_task(move || {
            std::thread::sleep(Duration::from_millis(40));
            completed_in_thread.store(true, Ordering::Release);
        });

        assert!(
            !completed.load(Ordering::Acquire),
            "auxiliary watch bootstrap should not complete inline on the caller thread"
        );

        for _ in 0..10 {
            if completed.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        panic!("auxiliary watch bootstrap task never completed");
    }
}
