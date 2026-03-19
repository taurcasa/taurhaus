use serde::Deserialize;

#[cfg(any(test, target_os = "windows"))]
use super::DisplaySession;
#[cfg(target_os = "windows")]
use super::RuntimeSession;

#[cfg_attr(not(any(test, target_os = "windows")), allow(dead_code))]
pub(crate) fn decode_daemon_session_response<T>(
    response: crate::daemon::protocol::DaemonResponse,
) -> Option<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if response.error.is_some() {
        return None;
    }

    match response.result {
        Some(value) => serde_json::from_value(value).ok(),
        None => Some(Vec::new()),
    }
}

#[cfg(target_os = "windows")]
fn scan_sessions_via_daemon<T>(method: &str) -> Option<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    use serde_json::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    const DAEMON_ADDR: &str = "127.0.0.1:17233";
    const DAEMON_TIMEOUT: Duration = Duration::from_millis(500);

    let request =
        crate::daemon::protocol::DaemonRequest::new("windows-session-scan", method, Value::Null)
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
    decode_daemon_session_response(response)
}

#[cfg(target_os = "windows")]
pub(crate) fn scan_display_sessions_via_daemon() -> Option<Vec<DisplaySession>> {
    scan_sessions_via_daemon(crate::daemon::protocol::method::LIST_DISPLAY_SESSIONS)
}

#[cfg(target_os = "windows")]
pub(crate) fn scan_runtime_sessions_via_daemon() -> Option<Vec<RuntimeSession>> {
    scan_sessions_via_daemon(crate::daemon::protocol::method::LIST_RUNTIME_SESSIONS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, CliTool, SessionGroupKind, SessionState,
    };

    #[test]
    fn decode_daemon_display_session_response_returns_sessions() {
        let session = DisplaySession {
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
        };

        let decoded: Vec<DisplaySession> = decode_daemon_session_response(
            crate::daemon::protocol::DaemonResponse::ok("list", vec![session.clone()]),
        )
        .expect("daemon session list should decode");

        assert_eq!(decoded, vec![session]);
    }

    #[test]
    fn decode_daemon_session_response_rejects_daemon_errors() {
        let response = crate::daemon::protocol::DaemonResponse {
            id: "list".to_string(),
            result: None,
            error: Some(crate::daemon::protocol::DaemonError {
                code: "UNAVAILABLE".to_string(),
                message: "daemon unavailable".to_string(),
            }),
        };

        let decoded: Option<Vec<DisplaySession>> = decode_daemon_session_response(response);
        assert!(decoded.is_none());
    }
}
