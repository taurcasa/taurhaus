use tauri::{AppHandle, Emitter, Manager};

use crate::{daemon_lifecycle, ProviderState};

use super::SetupContext;

pub(crate) fn spawn_background_bootstrap(app: AppHandle, context: &SetupContext) {
    let boot_distro = context.wsl_distro.clone();
    let boot_log_path = context.log_path.clone();
    let boot_connected = context.daemon_connected_at_startup;

    std::thread::spawn(move || {
        if !boot_connected {
            if let Some(ref distro) = boot_distro {
                tracing::info!("Background bootstrap: starting daemon");
                let port = crate::daemon::server::DEFAULT_PORT;

                if let Err(error) = crate::daemon::launcher::try_restart_daemon(distro, port) {
                    tracing::warn!(error = %error, "Failed to start daemon in background");
                } else {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let provider_state = app.state::<ProviderState>();
                    if let Some(ref daemon) = provider_state.daemon {
                        if daemon.reconnect().is_ok() {
                            tracing::info!("Background bootstrap: daemon connected");
                            daemon_lifecycle::respawn_daemon_watches(&app);
                            let _ = app.emit(
                                "daemon-status",
                                serde_json::json!({ "status": "connected" }),
                            );
                        }
                    }
                }
            }
        }

        let provider_state = app.state::<ProviderState>();
        if let Some(ref daemon) = provider_state.daemon {
            if daemon.is_connected() {
                let expected = crate::daemon::protocol::PROTOCOL_VERSION;
                match daemon.ping_protocol_version() {
                    Ok(version) if version < expected => {
                        tracing::error!(
                            daemon_version = version,
                            expected,
                            "DAEMON IS OUTDATED — rebuild with `just install-daemon`"
                        );
                    }
                    Ok(version) => {
                        tracing::info!(protocol_version = version, "Daemon protocol version OK");
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "Could not check daemon protocol version");
                    }
                }
            }
        }

        if let Some(ref distro) = boot_distro {
            crate::daemon::launcher::ensure_tmux_session(distro, &boot_log_path);
        }
    });
}

pub(crate) fn start_runtime_monitors(app: AppHandle, context: &SetupContext) {
    if context.wsl_distro.is_some() {
        let health_handle = app.clone();
        let connected_at_startup = context.daemon_connected_at_startup;
        std::thread::spawn(move || {
            daemon_lifecycle::daemon_health_check(health_handle, connected_at_startup);
        });

        daemon_lifecycle::start_session_updates_bridge(app);
    }
}
