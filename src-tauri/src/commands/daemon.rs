use tauri::{Emitter, State};

use crate::daemon::protocol::{self, PingResult, PROTOCOL_VERSION};
use crate::daemon::server::DEFAULT_PORT;
use crate::models::DaemonStatus;
use crate::ProviderState;

/// Get the current daemon connection status.
#[tauri::command]
pub fn get_daemon_status(provider: State<'_, ProviderState>) -> Result<DaemonStatus, String> {
    let port = DEFAULT_PORT;

    let Some(ref daemon) = provider.daemon else {
        return Ok(DaemonStatus {
            status: "not_configured".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        });
    };

    if !daemon.is_connected() {
        return Ok(DaemonStatus {
            status: "disconnected".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        });
    }

    // Try a ping to get version and uptime
    let id = "status-ping";
    let request = protocol::DaemonRequest::ping(id);
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            let ping: Option<PingResult> = response
                .result
                .and_then(|v| serde_json::from_value(v).ok());
            Ok(DaemonStatus {
                status: "connected".to_string(),
                version: ping.as_ref().map(|p| p.version.clone()),
                protocol_version: ping.as_ref().map(|p| p.protocol_version).unwrap_or(0),
                expected_protocol_version: PROTOCOL_VERSION,
                uptime_secs: ping.as_ref().map(|p| p.uptime_secs),
                port,
                wsl_distro: provider.wsl_distro.clone(),
            })
        }
        _ => Ok(DaemonStatus {
            status: "disconnected".to_string(),
            version: None,
            protocol_version: 0,
            expected_protocol_version: PROTOCOL_VERSION,
            uptime_secs: None,
            port,
            wsl_distro: provider.wsl_distro.clone(),
        }),
    }
}

/// Manually start the daemon process.
#[tauri::command]
pub fn start_daemon(
    provider: State<'_, ProviderState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let distro = provider
        .wsl_distro
        .as_deref()
        .ok_or("No WSL distro configured")?;

    let port = DEFAULT_PORT;

    crate::daemon::launcher::try_restart_daemon(distro, port)
        .map_err(|e| format!("Failed to start daemon: {e}"))?;

    // Wait a moment, then try to reconnect
    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(ref daemon) = provider.daemon {
        if daemon.reconnect().is_ok() {
            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "connected" }),
            );
            return Ok("Daemon started and connected".to_string());
        }
    }

    Ok("Daemon process started (not yet connected)".to_string())
}

/// Manually stop the daemon process.
#[tauri::command]
pub fn stop_daemon(
    provider: State<'_, ProviderState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let Some(ref daemon) = provider.daemon else {
        return Err("No daemon configured".to_string());
    };

    if !daemon.is_connected() {
        return Ok("Daemon already disconnected".to_string());
    }

    // Send shutdown command
    let id = "manual-shutdown";
    let request = protocol::DaemonRequest::new(id, protocol::method::SHUTDOWN, serde_json::Value::Null);
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "disconnected" }),
            );
            Ok("Daemon stopped".to_string())
        }
        Ok(response) => Err(format!(
            "Shutdown failed: {}",
            response.error.map(|e| e.message).unwrap_or_default()
        )),
        Err(e) => Err(format!("Failed to send shutdown: {e}")),
    }
}
