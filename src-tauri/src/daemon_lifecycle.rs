use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::{daemon, db, fs, models, provider, services, ProviderState, WatcherState};

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

/// Start daemon event listener for WSL projects.
///
/// Opens a dedicated TCP connection to the daemon, sends `watch` commands for
/// each WSL project, then runs the event loop. Events are forwarded to the
/// shared watcher channel, where `process_watch_events` handles them identically
/// to local watcher events.
///
/// On macOS/Linux (native daemon), this is a no-op — all project paths are local
/// and the local watcher handles them. The function still runs for consistency
/// but registers zero watches and exits immediately.
pub(crate) fn start_daemon_watches(
    daemon_addr: String,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    wsl_distro: Option<String>,
    projects: Vec<models::Project>,
) {
    let mut listener =
        match daemon::event_listener::DaemonEventListener::connect(&daemon_addr, event_tx) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to connect daemon event listener");
                return;
            }
        };

    // Register watches for all WSL projects
    let mut count = 0;
    let mut wsl_home: Option<String> = None;
    for project in &projects {
        if !provider::path::is_wsl_path(&project.path) {
            continue;
        }

        // Convert UNC path to Linux path for the daemon
        let linux_path = match provider::path::wsl_unc_to_linux(&project.path) {
            Some(p) => p,
            None => {
                tracing::warn!(path = %project.path, "Cannot convert WSL path to Linux");
                continue;
            }
        };

        // Extract WSL home from first successful conversion
        if wsl_home.is_none() {
            wsl_home = extract_wsl_home(&linux_path);
        }

        if let Err(e) = listener.watch(&project.id, &linux_path) {
            tracing::warn!(
                project = project.name,
                error = %e,
                "Failed to register daemon watch"
            );
        } else {
            count += 1;
        }
    }

    // Watch Claude task directories for event-driven task sync.
    // Uses a special "__claude_tasks__" project ID that process_watch_events
    // intercepts to trigger background task scanning instead of normal file handling.
    if let Some(ref home) = wsl_home {
        let claude_tasks_dir = format!("{home}/.claude/tasks");
        if let Err(e) = listener.watch("__claude_tasks__", &claude_tasks_dir) {
            tracing::debug!(
                error = %e,
                path = %claude_tasks_dir,
                "Could not watch Claude tasks directory (may not exist yet)"
            );
        } else {
            tracing::info!(path = %claude_tasks_dir, "Watching Claude tasks directory (daemon)");
        }
    }

    if count > 0 || wsl_home.is_some() {
        tracing::info!(
            count,
            distro = ?wsl_distro,
            "Daemon watching WSL projects"
        );
        // Run blocks until daemon disconnects
        listener.run();
    }
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
        Err(_) => return,
    };

    let db_state = app.state::<commands::projects::DbState>();
    let projects = match db_state.0.lock() {
        Ok(conn) => db::queries::list_projects(&conn).unwrap_or_default(),
        Err(_) => return,
    };

    tracing::info!(
        project_count = projects.len(),
        "Re-registering daemon watches after reconnection"
    );

    std::thread::spawn(move || {
        start_daemon_watches(daemon_addr, event_tx, distro, projects);
    });

    // Also re-scan sessions that may have been missed while disconnected
    {
        let db_state = app.state::<commands::projects::DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let all_projects = db::queries::list_projects(&conn).unwrap_or_default();
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
                    _ => {}
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
        // Use shorter interval while waiting for first connection
        let interval = if ever_connected {
            CHECK_INTERVAL
        } else {
            FAST_CHECK_INTERVAL
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
                    consecutive_failures = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        failures = consecutive_failures,
                        error = %e,
                        "Daemon health check failed"
                    );
                    if consecutive_failures >= 3 {
                        let _ = app.emit(
                            "daemon-status",
                            serde_json::json!({ "status": "disconnected" }),
                        );
                    }
                }
            }
        } else {
            // Daemon is disconnected — try to reconnect
            if restart_attempts >= MAX_RESTART_ATTEMPTS {
                tracing::warn!(
                    "Max daemon restart attempts reached ({MAX_RESTART_ATTEMPTS}), giving up"
                );
                let _ = app.emit("daemon-status", serde_json::json!({ "status": "failed" }));
                return;
            }

            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "reconnecting" }),
            );

            // Try reconnecting to existing daemon first
            if daemon.reconnect().is_ok() {
                tracing::info!("Reconnected to daemon");
                consecutive_failures = 0;
                restart_attempts = 0;
                ever_connected = true;
                respawn_daemon_watches(&app);
                let _ = app.emit(
                    "daemon-status",
                    serde_json::json!({ "status": "connected" }),
                );
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
                        respawn_daemon_watches(&app);
                        let _ = app.emit(
                            "daemon-status",
                            serde_json::json!({ "status": "connected" }),
                        );
                        continue;
                    }
                }
            }

            tracing::warn!(attempt = restart_attempts, "Daemon restart attempt failed");
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

        loop {
            let (daemon_addr, connected) = {
                let provider_state = app.state::<ProviderState>();
                let Some(ref daemon) = provider_state.daemon else {
                    return;
                };
                (daemon.addr().to_string(), daemon.is_connected())
            };

            if !connected {
                std::thread::sleep(RETRY_DELAY);
                continue;
            }

            let mut listener =
                match crate::daemon::session_listener::DaemonSessionListener::connect(
                    &daemon_addr,
                ) {
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
                            let _ = app.emit(
                                "sessions-updated",
                                serde_json::json!({
                                    "version": update.version,
                                    "sessions": update.sessions,
                                }),
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
