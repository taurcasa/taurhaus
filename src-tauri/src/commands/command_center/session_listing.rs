use tauri::Manager;

use super::*;
use crate::commands::projects::DbState;
use crate::coordination::activity_export::enrich_sessions_with_team_membership;

pub(super) fn list_cli_sessions_impl(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
) -> Result<Vec<ClaudeSession>, String> {
    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "list-sessions";
            let request = protocol::DaemonRequest::new(
                id,
                protocol::method::LIST_CLAUDE_SESSIONS,
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
        }
    }

    let mut fallback = crate::session_scanner::scan_sessions();
    tracing::debug!(count = fallback.len(), "list_cli_sessions: fallback scan");
    enrich_sessions_with_team_membership(
        app.state::<crate::coordination::state::CoordinationState>()
            .teams_dir(),
        &mut fallback,
    );
    promote_activity_from_sessions(app, db, &fallback);
    Ok(fallback)
}

pub(super) fn decode_daemon_session_list(
    payload: Option<serde_json::Value>,
) -> Result<Vec<ClaudeSession>, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|e| {
            tracing::warn!(error = %e, "Failed to deserialize session list from daemon");
            format!("Session list decode error: {e}")
        }),
        None => Ok(Vec::new()),
    }
}
