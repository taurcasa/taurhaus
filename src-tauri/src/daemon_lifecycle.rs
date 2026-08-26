use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::provider::platform_paths::PlatformPaths;
use crate::sentinels::CLAUDE_TASKS_PROJECT_ID;
use crate::session_scanner::tmux::TmuxFocus;
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

fn prefer_local_claude_tasks_watch_for_host(is_windows_host: bool, tasks_dir_exists: bool) -> bool {
    is_windows_host && tasks_dir_exists
}

fn prefer_local_claude_tasks_watch(tasks_dir_exists: bool) -> bool {
    prefer_local_claude_tasks_watch_for_host(cfg!(target_os = "windows"), tasks_dir_exists)
}

fn claude_tasks_watch_dir(projects: &[models::Project]) -> Option<String> {
    if prefer_local_claude_tasks_watch(PlatformPaths::claude_dir().join("tasks").is_dir()) {
        return None;
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    command_tx: Option<Sender<DaemonWatchCommand>>,
    owner_thread: Option<JoinHandle<()>>,
}

static DAEMON_WATCH_RUNTIME: LazyLock<Mutex<DaemonWatchRuntime>> =
    LazyLock::new(|| Mutex::new(DaemonWatchRuntime::default()));

#[derive(Debug)]
enum DaemonWatchCommand {
    ApplyPlan {
        daemon_addr: String,
        wsl_distro: Option<String>,
        event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
        desired_plan: DaemonWatchPlan,
        reason: String,
    },
    Shutdown {
        reason: String,
    },
}

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

fn watch_targets_by_path(targets: &[DaemonWatchTarget]) -> HashMap<String, DaemonWatchTarget> {
    targets
        .iter()
        .cloned()
        .map(|target| (target.linux_path.clone(), target))
        .collect()
}

fn apply_plan_to_listener(
    listener: &mut daemon::event_listener::DaemonEventListener,
    active_plan: &mut DaemonWatchPlan,
    desired_plan: &DaemonWatchPlan,
    reason: &str,
) -> Result<(), crate::errors::AppError> {
    let active_targets = watch_targets_by_path(&active_plan.project_targets);
    let desired_targets = watch_targets_by_path(&desired_plan.project_targets);

    for (linux_path, active_target) in &active_targets {
        let should_keep = desired_targets
            .get(linux_path)
            .is_some_and(|desired_target| desired_target == active_target);
        if should_keep {
            continue;
        }
        listener.unwatch(linux_path)?;
        tracing::info!(
            project = active_target.project_name,
            reason,
            "Removed daemon watch subscription"
        );
    }

    if active_plan.claude_tasks_dir != desired_plan.claude_tasks_dir {
        if let Some(claude_tasks_dir) = active_plan.claude_tasks_dir.as_ref() {
            listener.unwatch(claude_tasks_dir)?;
            tracing::info!(
                path = %claude_tasks_dir,
                reason,
                "Stopped watching Claude tasks directory (daemon)"
            );
        }
    }

    let mut next_project_targets = Vec::new();
    for target in &desired_plan.project_targets {
        let already_active = active_targets
            .get(&target.linux_path)
            .is_some_and(|active_target| active_target == target);
        if !already_active {
            listener.watch(&target.project_id, &target.linux_path)?;
            tracing::info!(
                project = target.project_name,
                reason,
                "Registered daemon watch"
            );
        }
        next_project_targets.push(target.clone());
    }

    let mut next_claude_tasks_dir = None;
    if let Some(claude_tasks_dir) = desired_plan.claude_tasks_dir.as_ref() {
        if active_plan.claude_tasks_dir.as_ref() != Some(claude_tasks_dir) {
            listener.watch(CLAUDE_TASKS_PROJECT_ID, claude_tasks_dir)?;
            tracing::info!(
                path = %claude_tasks_dir,
                reason,
                "Watching Claude tasks directory (daemon)"
            );
        }
        next_claude_tasks_dir = Some(claude_tasks_dir.clone());
    }

    *active_plan = DaemonWatchPlan {
        project_targets: next_project_targets,
        claude_tasks_dir: next_claude_tasks_dir,
    };

    Ok(())
}

fn handle_daemon_watch_command(
    command: DaemonWatchCommand,
    listener: &mut Option<daemon::event_listener::DaemonEventListener>,
    active_plan: &mut DaemonWatchPlan,
    current_addr: &mut Option<String>,
) -> bool {
    match command {
        DaemonWatchCommand::Shutdown { reason } => {
            tracing::info!(reason, "Shutting down daemon watch owner");
            false
        }
        DaemonWatchCommand::ApplyPlan {
            daemon_addr,
            wsl_distro,
            event_tx,
            desired_plan,
            reason,
        } => {
            if desired_plan.is_empty() {
                *listener = None;
                *active_plan = DaemonWatchPlan::default();
                *current_addr = None;
                return true;
            }

            let reconnect_needed =
                listener.is_none() || current_addr.as_deref() != Some(daemon_addr.as_str());
            if reconnect_needed {
                *listener = match daemon::event_listener::DaemonEventListener::connect_with_distro(
                    &daemon_addr,
                    event_tx,
                    wsl_distro.as_deref(),
                ) {
                    Ok(listener) => Some(listener),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            reason,
                            "Failed to connect daemon event listener for watch owner"
                        );
                        *active_plan = DaemonWatchPlan::default();
                        *current_addr = None;
                        None
                    }
                };

                if listener.is_some() {
                    *current_addr = Some(daemon_addr);
                }
            }

            let Some(listener_ref) = listener.as_mut() else {
                return true;
            };
            if let Err(error) =
                apply_plan_to_listener(listener_ref, active_plan, &desired_plan, &reason)
            {
                tracing::warn!(
                    error = %error,
                    reason,
                    "Failed to apply daemon watch plan; dropping listener state"
                );
                *listener = None;
                *active_plan = DaemonWatchPlan::default();
                *current_addr = None;
            }
            true
        }
    }
}

fn daemon_watch_owner_loop(rx: Receiver<DaemonWatchCommand>) {
    let mut listener: Option<daemon::event_listener::DaemonEventListener> = None;
    let mut active_plan = DaemonWatchPlan::default();
    let mut current_addr: Option<String> = None;

    loop {
        while let Ok(command) = rx.try_recv() {
            if !handle_daemon_watch_command(
                command,
                &mut listener,
                &mut active_plan,
                &mut current_addr,
            ) {
                return;
            }
        }

        if listener.is_none() {
            match rx.recv() {
                Ok(command) => {
                    if !handle_daemon_watch_command(
                        command,
                        &mut listener,
                        &mut active_plan,
                        &mut current_addr,
                    ) {
                        return;
                    }
                }
                Err(_) => return,
            }
            continue;
        }

        match listener
            .as_mut()
            .expect("listener must exist when owner loop pumps")
            .pump_once(std::time::Duration::from_millis(250))
        {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!("Daemon watch owner lost event listener connection");
                listener = None;
                active_plan = DaemonWatchPlan::default();
                current_addr = None;
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Daemon watch owner poll failed; dropping listener state"
                );
                listener = None;
                active_plan = DaemonWatchPlan::default();
                current_addr = None;
            }
        }
    }
}

fn ensure_daemon_watch_owner() -> Sender<DaemonWatchCommand> {
    let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Daemon watch runtime lock poisoned while ensuring owner; recovering"
        );
        error.into_inner()
    });
    if let Some(sender) = runtime.command_tx.as_ref() {
        return sender.clone();
    }

    let (tx, rx) = mpsc::channel();
    let owner_thread = std::thread::spawn(move || daemon_watch_owner_loop(rx));
    runtime.command_tx = Some(tx.clone());
    runtime.owner_thread = Some(owner_thread);
    tx
}

fn stop_daemon_watch_runtime(reason: &str) {
    let (command_tx, owner_thread) = {
        let mut runtime = DAEMON_WATCH_RUNTIME.lock().unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                reason,
                "Daemon watch runtime lock poisoned while stopping; recovering"
            );
            error.into_inner()
        });
        (runtime.command_tx.take(), runtime.owner_thread.take())
    };

    if let Some(command_tx) = command_tx {
        let _ = command_tx.send(DaemonWatchCommand::Shutdown {
            reason: reason.to_string(),
        });
    }
    if let Some(owner_thread) = owner_thread {
        if owner_thread.join().is_err() {
            tracing::warn!(reason, "daemon watch owner thread panicked while stopping");
        }
    }
}

fn apply_daemon_watch_plan(
    daemon_addr: String,
    wsl_distro: Option<String>,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    desired_plan: DaemonWatchPlan,
    reason: &str,
) {
    let command_tx = ensure_daemon_watch_owner();
    let _ = command_tx.send(DaemonWatchCommand::ApplyPlan {
        daemon_addr,
        wsl_distro,
        event_tx,
        desired_plan,
        reason: reason.to_string(),
    });
}

pub(crate) fn reconcile_daemon_activity_watches(
    app: &AppHandle,
    projects: &[models::Project],
    thresholds: &models::ActivityThresholds,
    reason: &str,
) {
    let (daemon_addr, wsl_distro) = {
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
    apply_daemon_watch_plan(daemon_addr, wsl_distro, event_tx, desired_plan, reason);
}

/// Re-register all daemon watches after a reconnection.
///
/// Recomputes the desired watch plan and applies it through the single daemon
/// watch owner. The old event listener state will reconnect in place.
pub(crate) fn respawn_daemon_watches(app: &AppHandle) {
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

    reconcile_daemon_activity_watches(app, &projects, &thresholds, "reconnect");

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
pub(crate) fn daemon_health_check(
    app: AppHandle,
    connected_at_startup: bool,
    bootstrap_complete: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    const CHECK_INTERVAL: Duration = Duration::from_secs(30);
    /// Shorter interval when daemon hasn't connected yet (first-time connect).
    const FAST_CHECK_INTERVAL: Duration = Duration::from_secs(2);
    /// Long interval after exhausting restart attempts — still watching for
    /// the daemon to come back, but not burning CPU.
    const DORMANT_CHECK_INTERVAL: Duration = Duration::from_secs(15);
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
        let interval = if restart_attempts >= MAX_RESTART_ATTEMPTS {
            DORMANT_CHECK_INTERVAL
        } else {
            daemon_health_check_interval(
                connected,
                ever_connected,
                recovering,
                CHECK_INTERVAL,
                FAST_CHECK_INTERVAL,
            )
        };
        std::thread::sleep(interval);

        let provider_state = app.state::<ProviderState>();
        let Some(ref daemon) = provider_state.daemon else {
            return;
        };

        if daemon.is_connected() {
            match classify_daemon_health(daemon.ping_protocol_version().map_err(|e| e.to_string()))
            {
                DaemonHealth::Healthy => {
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
                DaemonHealth::ProtocolMismatch { running, expected } => {
                    // Reachable but useless: drop it so the reconnect/restart
                    // path below replaces it with a daemon this app can bridge.
                    daemon.disconnect("health_protocol_mismatch");
                    if !recovering {
                        tracing::error!(
                            daemon_protocol_version = running,
                            expected,
                            "DAEMON IS OUTDATED — rebuild with `just install-daemon`"
                        );
                        emit_frontend_event(
                            &app,
                            "daemon-status",
                            serde_json::json!({ "status": "disconnected" }),
                        );
                        recovering = true;
                    }
                }
                DaemonHealth::Unreachable(error) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        failures = consecutive_failures,
                        error = %error,
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
            let dormant = restart_attempts >= MAX_RESTART_ATTEMPTS;

            if dormant && !recovering {
                // Exhausted active restart attempts — enter dormant recovery.
                // We keep the thread alive with a long poll so that if the
                // daemon comes back (manual start, WSL recovery, etc.) we
                // pick it up automatically instead of requiring an app restart.
                tracing::warn!(
                    "Max daemon restart attempts reached ({MAX_RESTART_ATTEMPTS}), \
                     entering dormant recovery (reconnect-only, no restarts)"
                );
                emit_frontend_event(
                    &app,
                    "daemon-status",
                    serde_json::json!({ "status": "failed" }),
                );
                recovering = true;
            }

            if !recovering {
                emit_frontend_event(
                    &app,
                    "daemon-status",
                    serde_json::json!({ "status": "reconnecting" }),
                );
                recovering = true;
            }

            let distro = provider_state.wsl_distro.as_deref();
            let port = daemon::server::DEFAULT_PORT;
            match recover_daemon_connection(
                || {
                    daemon::launcher::reconnect_existing_provider_until_reachable(daemon, port)
                        .is_ok()
                        && confirm_daemon_protocol(
                            || daemon.ping_protocol_version().map_err(|e| e.to_string()),
                            |reason| daemon.disconnect(reason),
                        )
                },
                || {
                    // Don't restart the daemon while bootstrap is still running —
                    // two threads doing stop/start concurrently race and kill each
                    // other's daemon, exhausting restart attempts before either
                    // succeeds.
                    if !bootstrap_complete.load(Ordering::Acquire) {
                        tracing::debug!("Skipping daemon restart — bootstrap still in progress");
                        return false;
                    }
                    // In dormant mode: reconnect-only, no restart attempts.
                    if dormant {
                        return false;
                    }
                    let Some(distro) = distro else {
                        return false;
                    };
                    restart_attempts += 1;
                    tracing::info!(
                        attempt = restart_attempts,
                        max = MAX_RESTART_ATTEMPTS,
                        "Attempting daemon restart after sustained reconnect failure"
                    );
                    daemon::launcher::try_restart_daemon(distro, port).is_ok()
                },
            ) {
                DaemonRecoveryResult::Reconnected
                | DaemonRecoveryResult::RestartedAndReconnected => {
                    tracing::info!("Daemon connection recovered");
                    consecutive_failures = 0;
                    restart_attempts = 0;
                    ever_connected = true;
                    handle_daemon_recovered(&app);
                    recovering = false;
                    continue;
                }
                DaemonRecoveryResult::Failed => {
                    if !dormant {
                        tracing::warn!(
                            attempt = restart_attempts,
                            "Daemon recovery attempt failed"
                        );
                    }
                }
            }
        }
    }
}

/// What a health ping says about the daemon on the other end.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DaemonHealth {
    /// Reachable and speaking this app's protocol.
    Healthy,
    /// Reachable, but speaking a protocol this app cannot bridge.
    ProtocolMismatch { running: u32, expected: u32 },
    /// Not reachable.
    Unreachable(String),
}

/// Judge a health ping.
///
/// Startup rejects a daemon whose protocol differs (`startup::daemon`), but the
/// health monitor runs for the rest of the app's life, and a daemon that comes
/// back late — or that a developer starts by hand — meets no other gate. Since
/// protocol v8 the hub snapshot is the only live focus transport, so accepting a
/// v7 daemon here would leave the foreground indicator dark forever: its omitted
/// focus fields decode as `None` and the hook chain that used to cover for it is
/// gone.
fn classify_daemon_health(ping: Result<u32, String>) -> DaemonHealth {
    let expected = crate::daemon::protocol::PROTOCOL_VERSION;
    match ping {
        Ok(running) if running == expected => DaemonHealth::Healthy,
        Ok(running) => DaemonHealth::ProtocolMismatch { running, expected },
        Err(error) => DaemonHealth::Unreachable(error),
    }
}

/// Confirm a freshly reconnected daemon speaks this app's protocol.
///
/// Reachability is not compatibility: an outdated daemon still accepts TCP, so
/// treating a successful reconnect as recovery walks around the startup gate.
/// A daemon that fails here is disconnected, which sends the caller on to its
/// restart path.
fn confirm_daemon_protocol<P, D>(ping_protocol_version: P, disconnect: D) -> bool
where
    P: FnOnce() -> Result<u32, String>,
    D: FnOnce(&str),
{
    match classify_daemon_health(ping_protocol_version()) {
        DaemonHealth::Healthy => true,
        DaemonHealth::ProtocolMismatch { running, expected } => {
            tracing::warn!(
                daemon_protocol_version = running,
                expected,
                "Reconnected daemon is outdated — rebuild with `just install-daemon`"
            );
            disconnect("reconnect_protocol_mismatch");
            false
        }
        DaemonHealth::Unreachable(error) => {
            tracing::debug!(error = %error, "Reconnected daemon did not answer the protocol ping");
            disconnect("reconnect_ping_failed");
            false
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonRecoveryResult {
    Reconnected,
    RestartedAndReconnected,
    Failed,
}

fn recover_daemon_connection<R, S>(
    mut reconnect_until_reachable: R,
    mut restart_daemon: S,
) -> DaemonRecoveryResult
where
    R: FnMut() -> bool,
    S: FnMut() -> bool,
{
    if reconnect_until_reachable() {
        return DaemonRecoveryResult::Reconnected;
    }

    if restart_daemon() && reconnect_until_reachable() {
        return DaemonRecoveryResult::RestartedAndReconnected;
    }

    DaemonRecoveryResult::Failed
}

fn handle_daemon_recovered(app: &AppHandle) {
    #[cfg(feature = "mesh-bridged-backend")]
    crate::startup::compaction::release_app_owned_compaction(app, "daemon_recovered");
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

#[derive(Debug, Default)]
struct SessionBridgeRecoveryTracker {
    disconnected_at: Option<Instant>,
}

impl SessionBridgeRecoveryTracker {
    fn note_disconnect(&mut self, now: Instant) {
        self.disconnected_at.get_or_insert(now);
    }

    fn take_duration_ms(&mut self, now: Instant) -> Option<u64> {
        let disconnected_at = self.disconnected_at.take()?;
        Some(now.saturating_duration_since(disconnected_at).as_millis() as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionSnapshotEmission {
    version: u64,
    session_count: usize,
}

fn emit_session_bridge_recovery_measurement(duration_ms: u64, emission: SessionSnapshotEmission) {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "duration_ms".to_string(),
        serde_json::Value::Number(serde_json::Number::from(duration_ms)),
    );
    fields.insert(
        "snapshot_version".to_string(),
        serde_json::Value::Number(serde_json::Number::from(emission.version)),
    );
    fields.insert(
        "session_count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(emission.session_count as u64)),
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "daemon.session_updates_bridge.recovered",
        Some("Session UI snapshot restored after daemon disconnect".to_string()),
        fields,
    );
}

/// What one focus-bridge iteration needs from `ProviderState`.
///
/// `wsl_distro` rides along with the address because both bridge connections
/// authenticate against the distro the daemon actually runs in — reading the
/// default distro's token file instead leaves an authenticated daemon rejecting
/// the app, and focus is the only thing this bridge carries live.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BridgeTarget {
    addr: String,
    connected: bool,
    wsl_distro: Option<String>,
}

fn bridge_target(provider: &ProviderState) -> Option<BridgeTarget> {
    let daemon = provider.daemon.as_ref()?;
    Some(BridgeTarget {
        addr: daemon.addr().to_string(),
        connected: daemon.is_connected(),
        wsl_distro: provider.wsl_distro.clone(),
    })
}

/// Open the bridge's own long-poll connection, and only hand it back if the
/// daemon answering it speaks this app's protocol.
///
/// The shared provider's connected flag is not a protocol check: a daemon
/// replaced under a running app — the `just install-daemon` loop, or an older
/// build that wins the port after a restart — keeps that flag true until the
/// health monitor's next ping. Since protocol v8 the hub snapshot is the only
/// live focus transport, so a v7 daemon adopted in that window serves focus
/// fields that decode as `None` with no hook chain left to cover for it. Ping on
/// the socket the bridge is about to consume, before the seed fetch runs on the
/// same daemon.
///
/// `reported_mismatch` latches the loud log: this runs on a one-second retry
/// loop, so an outdated daemon nobody rebuilds would otherwise fill the log.
fn connect_bridge_listener(
    addr: &str,
    wsl_distro: Option<&str>,
    reported_mismatch: &mut bool,
) -> Option<crate::daemon::session_listener::DaemonSessionListener> {
    let mut listener =
        match crate::daemon::session_listener::DaemonSessionListener::connect(addr, wsl_distro) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::debug!(error = %error, "Session listener connect failed");
                return None;
            }
        };

    match classify_daemon_health(listener.ping_protocol_version().map_err(|e| e.to_string())) {
        DaemonHealth::Healthy => {
            *reported_mismatch = false;
            Some(listener)
        }
        DaemonHealth::ProtocolMismatch { running, expected } => {
            if !*reported_mismatch {
                tracing::error!(
                    daemon_protocol_version = running,
                    expected,
                    "DAEMON IS OUTDATED — rebuild with `just install-daemon`"
                );
                *reported_mismatch = true;
            }
            None
        }
        DaemonHealth::Unreachable(error) => {
            tracing::debug!(error = %error, "Session listener protocol ping failed");
            None
        }
    }
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
        let mut recovery_tracker = SessionBridgeRecoveryTracker::default();
        let mut last_focus: Option<FocusEmission> = None;
        let mut last_degraded = false;
        let mut last_degraded_revision: u64 = 0;
        let mut reported_protocol_mismatch = false;
        tracing::info!("session updates bridge thread started");

        loop {
            let Some(target) = bridge_target(&app.state::<ProviderState>()) else {
                return;
            };
            let daemon_addr = target.addr;
            let wsl_distro = target.wsl_distro;

            if !target.connected {
                if observed_connected {
                    tracing::info!("session updates bridge: daemon disconnected");
                    observed_connected = false;
                    recovery_tracker.note_disconnect(Instant::now());
                }
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
            if !observed_connected {
                tracing::info!("session updates bridge: daemon connected");
                observed_connected = true;
            }

            // A connection this bridge cannot trust is a disconnect: neither the
            // seed fetch below nor the long poll may run against that daemon.
            let Some(mut listener) = connect_bridge_listener(
                &daemon_addr,
                wsl_distro.as_deref(),
                &mut reported_protocol_mismatch,
            ) else {
                std::thread::sleep(RETRY_DELAY);
                continue;
            };

            if let Some(emission) = emit_current_session_snapshot(
                &app,
                &daemon_addr,
                wsl_distro.as_deref(),
                &mut since_version,
                &mut last_focus,
                &mut last_degraded,
                &mut last_degraded_revision,
            ) {
                if let Some(duration_ms) = recovery_tracker.take_duration_ms(Instant::now()) {
                    emit_session_bridge_recovery_measurement(duration_ms, emission);
                }
            }

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

                match listener.wait_for_updates(since_version, last_degraded_revision, WAIT_TIMEOUT)
                {
                    Ok(update) => {
                        // Daemon restart: its version counter may reset, and so
                        // does its degradation revision. Reset both cursors and
                        // retry from a fresh baseline.
                        if update.version < since_version {
                            since_version = 0;
                            last_degraded_revision = 0;
                            continue;
                        }

                        since_version = update.version;
                        if take_focus_change(
                            &mut last_focus,
                            update.focus.as_ref(),
                            update.focus_project_path.as_deref(),
                        ) {
                            emit_tmux_focus_changed(
                                &app,
                                update.focus.as_ref(),
                                update.focus_project_path.as_deref(),
                            );
                        }
                        // A degraded cycle bumps no version, so `changed` stays
                        // false while the scanner is blind. Its edges still have
                        // to reach the app: the sessions it is looking at just
                        // stopped being an observation (or became one again).
                        // The revision catches the case the flag cannot — a
                        // blackout that began and ended inside this one wait.
                        let degraded_moved =
                            take_degraded_change(&mut last_degraded, update.degraded);
                        let blind_gap =
                            take_blind_gap(&mut last_degraded_revision, update.degraded_revision);
                        let observation_gap = observation_gap(blind_gap, update.degraded);
                        if update.changed || degraded_moved || blind_gap {
                            let mut sessions = update.sessions;
                            let session_count = sessions.len();
                            normalize_sessions_for_frontend(
                                &mut sessions,
                                wsl_distro.as_deref(),
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
                                sessions_updated_payload(
                                    update.version,
                                    &sessions,
                                    update.degraded,
                                    observation_gap,
                                ),
                            );
                            tracing::debug!(
                                version = update.version,
                                session_count = session_count,
                                degraded = update.degraded,
                                observation_gap,
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

/// The focus the bridge last handed the frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusEmission {
    focus: Option<TmuxFocus>,
    project_path: Option<String>,
}

/// Fold the outcome of a seed fetch into the bridge's cursor and focus.
///
/// Connect and reconnect both land here. A fetched snapshot is the newest hub
/// state the app has, so its focus goes through the same change detection as a
/// long poll: a focus that moved while the bridge was down is emitted now
/// instead of waiting out the next `WAIT_TIMEOUT`.
///
/// `None` — every seed attempt failed — clears the cursor. Keeping the old one
/// is worse than starting over: a daemon that restarted counts versions from
/// zero again, so the next long poll would find nothing newer than a cursor from
/// the previous process and block for the full `WAIT_TIMEOUT` with stale focus
/// on screen. A zero cursor makes the very next poll return the current
/// snapshot. `last_focus` is kept, so that poll still emits only a real change.
///
/// Returns whether the focus transition must be emitted.
fn apply_seed_outcome(
    snapshot: Option<&daemon::protocol::RuntimeSessionSnapshotResult>,
    since_version: &mut u64,
    last_focus: &mut Option<FocusEmission>,
    last_degraded: &mut bool,
    last_degraded_revision: &mut u64,
) -> bool {
    let Some(snapshot) = snapshot else {
        *since_version = 0;
        *last_degraded_revision = 0;
        return false;
    };

    *since_version = snapshot.version;
    // The seed emits its snapshot unconditionally, so the degradation status is
    // only recorded here — the long poll that follows must not repeat it as an
    // edge, nor re-report blackouts that predate the connection.
    *last_degraded = snapshot.degraded;
    *last_degraded_revision = snapshot.degraded_revision;
    take_focus_change(
        last_focus,
        snapshot.focus.as_ref(),
        snapshot.foreground_project_path.as_deref(),
    )
}

/// Whether the hub's focus moved since the last emission, recording the new one.
///
/// The hub reports focus on every long-poll response; the app only wants the
/// transitions.
fn take_focus_change(
    last: &mut Option<FocusEmission>,
    focus: Option<&TmuxFocus>,
    project_path: Option<&str>,
) -> bool {
    let next = FocusEmission {
        focus: focus.cloned(),
        project_path: project_path.map(str::to_string),
    };
    if last.as_ref() == Some(&next) {
        return false;
    }
    *last = Some(next);
    true
}

/// `sessions-updated` payload.
///
/// `degraded` tells the app the sessions are the hub's last good snapshot,
/// replayed while its scanner is blind, so `sessionStore` can stamp them and
/// `activitySignal.js` can present them as uncertain instead of holding the
/// last green reading.
///
/// `observation_gap` says the interval that ended with this emission contains a
/// stretch nothing observed — the scanner is blind now, or it was blind and
/// recovered between two answers. The app measures session time against that
/// interval, so it drops it instead of crediting it to the last state it saw.
fn sessions_updated_payload(
    version: u64,
    sessions: &[crate::session_scanner::DisplaySession],
    degraded: bool,
    observation_gap: bool,
) -> serde_json::Value {
    serde_json::json!({
        "version": version,
        "sessions": sessions,
        "degraded": degraded,
        "observation_gap": observation_gap,
    })
}

/// Whether the hub's degraded flag flipped since the last emission, recording it.
///
/// A degraded scanner cycle bumps no version, so the long poll keeps answering
/// `changed: false` and the app would hold its last good indicator for as long
/// as the scanner stays blind. The edges — and only the edges — are worth an
/// event: one when the sessions become continuity data, one when they are an
/// observation again.
fn take_degraded_change(last: &mut bool, degraded: bool) -> bool {
    if *last == degraded {
        return false;
    }
    *last = degraded;
    true
}

/// Whether a blackout edge happened since the last answer, advancing the cursor.
///
/// The hub bumps `degraded_revision` on both edges without touching the version,
/// so this is the only thing that can tell the app about a blackout that started
/// *and* ended while it was parked in one long poll — by then `degraded` is
/// false again and the flag alone says nothing happened. A daemon older than the
/// revision answers 0 forever, which never advances and so never claims a gap.
fn take_blind_gap(last_revision: &mut u64, revision: u64) -> bool {
    if revision <= *last_revision {
        return false;
    }
    *last_revision = revision;
    true
}

/// Whether the interval this emission closes ran through a blackout the app
/// never heard start.
///
/// The edge *into* a blackout is not one: the scanner was reporting normally
/// until a cadence ago, so the interval that ends here was observed, and
/// `degraded` stops the clock for everything after it. What the app cannot see
/// for itself is a blackout that was over again before the answer came back —
/// there the flag is false on both sides and only the revision moved.
fn observation_gap(blind_gap: bool, degraded: bool) -> bool {
    blind_gap && !degraded
}

/// `tmux-focus-changed` payload; `null` means nothing is focused.
fn tmux_focus_event_payload(
    focus: Option<&TmuxFocus>,
    project_id: Option<&str>,
) -> serde_json::Value {
    match focus {
        Some(focus) => serde_json::json!({
            "session": focus.session,
            "window": focus.window_index,
            "pane_id": focus.pane_id,
            "project_id": project_id,
        }),
        None => serde_json::Value::Null,
    }
}

/// Emit the focus transition, resolving the project id app-side.
fn emit_tmux_focus_changed(app: &AppHandle, focus: Option<&TmuxFocus>, project_path: Option<&str>) {
    let project_id = project_path.and_then(|project_path| {
        let provider = app.state::<ProviderState>();
        let localized = crate::commands::command_center::localize_daemon_project_path(
            &provider,
            project_path.to_string(),
        );
        let db = app.state::<crate::commands::projects::DbState>();
        match crate::commands::command_center::resolve_project_id_from_path(&db, &localized) {
            Ok(project_id) => project_id,
            Err(error) => {
                tracing::debug!(error = %error, "tmux focus project lookup failed");
                None
            }
        }
    });

    tracing::debug!(
        session = ?focus.map(|focus| focus.session.as_str()),
        window = ?focus.map(|focus| focus.window_index.as_str()),
        pane_id = ?focus.map(|focus| focus.pane_id.as_str()),
        project_id = ?project_id,
        "tmux focus changed"
    );
    emit_frontend_event(
        app,
        "tmux-focus-changed",
        tmux_focus_event_payload(focus, project_id.as_deref()),
    );
}

/// Fetch the current runtime session snapshot via a short-lived direct connection.
///
/// Bypasses the shared `DaemonProvider` to avoid contention during the startup
/// burst. The TCP connection is dropped when this function returns.
fn fetch_snapshot_direct(
    addr: &str,
    wsl_distro: Option<&str>,
) -> Result<daemon::protocol::RuntimeSessionSnapshotResult, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let stream = TcpStream::connect(addr)
        .map_err(|e| format!("Bridge snapshot connect to {addr} failed: {e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Bridge snapshot set timeout failed: {e}"))?;
    stream
        .set_nodelay(true)
        .map_err(|e| format!("Bridge snapshot set nodelay failed: {e}"))?;

    let auth_token = crate::daemon::auth::read_auth_token_for_distro(wsl_distro);
    let request = daemon::protocol::DaemonRequest::new(
        "bridge-snapshot",
        daemon::protocol::method::GET_RUNTIME_SESSION_SNAPSHOT,
        serde_json::Value::Null,
    )
    .with_auth(auth_token);

    let json = serde_json::to_string(&request)
        .map_err(|e| format!("Serialize bridge snapshot request failed: {e}"))?;
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("Clone bridge snapshot stream failed: {e}"))?;
    writer
        .write_all(json.as_bytes())
        .map_err(|e| format!("Write bridge snapshot request failed: {e}"))?;
    writer
        .write_all(b"\n")
        .map_err(|e| format!("Write bridge snapshot newline failed: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("Flush bridge snapshot request failed: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("Read bridge snapshot response failed: {e}"))?;
    if line.trim().is_empty() {
        return Err("Daemon returned empty bridge snapshot response".to_string());
    }

    let response: daemon::protocol::DaemonResponse = serde_json::from_str(&line)
        .map_err(|e| format!("Parse bridge snapshot response failed: {e}"))?;

    if let Some(err) = response.error {
        return Err(format!(
            "Daemon bridge snapshot error [{}]: {}",
            err.code, err.message
        ));
    }

    crate::commands::runtime_snapshot::decode_daemon_runtime_session_snapshot(response.result)
}

fn emit_current_session_snapshot(
    app: &AppHandle,
    addr: &str,
    wsl_distro: Option<&str>,
    since_version: &mut u64,
    last_focus: &mut Option<FocusEmission>,
    last_degraded: &mut bool,
    last_degraded_revision: &mut u64,
) -> Option<SessionSnapshotEmission> {
    use std::time::Duration;

    const MAX_SEED_RETRIES: u32 = 3;
    const SEED_RETRY_DELAY: Duration = Duration::from_millis(500);

    let mut snapshot = None;
    for attempt in 1..=MAX_SEED_RETRIES {
        match fetch_snapshot_direct(addr, wsl_distro) {
            Ok(s) => {
                snapshot = Some(s);
                break;
            }
            Err(error) => {
                tracing::debug!(
                    attempt,
                    max_retries = MAX_SEED_RETRIES,
                    error = %error,
                    "session bridge initial snapshot fetch failed"
                );
                if attempt < MAX_SEED_RETRIES {
                    std::thread::sleep(SEED_RETRY_DELAY);
                }
            }
        }
    }

    let focus_changed = apply_seed_outcome(
        snapshot.as_ref(),
        since_version,
        last_focus,
        last_degraded,
        last_degraded_revision,
    );
    let snapshot = snapshot?;

    // Cache for the polling path (list_cli_sessions)
    crate::session_snapshot_cache::store(&snapshot);

    if focus_changed {
        emit_tmux_focus_changed(
            app,
            snapshot.focus.as_ref(),
            snapshot.foreground_project_path.as_deref(),
        );
    }

    let mut sessions = snapshot.display_sessions;
    let session_count = sessions.len();
    normalize_sessions_for_frontend(
        &mut sessions,
        wsl_distro,
        crate::daemon::launcher::is_native_daemon(),
    );
    crate::coordination::activity_export::enrich_sessions_with_team_membership(
        app.state::<crate::coordination::state::CoordinationState>()
            .teams_dir(),
        &mut sessions,
    );

    emit_frontend_event(
        app,
        "sessions-updated",
        // A seed follows a stretch during which the app received no updates at
        // all — a connect, or a reconnect after the long poll failed — so the
        // interval it closes was not observed, whatever the hub says now.
        sessions_updated_payload(snapshot.version, &sessions, snapshot.degraded, true),
    );
    tracing::debug!(
        version = snapshot.version,
        session_count = session_count,
        degraded = snapshot.degraded,
        "session updates bridge emitted current snapshot after connect"
    );
    Some(SessionSnapshotEmission {
        version: snapshot.version,
        session_count,
    })
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
            claude_account_id: None,
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

    #[test]
    fn recover_daemon_connection_does_not_restart_when_reconnect_succeeds() {
        let mut reconnect_calls = 0;
        let mut restart_calls = 0;

        let result = recover_daemon_connection(
            || {
                reconnect_calls += 1;
                true
            },
            || {
                restart_calls += 1;
                true
            },
        );

        assert_eq!(result, DaemonRecoveryResult::Reconnected);
        assert_eq!(reconnect_calls, 1);
        assert_eq!(restart_calls, 0);
    }

    #[test]
    fn recover_daemon_connection_retries_after_restart() {
        let mut reconnect_calls = 0;
        let mut restart_calls = 0;

        let result = recover_daemon_connection(
            || {
                reconnect_calls += 1;
                reconnect_calls >= 2
            },
            || {
                restart_calls += 1;
                true
            },
        );

        assert_eq!(result, DaemonRecoveryResult::RestartedAndReconnected);
        assert_eq!(reconnect_calls, 2);
        assert_eq!(restart_calls, 1);
    }

    #[test]
    fn recover_daemon_connection_reports_failure_when_restart_does_not_help() {
        let mut reconnect_calls = 0;
        let mut restart_calls = 0;

        let result = recover_daemon_connection(
            || {
                reconnect_calls += 1;
                false
            },
            || {
                restart_calls += 1;
                true
            },
        );

        assert_eq!(result, DaemonRecoveryResult::Failed);
        assert_eq!(reconnect_calls, 2);
        assert_eq!(restart_calls, 1);
    }

    // Regression: health check and bootstrap both called try_restart_daemon
    // concurrently — each thread's stop killed the other's freshly-spawned
    // daemon, exhausting restart attempts before either succeeded.
    // Fix: the restart closure checks `bootstrap_complete` and returns false
    // (no restart, no attempt counted) while bootstrap is still in progress.
    #[test]
    fn recover_daemon_connection_skips_restart_when_gate_returns_false() {
        let mut reconnect_calls = 0;
        let mut restart_calls = 0;

        let result = recover_daemon_connection(
            || {
                reconnect_calls += 1;
                false
            },
            || {
                // Simulates the bootstrap_complete gate returning false
                restart_calls += 1;
                false
            },
        );

        assert_eq!(result, DaemonRecoveryResult::Failed);
        assert_eq!(
            reconnect_calls, 1,
            "should not retry reconnect after skipped restart"
        );
        assert_eq!(restart_calls, 1);
    }

    #[test]
    fn session_bridge_recovery_tracker_measures_disconnect_until_snapshot_restore() {
        let mut tracker = SessionBridgeRecoveryTracker::default();
        let disconnected_at = Instant::now();

        tracker.note_disconnect(disconnected_at);

        let duration_ms = tracker
            .take_duration_ms(disconnected_at + std::time::Duration::from_millis(1750))
            .expect("disconnect duration");

        assert_eq!(duration_ms, 1750);
        assert!(tracker
            .take_duration_ms(disconnected_at + std::time::Duration::from_millis(2000))
            .is_none());
    }

    #[test]
    fn session_bridge_recovery_tracker_keeps_first_disconnect_until_recovered() {
        let mut tracker = SessionBridgeRecoveryTracker::default();
        let disconnected_at = Instant::now();

        tracker.note_disconnect(disconnected_at);
        tracker.note_disconnect(disconnected_at + std::time::Duration::from_secs(4));

        let duration_ms = tracker
            .take_duration_ms(disconnected_at + std::time::Duration::from_secs(5))
            .expect("disconnect duration");

        assert_eq!(duration_ms, 5000);
    }

    #[test]
    fn local_claude_tasks_watch_is_preferred_when_windows_path_is_accessible() {
        assert!(prefer_local_claude_tasks_watch_for_host(true, true));
    }

    #[test]
    fn daemon_claude_tasks_watch_remains_enabled_without_local_windows_path() {
        assert!(!prefer_local_claude_tasks_watch_for_host(true, false));
    }

    fn focus(session: &str, window_index: &str, pane_id: &str) -> TmuxFocus {
        TmuxFocus {
            session: session.to_string(),
            window_index: window_index.to_string(),
            pane_id: pane_id.to_string(),
        }
    }

    // Regression: commits a53ad31 and f9c1e89. The focus signal used to reach
    // the app through tmux hooks writing a file; it is now a hub snapshot field
    // that arrives on every long-poll response, so the bridge must emit the
    // Tauri event only when the focus actually changed.
    #[test]
    fn focus_bridge_emits_once_per_change() {
        let mut last = None;

        assert!(take_focus_change(
            &mut last,
            Some(&focus("taurhaus", "2", "%9")),
            Some("/projects/mesh"),
        ));
        assert!(!take_focus_change(
            &mut last,
            Some(&focus("taurhaus", "2", "%9")),
            Some("/projects/mesh"),
        ));
        assert!(take_focus_change(
            &mut last,
            Some(&focus("taurhaus", "3", "%11")),
            Some("/projects/other"),
        ));
        assert!(take_focus_change(&mut last, None, None));
        assert!(!take_focus_change(&mut last, None, None));
    }

    // The frontend reads `degraded` off this payload (`sessionStore.svelte.js`)
    // to stamp the sessions it retains; the key must be present on every
    // emission, healthy ones included, so absence never reads as "unknown".
    #[test]
    fn sessions_updated_payload_always_carries_the_degraded_flag() {
        let healthy = sessions_updated_payload(4, &[], false, false);
        assert_eq!(healthy["version"], 4);
        assert_eq!(healthy["degraded"], serde_json::Value::Bool(false));
        assert!(healthy["sessions"].is_array());

        assert_eq!(
            sessions_updated_payload(4, &[], true, true)["degraded"],
            serde_json::Value::Bool(true)
        );
    }

    // Regression: 6c6f1cb presented a `degraded` record as uncertain, but a
    // degraded scanner cycle bumps no hub version, so the long poll answers
    // `changed: false` and the bridge emitted nothing — the app kept the last
    // good indicator for as long as the scanner stayed blind. The bridge emits
    // on the healthy->degraded and degraded->healthy edges, and only there: a
    // scanner that is blind for a minute must not produce an event per poll.
    #[test]
    fn degraded_bridge_emits_once_per_edge() {
        let mut last = false;

        assert!(!take_degraded_change(&mut last, false));
        assert!(take_degraded_change(&mut last, true));
        assert!(!take_degraded_change(&mut last, true));
        assert!(!take_degraded_change(&mut last, true));
        assert!(take_degraded_change(&mut last, false));
        assert!(!take_degraded_change(&mut last, false));
    }

    // Regression: fa572d4 emitted a degradation edge only when the long poll
    // happened to return while the scanner was still blind. Both edges of a
    // blackout that started and ended inside one 20 s wait left `degraded`
    // false on the answer, the bridge stayed silent, and the app credited the
    // blind interval as if it had been observed. The hub's degradation
    // revision now rides the answer: any advance means the interval the app is
    // about to measure contains a stretch nothing observed.
    #[test]
    fn a_blackout_between_two_answers_is_reported_as_an_unobserved_gap() {
        let mut cursor = 0;
        assert!(
            !take_blind_gap(&mut cursor, 0),
            "a healthy stretch reports no gap"
        );

        // Went blind and came back inside one wait: two edges, one answer.
        assert!(take_blind_gap(&mut cursor, 2));
        assert_eq!(cursor, 2);
        assert!(
            !take_blind_gap(&mut cursor, 2),
            "the same answer is not a second gap"
        );

        // Blind again at the next answer: a new edge, a new gap.
        assert!(take_blind_gap(&mut cursor, 3));
        assert!(!take_blind_gap(&mut cursor, 3));

        // A daemon older than the revision answers 0 forever and never claims
        // a gap; its `degraded` flag is all the app gets, as before.
        let mut old_daemon = 0;
        assert!(!take_blind_gap(&mut old_daemon, 0));
        assert!(!take_blind_gap(&mut old_daemon, 0));
    }

    // The app credits the interval between two emissions to the state it last
    // saw, so an emission has to say when that interval ran through a blackout
    // nobody watched. Going blind is not that interval: the scanner was fine
    // until a cadence ago, and `degraded` stops the clock from there.
    #[test]
    fn only_a_blackout_the_app_never_heard_start_is_an_unobserved_gap() {
        assert!(
            !observation_gap(false, false),
            "a healthy stretch closes an observed interval"
        );
        assert!(
            !observation_gap(true, true),
            "going blind now says nothing about the interval that just ended"
        );
        assert!(
            observation_gap(true, false),
            "recovered before the answer came back: the app never saw the blind stretch"
        );
        assert!(
            !observation_gap(false, true),
            "still blind, already reported: the clock is already stopped"
        );
    }

    // The app measures per-session time against the interval between two
    // observations (`sessionStore.svelte.js`), so an emission has to say
    // whether that interval was observed at all — `degraded` alone only covers
    // a blackout still in progress.
    #[test]
    fn sessions_updated_payload_reports_an_unobserved_gap() {
        let clean = sessions_updated_payload(4, &[], false, false);
        assert_eq!(clean["observation_gap"], serde_json::Value::Bool(false));

        let recovered = sessions_updated_payload(5, &[], false, true);
        assert_eq!(recovered["degraded"], serde_json::Value::Bool(false));
        assert_eq!(
            recovered["observation_gap"],
            serde_json::Value::Bool(true),
            "a recovered blackout still leaves an interval nobody watched"
        );
    }

    // The connect/reconnect seed always emits, so it records the flag rather
    // than folding an edge: the long poll that follows must not repeat it.
    #[test]
    fn seeding_records_the_degraded_flag_without_re_emitting_it() {
        let mut since_version = 0;
        let mut last_focus = None;
        let mut last_degraded = false;

        let mut last_degraded_revision = 0;

        let mut snapshot = seed_snapshot(3, None, None);
        snapshot.degraded = true;
        snapshot.degraded_revision = 5;
        let _ = apply_seed_outcome(
            Some(&snapshot),
            &mut since_version,
            &mut last_focus,
            &mut last_degraded,
            &mut last_degraded_revision,
        );

        assert!(last_degraded);
        assert!(!take_degraded_change(&mut last_degraded, true));
        assert!(take_degraded_change(&mut last_degraded, false));

        // Same for the blackout cursor: the seed adopts it, so the long poll it
        // hands over to does not re-report the blackouts that came before.
        assert_eq!(last_degraded_revision, 5);
        assert!(!take_blind_gap(&mut last_degraded_revision, 5));
        assert!(take_blind_gap(&mut last_degraded_revision, 6));
    }

    fn seed_snapshot(
        version: u64,
        focus: Option<TmuxFocus>,
        project_path: Option<&str>,
    ) -> daemon::protocol::RuntimeSessionSnapshotResult {
        daemon::protocol::RuntimeSessionSnapshotResult {
            version,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus,
            foreground_project_path: project_path.map(str::to_string),
            degraded: false,
            degraded_revision: 0,
        }
    }

    // Regression: commit 07ab6c5 routed focus through the long poll only. The
    // snapshot fetched on connect and on every reconnect advanced the version
    // cursor but never went through the focus fold, so a focus that moved while
    // the bridge was down stayed wrong on screen until an otherwise unchanged
    // poll timed out 20 s later — against a 500 ms hub cadence.
    #[test]
    fn focus_bridge_seeds_focus_from_the_connect_snapshot() {
        let mut since_version = 0;
        let mut last = None;

        assert!(
            apply_seed_outcome(
                Some(&seed_snapshot(
                    4,
                    Some(focus("taurhaus", "2", "%9")),
                    Some("/projects/a")
                )),
                &mut since_version,
                &mut last,
                &mut false,
                &mut 0,
            ),
            "the first snapshot carries the focus the hub already knows"
        );
        assert_eq!(since_version, 4);

        // The long poll that follows must not repeat what the seed emitted.
        assert!(!take_focus_change(
            &mut last,
            Some(&focus("taurhaus", "2", "%9")),
            Some("/projects/a"),
        ));

        // Focus moved while the bridge was disconnected: the reconnect snapshot
        // is the newest state the app has, so it emits now, not in 20 s.
        assert!(apply_seed_outcome(
            Some(&seed_snapshot(
                9,
                Some(focus("taurhaus", "3", "%11")),
                Some("/projects/b")
            )),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));
        assert_eq!(since_version, 9);

        // A reconnect that changed nothing stays quiet.
        assert!(!apply_seed_outcome(
            Some(&seed_snapshot(
                9,
                Some(focus("taurhaus", "3", "%11")),
                Some("/projects/b")
            )),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));

        // A hub with no focus at all clears the indicator once.
        assert!(apply_seed_outcome(
            Some(&seed_snapshot(11, None, None)),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));
        assert!(!apply_seed_outcome(
            Some(&seed_snapshot(11, None, None)),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));
    }

    // Regression: commit 07ab6c5 deleted the tmux hook -> file -> inotify focus
    // chain and b816dc7 bumped the protocol to 8 so startup replaces daemons
    // that predate hub-owned focus. The health monitor that runs for the rest of
    // the app's life still accepted anything that answered a ping, so a v7
    // daemon reconnecting late (or started by hand) passed the gate: its omitted
    // focus fields decode as `None` and the deleted chain has no replacement.
    #[test]
    fn health_check_rejects_a_daemon_that_predates_hub_owned_focus() {
        let v7: daemon::protocol::PingResult = serde_json::from_str(
            r#"{"version":"0.9.9","protocol_version":7,"uptime_secs":41,"data_root":"/home/u/.local/share/taurhaus"}"#,
        )
        .expect("v7 ping payload");

        assert_eq!(
            classify_daemon_health(Ok(v7.protocol_version)),
            DaemonHealth::ProtocolMismatch {
                running: 7,
                expected: daemon::protocol::PROTOCOL_VERSION,
            }
        );
        assert_eq!(
            classify_daemon_health(Ok(daemon::protocol::PROTOCOL_VERSION)),
            DaemonHealth::Healthy
        );
        assert_eq!(
            classify_daemon_health(Err("connection refused".to_string())),
            DaemonHealth::Unreachable("connection refused".to_string())
        );
    }

    // Regression: same commits. Recovery treated any successful TCP reconnect as
    // a recovered daemon, so the protocol gate could be walked around by simply
    // dropping and restoring the connection.
    #[test]
    fn a_reconnected_daemon_with_the_wrong_protocol_is_not_treated_as_recovered() {
        let mut disconnects = Vec::new();
        assert!(!confirm_daemon_protocol(
            || Ok(daemon::protocol::PROTOCOL_VERSION - 1),
            |reason| disconnects.push(reason.to_string()),
        ));
        assert_eq!(
            disconnects.len(),
            1,
            "the stale daemon must be dropped so the restart path replaces it"
        );

        let mut disconnects = Vec::new();
        assert!(!confirm_daemon_protocol(
            || Err("read timed out".to_string()),
            |reason| disconnects.push(reason.to_string()),
        ));
        assert_eq!(disconnects.len(), 1);

        let mut disconnects = Vec::new();
        assert!(confirm_daemon_protocol(
            || Ok(daemon::protocol::PROTOCOL_VERSION),
            |reason| disconnects.push(reason.to_string()),
        ));
        assert!(disconnects.is_empty());
    }

    // Regression: commit 07ab6c5 seeded the bridge from a snapshot fetched on
    // connect, but when every seed attempt failed the cursor kept its
    // pre-disconnect value. A restarted daemon counts from a lower version, so
    // the following long poll had nothing newer to report and blocked for the
    // full 20 s WAIT_TIMEOUT — on top of three 5 s seed timeouts — while a stale
    // focus stayed on screen.
    #[test]
    fn a_failed_seed_clears_the_cursor_so_a_restarted_daemon_is_seen_at_once() {
        let mut since_version = 42;
        let mut last = None;
        assert!(apply_seed_outcome(
            Some(&seed_snapshot(
                42,
                Some(focus("taurhaus", "2", "%9")),
                Some("/projects/a")
            )),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));

        assert!(
            !apply_seed_outcome(None, &mut since_version, &mut last, &mut false, &mut 0),
            "a seed that never arrived has no focus to emit"
        );
        assert_eq!(
            since_version, 0,
            "the cursor must not outlive the connection it was counted on"
        );

        // The restarted daemon's counter is lower than the pre-restart cursor:
        // with the stale cursor kept, this snapshot would have been withheld.
        let restarted = seed_snapshot(3, Some(focus("taurhaus", "5", "%21")), Some("/projects/b"));
        assert!(restarted.version > since_version);
        assert!(apply_seed_outcome(
            Some(&restarted),
            &mut since_version,
            &mut last,
            &mut false,
            &mut 0,
        ));
        assert_eq!(since_version, 3);
    }

    // Regression: 07ab6c5 deleted the hook -> file -> inotify focus chain, leaving
    // this bridge as the app's only live tmux-focus transport. It read just the
    // daemon address and connection flag from `ProviderState`, so both of its
    // connections — the long-poll listener and the direct seed fetch — loaded
    // their auth token with `read_auth_token()`, i.e. from whichever WSL distro
    // is default on Windows rather than the one the daemon runs in. An
    // authenticated daemon in a non-default distro rejected both, and focus
    // never moved.
    #[test]
    fn the_focus_bridge_carries_the_configured_distro_to_both_daemon_connections() {
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: Some(
                crate::provider::daemon_client::DaemonProvider::new_disconnected_with_distro(
                    "127.0.0.1:9",
                    Some("Taurhaus-Ubuntu"),
                ),
            ),
            wsl_distro: Some("Taurhaus-Ubuntu".to_string()),
        };

        let target = bridge_target(&provider).expect("bridge target");

        assert_eq!(target.addr, "127.0.0.1:9");
        assert!(!target.connected);
        assert_eq!(
            target.wsl_distro.as_deref(),
            Some("Taurhaus-Ubuntu"),
            "the bridge must authenticate against the distro the daemon runs in"
        );
    }

    #[test]
    fn a_bridge_without_a_daemon_has_no_target() {
        let provider = ProviderState {
            local: crate::provider::local::LocalProvider,
            daemon: None,
            wsl_distro: Some("Taurhaus-Ubuntu".to_string()),
        };

        assert!(bridge_target(&provider).is_none());
    }

    // Regression: 07ab6c5 deleted the hook -> file -> inotify focus chain and
    // b816dc7 bumped the protocol to 8, and a0f3545/108481f then gated the
    // health monitor and every inline reconnect. The focus bridge still opened
    // its own two connections — the long-poll listener and the seed fetch —
    // after reading nothing but the shared provider's `is_connected` flag. A
    // daemon replaced under a live app (the `just install-daemon` loop, or an
    // older build that wins the port on restart) leaves that flag true for as
    // long as it takes the health monitor to notice, so the bridge adopted a v7
    // connection whose omitted focus fields decode as `None` and drove the
    // foreground indicator from it.
    #[test]
    fn the_focus_bridge_refuses_a_listener_connection_from_an_outdated_daemon() {
        let _guard = crate::test_support::acquire_heavy_test_guard();

        let stub = crate::test_support::StubDaemon::start(
            daemon::protocol::PROTOCOL_VERSION - 1,
            serde_json::Value::Null,
        );

        let mut reported_mismatch = false;
        assert!(
            connect_bridge_listener(stub.addr(), None, &mut reported_mismatch).is_none(),
            "the bridge must validate the protocol on the connection it is about to consume"
        );
        assert!(
            reported_mismatch,
            "the first refusal must say the daemon is outdated"
        );

        assert!(
            connect_bridge_listener(stub.addr(), None, &mut reported_mismatch).is_none(),
            "the refusal holds for every retry against the same daemon"
        );
        assert!(
            reported_mismatch,
            "the mismatch stays reported so the one-second retry loop does not repeat it"
        );
    }

    #[test]
    fn the_focus_bridge_consumes_a_listener_connection_speaking_this_protocol() {
        let _guard = crate::test_support::acquire_heavy_test_guard();

        let stub = crate::test_support::StubDaemon::start(
            daemon::protocol::PROTOCOL_VERSION,
            serde_json::Value::Null,
        );

        // Latched by an earlier outage: a healthy daemon has to clear it so the
        // next mismatch is loud again.
        let mut reported_mismatch = true;
        assert!(connect_bridge_listener(stub.addr(), None, &mut reported_mismatch).is_some());
        assert!(!reported_mismatch);
    }

    #[test]
    fn a_bridge_listener_that_cannot_connect_is_not_reported_as_outdated() {
        // Bind and release an ephemeral port so nothing answers on it: an
        // unreachable daemon is a retry, not an outdated build, and it must not
        // latch the "rebuild the daemon" message over a real mismatch.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("bind closed port");
        let addr = closed.local_addr().expect("closed port addr").to_string();
        drop(closed);

        let mut reported_mismatch = false;
        assert!(connect_bridge_listener(&addr, None, &mut reported_mismatch).is_none());
        assert!(!reported_mismatch);
    }

    #[test]
    fn focus_event_payload_carries_the_resolved_project_id() {
        let payload = tmux_focus_event_payload(Some(&focus("taurhaus", "2", "%9")), Some("p1"));
        assert_eq!(payload["session"], "taurhaus");
        assert_eq!(payload["window"], "2");
        assert_eq!(payload["pane_id"], "%9");
        assert_eq!(payload["project_id"], "p1");

        let unresolved = tmux_focus_event_payload(Some(&focus("taurhaus", "2", "%9")), None);
        assert!(unresolved["project_id"].is_null());

        assert!(tmux_focus_event_payload(None, None).is_null());
    }
}
