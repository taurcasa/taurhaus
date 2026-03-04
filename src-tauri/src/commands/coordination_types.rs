use serde::{Deserialize, Serialize};

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

/// Team-lead startup mode selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Intentionally snake_case: persisted/requested by established mesh coordination IPC payloads.
#[serde(rename_all = "snake_case")]
pub enum LeadMode {
    AttachExisting,
    LaunchNew,
}

/// Role descriptor shown in the live team roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Intentionally snake_case: runtime mesh status payload contract consumed by existing frontend code.
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Lead,
    Member,
}

/// Session runtime status for one roster member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Intentionally snake_case: runtime mesh status payload contract consumed by existing frontend code.
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Active,
    Idle,
    Offline,
}

/// Step status used by initialize/hot-add progress models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Intentionally snake_case: progress status values are part of the long-lived mesh event contract.
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
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

/// Per-step progress shape shared by reports and streamed events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepProgress {
    pub step: String,
    pub status: StepStatus,
    pub message: Option<String>,
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

/// Context mode for resume operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Intentionally snake_case: IPC request contract already published to clients.
#[serde(rename_all = "snake_case")]
pub enum ResumeContextMode {
    Continue,
    Fresh,
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
    pub project_id: String,
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
