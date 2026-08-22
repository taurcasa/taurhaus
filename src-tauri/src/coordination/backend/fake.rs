//! Fake coordination backend for tests and integration contracts.

use std::sync::{Arc, Mutex};

use super::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::domain::HealthState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    DeliveryMethod, DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult, ProbeEvidence,
    ProbeRequest, ProbeResult, TeardownRequest, TeardownResult,
};

/// In-memory fake backend used for contract testing.
#[derive(Debug, Default, Clone)]
pub struct FakeBackend {
    calls: Arc<Mutex<FakeCalls>>,
    deliver_failure: Arc<Mutex<Option<ProgrammedError>>>,
    delivered_requests: Arc<Mutex<Vec<DeliveryRequest>>>,
}

#[derive(Debug, Default)]
struct FakeCalls {
    launch_count: usize,
    delivery_count: usize,
    probe_count: usize,
    teardown_count: usize,
}

#[derive(Debug, Clone)]
enum ProgrammedError {
    Validation(String),
    Io(String),
    Backend(String),
    NotFound(String),
    Conflict(String),
    StoreError(String),
}

impl ProgrammedError {
    fn from_error(err: CoordinationError) -> Self {
        match err {
            CoordinationError::Validation(msg) => Self::Validation(msg),
            CoordinationError::Io(io) => Self::Io(io.to_string()),
            CoordinationError::Backend(msg) => Self::Backend(msg),
            CoordinationError::NotFound(msg) => Self::NotFound(msg),
            CoordinationError::Conflict(msg) => Self::Conflict(msg),
            CoordinationError::StoreError(msg) => Self::StoreError(msg),
        }
    }

    fn to_error(&self) -> CoordinationError {
        match self {
            Self::Validation(msg) => CoordinationError::Validation(msg.clone()),
            Self::Io(msg) => CoordinationError::Io(std::io::Error::other(msg.clone())),
            Self::Backend(msg) => CoordinationError::Backend(msg.clone()),
            Self::NotFound(msg) => CoordinationError::NotFound(msg.clone()),
            Self::Conflict(msg) => CoordinationError::Conflict(msg.clone()),
            Self::StoreError(msg) => CoordinationError::StoreError(msg.clone()),
        }
    }
}

impl FakeBackend {
    pub fn call_counts(&self) -> (usize, usize, usize, usize) {
        let calls = self.calls.lock().expect("fake backend mutex poisoned");
        (
            calls.launch_count,
            calls.delivery_count,
            calls.probe_count,
            calls.teardown_count,
        )
    }

    /// Program this fake to fail all `deliver()` calls with the given error.
    pub fn set_deliver_error(&self, err: CoordinationError) {
        let mut slot = self
            .deliver_failure
            .lock()
            .expect("fake backend failure mutex poisoned");
        *slot = Some(ProgrammedError::from_error(err));
    }

    /// Clear any programmed delivery failure.
    pub fn clear_deliver_error(&self) {
        let mut slot = self
            .deliver_failure
            .lock()
            .expect("fake backend failure mutex poisoned");
        *slot = None;
    }

    pub fn delivered_requests(&self) -> Vec<DeliveryRequest> {
        self.delivered_requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

impl CoordinationBackend for FakeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::MeshBridged
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::mesh_bridged()
    }

    fn launch(&self, req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        let mut calls = self.calls.lock().expect("fake backend mutex poisoned");
        calls.launch_count += 1;
        Ok(LaunchResult {
            pane_id: req.pane_target.unwrap_or_else(|| "fake-pane".to_string()),
            process_id: Some(4242),
        })
    }

    fn deliver(&self, req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        let mut calls = self.calls.lock().expect("fake backend mutex poisoned");
        calls.delivery_count += 1;
        if let Ok(mut requests) = self.delivered_requests.lock() {
            requests.push(req.clone());
        }

        if let Some(err) = self
            .deliver_failure
            .lock()
            .expect("fake backend failure mutex poisoned")
            .as_ref()
        {
            return Err(err.to_error());
        }

        Ok(DeliveryResult {
            delivered: true,
            method: DeliveryMethod::TmuxInjection,
        })
    }

    fn probe(&self, _req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        let mut calls = self.calls.lock().expect("fake backend mutex poisoned");
        calls.probe_count += 1;
        Ok(ProbeResult {
            alive: true,
            health: HealthState::Healthy,
            evidence: ProbeEvidence::WeakIo,
        })
    }

    fn teardown(&self, _req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        let mut calls = self.calls.lock().expect("fake backend mutex poisoned");
        calls.teardown_count += 1;
        Ok(TeardownResult { success: true })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::requests::{LaunchPermissions, OperatorNoticeDelivery, TeardownMode};
    use crate::session_scanner::cli_tool::CliTool;

    fn sample_member(name: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            model: None,
            reasoning_effort: None,
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    #[test]
    fn trait_contract_methods_increment_counts() {
        let backend = FakeBackend::default();

        let _ = backend
            .launch(LaunchRequest {
                member: sample_member("alice"),
                team_name: "architecture-final".to_string(),
                pane_target: None,
                permissions: LaunchPermissions::Standard,
            })
            .expect("launch");

        let _ = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
                message: "status?".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect("deliver");

        let _ = backend
            .probe(ProbeRequest {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
            })
            .expect("probe");

        let _ = backend
            .teardown(TeardownRequest {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
                mode: TeardownMode::Graceful,
            })
            .expect("teardown");

        assert_eq!(backend.call_counts(), (1, 1, 1, 1));
    }

    #[test]
    fn programmed_delivery_failure_can_be_set_and_cleared() {
        let backend = FakeBackend::default();

        backend.set_deliver_error(CoordinationError::Backend(
            "simulated delivery failure".to_string(),
        ));
        let err = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
                message: "status?".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect_err("delivery should fail");
        match err {
            CoordinationError::Backend(message) => {
                assert!(message.contains("simulated delivery failure"))
            }
            other => panic!("expected backend error, got {other:?}"),
        }

        backend.clear_deliver_error();
        let result = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
                message: "status?".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect("delivery should recover");
        assert!(result.delivered);
    }

    #[test]
    fn programmed_io_failure_is_returned_as_io_error() {
        let backend = FakeBackend::default();
        backend.set_deliver_error(CoordinationError::Io(std::io::Error::other(
            "disk exploded",
        )));

        let err = backend
            .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
                message: "status?".to_string(),
                sender_name: None,
                operational_context: None,
            }))
            .expect_err("delivery should fail");
        match err {
            CoordinationError::Io(inner) => assert_eq!(inner.kind(), std::io::ErrorKind::Other),
            other => panic!("expected io error, got {other:?}"),
        }
    }
}
