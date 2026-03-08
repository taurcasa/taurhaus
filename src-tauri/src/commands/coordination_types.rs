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
    pub context_mode: ResumeContextMode,
}

/// IPC request for resuming all persisted members in a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamRequest {
    pub team_name: String,
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

/// One compaction reinjection audit row for debug/inspection surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionAuditEntry {
    pub member_name: String,
    pub tool: String,
    pub last_session_id: String,
    pub last_compaction_timestamp: String,
    pub last_delivery_result: String,
}

/// One tracked transcript file inside the compaction extractor state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionExtractorFileDiagnostics {
    pub jsonl_path: String,
    pub offset: u64,
    pub last_error: Option<String>,
}

/// Extractor-side state for Codex compaction transcript parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionExtractorDiagnostics {
    pub heartbeat_at: Option<String>,
    pub last_processed_signal_id: Option<String>,
    pub last_processed_jsonl_path: Option<String>,
    pub last_processed_jsonl_offset: Option<u64>,
    pub active_files: Vec<CompactionExtractorFileDiagnostics>,
}

/// One recent compaction signal from the canonical signal log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSignalAuditEntry {
    pub signal_id: String,
    pub emitted_at: String,
    pub session_id: String,
    pub pane_id: String,
    pub project_path: String,
    pub transcript_timestamp: String,
    pub signal_kind: String,
}

/// Signal-log state relative to the watcher offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSignalDiagnostics {
    pub signal_log_path: String,
    pub file_size_bytes: u64,
    pub total_signals: usize,
    pub last_consumed_offset: u64,
    pub unconsumed_count: usize,
    pub recent_signals: Vec<CompactionSignalAuditEntry>,
}

/// Watcher health and persisted offset state for signal consumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionWatcherDiagnostics {
    pub last_consumed_offset: u64,
    pub last_event_at: Option<String>,
    pub last_reconciliation_at: Option<String>,
    pub reconciliation_poll_count: u64,
    pub missed_event_recovery_count: u64,
}

/// Full runtime diagnostics for the compaction pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDiagnostics {
    pub extractor: CompactionExtractorDiagnostics,
    pub signal_log: CompactionSignalDiagnostics,
    pub watcher: CompactionWatcherDiagnostics,
}

/// Recent compaction reinjection audit rows for a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionAuditResponse {
    pub team_name: String,
    pub entries: Vec<CompactionAuditEntry>,
    pub diagnostics: CompactionDiagnostics,
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
