use serde::{Deserialize, Serialize};

pub use crate::coordination::requests::{
    AgentRole, LeadMode, ResumeContextMode, SessionStatus, StepProgress, StepStatus,
};
use crate::templates::types::BehavioralContract;

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
    pub project_id: String,
    pub description: Option<String>,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub instructions: Option<String>,
    pub behavioral_contract: Option<BehavioralContract>,
    pub capabilities: Option<Vec<String>>,
}

/// IPC request for one-click team initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeTeamRequest {
    pub team_name: String,
    pub team_description: Option<String>,
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
    pub context_mode: ResumeContextMode,
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
}

/// Live-team payload for the frontend mesh roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTeamStatus {
    pub team_name: String,
    pub lead_name: String,
    pub members: Vec<LiveAgentStatus>,
}

/// Fast snapshot row returned without tmux/daemon reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastAgentSnapshot {
    pub name: String,
    pub role: AgentRole,
    pub cli_tool: String,
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
}

/// Fast team snapshot built from persisted config + runtime only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastTeamSnapshot {
    pub lead_name: String,
    pub members: Vec<FastAgentSnapshot>,
}

/// One-shot mesh snapshot used to avoid frontend waterfall loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeshSnapshotResponse {
    pub mesh_available: bool,
    pub tmux_available: bool,
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
