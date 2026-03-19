//! Coordination orchestrator service.

mod audit_logging;
mod delivery;
mod helpers;
mod lifecycle;
mod liveness;
mod teardown;

use std::path::PathBuf;
use std::sync::Arc;

use crate::coordination::audit::AuditEvent;
use crate::coordination::backend::CoordinationBackend;
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{MemberRuntimeRecord, TeamConfig};

/// Aggregated status view for a single team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamStatus {
    pub config: TeamConfig,
    pub members_runtime: Vec<(String, MemberRuntimeRecord)>,
}

/// Discovered team status used to restore mesh tabs on app reopen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTeamStatus {
    pub team_name: String,
    pub lead_project_path: Option<PathBuf>,
}

/// Team discovery payload with recoverable warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDiscoveryStatus {
    pub teams: Vec<DiscoveredTeamStatus>,
    pub warnings: Vec<String>,
}

/// Result of disbanding a team with idempotent semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisbandTeamResult {
    pub team_name: String,
    pub disbanded: bool,
    pub already_disbanded: bool,
}

/// Result of removing a member with teardown diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMemberResult {
    pub team_name: String,
    pub member_name: String,
    pub removed: bool,
    pub steps: Vec<RemoveMemberStepResult>,
    pub warnings: Vec<String>,
}

/// Result of a bounded team self-heal pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSelfHealResult {
    pub team_name: String,
    pub runtime_candidate_found: bool,
    pub member_liveness_reconciled: bool,
    pub team_daemon_ensured: bool,
}

/// Per-step teardown status for runtime member removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveMemberStepResult {
    pub step: String,
    pub success: bool,
    pub message: Option<String>,
}

/// Top-level coordination service entrypoint.
pub struct CoordinationOrchestrator {
    pub(crate) teams_dir: PathBuf,
    pub(crate) audit_log: Vec<AuditEvent>,
    pub(crate) backend: Arc<dyn CoordinationBackend>,
    /// Optional per-tool backend for Claude agents (inbox file delivery).
    /// When set, `deliver_message()` routes Claude members through this backend
    /// instead of the default `backend` (which may be MeshBridged).
    /// Left as `None` in tests so FakeBackend captures all deliveries.
    pub(crate) claude_backend: Option<Arc<dyn CoordinationBackend>>,
    pub(crate) runtime: Arc<dyn CoordinationRuntime>,
}

impl std::fmt::Debug for CoordinationOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinationOrchestrator")
            .field("teams_dir", &self.teams_dir)
            .field("audit_log_len", &self.audit_log.len())
            .field("backend_kind", &self.backend.kind())
            .finish()
    }
}

impl CoordinationOrchestrator {
    pub fn new(teams_dir: PathBuf, backend: Arc<dyn CoordinationBackend>) -> Self {
        Self::new_with_runtime(teams_dir, backend, Arc::new(SystemCoordinationRuntime))
    }

    pub fn new_with_runtime(
        teams_dir: PathBuf,
        backend: Arc<dyn CoordinationBackend>,
        runtime: Arc<dyn CoordinationRuntime>,
    ) -> Self {
        Self {
            teams_dir,
            audit_log: Vec::new(),
            backend,
            claude_backend: None,
            runtime,
        }
    }
}

#[cfg(test)]
mod tests;
