use tauri::Manager;

use super::*;
use crate::commands::projects::DbState;
use crate::coordination::activity_export::enrich_sessions_with_team_membership;

pub(super) fn list_cli_sessions_impl(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
) -> Result<Vec<DisplaySession>, String> {
    if let Some(sessions) = daemon_display_sessions(provider)? {
        promote_activity_from_sessions(app, db, &sessions);
        return Ok(sessions);
    }

    if provider.daemon.is_some() {
        tracing::debug!(
            "list_cli_sessions: daemon unavailable, using cached snapshot or empty set"
        );
        return Ok(Vec::new());
    }

    let mut fallback = crate::session_scanner::scan_sessions_for_display();
    tracing::debug!(count = fallback.len(), "list_cli_sessions: fallback scan");
    enrich_sessions_with_team_membership(
        app.state::<crate::coordination::state::CoordinationState>()
            .teams_dir(),
        &mut fallback,
    );
    promote_activity_from_sessions(app, db, &fallback);
    Ok(fallback)
}

pub(crate) fn daemon_runtime_session_snapshot(
    provider: &ProviderState,
) -> Result<Option<protocol::RuntimeSessionSnapshotResult>, String> {
    let Some(ref daemon) = provider.daemon else {
        return Ok(None);
    };

    if !daemon.is_connected() && !daemon.try_reconnect() {
        return Ok(crate::session_snapshot_cache::load());
    }

    match request_daemon_runtime_session_snapshot(daemon) {
        Ok(Some(snapshot)) => {
            crate::session_snapshot_cache::store(&snapshot);
            Ok(Some(snapshot))
        }
        Ok(None) => Ok(crate::session_snapshot_cache::load()),
        Err(error) => {
            if taurhaus_lib::daemon_api::is_busy_transport_error(&error) {
                tracing::debug!(
                    error = %error,
                    "Daemon runtime session snapshot skipped because the shared daemon connection is busy"
                );
                return Ok(crate::session_snapshot_cache::load());
            }
            tracing::warn!(
                error = %error,
                "Failed to reach daemon for runtime session snapshot; attempting inline reconnect"
            );
            if daemon.try_reconnect() {
                match request_daemon_runtime_session_snapshot(daemon) {
                    Ok(Some(snapshot)) => {
                        crate::session_snapshot_cache::store(&snapshot);
                        Ok(Some(snapshot))
                    }
                    Ok(None) => Ok(crate::session_snapshot_cache::load()),
                    Err(retry_error) => {
                        tracing::warn!(
                            error = %retry_error,
                            "Inline reconnect succeeded but runtime session snapshot retry failed"
                        );
                        Ok(crate::session_snapshot_cache::load())
                    }
                }
            } else {
                Ok(crate::session_snapshot_cache::load())
            }
        }
    }
}

fn request_daemon_runtime_session_snapshot(
    daemon: &crate::provider::daemon_client::DaemonProvider,
) -> Result<Option<protocol::RuntimeSessionSnapshotResult>, String> {
    let request = protocol::DaemonRequest::new(
        "runtime-session-snapshot",
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT,
        serde_json::Value::Null,
    );
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            decode_daemon_runtime_session_snapshot(response.result).map(Some)
        }
        Ok(response) => {
            tracing::warn!(
                error = ?response.error,
                "Daemon returned error for runtime session snapshot"
            );
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

fn daemon_display_sessions(
    provider: &ProviderState,
) -> Result<Option<Vec<DisplaySession>>, String> {
    let Some(snapshot) = daemon_runtime_session_snapshot(provider)? else {
        return Ok(None);
    };

    let mut sessions = snapshot.display_sessions;
    if !crate::daemon::launcher::is_native_daemon() {
        if let Some(ref distro) = provider.wsl_distro {
            for session in &mut sessions {
                if session.project_path.starts_with('/') {
                    session.project_path =
                        crate::provider::path::to_windows(&session.project_path, distro);
                }
            }
        }
    }
    Ok(Some(sessions))
}

#[cfg(test)]
pub(super) fn decode_daemon_session_list(
    payload: Option<serde_json::Value>,
) -> Result<Vec<DisplaySession>, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|e| {
            tracing::warn!(error = %e, "Failed to deserialize session list from daemon");
            format!("Session list decode error: {e}")
        }),
        None => Ok(Vec::new()),
    }
}

pub(super) fn decode_daemon_runtime_session_snapshot(
    payload: Option<serde_json::Value>,
) -> Result<protocol::RuntimeSessionSnapshotResult, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|error| {
            tracing::warn!(
                error = %error,
                "Failed to deserialize runtime session snapshot from daemon"
            );
            format!("Runtime session snapshot decode error: {error}")
        }),
        None => Ok(protocol::RuntimeSessionSnapshotResult {
            version: 0,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus: None,
            foreground_project_path: None,
        }),
    }
}
