//! Backend-agnostic coordination requests and responses.

use serde::{Deserialize, Serialize};

use crate::coordination::domain::{HealthState, Member};

/// Launch-time policy controls for a member session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPermissions {
    Standard,
    Restricted,
    Elevated,
}

/// Request to launch or re-attach a managed team member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub member: Member,
    pub team_name: String,
    pub pane_target: Option<String>,
    pub permissions: LaunchPermissions,
}

/// Result of a backend launch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchResult {
    pub pane_id: String,
    pub process_id: Option<u32>,
}

/// Typed payload for first-contact delivery after launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapDelivery {
    pub member_name: String,
    pub team_name: String,
    pub message: String,
}

/// Typed payload for recovery nudges when health degrades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryNudgeDelivery {
    pub member_name: String,
    pub team_name: String,
    pub reason: String,
}

/// Typed payload for operator-authored notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorNoticeDelivery {
    pub member_name: String,
    pub team_name: String,
    pub message: String,
}

/// Typed delivery request variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "payload")]
pub enum DeliveryRequest {
    Bootstrap(BootstrapDelivery),
    RecoveryNudge(RecoveryNudgeDelivery),
    OperatorNotice(OperatorNoticeDelivery),
}

/// Mechanism used by backend to deliver a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMethod {
    InboxFile,
    TmuxInjection,
    NativeMessageApi,
}

/// Delivery completion response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryResult {
    pub delivered: bool,
    pub method: DeliveryMethod,
}

/// Request to probe a member's process and interaction health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRequest {
    pub member_name: String,
    pub team_name: String,
}

/// Signal quality produced by probe execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeEvidence {
    None,
    WeakIo,
    StrongInbox,
}

/// Probe response used by health monitoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub alive: bool,
    pub health: HealthState,
    pub evidence: ProbeEvidence,
}

/// Teardown mode for stopping member sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeardownMode {
    Graceful,
    Force,
}

/// Request to tear down a member session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeardownRequest {
    pub member_name: String,
    pub team_name: String,
    pub mode: TeardownMode,
}

/// Result of a teardown attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeardownResult {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::coordination::domain::MemberRole;
    use crate::session_scanner::cli_tool::CliTool;

    #[test]
    fn launch_request_round_trip() {
        let req = LaunchRequest {
            member: Member {
                name: "agent-1".to_string(),
                role: MemberRole::Agent,
                instructions: Some("Focus on implementation".to_string()),
                project_path: PathBuf::from("/tmp/taurhaus"),
                cli_tool: CliTool::Codex,
            },
            team_name: "architecture-final".to_string(),
            pane_target: Some("main.%0".to_string()),
            permissions: LaunchPermissions::Standard,
        };

        let encoded = serde_json::to_string(&req).expect("request should serialize");
        let decoded: LaunchRequest =
            serde_json::from_str(&encoded).expect("request should deserialize");
        assert_eq!(decoded, req);
    }

    #[test]
    fn delivery_request_round_trip() {
        let req = DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: "agent-1".to_string(),
            team_name: "architecture-final".to_string(),
            message: "Check your inbox".to_string(),
        });

        let encoded = serde_json::to_value(&req).expect("request should serialize");
        let decoded: DeliveryRequest =
            serde_json::from_value(encoded).expect("request should deserialize");
        assert_eq!(decoded, req);
    }
}
