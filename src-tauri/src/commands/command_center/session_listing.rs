use tauri::Manager;

use super::*;
use crate::commands::projects::DbState;
use crate::coordination::activity_export::enrich_sessions_with_team_membership;

fn store_session_snapshot_cache(app: &tauri::AppHandle, sessions: &[DisplaySession]) {
    let cache_state = app.state::<crate::SessionSnapshotCacheState>();
    let mut cache_guard = cache_state.0.lock().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Session snapshot cache lock poisoned while storing last known good sessions; recovering"
        );
        error.into_inner()
    });
    *cache_guard = Some(sessions.to_vec());
}

fn cached_session_snapshot(app: &tauri::AppHandle) -> Option<Vec<DisplaySession>> {
    app.state::<crate::SessionSnapshotCacheState>()
        .0
        .lock()
        .unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                "Session snapshot cache lock poisoned while loading cached sessions; recovering"
            );
            error.into_inner()
        })
        .clone()
}

pub(super) fn list_cli_sessions_impl(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
) -> Result<Vec<DisplaySession>, String> {
    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "list-sessions";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LIST_DISPLAY_SESSIONS,
                serde_json::Value::Null,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let mut sessions = decode_daemon_session_list(response.result)?;

                    if !crate::daemon::launcher::is_native_daemon() {
                        if let Some(ref distro) = provider.wsl_distro {
                            for session in &mut sessions {
                                if session.project_path.starts_with('/') {
                                    session.project_path = crate::provider::path::to_windows(
                                        &session.project_path,
                                        distro,
                                    );
                                }
                            }
                        }
                    }

                    enrich_sessions_with_team_membership(
                        app.state::<crate::coordination::state::CoordinationState>()
                            .teams_dir(),
                        &mut sessions,
                    );
                    store_session_snapshot_cache(app, &sessions);
                    promote_activity_from_sessions(app, db, &sessions);
                    return Ok(sessions);
                }
                Ok(response) => {
                    tracing::warn!(error = ?response.error, "Daemon returned error for session listing");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to reach daemon for session listing");
                }
            }

            if let Some(cached) = cached_session_snapshot(app) {
                tracing::debug!(
                    count = cached.len(),
                    "list_cli_sessions: using cached session snapshot after daemon failure"
                );
                promote_activity_from_sessions(app, db, &cached);
                return Ok(cached);
            }
        }
    }

    let mut fallback = crate::session_scanner::scan_sessions_for_display();
    tracing::debug!(count = fallback.len(), "list_cli_sessions: fallback scan");
    enrich_sessions_with_team_membership(
        app.state::<crate::coordination::state::CoordinationState>()
            .teams_dir(),
        &mut fallback,
    );
    store_session_snapshot_cache(app, &fallback);
    promote_activity_from_sessions(app, db, &fallback);
    Ok(fallback)
}

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
