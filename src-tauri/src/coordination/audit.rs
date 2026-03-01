//! Typed audit events for coordination operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::backend::BackendKind;
use crate::coordination::domain::MemberRole;
use crate::coordination::requests::DeliveryMethod;

/// Typed event envelope for coordination audit logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AuditEvent {
    TeamCreated(TeamCreatedEvent),
    TeamDisbanded(TeamDisbandedEvent),
    MemberAdded(MemberAddedEvent),
    MemberRemoved(MemberRemovedEvent),
    DeliveryAttempted(DeliveryAttemptedEvent),
    DeliverySucceeded(DeliverySucceededEvent),
    DeliveryFailed(DeliveryFailedEvent),
    LeaseClaimed(LeaseClaimedEvent),
    LeaseReclaimed(LeaseReclaimedEvent),
}

impl AuditEvent {
    /// Stable event-name helper for lightweight logging/metrics labels.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TeamCreated(_) => "team_created",
            Self::TeamDisbanded(_) => "team_disbanded",
            Self::MemberAdded(_) => "member_added",
            Self::MemberRemoved(_) => "member_removed",
            Self::DeliveryAttempted(_) => "delivery_attempted",
            Self::DeliverySucceeded(_) => "delivery_succeeded",
            Self::DeliveryFailed(_) => "delivery_failed",
            Self::LeaseClaimed(_) => "lease_claimed",
            Self::LeaseReclaimed(_) => "lease_reclaimed",
        }
    }
}

/// Team created mutation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamCreatedEvent {
    pub team_name: String,
    pub member_count: usize,
    pub created_at: DateTime<Utc>,
}

/// Team disbanded mutation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDisbandedEvent {
    pub team_name: String,
    pub reason: Option<String>,
    pub disbanded_at: DateTime<Utc>,
}

/// Member added mutation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberAddedEvent {
    pub team_name: String,
    pub member_name: String,
    pub role: MemberRole,
    pub backend: BackendKind,
    pub added_at: DateTime<Utc>,
}

/// Member removed mutation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRemovedEvent {
    pub team_name: String,
    pub member_name: String,
    pub reason: Option<String>,
    pub removed_at: DateTime<Utc>,
}

/// Delivery started event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAttemptedEvent {
    pub team_name: String,
    pub member_name: String,
    pub delivery_type: String,
    pub method: DeliveryMethod,
    pub attempted_at: DateTime<Utc>,
}

/// Delivery success event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliverySucceededEvent {
    pub team_name: String,
    pub member_name: String,
    pub delivery_type: String,
    pub method: DeliveryMethod,
    pub succeeded_at: DateTime<Utc>,
}

/// Delivery failure event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryFailedEvent {
    pub team_name: String,
    pub member_name: String,
    pub delivery_type: String,
    pub error: String,
    pub failed_at: DateTime<Utc>,
}

/// Lease claim event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseClaimedEvent {
    pub team_name: String,
    pub member_name: String,
    pub owner_pid: u32,
    pub instance_uuid: String,
    pub claimed_at: DateTime<Utc>,
}

/// Lease reclaim event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseReclaimedEvent {
    pub team_name: String,
    pub member_name: String,
    pub previous_pid: u32,
    pub new_pid: u32,
    pub reclaimed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn event_type_matches_variant() {
        let event = AuditEvent::TeamCreated(TeamCreatedEvent {
            team_name: "architecture-final".to_string(),
            member_count: 2,
            created_at: ts(),
        });
        assert_eq!(event.event_type(), "team_created");
    }

    #[test]
    fn team_created_schema_snapshot() {
        let event = AuditEvent::TeamCreated(TeamCreatedEvent {
            team_name: "architecture-final".to_string(),
            member_count: 2,
            created_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "team_created",
            "payload": {
                "team_name": "architecture-final",
                "member_count": 2,
                "created_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn team_disbanded_schema_snapshot() {
        let event = AuditEvent::TeamDisbanded(TeamDisbandedEvent {
            team_name: "architecture-final".to_string(),
            reason: Some("operator request".to_string()),
            disbanded_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "team_disbanded",
            "payload": {
                "team_name": "architecture-final",
                "reason": "operator request",
                "disbanded_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn member_added_schema_snapshot() {
        let event = AuditEvent::MemberAdded(MemberAddedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            role: MemberRole::Agent,
            backend: BackendKind::MeshBridged,
            added_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "member_added",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "role": "agent",
                "backend": "mesh_bridged",
                "added_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn member_removed_schema_snapshot() {
        let event = AuditEvent::MemberRemoved(MemberRemovedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            reason: Some("manual removal".to_string()),
            removed_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "member_removed",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "reason": "manual removal",
                "removed_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn delivery_attempted_schema_snapshot() {
        let event = AuditEvent::DeliveryAttempted(DeliveryAttemptedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            delivery_type: "operator_notice".to_string(),
            method: DeliveryMethod::TmuxInjection,
            attempted_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "delivery_attempted",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "delivery_type": "operator_notice",
                "method": "tmux_injection",
                "attempted_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn delivery_succeeded_schema_snapshot() {
        let event = AuditEvent::DeliverySucceeded(DeliverySucceededEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            delivery_type: "operator_notice".to_string(),
            method: DeliveryMethod::TmuxInjection,
            succeeded_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "delivery_succeeded",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "delivery_type": "operator_notice",
                "method": "tmux_injection",
                "succeeded_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn delivery_failed_schema_snapshot() {
        let event = AuditEvent::DeliveryFailed(DeliveryFailedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            delivery_type: "recovery_nudge".to_string(),
            error: "pane missing".to_string(),
            failed_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "delivery_failed",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "delivery_type": "recovery_nudge",
                "error": "pane missing",
                "failed_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn lease_claimed_schema_snapshot() {
        let event = AuditEvent::LeaseClaimed(LeaseClaimedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            owner_pid: 4242,
            instance_uuid: "instance-1".to_string(),
            claimed_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "lease_claimed",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "owner_pid": 4242,
                "instance_uuid": "instance-1",
                "claimed_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }

    #[test]
    fn lease_reclaimed_schema_snapshot() {
        let event = AuditEvent::LeaseReclaimed(LeaseReclaimedEvent {
            team_name: "architecture-final".to_string(),
            member_name: "codex-reviewer".to_string(),
            previous_pid: 4000,
            new_pid: 4242,
            reclaimed_at: ts(),
        });
        let got = serde_json::to_value(event).expect("serialize");
        let expected = json!({
            "type": "lease_reclaimed",
            "payload": {
                "team_name": "architecture-final",
                "member_name": "codex-reviewer",
                "previous_pid": 4000,
                "new_pid": 4242,
                "reclaimed_at": "2026-03-01T21:00:00Z"
            }
        });
        assert_eq!(got, expected);
    }
}
