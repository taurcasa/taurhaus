use serde::{Deserialize, Serialize};

pub use crate::coordination::requests::{
    AgentRole, LeadMode, MemberActivationStage, SessionStatus, StepProgress, StepStatus,
};
use crate::templates::types::{BehavioralContract, RuntimeCompactSummary};

/// Lightweight team list entry returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub team_name: String,
    pub lead_project_path: Option<String>,
}

/// Discovery response with valid teams plus skipped-folder warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamDiscoveryResponse {
    pub teams: Vec<TeamSummary>,
    pub warnings: Vec<String>,
}

/// Team status payload returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatus {
    pub team_name: String,
    pub members: Vec<String>,
}

/// Disband response describing whether state was removed or already absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisbandTeamResponse {
    pub team_name: String,
    pub disbanded: bool,
    pub already_disbanded: bool,
    pub message: String,
}

/// Agent setup card payload from the frontend setup form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetupConfig {
    pub name: String,
    pub cli_tool: String,
    pub model: String,
    // Both wire spellings arrive here: the camelCase IPC contract sends
    // `reasoningEffort`, while the request-shaped payload builders send the
    // canonical `reasoning_effort` next to `model`.
    #[serde(default, alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
    pub project_id: String,
    pub description: Option<String>,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub communication_style: Option<String>,
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    pub instructions: Option<String>,
    pub behavioral_contract: Option<BehavioralContract>,
    pub quality_gates: Option<Vec<String>>,
    pub handoff_expectations: Option<Vec<String>>,
    pub definition_of_done: Option<Vec<String>>,
    pub phase_scope: Option<Vec<String>>,
    pub mode: Option<String>,
    pub inherits_from: Option<String>,
    pub required_artifacts: Option<Vec<String>>,
    pub capabilities: Option<Vec<String>>,
}

/// IPC request for one-click team initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeTeamRequest {
    pub team_name: String,
    pub team_description: Option<String>,
    pub preset_id: Option<String>,
    pub lead_mode: LeadMode,
    pub lead: AgentSetupConfig,
    pub agents: Vec<AgentSetupConfig>,
}

/// IPC response for initialize operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeReport {
    pub team_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// IPC request for hot-adding one agent to a running team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAgentRequest {
    pub team_name: String,
    pub agent: AgentSetupConfig,
}

/// IPC request for resuming a team member session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeMemberRequest {
    pub team_name: String,
    pub member_name: String,
}

/// IPC request for resuming all persisted members in a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamRequest {
    pub team_name: String,
}

/// IPC response for hot-add operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAgentReport {
    pub team_name: String,
    pub member_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// IPC response for resume operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeAgentReport {
    pub team_name: String,
    pub member_name: String,
    pub resumed: bool,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
    pub warnings: Vec<String>,
    pub pane_id: Option<String>,
    pub reused_pane: bool,
}

/// One failed member entry inside a team resume response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamMemberFailure {
    pub member_name: String,
    pub message: String,
    pub retryable: bool,
}

/// IPC response for team resume aggregation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamReport {
    pub team_name: String,
    pub resumed: bool,
    pub total_members: usize,
    pub resumed_members: Vec<String>,
    pub failed_members: Vec<ResumeTeamMemberFailure>,
    pub warnings: Vec<String>,
    pub started_team_daemon: bool,
    pub team_daemon_warning: Option<String>,
}

/// IPC response for runtime member removal with teardown diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAgentReport {
    pub team_name: String,
    pub member_name: String,
    pub removed: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
    pub warnings: Vec<String>,
}

/// IPC request for re-sending onboarding to an existing team member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReonboardRequest {
    pub team_name: String,
    pub member_name: String,
}

/// Live-team row rendered in runtime roster mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAgentStatus {
    pub name: String,
    pub role: AgentRole,
    pub cli_tool: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub project_id: String,
    pub is_cross_project: bool,
    pub project_label: String,
    pub description: Option<String>,
    pub session_status: SessionStatus,
    pub pane_id: Option<String>,
    /// Claude session the member's runtime record is attached to, when it has
    /// one. Additive and defaulted: a member with no runtime attachment, and an
    /// older payload that predates the field, both decode as `None`.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Recent writes from live workflow subagents under that session.
    ///
    /// A headless workflow parent never reports busy, so coordination health is
    /// the wrong evidence for a member that is running one: without this the
    /// canvas node says Idle while its own run tree is visibly live. Same shape
    /// and same window as the session listing's hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
}

/// Live-team payload for the frontend mesh roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTeamStatus {
    pub team_name: String,
    pub lead_name: String,
    pub runtime_snapshot_freshness: LiveRuntimeSnapshotFreshness,
    pub members: Vec<LiveAgentStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveRuntimeSnapshotFreshness {
    Fresh,
    Cached,
    AttachmentsOnly,
}

/// Fast snapshot row returned without tmux/daemon reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastAgentSnapshot {
    pub name: String,
    pub role: AgentRole,
    pub cli_tool: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub project_id: String,
    pub is_cross_project: bool,
    pub project_label: String,
    pub description: Option<String>,
    pub session_status: SessionStatus,
    pub pane_id: Option<String>,
    /// See `LiveAgentStatus::session_id`.
    #[serde(default)]
    pub session_id: Option<String>,
    /// See `LiveAgentStatus::workflow_activity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
}

/// Fast team snapshot built from persisted config + runtime only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastTeamSnapshot {
    pub lead_name: String,
    pub members: Vec<FastAgentSnapshot>,
}

/// Cold-start classification for a discovered team's reconciled runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TeamRuntimeState {
    None,
    Active,
    Degraded,
    ColdResume,
}

/// One-shot mesh snapshot used to avoid frontend waterfall loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeshSnapshotResponse {
    pub mesh_available: bool,
    pub tmux_available: bool,
    pub team_runtime_state: TeamRuntimeState,
    pub team_name: Option<String>,
    pub team_status: Option<FastTeamSnapshot>,
    pub warnings: Vec<String>,
}

/// Streamed progress event payload emitted during long operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct StepProgressEvent {
    pub team_name: String,
    pub operation: String,
    pub progress: StepProgress,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canonical_stages: Vec<MemberActivationStage>,
}

/// Streamed team-resume progress event emitted for one member stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamProgressEvent {
    pub operation: String,
    pub team_name: String,
    pub member_name: String,
    pub member_index: usize,
    pub member_count: usize,
    pub stage: MemberActivationStage,
    pub status: StepStatus,
    pub message: Option<String>,
}

/// Agent-scoped warning from the environment preflight check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPreflightWarning {
    pub agent_name: String,
    pub cli_tool: String,
    pub message: String,
}

/// Initialization preflight report with blockers and per-agent warnings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub can_initialize: bool,
    pub blocking_errors: Vec<String>,
    pub agent_warnings: Vec<AgentPreflightWarning>,
}

/// Baseline feature availability for Mesh tab gating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureAvailabilityReport {
    pub can_initialize: bool,
    pub mesh_available: bool,
    pub tmux_available: bool,
    pub blocking_errors: Vec<String>,
}

#[cfg(test)]
#[path = "coordination_types/tests.rs"]
mod tests;
