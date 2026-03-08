use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;

use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::sentinels::CLAUDE_TASKS_PROJECT_ID;
use crate::session_scanner::DisplaySession;
use crate::{
    daemon, db, fs, models, provider, services, watch_targets, ProviderState, WatcherState,
};

/// Extract the WSL home directory from a Linux path.
///
/// `/home/user/projects/foo` → `/home/user`
pub(crate) fn extract_wsl_home(linux_path: &str) -> Option<String> {
    let parts: Vec<&str> = linux_path.splitn(4, '/').collect();
    if parts.len() >= 3 && parts[1] == "home" {
        Some(format!("/{}/{}", parts[1], parts[2]))
    } else {
        None
    }
}

fn derive_wsl_home_from_projects(projects: &[models::Project]) -> Option<String> {
    projects
        .iter()
        .filter(|project| provider::path::is_wsl_path(&project.path))
        .filter_map(|project| provider::path::wsl_unc_to_linux(&project.path))
        .find_map(|linux_path| extract_wsl_home(&linux_path))
}

fn claude_tasks_watch_dir(projects: &[models::Project]) -> Option<String> {
    derive_wsl_home_from_projects(projects).map(|home| format!("{home}/.claude/tasks"))
}

/// Convert daemon-emitted Linux session paths to frontend-visible Windows paths
/// when the daemon runs in WSL mode.
fn normalize_sessions_for_frontend(
    sessions: &mut [DisplaySession],
    wsl_distro: Option<&str>,
    native_daemon: bool,
) {
    if native_daemon {
        return;
    }
    let Some(distro) = wsl_distro else {
        return;
    };

    for session in sessions {
        if session.project_path.starts_with('/') {
            session.project_path = provider::path::to_windows(&session.project_path, distro);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaemonWatchTarget {
    project_id: String,
    project_name: String,
    linux_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DaemonWatchPlan {
    project_targets: Vec<DaemonWatchTarget>,
    claude_tasks_dir: Option<String>,
}

impl DaemonWatchPlan {
    fn is_empty(&self) -> bool {
        self.project_targets.is_empty() && self.claude_tasks_dir.is_none()
    }
}

#[derive(Default)]
struct DaemonWatchRuntime {
    plan: Option<DaemonWatchPlan>,
    stop_signal: Option<Arc<AtomicBool>>,
    listener_thread: Option<JoinHandle<()>>,
}

static DAEMON_WATCH_RUNTIME: LazyLock<Mutex<DaemonWatchRuntime>> =
    LazyLock::new(|| Mutex::new(DaemonWatchRuntime::default()));

fn emit_frontend_event(app: &AppHandle, event_name: &'static str, payload: serde_json::Value) {
    if let Err(error) = app.emit(event_name, payload) {
        tracing::warn!(
            event_name,
            error = %error,
            "Failed to emit frontend event"
        );
    }
}

fn build_daemon_watch_plan_at(
    projects: &[models::Project],
    thresholds: &models::ActivityThresholds,
    now: chrono::DateTime<chrono::Utc>,
) -> DaemonWatchPlan {
    let planned_targets = watch_targets::plan_activity_watch_targets_at(projects, thresholds, now);
    let mut project_targets = Vec::new();

    for target in planned_targets {
        if !provider::path::is_wsl_path(&target.project_path) {
            continue;
        }
        if !target.should_watch {
            continue;
        }

        let Some(linux_path) = provider::path::wsl_unc_to_linux(&target.project_path) else {
            tracing::warn!(
                project = target.project_name,
                path = %target.project_path,
                "Cannot convert WSL path to Linux while planning daemon watches"
            );
            continue;
        };

        project_targets.push(DaemonWatchTarget {
            project_id: target.project_id,
            project_name: target.project_name,
            linux_path,
        });
    }

    project_targets.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then_with(|| left.linux_path.cmp(&right.linux_path))
    });

    DaemonWatchPlan {
        project_targets,
        claude_tasks_dir: claude_tasks_watch_dir(projects),
    }
}

fn build_daemon_watch_plan(
    projects: &[models::Project],
    thresholds: &models::ActivityThresholds,
) -> DaemonWatchPlan {
    build_daemon_watch_plan_at(projects, thresholds, chrono::Utc::now())
}

fn stop_daemon_watch_runtime(reason: &str) {
    let (stop_signal, listener_thread) = {
        let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Daemon watch runtime lock poisoned while stopping; recovering"
            );
            error.into_inner()
        });
        runtime.plan = None;
        (runtime.stop_signal.take(), runtime.listener_thread.take())
    };

    if let Some(signal) = stop_signal {
        signal.store(true, Ordering::Relaxed);
    }
    if let Some(listener_thread) = listener_thread {
        if listener_thread.join().is_err() {
            tracing::warn!(
                reason,
                "daemon watch listener thread panicked while stopping"
            );
        }
    }
}

fn apply_daemon_watch_plan(
    daemon_addr: String,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    wsl_distro: Option<String>,
    desired_plan: DaemonWatchPlan,
    reason: &str,
    force_restart: bool,
) {
    let unchanged = {
        let runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Daemon watch runtime lock poisoned while checking watch plan; recovering"
            );
            error.into_inner()
        });
        !force_restart
            && runtime
                .plan
                .as_ref()
                .is_some_and(|current| current == &desired_plan)
    };
    if unchanged {
        return;
    }

    stop_daemon_watch_runtime(reason);

    if desired_plan.is_empty() {
        return;
    }

    let mut listener =
        match daemon::event_listener::DaemonEventListener::connect(&daemon_addr, event_tx) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Failed to connect daemon event listener for reconciliation"
                );
                return;
            }
        };

    let mut watched_project_count = 0usize;
    for target in &desired_plan.project_targets {
        if let Err(error) = listener.watch(&target.project_id, &target.linux_path) {
            tracing::warn!(
                project = target.project_name,
                error = %error,
                reason,
                "Failed to register daemon watch"
            );
        } else {
            watched_project_count += 1;
        }
    }

    if let Some(claude_tasks_dir) = desired_plan.claude_tasks_dir.as_ref() {
        if let Err(error) = listener.watch(CLAUDE_TASKS_PROJECT_ID, claude_tasks_dir) {
            tracing::debug!(
                error = %error,
                path = %claude_tasks_dir,
                reason,
                "Could not watch Claude tasks directory (daemon)"
            );
        } else {
            tracing::info!(
                path = %claude_tasks_dir,
                reason,
                "Watching Claude tasks directory (daemon)"
            );
        }
    }

    if watched_project_count == 0 && desired_plan.claude_tasks_dir.is_none() {
        return;
    }

    let stop_signal = Arc::new(AtomicBool::new(false));
    {
        let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Daemon watch runtime lock poisoned while storing watch plan; recovering"
            );
            error.into_inner()
        });
        runtime.plan = Some(desired_plan.clone());
        runtime.stop_signal = Some(stop_signal.clone());
    }

    tracing::info!(
        watched = watched_project_count,
        reason,
        distro = ?wsl_distro,
        "Daemon watching WSL projects"
    );

    let listener_thread = std::thread::spawn(move || {
        listener.run_until_stopped(stop_signal.clone());

        let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                "Daemon watch runtime lock poisoned while clearing exited listener; recovering"
            );
            error.into_inner()
        });
        if runtime
            .stop_signal
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &stop_signal))
        {
            runtime.plan = None;
            runtime.stop_signal = None;
            runtime.listener_thread = None;
        }
    });

    {
        let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Daemon watch runtime lock poisoned while storing listener handle; recovering"
            );
            error.into_inner()
        });
        runtime.listener_thread = Some(listener_thread);
    }
}

pub(crate) fn reconcile_daemon_activity_watches(
    app: &AppHandle,
    projects: &[models::Project],
    thresholds: &models::ActivityThresholds,
    reason: &str,
) {
    let (daemon_addr, distro) = {
        let provider_state = app.state::<ProviderState>();
        let Some(ref daemon) = provider_state.daemon else {
            stop_daemon_watch_runtime(reason);
            return;
        };
        if !daemon.is_connected() {
            stop_daemon_watch_runtime(reason);
            return;
        }
        (daemon.addr().to_string(), provider_state.wsl_distro.clone())
    };

    let event_tx = {
        let watcher_state = app.state::<WatcherState>();
        let sender = match watcher_state.0.lock() {
            Ok(watcher) => watcher.event_sender(),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Failed to reconcile daemon watches: watcher lock poisoned"
                );
                return;
            }
        };
        sender
    };

    let desired_plan = build_daemon_watch_plan(projects, thresholds);
    apply_daemon_watch_plan(daemon_addr, event_tx, distro, desired_plan, reason, false);
}

/// Start daemon event listener for WSL projects.
///
/// Opens a dedicated TCP connection to the daemon, sends `watch` commands for
/// currently active/recent WSL projects, then spawns the listener event loop.
/// Events are forwarded to the shared watcher channel, where
/// `process_watch_events` handles them identically to local watcher events.
///
/// On macOS/Linux (native daemon), this is a no-op — all project paths are local
/// and the local watcher handles them. The function still runs for consistency
/// but registers zero watches and exits immediately.
pub(crate) fn start_daemon_watches(
    daemon_addr: String,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    wsl_distro: Option<String>,
    projects: Vec<models::Project>,
    thresholds: models::ActivityThresholds,
) {
    let plan = build_daemon_watch_plan(&projects, &thresholds);
    apply_daemon_watch_plan(daemon_addr, event_tx, wsl_distro, plan, "bootstrap", true);
}

/// Re-register all daemon watches after a reconnection.
///
/// Spawns a new `start_daemon_watches` thread using current project list from DB.
/// The old event listener thread has already exited (daemon connection was lost),
/// so this creates a fresh TCP connection for the event stream.
pub(crate) fn respawn_daemon_watches(app: &AppHandle) {
    let provider_state = app.state::<ProviderState>();
    let Some(ref daemon) = provider_state.daemon else {
        return;
    };
    let daemon_addr = daemon.addr().to_string();
    let distro = provider_state.wsl_distro.clone();

    let watcher_state = app.state::<WatcherState>();
    let event_tx = match watcher_state.0.lock() {
        Ok(w) => w.event_sender(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to respawn daemon watches: watcher lock poisoned"
            );
            return;
        }
    };

    let db_state = app.state::<commands::projects::DbState>();
    let projects = match db_state.0.lock() {
        Ok(conn) => {
            let projects = match db::queries::list_projects(&conn) {
                Ok(list) => list,
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Failed to respawn daemon watches: project list query failed"
                    );
                    return;
                }
            };
            let thresholds = match crate::db::settings_queries::get_all_settings(&conn) {
                Ok(settings) => settings.thresholds,
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Failed to load thresholds for daemon watch respawn; using defaults"
                    );
                    models::ActivityThresholds::default()
                }
            };
            (projects, thresholds)
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to respawn daemon watches: db lock poisoned"
            );
            return;
        }
    };

    let (projects, thresholds) = projects;
    tracing::info!(
        project_count = projects.len(),
        "Re-registering daemon watches after reconnection"
    );

    std::thread::spawn(move || {
        start_daemon_watches(daemon_addr, event_tx, distro, projects, thresholds);
    });

    // Also re-scan sessions that may have been missed while disconnected
    {
        let db_state = app.state::<commands::projects::DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Skipping daemon reconnection session backfill: db lock poisoned"
                );
                return;
            }
        };
        let all_projects = match db::queries::list_projects(&conn) {
            Ok(list) => list,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Skipping daemon reconnection session backfill: project list query failed"
                );
                return;
            }
        };
        for project in &all_projects {
            let root = if provider::path::is_wsl_path(&project.path) {
                provider::path::wsl_unc_to_linux(&project.path).map(std::path::PathBuf::from)
            } else {
                Some(std::path::PathBuf::from(&project.path))
            };
            if let Some(root) = root {
                match services::session_import::scan_and_import_sessions(&conn, &project.id, &root)
                {
                    Ok(imported) if !imported.is_empty() => {
                        tracing::info!(
                            project = project.name,
                            count = imported.len(),
                            "Imported missed sessions after reconnection"
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(
                            project_id = project.id.as_str(),
                            path = project.path.as_str(),
                            error = %error,
                            "Failed to backfill sessions after daemon reconnection"
                        );
                    }
                }
            }
        }
    }
}

/// Background thread that monitors daemon health via periodic pings.
///
/// On disconnect: attempts restart and reconnection (max 3 attempts per session).
/// Emits `daemon-status` events to the frontend for UI indicators.
/// Works for both initially-connected and initially-disconnected providers.
pub(crate) fn daemon_health_check(app: AppHandle, connected_at_startup: bool) {
    use std::time::Duration;

    const CHECK_INTERVAL: Duration = Duration::from_secs(30);
    /// Shorter interval when daemon hasn't connected yet (first-time connect).
    const FAST_CHECK_INTERVAL: Duration = Duration::from_secs(2);
    const MAX_RESTART_ATTEMPTS: u32 = 3;

    let mut consecutive_failures: u32 = 0;
    let mut restart_attempts: u32 = 0;
    let mut ever_connected = connected_at_startup;
    let mut recovering = !connected_at_startup;

    // Initial delay — let the app finish starting.
    // Short delay when daemon wasn't connected: it was already spawned in setup(),
    // just needs a moment to bind the port. Long delay when already connected:
    // no urgency, just periodic health monitoring.
    let initial_delay = if connected_at_startup {
        Duration::from_secs(5)
    } else {
        Duration::from_secs(1)
    };
    std::thread::sleep(initial_delay);

    loop {
        // Use shorter interval while waiting for an initial connection or while recovering from
        // a disconnect so status can clear quickly when the daemon comes back.
        let connected = {
            let provider_state = app.state::<ProviderState>();
            provider_state
                .daemon
                .as_ref()
                .is_some_and(|daemon| daemon.is_connected())
        };
        let interval = daemon_health_check_interval(
            connected,
            ever_connected,
            recovering,
            CHECK_INTERVAL,
            FAST_CHECK_INTERVAL,
        );
        std::thread::sleep(interval);

        let provider_state = app.state::<ProviderState>();
        let Some(ref daemon) = provider_state.daemon else {
            return;
        };

        if daemon.is_connected() {
            match daemon.ping() {
                Ok(()) => {
                    if consecutive_failures > 0 {
                        tracing::debug!("Daemon health check recovered");
                    }
                    if recovering {
                        tracing::info!("Daemon health monitor observed recovered connectivity");
                        handle_daemon_recovered(&app);
                        recovering = false;
                    }
                    consecutive_failures = 0;
                    restart_attempts = 0;
                    ever_connected = true;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        failures = consecutive_failures,
                        error = %e,
                        "Daemon health check failed"
                    );
                    if consecutive_failures >= 3 && !recovering {
                        emit_frontend_event(
                            &app,
                            "daemon-status",
                            serde_json::json!({ "status": "disconnected" }),
                        );
                        recovering = true;
                    }
                }
            }
        } else {
            // Daemon is disconnected — try to reconnect
            if restart_attempts >= MAX_RESTART_ATTEMPTS {
                tracing::warn!(
                    "Max daemon restart attempts reached ({MAX_RESTART_ATTEMPTS}), giving up"
                );
                emit_frontend_event(
                    &app,
                    "daemon-status",
                    serde_json::json!({ "status": "failed" }),
                );
                return;
            }

            if !recovering {
                emit_frontend_event(
                    &app,
                    "daemon-status",
                    serde_json::json!({ "status": "reconnecting" }),
                );
                recovering = true;
            }

            // Try reconnecting to existing daemon first
            if daemon.reconnect().is_ok() {
                tracing::info!("Reconnected to daemon");
                consecutive_failures = 0;
                restart_attempts = 0;
                ever_connected = true;
                handle_daemon_recovered(&app);
                recovering = false;
                continue;
            }

            // Try restarting daemon process
            restart_attempts += 1;
            tracing::info!(
                attempt = restart_attempts,
                max = MAX_RESTART_ATTEMPTS,
                "Attempting daemon restart"
            );

            let distro = provider_state.wsl_distro.as_deref();
            let port = daemon::server::DEFAULT_PORT;

            if let Some(d) = distro {
                if daemon::launcher::try_restart_daemon(d, port).is_ok() {
                    std::thread::sleep(Duration::from_secs(2));
                    if daemon.reconnect().is_ok() {
                        tracing::info!("Reconnected after daemon restart");
                        consecutive_failures = 0;
                        restart_attempts = 0;
                        ever_connected = true;
                        handle_daemon_recovered(&app);
                        recovering = false;
                        continue;
                    }
                }
            }

            tracing::warn!(attempt = restart_attempts, "Daemon restart attempt failed");
        }
    }
}

fn daemon_health_check_interval(
    connected: bool,
    ever_connected: bool,
    recovering: bool,
    normal_interval: std::time::Duration,
    fast_interval: std::time::Duration,
) -> std::time::Duration {
    if !connected || !ever_connected || recovering {
        fast_interval
    } else {
        normal_interval
    }
}

fn handle_daemon_recovered(app: &AppHandle) {
    respawn_daemon_watches(app);
    {
        let app_for_reseed = app.clone();
        std::thread::spawn(move || {
            crate::event_processor::reseed_daemon_watched_git_status(&app_for_reseed);
        });
    }
    emit_frontend_event(
        app,
        "daemon-status",
        serde_json::json!({ "status": "connected" }),
    );
}

/// Bridge daemon-owned session updates into frontend Tauri events.
///
/// Uses a dedicated daemon connection and long-poll update requests so the UI
/// can stay event-driven. The daemon owns scanner polling and versioning.
pub(crate) fn start_session_updates_bridge(app: AppHandle) {
    use std::time::Duration;

    const WAIT_TIMEOUT: Duration = Duration::from_secs(20);
    const RETRY_DELAY: Duration = Duration::from_secs(1);

    std::thread::spawn(move || {
        let mut since_version: u64 = 0;
        let mut observed_connected = false;
        tracing::info!("session updates bridge thread started");

        loop {
            let (daemon_addr, connected) = {
                let provider_state = app.state::<ProviderState>();
                let Some(ref daemon) = provider_state.daemon else {
                    return;
                };
                (daemon.addr().to_string(), daemon.is_connected())
            };

            if !connected {
                if observed_connected {
                    tracing::info!("session updates bridge: daemon disconnected");
                    observed_connected = false;
                }
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
            if !observed_connected {
                tracing::info!("session updates bridge: daemon connected");
                observed_connected = true;
            }

            let mut listener =
                match crate::daemon::session_listener::DaemonSessionListener::connect(&daemon_addr)
                {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::debug!(error = %e, "Session listener connect failed");
                        std::thread::sleep(RETRY_DELAY);
                        continue;
                    }
                };

            loop {
                let still_connected = {
                    let provider_state = app.state::<ProviderState>();
                    provider_state
                        .daemon
                        .as_ref()
                        .is_some_and(|daemon| daemon.is_connected())
                };
                if !still_connected {
                    break;
                }

                match listener.wait_for_updates(since_version, WAIT_TIMEOUT) {
                    Ok(update) => {
                        // Daemon restart: its version counter may reset.
                        // Reset our cursor and retry from a fresh baseline.
                        if update.version < since_version {
                            since_version = 0;
                            continue;
                        }

                        since_version = update.version;
                        if update.changed {
                            let mut sessions = update.sessions;
                            let session_count = sessions.len();
                            let distro = {
                                let provider_state = app.state::<ProviderState>();
                                provider_state.wsl_distro.clone()
                            };
                            normalize_sessions_for_frontend(
                                &mut sessions,
                                distro.as_deref(),
                                crate::daemon::launcher::is_native_daemon(),
                            );
                            crate::coordination::activity_export::enrich_sessions_with_team_membership(
                                app.state::<crate::coordination::state::CoordinationState>()
                                    .teams_dir(),
                                &mut sessions,
                            );

                            emit_frontend_event(
                                &app,
                                "sessions-updated",
                                serde_json::json!({
                                    "version": update.version,
                                    "sessions": sessions,
                                }),
                            );
                            tracing::debug!(
                                version = update.version,
                                session_count = session_count,
                                "session updates bridge emitted sessions-updated event"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "Session listener poll failed");
                        break;
                    }
                }
            }

            std::thread::sleep(RETRY_DELAY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::cli_tool::CliTool;
    use crate::session_scanner::{ActivityAttribution, ActivityConfidence, SessionState};
    use chrono::{Duration, Utc};

    fn test_project(path: &str, last_activity_at: Option<String>) -> models::Project {
        test_project_with("p1", "Project", path, last_activity_at)
    }

    fn test_project_with(
        id: &str,
        name: &str,
        path: &str,
        last_activity_at: Option<String>,
    ) -> models::Project {
        models::Project {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            description: None,
            last_activity_at,
            hero_preference: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            cached_branch: None,
            cached_is_dirty: None,
        }
    }

    fn test_session(path: &str) -> DisplaySession {
        DisplaySession {
            pid: 1234,
            project_path: path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%1".to_string()),
            tmux_window_name: Some("work".to_string()),
            state: SessionState::Active,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: crate::session_scanner::SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn normalize_sessions_for_frontend_converts_linux_paths_in_wsl_mode() {
        let mut sessions = vec![test_session("/home/dev/projects/taurhaus")];
        normalize_sessions_for_frontend(&mut sessions, Some("Ubuntu"), false);
        assert_eq!(
            sessions[0].project_path,
            r"\\wsl.localhost\Ubuntu\home\dev\projects\taurhaus"
        );
    }

    #[test]
    fn normalize_sessions_for_frontend_skips_native_daemon() {
        let original = "/home/dev/projects/taurhaus";
        let mut sessions = vec![test_session(original)];
        normalize_sessions_for_frontend(&mut sessions, Some("native"), true);
        assert_eq!(sessions[0].project_path, original);
    }

    #[test]
    fn normalize_sessions_for_frontend_requires_distro() {
        let original = "/home/dev/projects/taurhaus";
        let mut sessions = vec![test_session(original)];
        normalize_sessions_for_frontend(&mut sessions, None, false);
        assert_eq!(sessions[0].project_path, original);
    }

    #[test]
    fn derives_wsl_home_even_when_project_is_not_active_or_recent() {
        let dormant = (Utc::now() - Duration::days(365)).to_rfc3339();
        let projects = vec![test_project(
            r"\\wsl.localhost\Ubuntu\home\dev\projects\taurhaus",
            Some(dormant),
        )];

        let home = derive_wsl_home_from_projects(&projects);
        let tasks_dir = claude_tasks_watch_dir(&projects);

        assert_eq!(home.as_deref(), Some("/home/dev"));
        assert_eq!(tasks_dir.as_deref(), Some("/home/dev/.claude/tasks"));
    }

    #[test]
    fn daemon_watch_plan_only_includes_active_or_recent_wsl_projects() {
        let now = Utc::now();
        let thresholds = models::ActivityThresholds::default();
        let projects = vec![
            test_project_with(
                "active",
                "Active",
                r"\\wsl.localhost\Ubuntu\home\dev\projects\active",
                Some((now - Duration::days(1)).to_rfc3339()),
            ),
            test_project_with(
                "recent",
                "Recent",
                r"\\wsl.localhost\Ubuntu\home\dev\projects\recent",
                Some((now - Duration::days(15)).to_rfc3339()),
            ),
            test_project_with(
                "stale",
                "Stale",
                r"\\wsl.localhost\Ubuntu\home\dev\projects\stale",
                Some((now - Duration::days(60)).to_rfc3339()),
            ),
            test_project_with(
                "local",
                "Local",
                "/home/dev/projects/local",
                Some((now - Duration::days(1)).to_rfc3339()),
            ),
        ];

        let plan = build_daemon_watch_plan_at(&projects, &thresholds, now);
        let watched_ids: Vec<&str> = plan
            .project_targets
            .iter()
            .map(|target| target.project_id.as_str())
            .collect();

        assert_eq!(watched_ids, vec!["active", "recent"]);
        assert!(plan
            .project_targets
            .iter()
            .all(|target| target.linux_path.starts_with("/home/dev/projects/")));
    }

    #[test]
    fn daemon_watch_plan_removes_dormant_projects_but_keeps_tasks_watch() {
        let now = Utc::now();
        let thresholds = models::ActivityThresholds::default();
        let active_project = test_project_with(
            "wsl",
            "WSL",
            r"\\wsl.localhost\Ubuntu\home\dev\projects\taurhaus",
            Some((now - Duration::days(2)).to_rfc3339()),
        );
        let dormant_project = test_project_with(
            "wsl",
            "WSL",
            r"\\wsl.localhost\Ubuntu\home\dev\projects\taurhaus",
            Some((now - Duration::days(200)).to_rfc3339()),
        );

        let active_plan =
            build_daemon_watch_plan_at(std::slice::from_ref(&active_project), &thresholds, now);
        let dormant_plan =
            build_daemon_watch_plan_at(std::slice::from_ref(&dormant_project), &thresholds, now);

        assert_eq!(active_plan.project_targets.len(), 1);
        assert!(dormant_plan.project_targets.is_empty());
        assert_eq!(
            dormant_plan.claude_tasks_dir.as_deref(),
            Some("/home/dev/.claude/tasks")
        );
    }

    #[test]
    fn daemon_health_check_uses_fast_interval_while_recovering_or_disconnected() {
        let normal = std::time::Duration::from_secs(30);
        let fast = std::time::Duration::from_secs(2);

        assert_eq!(
            daemon_health_check_interval(false, true, false, normal, fast),
            fast
        );
        assert_eq!(
            daemon_health_check_interval(true, false, false, normal, fast),
            fast
        );
        assert_eq!(
            daemon_health_check_interval(true, true, true, normal, fast),
            fast
        );
        assert_eq!(
            daemon_health_check_interval(true, true, false, normal, fast),
            normal
        );
    }
}
