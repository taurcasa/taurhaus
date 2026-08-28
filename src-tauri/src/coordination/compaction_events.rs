//! Structured observability for native compaction-hook delivery.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::session_scanner::cli_tool::CliTool;
use taurhaus_lib::logging::emit_global;

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

fn delivery_fields(event: CompactionDeliveryEvent) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("tool".to_string(), Value::String(event.tool.to_string()));
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

fn insert_optional_string(fields: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        fields.insert(key.to_string(), Value::String(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
