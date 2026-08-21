use tauri::Manager;

use super::*;
use crate::commands::projects::DbState;
use crate::commands::runtime_snapshot::daemon_runtime_session_snapshot;
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

    let (mut fallback, degraded) = crate::session_scanner::scan_sessions_for_display();
    tracing::debug!(
        count = fallback.len(),
        degraded,
        "list_cli_sessions: fallback scan"
    );
    enrich_sessions_with_team_membership(
        app.state::<crate::coordination::state::CoordinationState>()
            .teams_dir(),
        &mut fallback,
    );
    promote_activity_from_sessions(app, db, &fallback);
    Ok(fallback)
}

fn daemon_display_sessions(
    provider: &ProviderState,
) -> Result<Option<Vec<DisplaySession>>, String> {
    let Some(snapshot) = daemon_runtime_session_snapshot(provider)?.snapshot else {
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
