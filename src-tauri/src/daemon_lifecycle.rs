use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::provider::platform_paths::PlatformPaths;
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
                    recovery_tracker.note_disconnect(Instant::now());
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

            if let Some(emission) = emit_current_session_snapshot(&app, &mut since_version) {
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

fn emit_current_session_snapshot(
    app: &AppHandle,
    since_version: &mut u64,
) -> Option<SessionSnapshotEmission> {
    let provider_state = app.state::<ProviderState>();
    let snapshot =
        crate::commands::runtime_snapshot::daemon_runtime_session_snapshot(&provider_state)
            .unwrap_or_else(|error| {
                tracing::debug!(
                    error = %error,
                    "session updates bridge failed to fetch current snapshot after reconnect"
                );
                crate::commands::runtime_snapshot::RuntimeSnapshotOutcome {
                    snapshot: None,
                    freshness:
                        crate::commands::runtime_snapshot::RuntimeSnapshotFreshness::Unavailable,
                }
            })
            .snapshot?;

    let mut sessions = snapshot.display_sessions;
    let session_count = sessions.len();
    let distro = provider_state.wsl_distro.clone();
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

    *since_version = snapshot.version;
    emit_frontend_event(
        app,
        "sessions-updated",
        serde_json::json!({
            "version": snapshot.version,
            "sessions": sessions,
        }),
    );
    tracing::debug!(
        version = snapshot.version,
        session_count = session_count,
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
}
