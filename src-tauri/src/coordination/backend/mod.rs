//! Backend abstractions and backend-specific implementations.

pub mod bridged;
pub mod claude;
#[cfg(test)]
pub mod fake;
pub mod selector;

pub use bridged::MeshBridgedBackend;
pub use claude::ClaudeNativeBackend;
#[cfg(test)]
pub use fake::FakeBackend;
pub use selector::BackendSelector;

use serde::{Deserialize, Serialize};

use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    DeliveryRequest, DeliveryResult, LaunchRequest, LaunchResult, ProbeRequest, ProbeResult,
    TeardownRequest, TeardownResult,
};

/// Available coordination backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    ClaudeNative,
    MeshBridged,
}

/// Closed capability model for backend operational semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub can_launch_with_identity: bool,
    pub supports_out_of_band_delivery: bool,
    pub supports_native_peer_messaging: bool,
    pub supports_native_shared_tasks: bool,
    pub supports_attachment_rebind: bool,
    pub requires_sidecar_delivery: bool,
}

/// Backend contract for launch, delivery, probing, and teardown.
pub trait CoordinationBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;
    fn launch(&self, req: LaunchRequest) -> Result<LaunchResult, CoordinationError>;
    fn deliver(&self, req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError>;
    fn probe(&self, req: ProbeRequest) -> Result<ProbeResult, CoordinationError>;
    fn teardown(&self, req: TeardownRequest) -> Result<TeardownResult, CoordinationError>;
}

impl BackendCapabilities {
    /// Capability profile for the Claude-native backend.
    pub fn claude_native() -> Self {
        Self {
            can_launch_with_identity: true,
            supports_out_of_band_delivery: true,
            supports_native_peer_messaging: true,
            supports_native_shared_tasks: true,
            supports_attachment_rebind: true,
            requires_sidecar_delivery: false,
        }
    }

    /// Capability profile for the mesh-bridged backend.
    pub fn mesh_bridged() -> Self {
        Self {
            can_launch_with_identity: true,
            supports_out_of_band_delivery: true,
            supports_native_peer_messaging: false,
            supports_native_shared_tasks: false,
            supports_attachment_rebind: true,
            requires_sidecar_delivery: true,
        }
    }
}
