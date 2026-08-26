use serde::Serialize;
use tauri::Manager;

use super::*;
use crate::commands::projects::DbState;
use crate::commands::runtime_snapshot::{
    daemon_runtime_session_snapshot, RuntimeSnapshotFreshness,
};
use crate::coordination::activity_export::enrich_sessions_with_team_membership;

/// How a session list was obtained — the difference between "these are the
/// sessions" and "these are the sessions I last saw".
///
/// The app measures per-session time against the interval between two
/// observations and suspends measurement across intervals it did not observe
/// (`sessionStore.svelte.js`), so it cannot be handed a bare list: a replayed
/// or cached list would resume the clock and credit an outage to whatever state
/// preceded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliSessionFreshness {
    /// Read now, by a scanner that could see.
    Fresh,
    /// The hub's last good sessions, replayed while its scanner is blind.
    Degraded,
    /// The on-disk snapshot cache: the daemon could not be reached at all.
    Cached,
    /// Nothing to report — no live snapshot, no cache.
    Unavailable,
}

impl CliSessionFreshness {
    pub(super) fn classify(snapshot: RuntimeSnapshotFreshness, degraded: bool) -> Self {
        match snapshot {
            RuntimeSnapshotFreshness::Unavailable => Self::Unavailable,
            // A cached read is older than a degraded one: the daemon was not
            // even reachable, so the hub's own view of its health is unknown.
            RuntimeSnapshotFreshness::Cached => Self::Cached,
            RuntimeSnapshotFreshness::Fresh if degraded => Self::Degraded,
            RuntimeSnapshotFreshness::Fresh => Self::Fresh,
        }
    }
}

/// A session list plus how it was obtained.
#[derive(Debug, Clone, Serialize)]
pub struct CliSessionSnapshot {
    pub sessions: Vec<DisplaySession>,
    pub freshness: CliSessionFreshness,
}

impl CliSessionSnapshot {
    pub(super) fn unavailable() -> Self {
        Self {
            sessions: Vec::new(),
            freshness: CliSessionFreshness::Unavailable,
        }
    }
}

pub(super) fn list_cli_sessions_impl(
    app: &tauri::AppHandle,
    db: &DbState,
    provider: &ProviderState,
) -> Result<CliSessionSnapshot, String> {
    if let Some(snapshot) = daemon_display_sessions(provider)? {
        // Continuity read: a degraded or cached daemon snapshot is a view the
        // app last had, not an observation — it must not promote project
        // activity.
        if snapshot.freshness == CliSessionFreshness::Fresh {
            promote_activity_from_sessions(app, db, &snapshot.sessions);
        }
        return Ok(snapshot);
    }

    if provider.daemon.is_some() {
        tracing::debug!("list_cli_sessions: daemon unavailable, no snapshot and no cache");
        return Ok(CliSessionSnapshot::unavailable());
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
    // Continuity read: a degraded scan returns the last good snapshot so the
    // list does not blank out. It is not an observation, so it must not write
    // project activity — promotion only runs on a healthy scan.
    if !degraded {
        promote_activity_from_sessions(app, db, &fallback);
    }
    Ok(CliSessionSnapshot {
        sessions: fallback,
        freshness: if degraded {
            CliSessionFreshness::Degraded
        } else {
            CliSessionFreshness::Fresh
        },
    })
}

/// The daemon's display sessions and how fresh they are — `None` when there is
/// no snapshot at all, live or cached.
pub(super) fn daemon_display_sessions(
    provider: &ProviderState,
) -> Result<Option<CliSessionSnapshot>, String> {
    let outcome = daemon_runtime_session_snapshot(provider)?;
    let Some(snapshot) = outcome.snapshot else {
        return Ok(None);
    };

    let freshness = CliSessionFreshness::classify(outcome.freshness, snapshot.degraded);
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
    Ok(Some(CliSessionSnapshot {
        sessions,
        freshness,
    }))
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
