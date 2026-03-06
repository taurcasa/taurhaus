use std::fmt::Display;
use std::time::Instant;

use serde_json::{Map, Value};

use crate::commands::logging;

fn json_number_u64(value: u64) -> Value {
    Value::Number(serde_json::Number::from(value))
}

fn build_error_fields(error: &str) -> (String, String) {
    let trimmed = error.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let code = value
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("IPC_ERROR")
                .to_string();
            let message = value
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or(trimmed)
                .to_string();
            return (code, message);
        }
    }
    ("IPC_ERROR".to_string(), trimmed.to_string())
}

fn next_request_id() -> String {
    format!("ipc_{}", uuid::Uuid::new_v4().simple())
}

pub struct IpcCommandSpan {
    command: &'static str,
    request_id: String,
    started_at: Instant,
}

impl IpcCommandSpan {
    pub fn start(command: &'static str) -> Self {
        let request_id = next_request_id();
        let mut fields = Map::new();
        fields.insert("command".to_string(), Value::String(command.to_string()));
        fields.insert("request_id".to_string(), Value::String(request_id.clone()));
        fields.insert("status".to_string(), Value::String("received".to_string()));
        logging::emit_global(
            "info",
            "backend",
            "ipc.command.received",
            Some("IPC command received".to_string()),
            fields,
        );
        Self {
            command,
            request_id,
            started_at: Instant::now(),
        }
    }

    #[cfg(test)]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn emit_lock_wait(&self, lock_name: &'static str, wait_ms: u64) {
        let mut fields = Map::new();
        fields.insert(
            "command".to_string(),
            Value::String(self.command.to_string()),
        );
        fields.insert(
            "request_id".to_string(),
            Value::String(self.request_id.clone()),
        );
        fields.insert(
            "lock_name".to_string(),
            Value::String(lock_name.to_string()),
        );
        fields.insert("wait_ms".to_string(), json_number_u64(wait_ms));
        logging::emit_global(
            "debug",
            "backend",
            "ipc.lock.wait",
            Some("IPC lock acquisition wait observed".to_string()),
            fields,
        );
    }

    pub fn complete(&self) {
        let mut fields = Map::new();
        fields.insert(
            "command".to_string(),
            Value::String(self.command.to_string()),
        );
        fields.insert(
            "request_id".to_string(),
            Value::String(self.request_id.clone()),
        );
        fields.insert(
            "duration_ms".to_string(),
            json_number_u64(self.started_at.elapsed().as_millis() as u64),
        );
        fields.insert("status".to_string(), Value::String("ok".to_string()));
        logging::emit_global(
            "info",
            "backend",
            "ipc.command.completed",
            Some("IPC command completed".to_string()),
            fields,
        );
    }

    pub fn fail_msg(&self, error: &str) {
        let (error_code, error_message) = build_error_fields(error);
        let mut fields = Map::new();
        fields.insert(
            "command".to_string(),
            Value::String(self.command.to_string()),
        );
        fields.insert(
            "request_id".to_string(),
            Value::String(self.request_id.clone()),
        );
        fields.insert(
            "duration_ms".to_string(),
            json_number_u64(self.started_at.elapsed().as_millis() as u64),
        );
        fields.insert("status".to_string(), Value::String("error".to_string()));
        fields.insert("error.code".to_string(), Value::String(error_code));
        fields.insert("error.message".to_string(), Value::String(error_message));
        logging::emit_global(
            "error",
            "backend",
            "ipc.command.failed",
            Some("IPC command failed".to_string()),
            fields,
        );
    }

    pub fn finish_result<T, E: Display>(&self, result: &Result<T, E>) {
        match result {
            Ok(_) => self.complete(),
            Err(error) => self.fail_msg(&error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use std::path::PathBuf;

    fn wait_for_lines(path: &PathBuf, expected: usize) -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.to_string())
                    .collect();
                if lines.len() >= expected {
                    return lines;
                }
            }
            if std::time::Instant::now() >= deadline {
                panic!("timed out waiting for log lines in {}", path.display());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn span_emits_received_completed_and_lock_wait_events() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("lifecycle.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let span = IpcCommandSpan::start("list_projects");
        assert!(!span.request_id().is_empty());
        span.emit_lock_wait("db", 7);
        span.complete();

        let lines = wait_for_lines(&log_path, 3);
        let received: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        let lock_wait: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        let completed: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();

        assert_eq!(received["event"], "ipc.command.received");
        assert_eq!(received["command"], "list_projects");
        assert_eq!(received["status"], "received");

        assert_eq!(lock_wait["event"], "ipc.lock.wait");
        assert_eq!(lock_wait["lock_name"], "db");
        assert_eq!(lock_wait["wait_ms"], 7);

        assert_eq!(completed["event"], "ipc.command.completed");
        assert_eq!(completed["status"], "ok");
        assert!(completed["duration_ms"].as_u64().unwrap() <= 1_000);
    }

    #[test]
    fn span_emits_failed_with_error_fields() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("lifecycle-error.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let span = IpcCommandSpan::start("get_project");
        let result: Result<(), String> =
            Err(r#"{"code":"NOT_FOUND","message":"project missing"}"#.to_string());
        span.finish_result(&result);

        let lines = wait_for_lines(&log_path, 2);
        let failed: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(failed["event"], "ipc.command.failed");
        assert_eq!(failed["error.code"], "NOT_FOUND");
        assert_eq!(failed["error.message"], "project missing");
        assert_eq!(failed["status"], "error");
    }
}
