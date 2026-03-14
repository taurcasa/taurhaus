use tauri::{AppHandle, Emitter, Manager};

use crate::{daemon_lifecycle, ProviderState};
use serde_json::{Map, Value};

use super::SetupContext;

const STARTUP_DAEMON_RUNTIME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) fn spawn_background_bootstrap(app: AppHandle, context: &SetupContext) {
    let boot_distro = context.wsl_distro.clone();
    let boot_data_dir = context.data_dir.clone();
    let boot_log_path = context.log_path.clone();
    let boot_connected = context.daemon_connected_at_startup;
    let daemon_addr = context.daemon_addr.clone();

    emit_startup_event(
        "info",
        "startup.bootstrap_thread.spawned",
        "Startup bootstrap thread spawned",
        {
            let mut fields = Map::new();
            fields.insert(
                "thread_name".to_string(),
                Value::String("startup-bootstrap".to_string()),
            );
            fields.insert(
                "connected_at_startup".to_string(),
                Value::Bool(boot_connected),
            );
            fields
        },
    );

    std::thread::spawn(move || {
        if !boot_connected {
            if let Some(ref distro) = boot_distro {
                let bootstrap_started_at = std::time::Instant::now();
                let addr = daemon_addr.clone().unwrap_or_else(|| {
                    format!("127.0.0.1:{}", crate::daemon::server::DEFAULT_PORT)
                });
                emit_startup_event(
                    "info",
                    "startup.daemon_bootstrap.started",
                    "Startup daemon bootstrap started",
                    {
                        let mut fields = Map::new();
                        fields.insert("daemon_addr".to_string(), Value::String(addr.clone()));
                        fields.insert("wsl_distro".to_string(), Value::String(distro.clone()));
                        fields
                    },
                );
                let port = crate::daemon::server::DEFAULT_PORT;
                let provider_state = app.state::<ProviderState>();
                let Some(ref daemon) = provider_state.daemon else {
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
                        &boot_data_dir,
                        false,
                        true,
                        "daemon_provider_missing_local_fallback",
                    );
                    emit_startup_event(
                        "warn",
                        "startup.daemon_bootstrap.failed",
                        "Startup daemon bootstrap failed",
                        {
                            let mut fields = Map::new();
                            fields.insert("daemon_addr".to_string(), Value::String(addr));
                            fields.insert(
                                "duration_ms".to_string(),
                                Value::Number(serde_json::Number::from(
                                    bootstrap_started_at.elapsed().as_millis() as u64,
                                )),
                            );
                            fields.insert(
                                "error.code".to_string(),
                                Value::String("DAEMON_BOOTSTRAP_START_FAILED".to_string()),
                            );
                            fields.insert(
                                "error.message".to_string(),
                                Value::String(
                                    "daemon provider missing during startup bootstrap".to_string(),
                                ),
                            );
                            fields
                        },
                    );
                    return;
                };

                if let Err(error) = crate::commands::daemon::ensure_bundled_daemon_installed(&app) {
                    tracing::warn!(error = %error, "Failed to ensure bundled daemon install during startup");
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
                        &boot_data_dir,
                        false,
                        true,
                        "daemon_install_failed_local_fallback",
                    );
                    emit_startup_event(
                        "warn",
                        "startup.daemon_bootstrap.failed",
                        "Startup daemon bootstrap failed",
                        {
                            let mut fields = Map::new();
                            fields.insert("daemon_addr".to_string(), Value::String(addr));
                            fields.insert(
                                "duration_ms".to_string(),
                                Value::Number(serde_json::Number::from(
                                    bootstrap_started_at.elapsed().as_millis() as u64,
                                )),
                            );
                            fields.insert(
                                "error.code".to_string(),
                                Value::String("DAEMON_BOOTSTRAP_INSTALL_FAILED".to_string()),
                            );
                            fields.insert("error.message".to_string(), Value::String(error));
                            fields
                        },
                    );
                    return;
                }

                let connected = reconnect_existing_daemon_if_expected(daemon).is_ok()
                    || (crate::daemon::launcher::try_restart_daemon(distro, port).is_ok()
                        && crate::daemon::launcher::reconnect_existing_provider_until_reachable(
                            daemon, port,
                        )
                        .is_ok()
                        && validate_connected_daemon_runtime(daemon).is_ok());

                if connected {
                    tracing::info!("Background bootstrap: daemon connected");
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
                        &boot_data_dir,
                        true,
                        false,
                        "daemon_connected",
                    );
                    daemon_lifecycle::respawn_daemon_watches(&app);
                    emit_frontend_event(
                        &app,
                        "daemon-status",
                        serde_json::json!({ "status": "connected" }),
                    );
                    emit_startup_event(
                        "info",
                        "startup.daemon_bootstrap.completed",
                        "Startup daemon bootstrap completed",
                        {
                            let mut fields = Map::new();
                            fields.insert(
                                "status".to_string(),
                                Value::String("connected".to_string()),
                            );
                            fields.insert("daemon_addr".to_string(), Value::String(addr.clone()));
                            fields.insert(
                                "duration_ms".to_string(),
                                Value::Number(serde_json::Number::from(
                                    bootstrap_started_at.elapsed().as_millis() as u64,
                                )),
                            );
                            fields
                        },
                    );
                } else {
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
                        &boot_data_dir,
                        false,
                        true,
                        "daemon_failed_local_fallback",
                    );
                    emit_startup_event(
                        "warn",
                        "startup.daemon_bootstrap.failed",
                        "Startup daemon bootstrap failed",
                        {
                            let mut fields = Map::new();
                            fields.insert("daemon_addr".to_string(), Value::String(addr));
                            fields.insert(
                                "duration_ms".to_string(),
                                Value::Number(serde_json::Number::from(
                                    bootstrap_started_at.elapsed().as_millis() as u64,
                                )),
                            );
                            fields.insert(
                                "error.code".to_string(),
                                Value::String("DAEMON_BOOTSTRAP_RECONNECT_FAILED".to_string()),
                            );
                            fields.insert(
                                "error.message".to_string(),
                                Value::String(
                                    "daemon did not become reachable after startup bootstrap"
                                        .to_string(),
                                ),
                            );
                            fields
                        },
                    );
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
                        emit_startup_event(
                            "error",
                            "startup.daemon_protocol.checked",
                            "Startup daemon protocol checked",
                            {
                                let mut fields = Map::new();
                                fields.insert(
                                    "daemon_protocol_version".to_string(),
                                    Value::Number(serde_json::Number::from(version)),
                                );
                                fields.insert(
                                    "expected_protocol_version".to_string(),
                                    Value::Number(serde_json::Number::from(expected)),
                                );
                                fields.insert(
                                    "status".to_string(),
                                    Value::String("outdated".to_string()),
                                );
                                fields
                            },
                        );
                    }
                    Ok(version) => {
                        tracing::info!(protocol_version = version, "Daemon protocol version OK");
                        emit_startup_event(
                            "info",
                            "startup.daemon_protocol.checked",
                            "Startup daemon protocol checked",
                            {
                                let mut fields = Map::new();
                                fields.insert(
                                    "daemon_protocol_version".to_string(),
                                    Value::Number(serde_json::Number::from(version)),
                                );
                                fields.insert(
                                    "expected_protocol_version".to_string(),
                                    Value::Number(serde_json::Number::from(expected)),
                                );
                                fields
                                    .insert("status".to_string(), Value::String("ok".to_string()));
                                fields
                            },
                        );
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "Could not check daemon protocol version");
                        emit_startup_event(
                            "warn",
                            "startup.daemon_protocol.checked",
                            "Startup daemon protocol checked",
                            {
                                let mut fields = Map::new();
                                fields.insert(
                                    "expected_protocol_version".to_string(),
                                    Value::Number(serde_json::Number::from(expected)),
                                );
                                fields.insert(
                                    "status".to_string(),
                                    Value::String("check_failed".to_string()),
                                );
                                fields.insert(
                                    "error.code".to_string(),
                                    Value::String("DAEMON_PROTOCOL_CHECK_FAILED".to_string()),
                                );
                                fields.insert(
                                    "error.message".to_string(),
                                    Value::String(error.to_string()),
                                );
                                fields
                            },
                        );
                    }
                }
            }
        }

        if let Some(ref distro) = boot_distro {
            crate::daemon::launcher::ensure_tmux_session(distro, &boot_log_path);
        }
        if let Err(error) = crate::commands::mesh::ensure_bundled_mesh_installed(&app) {
            tracing::warn!(error = %error, "Failed to ensure bundled mesh install during startup");
        }
    });
}

fn reconnect_existing_daemon_if_expected(
    daemon: &crate::provider::daemon_client::DaemonProvider,
) -> Result<(), String> {
    daemon
        .reconnect()
        .map_err(|error| format!("daemon reconnect failed: {error}"))?;
    validate_connected_daemon_runtime(daemon)
}

fn validate_connected_daemon_runtime(
    daemon: &crate::provider::daemon_client::DaemonProvider,
) -> Result<(), String> {
    let ping = daemon
        .ping_info_with_timeout(STARTUP_DAEMON_RUNTIME_TIMEOUT)
        .map_err(|error| format!("daemon ping failed after reconnect: {error}"))?;

    ensure_expected_daemon_runtime(&ping).inspect_err(|_| {
        daemon.disconnect("startup_runtime_mismatch");
    })
}

fn ensure_expected_daemon_runtime(
    ping: &crate::daemon::protocol::PingResult,
) -> Result<(), String> {
    if ping.protocol_version != crate::daemon::protocol::PROTOCOL_VERSION {
        return Err(format!(
            "daemon protocol mismatch: running={}, expected={}",
            ping.protocol_version,
            crate::daemon::protocol::PROTOCOL_VERSION
        ));
    }

    let expected_version = env!("CARGO_PKG_VERSION");
    if ping.version.trim() != expected_version {
        return Err(format!(
            "daemon version mismatch: running={}, expected={expected_version}",
            ping.version.trim(),
        ));
    }

    Ok(())
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

fn emit_startup_event(level: &str, event: &str, message: &'static str, fields: Map<String, Value>) {
    crate::commands::logging::emit_global(
        level,
        "backend",
        event,
        Some(message.to_string()),
        fields,
    );
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

#[cfg(test)]
mod tests {
    use super::ensure_expected_daemon_runtime;

    #[test]
    fn ensure_expected_daemon_runtime_accepts_matching_contract() {
        let ping = crate::daemon::protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            uptime_secs: 1,
        };

        assert!(ensure_expected_daemon_runtime(&ping).is_ok());
    }

    #[test]
    fn ensure_expected_daemon_runtime_rejects_version_mismatch() {
        let ping = crate::daemon::protocol::PingResult {
            version: "0.0.1".to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            uptime_secs: 1,
        };

        let error = ensure_expected_daemon_runtime(&ping).expect_err("mismatch should fail");
        assert!(error.contains("daemon version mismatch"));
    }

    #[test]
    fn ensure_expected_daemon_runtime_rejects_protocol_mismatch() {
        let ping = crate::daemon::protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION.saturating_sub(1),
            uptime_secs: 1,
        };

        let error = ensure_expected_daemon_runtime(&ping).expect_err("mismatch should fail");
        assert!(error.contains("daemon protocol mismatch"));
    }
}
