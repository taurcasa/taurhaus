//! Boundary exports for daemon protocol/auth contracts used outside daemon internals.

pub use crate::daemon::protocol;
use serde_json::{Map, Value};
use std::time::Instant;

pub fn read_auth_token() -> Option<String> {
    crate::daemon::auth::read_auth_token()
}

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

fn emit_daemon_rpc_event(
    level: &str,
    event: &str,
    daemon_request_id: &str,
    method: &str,
    status: &str,
    duration_ms: u64,
    retry_count: u32,
) {
    let mut fields = Map::new();
    fields.insert(
        "daemon_request_id".to_string(),
        Value::String(daemon_request_id.to_string()),
    );
    fields.insert("method".to_string(), Value::String(method.to_string()));
    fields.insert("status".to_string(), Value::String(status.to_string()));
    fields.insert("duration_ms".to_string(), json_number_u64(duration_ms));
    fields.insert(
        "retry_count".to_string(),
        Value::Number(serde_json::Number::from(retry_count)),
    );
    crate::commands::logging::emit_global(
        level,
        "backend",
        event,
        Some("Daemon RPC lifecycle event".to_string()),
        fields,
    );
}

pub fn is_timeout_transport_error(message: &str) -> bool {
    message.to_ascii_lowercase().contains("timed out")
}

pub struct DaemonRpcSpan {
    daemon_request_id: String,
    method: String,
    retry_count: u32,
    started_at: Instant,
}

impl DaemonRpcSpan {
    pub fn start(request: &protocol::DaemonRequest, retry_count: u32) -> Self {
        emit_daemon_rpc_event(
            "debug",
            "daemon.rpc.sent",
            &request.id,
            &request.method,
            "sent",
            0,
            retry_count,
        );
        Self {
            daemon_request_id: request.id.clone(),
            method: request.method.clone(),
            retry_count,
            started_at: Instant::now(),
        }
    }

    pub fn response(&self, status: &'static str) {
        emit_daemon_rpc_event(
            "info",
            "daemon.rpc.response",
            &self.daemon_request_id,
            &self.method,
            status,
            self.started_at.elapsed().as_millis() as u64,
            self.retry_count,
        );
    }

    pub fn timeout(&self) {
        emit_daemon_rpc_event(
            "warn",
            "daemon.rpc.timeout",
            &self.daemon_request_id,
            &self.method,
            "timeout",
            self.started_at.elapsed().as_millis() as u64,
            self.retry_count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use std::path::Path;
    use std::time::Duration;

    fn read_lines(path: &Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn wait_for_lines(path: &Path, expected_minimum: usize) -> Vec<String> {
        for _ in 0..100 {
            let lines = read_lines(path);
            if lines.len() >= expected_minimum {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        read_lines(path)
    }

    #[test]
    fn daemon_rpc_span_emits_sent_and_response_fields() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("daemon-rpc.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let request = protocol::DaemonRequest::new("r42", protocol::method::PING, Value::Null);
        let span = DaemonRpcSpan::start(&request, 1);
        span.response("ok");

        let lines = wait_for_lines(&log_path, 2);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("valid json"))
            .collect();
        let sent = events
            .iter()
            .find(|value| value["event"] == "daemon.rpc.sent")
            .expect("sent event");
        let response = events
            .iter()
            .find(|value| value["event"] == "daemon.rpc.response")
            .expect("response event");

        assert_eq!(sent["event"], "daemon.rpc.sent");
        assert_eq!(sent["daemon_request_id"], "r42");
        assert_eq!(sent["method"], "ping");
        assert_eq!(sent["status"], "sent");
        assert_eq!(sent["duration_ms"], 0);
        assert_eq!(sent["retry_count"], 1);

        assert_eq!(response["event"], "daemon.rpc.response");
        assert_eq!(response["daemon_request_id"], "r42");
        assert_eq!(response["method"], "ping");
        assert_eq!(response["status"], "ok");
        assert_eq!(response["retry_count"], 1);
        assert!(response["duration_ms"].as_u64().unwrap() <= 1_000);
    }

    #[test]
    fn daemon_rpc_span_emits_timeout_fields() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("daemon-rpc-timeout.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let request =
            protocol::DaemonRequest::new("r-timeout", protocol::method::GIT_STATUS, Value::Null);
        let span = DaemonRpcSpan::start(&request, 0);
        span.timeout();

        let lines = wait_for_lines(&log_path, 2);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("valid json"))
            .collect();
        let timeout = events
            .iter()
            .find(|value| value["event"] == "daemon.rpc.timeout")
            .expect("timeout event");
        assert_eq!(timeout["event"], "daemon.rpc.timeout");
        assert_eq!(timeout["daemon_request_id"], "r-timeout");
        assert_eq!(timeout["method"], "git_status");
        assert_eq!(timeout["status"], "timeout");
        assert_eq!(timeout["retry_count"], 0);
        assert!(timeout["duration_ms"].as_u64().unwrap() <= 1_000);
    }

    #[test]
    fn timeout_transport_detection_checks_message_content() {
        assert!(is_timeout_transport_error(
            "Daemon request timed out after 5s: deadline"
        ));
        assert!(!is_timeout_transport_error(
            "Failed to write daemon request"
        ));
    }
}
