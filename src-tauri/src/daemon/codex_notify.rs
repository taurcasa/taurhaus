use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub const CODEX_NOTIFY_FILENAME: &str = "codex-notify.jsonl";
const MAX_CODEX_NOTIFY_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct CodexNotifyEvent {
    value: Value,
}

impl CodexNotifyEvent {
    pub fn event_type(&self) -> Option<&str> {
        self.value.get("type").and_then(Value::as_str)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.value
            .get("session_id")
            .or_else(|| self.value.get("session-id"))
            .or_else(|| self.value.get("thread_id"))
            .or_else(|| self.value.get("thread-id"))
            .and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodexNotifyRecord {
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub event: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexNotifyAppendOutcome {
    pub truncated: bool,
}

pub fn parse_event(raw: &str) -> Result<CodexNotifyEvent, serde_json::Error> {
    serde_json::from_str(raw).map(|value| CodexNotifyEvent { value })
}

/// Append a Codex notification as one JSONL record.
///
/// The notifier is a short-lived subprocess, so its startup is the natural
/// place to enforce the bounded append-only file. An exclusive file lock keeps
/// simultaneous turn completions from racing a cap truncation.
pub fn append_event_at(
    path: &Path,
    raw_event: &str,
    ts: DateTime<Utc>,
) -> Result<CodexNotifyAppendOutcome, String> {
    let parsed =
        parse_event(raw_event).map_err(|error| format!("invalid Codex notify JSON: {error}"))?;
    let record = CodexNotifyRecord {
        ts,
        session_id: parsed.session_id().map(str::to_string),
        event: parsed.value,
    };
    let parent = path
        .parent()
        .ok_or_else(|| format!("Codex notify path '{}' has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Codex notify directory '{}': {error}",
            parent.display()
        )
    })?;

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            format!(
                "failed to open Codex notify sink '{}': {error}",
                path.display()
            )
        })?;
    file.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock Codex notify sink '{}': {error}",
            path.display()
        )
    })?;

    let result = (|| {
        let truncated = file
            .metadata()
            .map_err(|error| {
                format!(
                    "failed to stat Codex notify sink '{}': {error}",
                    path.display()
                )
            })?
            .len()
            >= MAX_CODEX_NOTIFY_BYTES;
        if truncated {
            file.set_len(0).map_err(|error| {
                format!(
                    "failed to cap Codex notify sink '{}': {error}",
                    path.display()
                )
            })?;
        }
        serde_json::to_writer(&mut file, &record).map_err(|error| {
            format!(
                "failed to serialize Codex notify record '{}': {error}",
                path.display()
            )
        })?;
        file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to append Codex notify record '{}': {error}",
                path.display()
            )
        })?;
        file.flush().map_err(|error| {
            format!(
                "failed to flush Codex notify sink '{}': {error}",
                path.display()
            )
        })?;
        Ok(CodexNotifyAppendOutcome { truncated })
    })();

    let _ = FileExt::unlock(&file);
    result
}

pub(crate) fn latest_record_for_session(
    path: &Path,
    session_id: &str,
) -> Option<CodexNotifyRecord> {
    let contents = fs::read_to_string(path).ok()?;
    contents.lines().rev().find_map(|line| {
        let record: CodexNotifyRecord = serde_json::from_str(line).ok()?;
        (record.session_id.as_deref() == Some(session_id)).then_some(record)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::io::Write;

    const OBSERVED_CODEX_0_149_PAYLOAD: &str =
        include_str!("../session_scanner/idle/fixtures/codex-agent-turn-complete-0.149.0.json");

    // Regression: 791f6be centralized Codex launch rendering without a native
    // turn-complete sink, leaving managed sessions on delayed rchar hysteresis.
    #[test]
    fn observed_codex_0_149_payload_maps_thread_id() {
        let event = parse_event(OBSERVED_CODEX_0_149_PAYLOAD).expect("observed payload");

        assert_eq!(event.event_type(), Some("agent-turn-complete"));
        assert_eq!(
            event.session_id(),
            Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1")
        );
    }

    #[test]
    fn sink_appends_one_enveloped_json_line() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        let now = Utc
            .with_ymd_and_hms(2026, 8, 26, 12, 0, 0)
            .single()
            .expect("timestamp");

        let outcome =
            append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, now).expect("append notify event");
        let lines = std::fs::read_to_string(&path).expect("notify sink");
        let records = lines.lines().collect::<Vec<_>>();

        assert!(!outcome.truncated);
        assert_eq!(records.len(), 1);
        let record: CodexNotifyRecord = serde_json::from_str(records[0]).expect("record");
        assert_eq!(record.ts, now);
        assert_eq!(
            record.session_id.as_deref(),
            Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1")
        );
        assert_eq!(record.event["type"], "agent-turn-complete");
    }

    #[test]
    fn sink_truncates_at_five_megabytes_before_append() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("codex-notify.jsonl");
        let mut file = std::fs::File::create(&path).expect("oversized sink");
        file.set_len(MAX_CODEX_NOTIFY_BYTES + 1)
            .expect("extend fixture");
        file.flush().expect("flush fixture");

        let outcome = append_event_at(&path, OBSERVED_CODEX_0_149_PAYLOAD, Utc::now())
            .expect("append after cap");
        let contents = std::fs::read_to_string(&path).expect("capped sink");

        assert!(outcome.truncated);
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.len() < 4096);
    }
}
