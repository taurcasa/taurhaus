use serde_json::{Map, Value};

use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent, MemberAddedEvent, MemberRemovedEvent, TeamCreatedEvent,
    TeamDisbandedEvent,
};

pub(super) fn emit_audit_event_to_structured_log(event: &AuditEvent) {
    let event_name = format!("coordination.audit.{}", event.event_type());
    let mut fields = audit_event_log_fields(event);
    let audit_record = serde_json::to_value(event)
        .unwrap_or_else(|error| Value::String(format!("audit serialize failed: {error}")));
    fields.insert("audit_record".to_string(), audit_record);
    let level = match event {
        AuditEvent::DeliveryFailed(_) => "warn",
        _ => "info",
    };
    taurhaus_lib::logging::emit_global(
        level,
        "backend",
        &event_name,
        Some("Coordination audit event".to_string()),
        fields,
    );
}

pub(super) fn audit_event_log_fields(event: &AuditEvent) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "audit_event_type".to_string(),
        Value::String(event.event_type().to_string()),
    );
    match event {
        AuditEvent::TeamCreated(TeamCreatedEvent {
            team_name,
            member_count,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_count".to_string(),
                Value::Number(serde_json::Number::from(*member_count as u64)),
            );
        }
        AuditEvent::TeamDisbanded(TeamDisbandedEvent {
            team_name, reason, ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            if let Some(reason) = reason.as_ref() {
                fields.insert("reason".to_string(), Value::String(reason.clone()));
            }
        }
        AuditEvent::MemberAdded(MemberAddedEvent {
            team_name,
            member_name,
            role,
            backend,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            fields.insert(
                "role".to_string(),
                serde_json::to_value(role).unwrap_or(Value::Null),
            );
            fields.insert(
                "backend".to_string(),
                serde_json::to_value(backend).unwrap_or(Value::Null),
            );
        }
        AuditEvent::MemberRemoved(MemberRemovedEvent {
            team_name,
            member_name,
            reason,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            if let Some(reason) = reason.as_ref() {
                fields.insert("reason".to_string(), Value::String(reason.clone()));
            }
        }
        AuditEvent::DeliveryAttempted(DeliveryAttemptedEvent {
            team_name,
            member_name,
            delivery_type,
            method,
            ..
        })
        | AuditEvent::DeliverySucceeded(DeliverySucceededEvent {
            team_name,
            member_name,
            delivery_type,
            method,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            fields.insert(
                "delivery_type".to_string(),
                Value::String(delivery_type.clone()),
            );
            fields.insert(
                "method".to_string(),
                serde_json::to_value(method).unwrap_or_default(),
            );
        }
        AuditEvent::DeliveryFailed(DeliveryFailedEvent {
            team_name,
            member_name,
            delivery_type,
            error,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            fields.insert(
                "delivery_type".to_string(),
                Value::String(delivery_type.clone()),
            );
            fields.insert("error".to_string(), Value::String(error.clone()));
        }
        AuditEvent::LeaseClaimed(LeaseClaimedEvent {
            team_name,
            member_name,
            owner_pid,
            instance_uuid,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            fields.insert(
                "owner_pid".to_string(),
                Value::Number(serde_json::Number::from(*owner_pid)),
            );
            fields.insert(
                "instance_uuid".to_string(),
                Value::String(instance_uuid.clone()),
            );
        }
        AuditEvent::LeaseReclaimed(LeaseReclaimedEvent {
            team_name,
            member_name,
            previous_pid,
            new_pid,
            ..
        }) => {
            fields.insert("team_name".to_string(), Value::String(team_name.clone()));
            fields.insert(
                "member_name".to_string(),
                Value::String(member_name.clone()),
            );
            fields.insert(
                "previous_pid".to_string(),
                Value::Number(serde_json::Number::from(*previous_pid)),
            );
            fields.insert(
                "new_pid".to_string(),
                Value::Number(serde_json::Number::from(*new_pid)),
            );
        }
    }
    fields
}
