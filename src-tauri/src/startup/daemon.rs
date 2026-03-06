use tauri::{AppHandle, Emitter, Manager};

use crate::{daemon_lifecycle, ProviderState};
use serde_json::{Map, Value};

use super::SetupContext;

pub(crate) fn spawn_background_bootstrap(app: AppHandle, context: &SetupContext) {
    let boot_distro = context.wsl_distro.clone();
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
                tracing::info!("Background bootstrap: starting daemon");
                let port = crate::daemon::server::DEFAULT_PORT;

                if let Err(error) = crate::daemon::launcher::try_restart_daemon(distro, port) {
                    tracing::warn!(error = %error, "Failed to start daemon in background");
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
                                Value::String(error.to_string()),
                            );
                            fields
                        },
                    );
                } else {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let provider_state = app.state::<ProviderState>();
                    if let Some(ref daemon) = provider_state.daemon {
                        if daemon.reconnect().is_ok() {
                            tracing::info!("Background bootstrap: daemon connected");
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
                                    fields.insert(
                                        "daemon_addr".to_string(),
                                        Value::String(addr.clone()),
                                    );
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
                                        Value::String(
                                            "DAEMON_BOOTSTRAP_RECONNECT_FAILED".to_string(),
                                        ),
                                    );
                                    fields.insert(
                                        "error.message".to_string(),
                                        Value::String(
                                            "daemon reconnect failed after bootstrap start"
                                                .to_string(),
                                        ),
                                    );
                                    fields
                                },
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
