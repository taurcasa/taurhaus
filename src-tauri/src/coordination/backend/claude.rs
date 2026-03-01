//! Claude-native backend placeholder.

use super::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::domain::HealthState;
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{DeliveryMethod, ProbeEvidence};
use crate::coordination::requests::{
    DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult, ProbeRequest, ProbeResult,
    TeardownRequest, TeardownResult,
};

/// Placeholder for the Claude-native backend implementation.
#[derive(Debug, Default)]
pub struct ClaudeNativeBackend;

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

    fn deliver(&self, _req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        Ok(DeliveryResult {
            delivered: false,
            method: DeliveryMethod::NativeMessageApi,
        })
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
