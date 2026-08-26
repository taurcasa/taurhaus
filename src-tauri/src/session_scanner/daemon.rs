//! Windows app side of the session scan: the CLI tools run inside WSL2, so
//! the scan is the WSL daemon's hub snapshot, fetched over the daemon protocol.
//!
//! The daemon hub keeps its last good sessions across degraded scanner cycles
//! and reports that on the snapshot (`degraded`). That flag is carried back to
//! the scan entry points so a degraded daemon snapshot stays continuity data
//! and is never read as an observation.

use crate::daemon::protocol::{DaemonResponse, RuntimeSessionSnapshotResult};

/// Decode a `get_runtime_session_snapshot` response.
///
/// Daemon errors and undecodable payloads yield `None` (no snapshot). A
/// missing result is an empty healthy snapshot. Daemons built before the
/// `degraded` field decode as healthy (their behavior so far).
#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
pub(crate) fn decode_daemon_runtime_snapshot_response(
    response: DaemonResponse,
) -> Option<RuntimeSessionSnapshotResult> {
    if response.error.is_some() {
        return None;
    }

    match response.result {
        Some(value) => serde_json::from_value(value).ok(),
        None => Some(RuntimeSessionSnapshotResult {
            version: 0,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus: None,
            foreground_project_path: None,
            degraded: false,
            degraded_revision: 0,
        }),
    }
}

/// The daemon's runtime session snapshot, or `None` when there is no daemon
/// to ask (non-Windows hosts scan locally) or it cannot be reached.
pub(crate) fn runtime_session_snapshot_via_daemon() -> Option<RuntimeSessionSnapshotResult> {
    #[cfg(test)]
    if let Some(scripted) = *DAEMON_SNAPSHOT_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
    {
        return scripted();
    }

    fetch_runtime_session_snapshot()
}

/// Test seam: stands in for the daemon so the scan entry points can be driven
/// through healthy and degraded daemon snapshots on any host.
#[cfg(test)]
pub(crate) type DaemonSnapshotOverride = fn() -> Option<RuntimeSessionSnapshotResult>;
#[cfg(test)]
static DAEMON_SNAPSHOT_OVERRIDE: std::sync::Mutex<Option<DaemonSnapshotOverride>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_daemon_snapshot_override(scripted: Option<DaemonSnapshotOverride>) {
    *DAEMON_SNAPSHOT_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = scripted;
}

#[cfg(target_os = "windows")]
fn fetch_runtime_session_snapshot() -> Option<RuntimeSessionSnapshotResult> {
    use serde_json::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    const DAEMON_ADDR: &str = "127.0.0.1:17233";
    const DAEMON_TIMEOUT: Duration = Duration::from_millis(500);

    let request = crate::daemon::protocol::DaemonRequest::new(
        "windows-session-scan",
        crate::daemon::protocol::method::GET_RUNTIME_SESSION_SNAPSHOT,
        Value::Null,
    )
    .with_auth(crate::daemon::auth::read_auth_token());

    let mut stream = TcpStream::connect(DAEMON_ADDR).ok()?;
    stream.set_nodelay(true).ok()?;
    stream.set_read_timeout(Some(DAEMON_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(DAEMON_TIMEOUT)).ok()?;

    let payload = serde_json::to_string(&request).ok()?;
    stream.write_all(payload.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.flush().ok()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.trim().is_empty() {
        return None;
    }

    let response = serde_json::from_str(&line).ok()?;
    decode_daemon_runtime_snapshot_response(response)
}

#[cfg(not(target_os = "windows"))]
fn fetch_runtime_session_snapshot() -> Option<RuntimeSessionSnapshotResult> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, CliTool, DisplaySession, RuntimeSession,
        SessionGroupKind, SessionState,
    };

    fn display_session() -> DisplaySession {
        DisplaySession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            recent_io: false,
            last_output_age_secs: Some(1),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    fn runtime_session() -> RuntimeSession {
        RuntimeSession {
            pid: 42,
            project_path: "/home/user/projects/taurhaus".to_string(),
            tty: "/dev/pts/7".to_string(),
            args: "codex --yolo".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("0".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            session_id: Some("sess-42".to_string()),
            jsonl_path: Some("/home/user/.codex/sessions/sess-42.jsonl".to_string()),
            recent_io: false,
            last_output_age_secs: Some(1),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn decode_daemon_runtime_snapshot_response_returns_snapshot() {
        let snapshot = RuntimeSessionSnapshotResult {
            version: 4,
            display_sessions: vec![display_session()],
            runtime_sessions: vec![runtime_session()],
            focus: None,
            foreground_project_path: Some("/home/user/projects/taurhaus".to_string()),
            degraded: false,
            degraded_revision: 0,
        };

        let decoded = decode_daemon_runtime_snapshot_response(DaemonResponse::ok(
            "snapshot",
            snapshot.clone(),
        ))
        .expect("daemon snapshot should decode");

        assert_eq!(decoded, snapshot);
    }

    // Regression: the Windows client decoded only a session Vec, so the
    // daemon hub's degradation status never reached the app. The snapshot
    // decoder carries it; a daemon built before the field decodes as healthy.
    #[test]
    fn decode_daemon_runtime_snapshot_response_carries_degraded_and_defaults_for_old_daemons() {
        let degraded = decode_daemon_runtime_snapshot_response(DaemonResponse::ok(
            "snapshot",
            serde_json::json!({
                "version": 4,
                "display_sessions": [],
                "runtime_sessions": [serde_json::to_value(runtime_session()).unwrap()],
                "focus": null,
                "foreground_project_path": null,
                "degraded": true
            }),
        ))
        .expect("daemon snapshot should decode");
        assert!(degraded.degraded);
        assert_eq!(degraded.runtime_sessions, vec![runtime_session()]);

        let legacy = decode_daemon_runtime_snapshot_response(DaemonResponse::ok(
            "snapshot",
            serde_json::json!({
                "version": 4,
                "display_sessions": [],
                "runtime_sessions": [],
                "focus": null,
                "foreground_project_path": null
            }),
        ))
        .expect("legacy daemon snapshot should decode");
        assert!(!legacy.degraded);
    }

    #[test]
    fn decode_daemon_runtime_snapshot_response_rejects_daemon_errors() {
        let response = DaemonResponse {
            id: "snapshot".to_string(),
            result: None,
            error: Some(crate::daemon::protocol::DaemonError {
                code: "UNAVAILABLE".to_string(),
                message: "daemon unavailable".to_string(),
            }),
        };

        assert!(decode_daemon_runtime_snapshot_response(response).is_none());
    }

    #[test]
    fn decode_daemon_runtime_snapshot_response_treats_missing_result_as_empty() {
        let response = DaemonResponse {
            id: "snapshot".to_string(),
            result: None,
            error: None,
        };

        let decoded = decode_daemon_runtime_snapshot_response(response).expect("empty snapshot");
        assert_eq!(decoded.version, 0);
        assert!(decoded.display_sessions.is_empty());
        assert!(decoded.runtime_sessions.is_empty());
        assert!(!decoded.degraded);
    }
}
