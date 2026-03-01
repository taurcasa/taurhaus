//! Domain models for team coordination.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::session_scanner::cli_tool::CliTool;

/// Managed team configuration document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
}

/// Logical team member that persists independent of process attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub role: MemberRole,
    pub instructions: Option<String>,
    pub project_path: PathBuf,
    pub cli_tool: CliTool,
}

/// Team role for a logical member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Lead,
    Agent,
}

/// Reconstructible runtime state for an attached member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRuntimeState {
    pub pane_id: Option<String>,
    pub health: HealthState,
    pub delivery_lease: Option<DeliveryLease>,
    pub attached_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Backward-compatible alias used by higher-level orchestrator contracts.
pub type RuntimeState = MemberRuntimeState;

/// Per-member lease used to coordinate delivery ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryLease {
    pub owner_pid: u32,
    pub instance_uuid: String,
    pub hostname: String,
    pub heartbeat_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
}

/// Health status consumed by orchestrator and backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Healthy,
    AwaitingRead,
    SuspectedStuck,
    Rebriefed,
    Suppressed,
    SessionDead,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_serde_round_trip() {
        let team = Team {
            name: "architecture-final".to_string(),
            description: Some("Multi-agent architecture team".to_string()),
            created_at: DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
                .expect("valid RFC3339 timestamp")
                .with_timezone(&Utc),
            schema_version: 1,
        };

        let encoded = serde_json::to_string(&team).expect("team should serialize");
        let decoded: Team = serde_json::from_str(&encoded).expect("team should deserialize");
        assert_eq!(decoded.name, team.name);
        assert_eq!(decoded.description, team.description);
        assert_eq!(decoded.schema_version, team.schema_version);
        assert_eq!(decoded.created_at, team.created_at);
    }

    #[test]
    fn member_serde_round_trip() {
        let member = Member {
            name: "codex-reviewer".to_string(),
            role: MemberRole::Agent,
            instructions: Some("Review architecture tasks".to_string()),
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
        };

        let encoded = serde_json::to_string(&member).expect("member should serialize");
        let decoded: Member = serde_json::from_str(&encoded).expect("member should deserialize");
        assert_eq!(decoded, member);
    }

    #[test]
    fn runtime_state_serde_round_trip() {
        let started = DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc);
        let heartbeat = DateTime::parse_from_rfc3339("2026-03-01T21:05:00Z")
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc);

        let runtime = MemberRuntimeState {
            pane_id: Some("%12".to_string()),
            health: HealthState::Healthy,
            delivery_lease: Some(DeliveryLease {
                owner_pid: 4242,
                instance_uuid: "instance-1".to_string(),
                hostname: "devbox".to_string(),
                heartbeat_at: heartbeat,
                started_at: started,
            }),
            attached_at: Some(started),
            last_seen_at: Some(heartbeat),
        };

        let encoded = serde_json::to_value(&runtime).expect("runtime should serialize");
        let decoded: MemberRuntimeState =
            serde_json::from_value(encoded).expect("runtime should deserialize");
        assert_eq!(decoded, runtime);
    }
}
