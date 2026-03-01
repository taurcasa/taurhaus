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

    fn deliver(&self, _req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        let mut calls = self.calls.lock().expect("fake backend mutex poisoned");
        calls.delivery_count += 1;

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
