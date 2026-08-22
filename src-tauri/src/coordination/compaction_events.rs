//! Structured compaction observability event helpers.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::session_scanner::cli_tool::CliTool;
use taurhaus_lib::logging::emit_global;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionSignalKind {
    Compacted,
    ContextCompacted,
}

impl CompactionSignalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Compacted => "compacted",
            Self::ContextCompacted => "context_compacted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionUnresolvedReason {
    ManagedMemberResolutionUnavailable,
    MissingSessionId,
    MissingPaneId,
    MissingOperationalSnapshot,
}

impl CompactionUnresolvedReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::ManagedMemberResolutionUnavailable => "managed_member_resolution_unavailable",
            Self::MissingSessionId => "missing_session_id",
            Self::MissingPaneId => "missing_pane_id",
            Self::MissingOperationalSnapshot => "missing_operational_snapshot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSignalEvent {
    pub tool: CliTool,
    pub team_name: Option<String>,
    pub member_name: Option<String>,
    pub session_id: Option<String>,
    pub pane_id: Option<String>,
    pub project_path: Option<String>,
    pub jsonl_path: Option<String>,
    pub compaction_timestamp: Option<DateTime<Utc>>,
    pub signal_kind: Option<CompactionSignalKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionUnresolvedEvent {
    pub tool: CliTool,
    pub session_id: Option<String>,
    pub pane_id: Option<String>,
    pub project_path: String,
    pub jsonl_path: Option<String>,
    pub compaction_timestamp: DateTime<Utc>,
    pub signal_kind: Option<CompactionSignalKind>,
    pub reason: CompactionUnresolvedReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionExtractorHeartbeatEvent {
    pub tool: CliTool,
    pub active_file_count: usize,
    pub tracked_offset_count: usize,
    pub pending_signal_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionExtractorFailedEvent {
    pub tool: CliTool,
    pub jsonl_path: String,
    pub stage: String,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionWatcherMissedEventRecovered {
    pub tool: CliTool,
    pub recovered_count: usize,
    pub team_name: Option<String>,
    pub member_name: Option<String>,
    pub session_id: Option<String>,
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionDeliveryEvent {
    pub tool: CliTool,
    pub team_name: String,
    pub member_name: String,
    pub session_id: String,
    pub compaction_timestamp: DateTime<Utc>,
    pub delivery_result: String,
    pub skip_reason: Option<String>,
    pub fail_reason: Option<String>,
}

pub fn emit_compaction_signal_emitted(event: CompactionSignalEvent) {
    emit_global(
        "info",
        "coordination",
        "compaction.signal_emitted",
        Some("Compaction signal emitted".to_string()),
        signal_fields(event),
    );
}

pub fn emit_compaction_signal_consumed(event: CompactionSignalEvent) {
    emit_global(
        "info",
        "coordination",
        "compaction.signal_consumed",
        Some("Compaction signal consumed".to_string()),
        signal_fields(event),
    );
}

pub fn emit_compaction_signal_failed(event: CompactionSignalEvent, error_message: &str) {
    let mut fields = signal_fields(event);
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    emit_global(
        "warn",
        "coordination",
        "compaction.signal_failed",
        Some("Compaction signal processing failed and was committed".to_string()),
        fields,
    );
}

pub fn emit_compaction_owner_selected(owner: &str, status: &str, reason: &str) {
    let mut fields = Map::new();
    fields.insert("owner".to_string(), Value::String(owner.to_string()));
    fields.insert("status".to_string(), Value::String(status.to_string()));
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    emit_global(
        "info",
        "coordination",
        "compaction.owner.selected",
        Some("Compaction pipeline owner selected".to_string()),
        fields,
    );
}

pub fn emit_compaction_owner_failed(owner: &str, reason: &str, error_message: &str) {
    let mut fields = Map::new();
    fields.insert("owner".to_string(), Value::String(owner.to_string()));
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    emit_global(
        "warn",
        "coordination",
        "compaction.owner.failed",
        Some("Compaction pipeline owner failed to start".to_string()),
        fields,
    );
}

pub fn emit_compaction_signal_replayed(event: CompactionSignalEvent) {
    emit_global(
        "info",
        "coordination",
        "compaction.signal_replayed",
        Some("Compaction signal replayed".to_string()),
        signal_fields(event),
    );
}

pub fn emit_compaction_unresolved(event: CompactionUnresolvedEvent) {
    emit_global(
        "warn",
        "coordination",
        "compaction.unresolved",
        Some("Compaction signal could not be resolved to a managed member".to_string()),
        unresolved_fields(event),
    );
}

pub fn emit_compaction_extractor_heartbeat(event: CompactionExtractorHeartbeatEvent) {
    emit_global(
        "debug",
        "coordination",
        "compaction.extractor.heartbeat",
        Some("Compaction extractor heartbeat".to_string()),
        extractor_heartbeat_fields(event),
    );
}

pub fn emit_compaction_extractor_failed(event: CompactionExtractorFailedEvent) {
    emit_global(
        "warn",
        "coordination",
        "compaction.extractor.failed",
        Some("Compaction extractor failed while processing a file".to_string()),
        extractor_failed_fields(event),
    );
}

pub fn emit_compaction_watcher_missed_event_recovered(
    event: CompactionWatcherMissedEventRecovered,
) {
    emit_global(
        "info",
        "coordination",
        "compaction.watcher.missed_event_recovered",
        Some("Compaction watcher recovered previously unconsumed signals".to_string()),
        watcher_recovered_fields(event),
    );
}

pub fn emit_compaction_delivery(event_name: &str, event: CompactionDeliveryEvent) {
    emit_global(
        if event.delivery_result == "failed" {
            "warn"
        } else {
            "info"
        },
        "coordination",
        event_name,
        Some("Compaction delivery outcome recorded".to_string()),
        delivery_fields(event),
    );
}

pub fn signal_event(
    tool: CliTool,
    session_id: Option<&str>,
    pane_id: Option<&str>,
    project_path: Option<&str>,
    jsonl_path: Option<&Path>,
    compaction_timestamp: Option<DateTime<Utc>>,
    signal_kind: Option<CompactionSignalKind>,
) -> CompactionSignalEvent {
    CompactionSignalEvent {
        tool,
        team_name: None,
        member_name: None,
        session_id: session_id.map(ToOwned::to_owned),
        pane_id: pane_id.map(ToOwned::to_owned),
        project_path: project_path.map(ToOwned::to_owned),
        jsonl_path: jsonl_path.map(path_display),
        compaction_timestamp,
        signal_kind,
    }
}

fn signal_fields(event: CompactionSignalEvent) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    insert_optional_string(&mut fields, "team_name", event.team_name);
    insert_optional_string(&mut fields, "member_name", event.member_name);
    insert_optional_string(&mut fields, "session_id", event.session_id);
    insert_optional_string(&mut fields, "pane_id", event.pane_id);
    insert_optional_string(&mut fields, "project_path", event.project_path);
    insert_optional_string(&mut fields, "jsonl_path", event.jsonl_path);
    if let Some(compaction_timestamp) = event.compaction_timestamp {
        fields.insert(
            "compaction_timestamp".to_string(),
            Value::String(compaction_timestamp.to_rfc3339()),
        );
    }
    if let Some(signal_kind) = event.signal_kind {
        fields.insert(
            "signal_kind".to_string(),
            Value::String(signal_kind.as_str().to_string()),
        );
    }
    fields
}

fn unresolved_fields(event: CompactionUnresolvedEvent) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    fields.insert(
        "project_path".to_string(),
        Value::String(event.project_path),
    );
    fields.insert(
        "compaction_timestamp".to_string(),
        Value::String(event.compaction_timestamp.to_rfc3339()),
    );
    fields.insert(
        "reason".to_string(),
        Value::String(event.reason.as_str().to_string()),
    );
    insert_optional_string(&mut fields, "session_id", event.session_id);
    insert_optional_string(&mut fields, "pane_id", event.pane_id);
    insert_optional_string(&mut fields, "jsonl_path", event.jsonl_path);
    if let Some(signal_kind) = event.signal_kind {
        fields.insert(
            "signal_kind".to_string(),
            Value::String(signal_kind.as_str().to_string()),
        );
    }
    fields
}

fn extractor_heartbeat_fields(event: CompactionExtractorHeartbeatEvent) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    fields.insert(
        "active_file_count".to_string(),
        Value::from(event.active_file_count as u64),
    );
    fields.insert(
        "tracked_offset_count".to_string(),
        Value::from(event.tracked_offset_count as u64),
    );
    fields.insert(
        "pending_signal_count".to_string(),
        Value::from(event.pending_signal_count as u64),
    );
    fields
}

fn extractor_failed_fields(event: CompactionExtractorFailedEvent) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    fields.insert("jsonl_path".to_string(), Value::String(event.jsonl_path));
    fields.insert("stage".to_string(), Value::String(event.stage));
    fields.insert(
        "error.message".to_string(),
        Value::String(event.error_message),
    );
    fields
}

fn watcher_recovered_fields(event: CompactionWatcherMissedEventRecovered) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    fields.insert(
        "recovered_count".to_string(),
        Value::from(event.recovered_count as u64),
    );
    insert_optional_string(&mut fields, "team_name", event.team_name);
    insert_optional_string(&mut fields, "member_name", event.member_name);
    insert_optional_string(&mut fields, "session_id", event.session_id);
    insert_optional_string(&mut fields, "pane_id", event.pane_id);
    fields
}

fn delivery_fields(event: CompactionDeliveryEvent) -> Map<String, Value> {
    let mut fields = base_fields(event.tool);
    fields.insert("team_name".to_string(), Value::String(event.team_name));
    fields.insert("member_name".to_string(), Value::String(event.member_name));
    fields.insert("session_id".to_string(), Value::String(event.session_id));
    fields.insert(
        "compaction_timestamp".to_string(),
        Value::String(event.compaction_timestamp.to_rfc3339()),
    );
    fields.insert(
        "delivery_result".to_string(),
        Value::String(event.delivery_result),
    );
    insert_optional_string(&mut fields, "skip_reason", event.skip_reason);
    insert_optional_string(&mut fields, "fail_reason", event.fail_reason);
    fields
}

fn base_fields(tool: CliTool) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("tool".to_string(), Value::String(tool.to_string()));
    fields
}

fn insert_optional_string(fields: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        fields.insert(key.to_string(), Value::String(value));
    }
}

fn path_display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use taurhaus_lib::logging::{install_global_sink, LogFileState};
    use tempfile::TempDir;

    #[test]
    fn signal_fields_include_optional_metadata_when_present() {
        let fields = signal_fields(CompactionSignalEvent {
            tool: CliTool::Codex,
            team_name: Some("taurhaus-team".to_string()),
            member_name: Some("developer2".to_string()),
            session_id: Some("session-1".to_string()),
            pane_id: Some("%7".to_string()),
            project_path: Some("/home/user/projects/taurhaus".to_string()),
            jsonl_path: Some("/tmp/session.jsonl".to_string()),
            compaction_timestamp: Some(
                DateTime::parse_from_rfc3339("2026-03-08T20:00:00Z")
                    .expect("timestamp")
                    .with_timezone(&Utc),
            ),
            signal_kind: Some(CompactionSignalKind::Compacted),
        });

        assert_eq!(
            fields.get("tool"),
            Some(&Value::String("codex".to_string()))
        );
        assert_eq!(
            fields.get("team_name"),
            Some(&Value::String("taurhaus-team".to_string()))
        );
        assert_eq!(
            fields.get("signal_kind"),
            Some(&Value::String("compacted".to_string()))
        );
        assert_eq!(
            fields.get("compaction_timestamp"),
            Some(&Value::String("2026-03-08T20:00:00+00:00".to_string()))
        );
    }

    #[test]
    fn unresolved_fields_capture_reason_and_raw_metadata() {
        let fields = unresolved_fields(CompactionUnresolvedEvent {
            tool: CliTool::Codex,
            session_id: Some("session-2".to_string()),
            pane_id: None,
            project_path: "/home/user/projects/2ksim".to_string(),
            jsonl_path: Some("/tmp/codex.jsonl".to_string()),
            compaction_timestamp: DateTime::parse_from_rfc3339("2026-03-08T20:01:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
            signal_kind: Some(CompactionSignalKind::ContextCompacted),
            reason: CompactionUnresolvedReason::ManagedMemberResolutionUnavailable,
        });

        assert_eq!(
            fields.get("reason"),
            Some(&Value::String(
                "managed_member_resolution_unavailable".to_string()
            ))
        );
        assert_eq!(
            fields.get("signal_kind"),
            Some(&Value::String("context_compacted".to_string()))
        );
        assert_eq!(
            fields.get("project_path"),
            Some(&Value::String("/home/user/projects/2ksim".to_string()))
        );
    }

    #[test]
    fn heartbeat_fields_report_counts() {
        let fields = extractor_heartbeat_fields(CompactionExtractorHeartbeatEvent {
            tool: CliTool::Codex,
            active_file_count: 3,
            tracked_offset_count: 4,
            pending_signal_count: 2,
        });

        assert_eq!(fields.get("active_file_count"), Some(&Value::from(3u64)));
        assert_eq!(fields.get("tracked_offset_count"), Some(&Value::from(4u64)));
        assert_eq!(fields.get("pending_signal_count"), Some(&Value::from(2u64)));
    }

    #[test]
    fn emit_compaction_unresolved_writes_structured_event() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = TempDir::new().expect("tempdir");
        let log_path = tmp.path().join("compaction-events.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        emit_compaction_unresolved(CompactionUnresolvedEvent {
            tool: CliTool::Codex,
            session_id: Some("session-3".to_string()),
            pane_id: Some("%9".to_string()),
            project_path: "/home/user/projects/taurhaus".to_string(),
            jsonl_path: Some("/tmp/codex.jsonl".to_string()),
            compaction_timestamp: DateTime::parse_from_rfc3339("2026-03-08T20:05:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
            signal_kind: Some(CompactionSignalKind::Compacted),
            reason: CompactionUnresolvedReason::ManagedMemberResolutionUnavailable,
        });

        let contents = wait_for_log_contains(&log_path, "\"event\":\"compaction.unresolved\"");
        assert!(contents.contains("\"reason\":\"managed_member_resolution_unavailable\""));
        assert!(contents.contains("\"tool\":\"codex\""));
        assert!(contents.contains("\"session_id\":\"session-3\""));
    }

    #[test]
    fn delivery_fields_include_skip_and_fail_reason_when_present() {
        let fields = delivery_fields(CompactionDeliveryEvent {
            tool: CliTool::Codex,
            team_name: "taurhaus-team".to_string(),
            member_name: "developer1".to_string(),
            session_id: "session-4".to_string(),
            compaction_timestamp: DateTime::parse_from_rfc3339("2026-03-08T20:07:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
            delivery_result: "failed".to_string(),
            skip_reason: Some("already_handled".to_string()),
            fail_reason: Some("append_inbox_failed".to_string()),
        });

        assert_eq!(
            fields.get("skip_reason"),
            Some(&Value::String("already_handled".to_string()))
        );
        assert_eq!(
            fields.get("fail_reason"),
            Some(&Value::String("append_inbox_failed".to_string()))
        );
        assert_eq!(
            fields.get("delivery_result"),
            Some(&Value::String("failed".to_string()))
        );
    }

    fn wait_for_log_contains(path: &std::path::Path, needle: &str) -> String {
        for _ in 0..50 {
            if let Ok(contents) = fs::read_to_string(path) {
                if contents.contains(needle) {
                    return contents;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        fs::read_to_string(path).unwrap_or_default()
    }
}
