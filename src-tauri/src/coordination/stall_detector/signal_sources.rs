use std::collections::HashMap;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::coordination::mesh_cli;
use crate::coordination::runtime::apply_background_command_settings;
use crate::session_scanner::scan_sessions_for_display;

use super::transitions::set_if_newer;
use super::types::{
    MemberKey, MemberStallState, MeshMemberSignal, MeshMemberStatus, SessionSignal, SignalSnapshot,
    SignalStrength,
};

pub(super) fn apply_signal_snapshots_to_member_states(
    member_states: &std::sync::Arc<std::sync::Mutex<HashMap<MemberKey, MemberStallState>>>,
    snapshots: &[SignalSnapshot],
    require_medium_confidence: bool,
) {
    let Ok(mut states) = member_states.lock() else {
        return;
    };
    for snapshot in snapshots {
        let key = MemberKey {
            team_name: snapshot.team_name.clone(),
            member_name: snapshot.member_name.clone(),
        };
        let Some(state) = states.get_mut(&key) else {
            continue;
        };

        if let Some(signal) = snapshot.selected_session_signal(require_medium_confidence) {
            set_if_newer(&mut state.last_any_signal_at, signal.observed_at);
            if signal.is_strong {
                set_if_newer(&mut state.last_strong_signal_at, signal.observed_at);
            }
        }

        if snapshot.pane_command_is_medium() {
            set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
        }
        if snapshot.strongest_signal == Some(SignalStrength::Medium) {
            if let Some(event_at) = snapshot.coordination_event_at {
                set_if_newer(&mut state.last_any_signal_at, event_at);
            }
        }
        if let Some(at) = snapshot.mesh_last_activity_at {
            set_if_newer(&mut state.last_any_signal_at, at);
        }
        if let Some(status) = snapshot.mesh_status {
            match status {
                MeshMemberStatus::Working => {
                    set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
                    set_if_newer(&mut state.last_strong_signal_at, snapshot.observed_at);
                }
                MeshMemberStatus::Investigating => {
                    set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
                }
                MeshMemberStatus::Blocked | MeshMemberStatus::Idle | MeshMemberStatus::Unknown => {}
            }
        }
    }
}

pub(super) fn default_session_scan(now: DateTime<Utc>) -> Vec<SessionSignal> {
    if !host_supports_tmux_signals() {
        return Vec::new();
    }

    let (sessions, degraded) = scan_sessions_for_display();
    if degraded {
        // The process inventory could not be read: no observation this tick.
        return Vec::new();
    }
    sessions
        .into_iter()
        .map(|session| SessionSignal {
            pane_id: session.tmux_pane,
            project_path: session.project_path,
            observed_at: now,
            state: session.state,
            confidence: session.activity_confidence,
        })
        .collect()
}

pub(super) fn default_mesh_signal_reader(team_name: &str) -> HashMap<String, MeshMemberSignal> {
    if !host_supports_mesh_signals() || team_name.trim().is_empty() {
        return HashMap::new();
    }

    let Some(raw) = fetch_mesh_who_json(team_name) else {
        return HashMap::new();
    };
    parse_mesh_who_json(&raw)
}

const MESH_WHO_TIMEOUT: Duration = Duration::from_secs(2);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn run_command_with_timeout(command: &mut Command, timeout: Duration) -> Option<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    tracing::warn!(
                        timeout_ms = timeout.as_millis() as u64,
                        "stall detector command timed out; terminating process"
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(COMMAND_POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn fetch_mesh_who_json(team_name: &str) -> Option<String> {
    if !host_supports_mesh_signals() || team_name.trim().is_empty() {
        return None;
    }

    let invocation = mesh_cli::mesh_command_invocation(&["who", "--json", "--team", team_name]);
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args);
        run_command_with_timeout(&mut cmd, MESH_WHO_TIMEOUT)?
    } else {
        let mut cmd = Command::new(&invocation.program);
        apply_background_command_settings(&mut cmd);
        cmd.args(&invocation.args);
        run_command_with_timeout(&mut cmd, MESH_WHO_TIMEOUT)?
    };
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn parse_mesh_who_json(raw: &str) -> HashMap<String, MeshMemberSignal> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return HashMap::new();
    };
    let Value::Array(rows) = value else {
        return HashMap::new();
    };
    let mut by_member = HashMap::new();
    for row in rows {
        let Value::Object(map) = row else {
            continue;
        };
        let Some(name) = map.get("name").and_then(Value::as_str) else {
            continue;
        };
        let last_activity_at = map
            .get("lastActivityAt")
            .or_else(|| map.get("last_activity_at"))
            .and_then(parse_mesh_timestamp);
        let status = map
            .get("status")
            .and_then(Value::as_str)
            .and_then(parse_mesh_status);
        by_member.insert(
            name.to_string(),
            MeshMemberSignal {
                last_activity_at,
                status,
            },
        );
    }
    by_member
}

fn parse_mesh_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(raw) = value.as_str() {
        if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
            return Some(ts.with_timezone(&Utc));
        }
        return None;
    }
    if let Some(epoch) = value.as_i64() {
        if epoch > 10_000_000_000 {
            return DateTime::<Utc>::from_timestamp_millis(epoch);
        }
        return DateTime::<Utc>::from_timestamp(epoch, 0);
    }
    None
}

fn parse_mesh_status(raw: &str) -> Option<MeshMemberStatus> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "working" => Some(MeshMemberStatus::Working),
        "blocked" => Some(MeshMemberStatus::Blocked),
        "investigating" => Some(MeshMemberStatus::Investigating),
        "idle" => Some(MeshMemberStatus::Idle),
        "unknown" => Some(MeshMemberStatus::Unknown),
        _ => None,
    }
}

pub(super) fn host_supports_tmux_signals() -> bool {
    !cfg!(target_os = "windows")
}

pub(super) fn host_supports_mesh_signals() -> bool {
    !cfg!(target_os = "windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn parse_mesh_who_json_parses_optional_activity_and_status_fields() {
        let raw = r#"[
          {
            "name": "agent-a",
            "lastActivityAt": 1772711785867,
            "status": "working"
          },
          {
            "name": "agent-b",
            "last_activity_at": "2026-03-05T12:00:00Z",
            "status": "investigating"
          }
        ]"#;

        let parsed = parse_mesh_who_json(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed.get("agent-a").and_then(|entry| entry.status),
            Some(MeshMemberStatus::Working)
        );
        assert_eq!(
            parsed
                .get("agent-b")
                .and_then(|entry| entry.last_activity_at),
            Some(ts("2026-03-05T12:00:00Z"))
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_returns_output_before_deadline() {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "printf '{\"ok\":true}'"]);
        let output =
            run_command_with_timeout(&mut cmd, Duration::from_millis(500)).expect("command output");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{\"ok\":true}");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_terminates_hanging_process() {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "sleep 2"]);
        let started_at = Instant::now();
        let output = run_command_with_timeout(&mut cmd, Duration::from_millis(100));
        assert!(output.is_none());
        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "timeout helper should return before command naturally exits"
        );
    }
}
