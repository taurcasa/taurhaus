//! Claude-native backend delivery via shared inbox files.

use std::path::PathBuf;

use chrono::Utc;

use super::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::domain::HealthState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{DeliveryMethod, ProbeEvidence};
use crate::coordination::requests::{
    DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult, OperatorNoticeDelivery,
    ProbeRequest, ProbeResult, TeardownRequest, TeardownResult,
};
use crate::coordination::stores::{MeshInboxMessage, MeshInboxStore};

const DEFAULT_OPERATOR_NAME: &str = "taurhaus";
const OPERATOR_NOTICE_SUMMARY: &str = "operator_notice";

/// Claude-native backend implemented through the shared inbox file contract.
#[derive(Debug, Clone)]
pub struct ClaudeNativeBackend {
    teams_dir: PathBuf,
}

impl ClaudeNativeBackend {
    pub fn new(teams_dir: PathBuf) -> Self {
        Self { teams_dir }
    }

    fn send_operator_notice(
        &self,
        payload: OperatorNoticeDelivery,
    ) -> Result<DeliveryResult, CoordinationError> {
        let sender_name = payload
            .sender_name
            .as_deref()
            .map(str::trim)
            .filter(|sender| !sender.is_empty())
            .unwrap_or(DEFAULT_OPERATOR_NAME);
        let message = MeshInboxMessage::new(
            sender_name,
            payload.message,
            Some(OPERATOR_NOTICE_SUMMARY.to_string()),
            Utc::now(),
        );
        MeshInboxStore::append(
            &self.teams_dir,
            &payload.team_name,
            &payload.member_name,
            &message,
        )?;

        Ok(DeliveryResult {
            delivered: true,
            method: DeliveryMethod::InboxFile,
        })
    }
}

impl Default for ClaudeNativeBackend {
    fn default() -> Self {
        Self::new(crate::provider::platform_paths::PlatformPaths::teams_dir())
    }
}

impl CoordinationBackend for ClaudeNativeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::ClaudeNative
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::claude_native()
    }

    fn launch(&self, _req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        Err(CoordinationError::Backend(
            "ClaudeNativeBackend.launch is not implemented yet".to_string(),
        ))
    }

    fn deliver(&self, req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        match req {
            DeliveryRequest::OperatorNotice(payload) => self.send_operator_notice(*payload),
            other => Err(CoordinationError::Validation(format!(
                "ClaudeNative backend in C1 only supports operator_notice delivery, got: {other:?}"
            ))),
        }
    }

    fn probe(&self, _req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        Ok(ProbeResult {
            alive: false,
            health: HealthState::SessionDead,
            evidence: ProbeEvidence::None,
        })
    }

    fn teardown(&self, _req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        Ok(TeardownResult { success: false })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn deliver_operator_notice_appends_to_claude_member_inbox() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = ClaudeNativeBackend::new(tmp.path().to_path_buf());

        // Regression: Claude-side task delivery used to report success without
        // writing the actionable notice into the member inbox at all.
        let result = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "taurhaus-team".to_string(),
                member_name: "design-taurhaus".to_string(),
                message: "ACTION REQUIRED: Review the design packet.".to_string(),
                sender_name: Some("team-lead".to_string()),
                operational_context: None,
            }))
            .expect("delivery should succeed");

        assert_eq!(result.method, DeliveryMethod::InboxFile);
        assert!(result.delivered);

        let inbox = MeshInboxStore::load(tmp.path(), "taurhaus-team", "design-taurhaus")
            .expect("load inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].from, "team-lead");
        assert_eq!(inbox[0].summary.as_deref(), Some(OPERATOR_NOTICE_SUMMARY));
        assert_eq!(inbox[0].text, "ACTION REQUIRED: Review the design packet.");
        assert!(!inbox[0].read);
    }

    #[test]
    fn deliver_operator_notice_falls_back_to_default_sender_name() {
        let tmp = TempDir::new().expect("tempdir");
        let backend = ClaudeNativeBackend::new(tmp.path().to_path_buf());

        let result = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "taurhaus-team".to_string(),
                member_name: "product-check-1".to_string(),
                message: "ACTION REQUIRED: Run the product-check lane.".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect("delivery should succeed");

        assert!(result.delivered);

        let inbox = MeshInboxStore::load(tmp.path(), "taurhaus-team", "product-check-1")
            .expect("load inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].from, DEFAULT_OPERATOR_NAME);
    }
}
