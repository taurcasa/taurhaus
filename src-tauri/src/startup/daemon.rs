use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::{daemon_lifecycle, ProviderState};
use serde_json::{Map, Value};

use super::telemetry::emit_startup_event;
use super::SetupContext;

const STARTUP_DAEMON_RUNTIME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) fn spawn_background_bootstrap(
    app: AppHandle,
    context: &SetupContext,
    bootstrap_complete: Arc<AtomicBool>,
) {
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
                    format!("127.0.0.1:{}", crate::daemon::server::app_daemon_port())
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
                let port = crate::daemon::server::app_daemon_port();
                let provider_state = app.state::<ProviderState>();
                let Some(ref daemon) = provider_state.daemon else {
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
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
                    bootstrap_complete.store(true, Ordering::Release);
                    return;
                };

                if let Err(error) = crate::commands::daemon::ensure_bundled_daemon_installed(&app) {
                    tracing::warn!(error = %error, "Failed to ensure bundled daemon install during startup");
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
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
                    bootstrap_complete.store(true, Ordering::Release);
                    return;
                }

                let connected = reconnect_existing_daemon_if_expected(daemon).is_ok()
                    || (crate::daemon::launcher::try_restart_daemon_at(
                        distro,
                        port,
                        &boot_log_path,
                    )
                    .is_ok()
                        && crate::daemon::launcher::reconnect_existing_provider_until_reachable(
                            daemon, port,
                        )
                        .is_ok()
                        && validate_connected_daemon_runtime(daemon).is_ok());

                if connected {
                    tracing::info!("Background bootstrap: daemon connected");
                    crate::startup::watchers::refresh_auxiliary_watches(
                        &app,
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

        // Signal that bootstrap is done so the health check can take over.
        bootstrap_complete.store(true, Ordering::Release);

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

        emit_startup_event(
            "info",
            "startup.mesh_install.started",
            "Startup mesh install check started",
            Map::new(),
        );
        match crate::commands::mesh::ensure_bundled_mesh_installed(&app) {
            Ok(Some(result)) => {
                let mut fields = Map::new();
                fields.insert("status".to_string(), Value::String("installed".to_string()));
                fields.insert("message".to_string(), Value::String(result.message.clone()));
                emit_startup_event(
                    "info",
                    "startup.mesh_install.completed",
                    "Startup mesh install check completed",
                    fields,
                );
            }
            Ok(None) => {
                let mut fields = Map::new();
                fields.insert(
                    "status".to_string(),
                    Value::String("already_current".to_string()),
                );
                emit_startup_event(
                    "info",
                    "startup.mesh_install.completed",
                    "Startup mesh install check completed",
                    fields,
                );
            }
            Err(error) => {
                tracing::warn!(error = %error, "Failed to ensure bundled mesh install during startup");
                let mut fields = Map::new();
                fields.insert(
                    "error.code".to_string(),
                    Value::String("STARTUP_MESH_INSTALL_FAILED".to_string()),
                );
                fields.insert("error.message".to_string(), Value::String(error));
                emit_startup_event(
                    "warn",
                    "startup.mesh_install.failed",
                    "Startup mesh install check failed",
                    fields,
                );
            }
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
    log_daemon_data_root_mismatch(ping);

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

pub(super) fn log_daemon_data_root_mismatch(ping: &crate::daemon::protocol::PingResult) {
    if ping.data_root.trim().is_empty() {
        return;
    }

    let app_data_root = crate::provider::platform_paths::PlatformPaths::app_data_root();
    let app_data_root_text = app_data_root.display().to_string();
    if crate::provider::path::normalize_project_path(&ping.data_root)
        == crate::provider::path::normalize_project_path(&app_data_root_text)
    {
        return;
    }

    tracing::warn!(
        daemon_data_root = %ping.data_root,
        app_data_root = %app_data_root.display(),
        "Daemon data root differs from the app data root"
    );
    let mut fields = Map::new();
    fields.insert(
        "daemon_data_root".to_string(),
        Value::String(ping.data_root.clone()),
    );
    fields.insert(
        "app_data_root".to_string(),
        Value::String(app_data_root_text),
    );
    emit_startup_event(
        "warn",
        "daemon.data_root.mismatch",
        "Daemon data root differs from the app data root",
        fields,
    );
}

pub(crate) fn start_runtime_monitors(
    app: AppHandle,
    context: &SetupContext,
    bootstrap_complete: Arc<AtomicBool>,
) {
    if context.wsl_distro.is_some() {
        let health_handle = app.clone();
        let connected_at_startup = context.daemon_connected_at_startup;
        // The health monitor restarts the daemon, so it needs the launch
        // context the app itself was set up with — a guessed log path puts the
        // restarted daemon on a different data root than the app.
        let log_path = context.log_path.clone();
        std::thread::spawn(move || {
            daemon_lifecycle::daemon_health_check(
                health_handle,
                connected_at_startup,
                bootstrap_complete,
                log_path,
            );
        });

        daemon_lifecycle::start_session_updates_bridge(app);
    }
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
    use crate::commands::logging::{
        clear_test_tap, install_global_sink, install_test_tap, LogFileState,
    };
    use std::time::Duration;

    fn wait_for_event(path: &std::path::Path, event: &str) -> Option<serde_json::Value> {
        for _ in 0..100 {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            if let Some(value) = content
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find(|value| value["event"] == event)
            {
                return Some(value);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    #[test]
    fn ensure_expected_daemon_runtime_accepts_matching_contract() {
        let ping = crate::daemon::protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            uptime_secs: 1,
            data_root: crate::provider::platform_paths::PlatformPaths::app_data_root()
                .display()
                .to_string(),
        };

        assert!(ensure_expected_daemon_runtime(&ping).is_ok());
    }

    #[test]
    fn ensure_expected_daemon_runtime_rejects_version_mismatch() {
        let ping = crate::daemon::protocol::PingResult {
            version: "0.0.1".to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            uptime_secs: 1,
            data_root: String::new(),
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
            data_root: String::new(),
        };

        let error = ensure_expected_daemon_runtime(&ping).expect_err("mismatch should fail");
        assert!(error.contains("daemon protocol mismatch"));
    }

    #[test]
    fn startup_logs_daemon_data_root_mismatch() {
        // Regression: commits a53ad31 (removal added) and f9c1e89 (None => remove-all)
        // left the app unable to diagnose a daemon using a different data root.
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::tempdir().expect("temp dir");
        let log_path = dir.path().join("startup-daemon.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);
        let ping: crate::daemon::protocol::PingResult = serde_json::from_value(serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_version": crate::daemon::protocol::PROTOCOL_VERSION,
            "uptime_secs": 1,
            "data_root": "/definitely/not/the/app/data/root"
        }))
        .expect("ping payload");

        ensure_expected_daemon_runtime(&ping).expect("runtime contract remains compatible");

        let event = wait_for_event(&log_path, "daemon.data_root.mismatch")
            .expect("startup must emit daemon.data_root.mismatch");
        assert_eq!(
            event["daemon_data_root"],
            "/definitely/not/the/app/data/root"
        );
        assert!(event["app_data_root"].is_string());
    }

    #[test]
    fn startup_does_not_log_daemon_data_root_mismatch_for_matching_root() {
        // Regression: commit 55fcf0c added mismatch telemetry; a matching daemon
        // identity must not turn the startup check into another heartbeat event.
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::tempdir().expect("temp dir");
        let state =
            LogFileState::new(dir.path().join("matching-root.log.jsonl")).expect("log state");
        install_global_sink(&state);
        let (sender, receiver) = std::sync::mpsc::channel();
        install_test_tap(sender);
        let ping = crate::daemon::protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: crate::daemon::protocol::PROTOCOL_VERSION,
            uptime_secs: 1,
            data_root: crate::provider::platform_paths::PlatformPaths::app_data_root()
                .display()
                .to_string(),
        };

        let result = ensure_expected_daemon_runtime(&ping);
        clear_test_tap();

        assert!(result.is_ok());
        assert!(
            receiver
                .try_iter()
                .all(|event| event["event"] != "daemon.data_root.mismatch"),
            "matching daemon data roots must not emit mismatch telemetry"
        );
    }
}
