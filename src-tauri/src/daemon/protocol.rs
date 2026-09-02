/// Daemon protocol types for NDJSON communication over TCP.
///
/// Warning:
/// - `LIST_DISPLAY_SESSIONS` is the UI-safe session view and strips transcript
///   metadata.
/// - `LIST_RUNTIME_SESSIONS` preserves transcript metadata and must be used for
///   coordination/runtime correlation.
///
/// Request/response pairs are matched by `id`. Events are push-only
/// messages from daemon to client, distinguished by having an `event`
/// field instead of `id`.
use serde::{Deserialize, Serialize};

/// Protocol version — bump this whenever the daemon API changes in a way
/// that requires the app to be rebuilt against the new daemon.
///
/// The app checks this on connect. If the daemon's protocol version is
/// lower than what the app expects, it warns the user to rebuild the daemon.
/// v8: tmux focus became a field of the hub's versioned snapshot
/// (`wait_session_updates.focus`), the only live focus transport. A v7 daemon
/// omits it and would leave the app with a permanently dark indicator.
/// v9: the app explicitly selects the daemon's Codex compaction mode instead of
/// making the daemon guess the desktop settings database path.
/// v10: a scanner blackout got its own cursor — the app sends
/// `since_degraded_revision` and the daemon answers `degraded` /
/// `degraded_revision`. The gate has to refuse both mixed pairs: a v9 app never
/// sends the cursor, so its long poll returns immediately forever once a
/// blackout has happened, and a v9 daemon never sends the flags, so a v10 app
/// would read every replayed snapshot as a live observation.
/// v11: account discovery and transcript lookup became tool-parameterised;
/// app and daemon ship together in 0.6.9 and must use the same wire names.
/// v12: the third harness wire vocabulary changed from the retired Google CLI
/// value to Antigravity CLI. Mixed v11 pairs cannot decode each other's tool.
/// v13: added the Grok CLI tool value to the shared wire vocabulary.
/// v14: retired the Codex compaction mode method with the transcript pipeline.
/// v15: moved the managed-task deadline pass from the app into the daemon.
/// v16: moved team initialization from the app into the daemon.
/// v17: moved add-agent, resume-member, and stop-member into the daemon.
/// v18: moved resume-team and reonboard into the daemon.
/// v19: moved standalone team create/disband and roster edits into the daemon.
/// v20: retired the superseded stop-member wire methods.
/// v21: moved self-heal and effort background passes into the daemon and added
/// the task-arrival effort intent.
/// v22: moved the final desktop-owned team-state writes (task snapshots,
/// live-presence reconciliation, and active-project mappings) into the daemon.
pub const PROTOCOL_VERSION: u32 = 22;

// ---------------------------------------------------------------------------
// Envelope types (wire format)
// ---------------------------------------------------------------------------

/// A request sent from the Windows app to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    /// Auth token for daemon authentication. Added in protocol v4.
    /// Old clients without auth will send None (backward compatible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

/// A response sent from the daemon to the Windows app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DaemonError>,
}

/// An error payload inside a DaemonResponse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonError {
    pub code: String,
    pub message: String,
}

/// A push event from the daemon (no request ID, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonEvent {
    pub event: String,
    pub data: serde_json::Value,
}

/// Any message that can arrive on the TCP stream from the daemon.
/// We deserialize into this to distinguish responses from events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DaemonMessage {
    Response(DaemonResponse),
    Event(DaemonEvent),
}

// ---------------------------------------------------------------------------
// Method constants
// ---------------------------------------------------------------------------

pub mod method {
    pub const PING: &str = "ping";
    pub const GIT_STATUS: &str = "git_status";
    pub const GIT_LOG: &str = "git_log";
    pub const GIT_LATEST_COMMIT_TIME: &str = "git_latest_commit_time";
    pub const FILE_TREE: &str = "file_tree";
    pub const READ_FILE: &str = "read_file";
    pub const READ_README: &str = "read_readme";
    pub const READ_ASSET: &str = "read_asset";
    pub const LIST_DIRECTORY: &str = "list_directory";
    pub const SCAN_SESSIONS: &str = "scan_sessions";
    pub const LIST_RUNTIME_SESSIONS: &str = "list_runtime_sessions";
    pub const WATCH: &str = "watch";
    pub const UNWATCH: &str = "unwatch";
    pub const SHUTDOWN: &str = "shutdown";
    pub const LIST_ACCOUNTS: &str = "list_accounts";
    pub const PROJECT_TRANSCRIPT: &str = "project_transcript";
    pub const RESOLVE_LAUNCH_BASE: &str = "resolve_launch_base";
    pub const REFRESH_USAGE: &str = "refresh_usage";
    pub const LIST_WORKFLOW_RUNS: &str = "list_workflow_runs";
    pub const GET_WORKFLOW_RUN: &str = "get_workflow_run";
    pub const COORDINATION_INITIALIZE_TEAM: &str = "coordination.initialize_team";
    pub const COORDINATION_INITIALIZE_STATUS: &str = "coordination.initialize_status";
    pub const COORDINATION_ADD_AGENT: &str = "coordination.add_agent";
    pub const COORDINATION_ADD_AGENT_STATUS: &str = "coordination.add_agent_status";
    pub const COORDINATION_RESUME_MEMBER: &str = "coordination.resume_member";
    pub const COORDINATION_RESUME_MEMBER_STATUS: &str = "coordination.resume_member_status";
    pub const COORDINATION_RESUME_TEAM: &str = "coordination.resume_team";
    pub const COORDINATION_RESUME_TEAM_STATUS: &str = "coordination.resume_team_status";
    pub const COORDINATION_REONBOARD: &str = "coordination.reonboard";
    pub const COORDINATION_REONBOARD_STATUS: &str = "coordination.reonboard_status";
    pub const COORDINATION_CREATE_TEAM: &str = "coordination.create_team";
    pub const COORDINATION_CREATE_TEAM_STATUS: &str = "coordination.create_team_status";
    pub const COORDINATION_DISBAND_TEAM: &str = "coordination.disband_team";
    pub const COORDINATION_DISBAND_TEAM_STATUS: &str = "coordination.disband_team_status";
    pub const COORDINATION_ADD_MEMBER: &str = "coordination.add_member";
    pub const COORDINATION_ADD_MEMBER_STATUS: &str = "coordination.add_member_status";
    pub const COORDINATION_REMOVE_MEMBER: &str = "coordination.remove_member";
    pub const COORDINATION_REMOVE_MEMBER_STATUS: &str = "coordination.remove_member_status";
    pub const COORDINATION_PUT_LAUNCH_SETTINGS: &str = "coordination.put_launch_settings";
    pub const COORDINATION_APPLY_TASK_EFFORT: &str = "coordination.apply_task_effort";
    pub const COORDINATION_APPLY_TASK_EFFORT_STATUS: &str = "coordination.apply_task_effort_status";
    pub const COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS: &str =
        "coordination.publish_operational_snapshots";
    pub const COORDINATION_RECONCILE_LIVE_PRESENCE: &str = "coordination.reconcile_live_presence";
    pub const COORDINATION_SET_ACTIVE_PROJECT_TEAM: &str = "coordination.set_active_project_team";

    // Command Center — session management
    pub const LIST_DISPLAY_SESSIONS: &str = "list_display_sessions";
    pub const GET_RUNTIME_SESSION_SNAPSHOT: &str = "get_runtime_session_snapshot";
    pub const WAIT_SESSION_UPDATES: &str = "wait_session_updates";
    pub const LAUNCH_SESSION: &str = "launch_session";
    pub const STOP_SESSION: &str = "stop_session";
    pub const NAVIGATE_TO_SESSION: &str = "navigate_to_session";

    // Task scanner
    pub const GET_PROJECT_TASKS: &str = "get_project_tasks";

    // Git range queries (for archived session enrichment)
    pub const GIT_COMMITS_IN_RANGE: &str = "git_commits_in_range";

    // Per-commit file changes (for Git tab detail view)
    pub const GIT_COMMIT_FILES: &str = "git_commit_files";

    // Per-file diff within a commit (for inline diff view)
    pub const GIT_COMMIT_DIFF: &str = "git_commit_diff";
}

pub mod event {
    pub const FILE_CHANGED: &str = "file_changed";
    pub const GIT_CHANGED: &str = "git_changed";
    pub const SESSION_FILE_CREATED: &str = "session_file_created";
}

// ---------------------------------------------------------------------------
// Method-specific param/result types
// ---------------------------------------------------------------------------

/// `list_accounts` — tool accounts the daemon's host can see.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListAccountsParams {
    pub tool: crate::session_scanner::cli_tool::CliTool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountsResult {
    pub accounts: Vec<crate::session_scanner::accounts::Account>,
    pub degraded: bool,
    pub error: Option<String>,
}

/// `project_transcript` — which account owns a project's history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectTranscriptParams {
    pub tool: crate::session_scanner::cli_tool::CliTool,
    pub project: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectTranscriptResult {
    pub transcript: Option<String>,
}

/// `resolve_launch_base` — what the daemon host's pane shell makes of a
/// configured base command. Additive: a daemon without it answers
/// `UNKNOWN_METHOD` and the app keeps reading the base literally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveLaunchBaseParams {
    pub tool: crate::session_scanner::cli_tool::CliTool,
    pub base: String,
    #[serde(default)]
    pub force: bool,
}

/// Self-contained team-initialization intent. The daemon derives host-local
/// account selectors and launch-base resolutions before running the pipeline.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationInitializeParams {
    pub request: crate::coordination::requests::InitializeTeamRequest,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
    #[serde(default)]
    pub operational_snapshots: Vec<crate::coordination::stores::OperationalContextSnapshot>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationInitializeAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationInitializeStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationInitializeOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::InitializeReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationInitializeStatus {
    pub run_id: String,
    pub steps: Vec<crate::coordination::requests::StepProgress>,
    pub outcome: CoordinationInitializeOutcome,
}

/// Self-contained hot-add intent. The daemon derives host-local account
/// selectors and launch-base resolutions before running the pipeline.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddAgentParams {
    pub request: crate::coordination::requests::AddAgentRequest,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
    #[serde(default)]
    pub operational_snapshot: Option<crate::coordination::stores::OperationalContextSnapshot>,
    #[serde(default)]
    pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddAgentAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddAgentStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationAddAgentOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::AddAgentReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddAgentStatus {
    pub run_id: String,
    pub steps: Vec<crate::coordination::requests::StepProgress>,
    pub outcome: CoordinationAddAgentOutcome,
}

/// Self-contained member-resume intent. The daemon resolves the persisted
/// member's tool before applying host-local launch settings.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeMemberParams {
    pub request: crate::coordination::requests::ResumeMemberRequest,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
    #[serde(default)]
    pub operational_snapshot: Option<crate::coordination::stores::OperationalContextSnapshot>,
    #[serde(default)]
    pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeMemberAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeMemberStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationResumeMemberOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::ResumeAgentReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeMemberStatus {
    pub run_id: String,
    pub steps: Vec<crate::coordination::requests::StepProgress>,
    pub outcome: CoordinationResumeMemberOutcome,
}

/// Self-contained team-resume intent. The daemon derives host-local launch
/// settings for every persisted member before executing the shared activation path.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeTeamParams {
    pub request: crate::coordination::requests::ResumeTeamRequest,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeTeamAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeTeamStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationResumeTeamOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::ResumeTeamReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationResumeTeamStatus {
    pub run_id: String,
    pub steps: Vec<crate::coordination::requests::ResumeTeamProgress>,
    pub outcome: CoordinationResumeTeamOutcome,
}

/// Self-contained reonboard intent. Launch settings travel with every
/// interactive coordination request even though this delivery-only pipeline
/// does not consume them today.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationReonboardParams {
    pub request: crate::coordination::requests::ReonboardRequest,
    // Carried for intent-shape uniformity across the coordination methods;
    // the reonboard worker renders from the saved member and does not read
    // these two today.
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
    #[serde(default)]
    pub operational_snapshot: Option<crate::coordination::stores::OperationalContextSnapshot>,
    #[serde(default)]
    pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationReonboardAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationReonboardStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationReonboardOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::DeliveryResult,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationReonboardStatus {
    pub run_id: String,
    pub outcome: CoordinationReonboardOutcome,
}

/// Self-contained standalone team-create intent.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationCreateTeamParams {
    pub request: crate::coordination::requests::CreateTeamRequest,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationCreateTeamAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationCreateTeamStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationCreateTeamOutcome {
    Running,
    Completed,
    Failed { error: String },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationCreateTeamStatus {
    pub run_id: String,
    pub outcome: CoordinationCreateTeamOutcome,
}

/// Self-contained team-disband intent. The daemon owns teardown, config
/// deletion, and active-project cleanup as one retained run.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationDisbandTeamParams {
    pub request: crate::coordination::requests::DisbandTeamRequest,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationDisbandTeamAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationDisbandTeamStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationDisbandTeamOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::DisbandTeamReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationDisbandTeamStatus {
    pub run_id: String,
    pub outcome: CoordinationDisbandTeamOutcome,
}

/// Self-contained config-only roster-add intent.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddMemberParams {
    pub request: crate::coordination::requests::AddMemberRequest,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddMemberAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddMemberStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationAddMemberOutcome {
    Running,
    Completed,
    Failed { error: String },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationAddMemberStatus {
    pub run_id: String,
    pub outcome: CoordinationAddMemberOutcome,
}

/// Self-contained roster-removal intent, distinct from activation-class stop.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationRemoveMemberParams {
    pub request: crate::coordination::requests::RemoveMemberRequest,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationRemoveMemberAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationRemoveMemberStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationRemoveMemberOutcome {
    Running,
    Completed {
        report: crate::coordination::requests::StopMemberReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationRemoveMemberStatus {
    pub run_id: String,
    pub outcome: CoordinationRemoveMemberOutcome,
}

/// Latest app-committed launch settings used only by the daemon retry sweep.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationPutLaunchSettingsParams {
    /// The app's settings-save counter; the daemon keeps the highest version
    /// it has seen. Monotonicity is global by design: this architecture runs
    /// one daemon per data dir and port (E2E workers get private ports), so a
    /// second app instance with an older counter is not a supported topology
    /// — documented rather than defended with per-client state.
    pub version: u64,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationPutLaunchSettingsResult {
    pub accepted: bool,
    pub version: u64,
}

/// Self-contained task-arrival intent. Host-local launch inputs are derived by
/// the daemon immediately before a member is relaunched.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationApplyTaskEffortParams {
    pub project_path: String,
    pub cli_commands: crate::models::CliCommandSettings,
    pub tmux_layout: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationApplyTaskEffortAccepted {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationApplyTaskEffortStatusParams {
    pub run_id: String,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationApplyTaskEffortReport {
    pub switched: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub skipped_teams: Vec<(String, String)>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CoordinationApplyTaskEffortOutcome {
    Running,
    Completed {
        report: CoordinationApplyTaskEffortReport,
    },
    Failed {
        error: String,
    },
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationApplyTaskEffortStatus {
    pub run_id: String,
    pub outcome: CoordinationApplyTaskEffortOutcome,
}

/// One app-DB-derived operational snapshot publication. The task timestamp is
/// carried separately so the daemon can preserve deadline marker semantics.
#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationOperationalSnapshotPublication {
    pub snapshot: crate::coordination::stores::OperationalContextSnapshot,
    #[serde(default)]
    pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationPublishOperationalSnapshotsParams {
    pub publications: Vec<CoordinationOperationalSnapshotPublication>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationPublishOperationalSnapshotsResult {
    pub published: usize,
    pub skipped: usize,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoordinationReconcileLivePresenceParams {
    pub team_name: String,
    #[serde(default)]
    pub runtime_sessions: Vec<crate::session_scanner::RuntimeSession>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationReconcileLivePresenceOutcome {
    Reconciled,
    Skipped,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationReconcileLivePresenceResult {
    pub outcome: CoordinationReconcileLivePresenceOutcome,
    pub reconciled_offline_members: Vec<String>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationSetActiveProjectTeamParams {
    pub project_path: String,
    #[serde(default)]
    pub team_name: Option<String>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinationSetActiveProjectTeamResult {}

/// `list_workflow_runs` — completed and live runs under one Claude session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSessionParams {
    pub session_id: String,
}

/// `get_workflow_run` — one full run including agents and result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowRunParams {
    pub session_id: String,
    pub run_id: String,
}

/// `ping` — health check
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingResult {
    pub version: String,
    /// Protocol version for compatibility checking. Old daemons that don't
    /// include this field will deserialize as 0 (the default).
    #[serde(default)]
    pub protocol_version: u32,
    pub uptime_secs: u64,
    /// Canonical app-data root used by the daemon. Additive for older daemons.
    #[serde(default)]
    pub data_root: String,
}

/// `git_status` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathParams {
    pub path: String,
}

/// `get_project_tasks` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectTasksParams {
    pub path: String,
    /// Optional scan cycle identifier for per-cycle daemon cache reuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_cycle_id: Option<u64>,
}

/// `git_log` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitLogParams {
    pub path: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

/// `git_commits_in_range` params — time-bounded commit query
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitsInRangeParams {
    pub path: String,
    pub after: String,  // RFC 3339 timestamp
    pub before: String, // RFC 3339 timestamp
    /// Optional cap for number of commits included in the response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_limit: Option<usize>,
}

/// `git_commits_in_range` result — commits + file paths
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitsInRangeResult {
    pub commits: Vec<crate::models::Commit>,
    pub files: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
}

/// `git_commit_files` params — get files changed by a specific commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitFilesParams {
    pub path: String,
    pub hash: String,
}

/// `git_commit_files` result — list of changed files with status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitFilesResult {
    pub files: Vec<crate::models::CommitFile>,
}

/// `git_commit_diff` params — get diff hunks for a specific file in a commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitDiffParams {
    pub path: String,
    pub hash: String,
    pub file_path: String,
}

/// `git_commit_diff` result — diff hunks with line detail
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitCommitDiffResult {
    pub hunks: Vec<crate::models::DiffHunk>,
}

/// `read_file` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadFileParams {
    pub path: String,
    pub relative: String,
}

/// `read_asset` result — binary data as base64
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadAssetResult {
    pub data: String, // base64-encoded
}

/// `scan_sessions` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanSessionsResult {
    pub paths: Vec<String>,
}

/// `watch`/`unwatch` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchResult {
    pub ok: bool,
}

/// `git_latest_commit_time` result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LatestCommitTimeResult {
    pub timestamp: Option<String>, // RFC 3339 or null
}

// ---------------------------------------------------------------------------
// Command Center — session management types
// ---------------------------------------------------------------------------

/// Launch mode for a new Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    /// `claude --dangerously-skip-permissions --continue`
    Continue,
    /// `claude --dangerously-skip-permissions`
    Fresh,
    /// `claude --dangerously-skip-permissions --resume`
    Resume,
}

/// `launch_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchSessionParams {
    pub project_path: String,
    pub mode: LaunchMode,
    /// Which CLI tool to launch. Defaults to Claude for backward compatibility.
    #[serde(default = "default_cli_tool")]
    pub cli_tool: crate::session_scanner::cli_tool::CliTool,
    /// Tmux layout strategy: "new_window", "split", "per_project".
    /// Defaults to "new_window" for backward compatibility.
    #[serde(default = "default_tmux_layout")]
    pub tmux_layout: String,
    /// Custom command to execute instead of the default for this tool/mode.
    /// Resolved from user settings on the app side. The daemon just executes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_override: Option<String>,
}

fn default_tmux_layout() -> String {
    "new_window".to_string()
}

fn default_cli_tool() -> crate::session_scanner::cli_tool::CliTool {
    crate::session_scanner::cli_tool::CliTool::default()
}

/// `launch_session` result
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LaunchSessionResult {
    /// Which tmux session the window was created in. Optional for backward
    /// compat with older daemons that don't send this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    pub tmux_window: String,
    pub tmux_pane: String,
    /// Whether the account this launch was asked to run on was applied.
    /// `None` when nothing asked for one; `Some(false)` when something else
    /// decided the config dir and the request could not be honoured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_applied: Option<bool>,
    /// Why `account_applied` is false, as a stable token the frontend matches
    /// on rather than a sentence it would have to parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_note: Option<String>,
    /// The one detail a note needs to name something the user wrote — the head
    /// of an opaque base command. Set by the app, never by the daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_note_detail: Option<String>,
}

/// `stop_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StopSessionParams {
    pub tmux_pane: String,
    /// Which CLI tool is running. Defaults to Claude for backward compatibility.
    #[serde(default = "default_cli_tool")]
    pub cli_tool: crate::session_scanner::cli_tool::CliTool,
}

/// `navigate_to_session` params
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigateToSessionParams {
    pub tmux_session: String,
    pub tmux_window: String,
    pub tmux_pane: String,
}

/// `wait_session_updates` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitSessionUpdatesParams {
    /// Client's last seen session snapshot version.
    #[serde(default)]
    pub since_version: u64,
    /// Client's last seen degradation revision. The hub bumps it on every
    /// scanner blackout edge without touching the version, so this is what
    /// wakes the long poll for a blackout. Additive: older clients omit it and
    /// the daemon answers exactly as it did before.
    #[serde(default)]
    pub since_degraded_revision: u64,
    /// Max time to wait for a newer snapshot. Clamped server-side.
    #[serde(default = "default_wait_session_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_wait_session_timeout_ms() -> u64 {
    15_000
}

/// `wait_session_updates` result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaitSessionUpdatesResult {
    /// Monotonic daemon-local version of the session snapshot.
    pub version: u64,
    /// Whether this response contains a version newer than `since_version`.
    pub changed: bool,
    /// Full session snapshot for the reported version.
    pub sessions: Vec<crate::session_scanner::DisplaySession>,
    /// Account bindings observed beside the session snapshot. The daemon owns
    /// process inspection; only the app persists these through its DbState.
    #[serde(default)]
    pub account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    /// tmux focus as of this version. Additive: older daemons omit it.
    #[serde(default)]
    pub focus: Option<crate::session_scanner::tmux::TmuxFocus>,
    /// Project path the focused tmux window belongs to, resolved by the hub.
    #[serde(default)]
    pub focus_project_path: Option<String>,
    /// The daemon scanner's latest cycle could not read its process inventory:
    /// `sessions` is the hub's last good snapshot, replayed for continuity, and
    /// the app must present it as unobserved rather than as the current truth.
    /// Additive: older daemons omit the field and decode as `false`.
    #[serde(default)]
    pub degraded: bool,
    /// The hub's degradation revision as of this answer: one bump per blackout
    /// edge. A client whose cursor is behind it spanned an interval the scanner
    /// did not observe, even when `degraded` is false because the blackout
    /// already ended. Additive: older daemons omit it and decode as `0`, which
    /// never advances and so never claims a gap.
    #[serde(default)]
    pub degraded_revision: u64,
}

/// `get_runtime_session_snapshot` result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeSessionSnapshotResult {
    pub version: u64,
    pub display_sessions: Vec<crate::session_scanner::DisplaySession>,
    pub runtime_sessions: Vec<crate::session_scanner::RuntimeSession>,
    #[serde(default)]
    pub account_observations: Vec<crate::session_scanner::accounts::LiveAccountObservation>,
    /// tmux focus owned by the daemon hub. Serializes with the legacy
    /// `session`/`window` keys so an older app still decodes it.
    #[serde(default)]
    pub focus: Option<crate::session_scanner::tmux::TmuxFocus>,
    /// Legacy wire name for the hub's `focus_project_path`.
    pub foreground_project_path: Option<String>,
    /// The daemon scanner's latest cycle could not read its process inventory:
    /// the sessions are the hub's last good snapshot, not an observation, and
    /// must not bind identities or promote activity. Additive: older daemons
    /// omit the field and decode as `false` (their behavior so far).
    #[serde(default)]
    pub degraded: bool,
    /// The hub's blackout-edge counter as of this snapshot. The bridge adopts it
    /// as its cursor when it seeds, so the long poll that follows reports only
    /// blackouts from here on. Additive: older daemons omit it and decode as 0.
    #[serde(default)]
    pub degraded_revision: u64,
}

// ---------------------------------------------------------------------------
// Event data types
// ---------------------------------------------------------------------------

/// Data for `file_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileChangedData {
    pub path: String,
    pub files: Vec<String>,
}

/// Data for `git_changed` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitChangedData {
    pub path: String,
}

/// Data for `session_file_created` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionFileCreatedData {
    pub path: String,
    pub file: String,
}

// ---------------------------------------------------------------------------
// Helper constructors
// ---------------------------------------------------------------------------

impl DaemonRequest {
    pub fn new(id: impl Into<String>, method: &str, params: impl Serialize) -> Self {
        Self {
            id: id.into(),
            method: method.to_string(),
            params: serde_json::to_value(params).unwrap_or(serde_json::Value::Null),
            auth: None,
        }
    }

    /// Create a request with an auth token attached.
    pub fn with_auth(mut self, token: Option<String>) -> Self {
        self.auth = token;
        self
    }

    pub fn ping(id: impl Into<String>) -> Self {
        Self::new(id, method::PING, serde_json::Value::Null)
    }
}

impl DaemonResponse {
    pub fn ok(id: impl Into<String>, result: impl Serialize) -> Self {
        Self {
            id: id.into(),
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    pub fn err(id: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(DaemonError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

impl DaemonEvent {
    pub fn new(event: &str, data: impl Serialize) -> Self {
        Self {
            event: event.to_string(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Commit, FileTreeNode, GitStatus};
    use serde_json::json;

    #[test]
    fn request_serialization_roundtrip() {
        let req = DaemonRequest::new(
            "r1",
            method::GIT_STATUS,
            PathParams {
                path: "/home/user/projects/foo".to_string(),
            },
        );
        let json = serde_json::to_string(&req).unwrap();
        let back: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn request_serializes_to_expected_json() {
        let req = DaemonRequest::new(
            "r1",
            method::GIT_STATUS,
            PathParams {
                path: "/home/user/foo".to_string(),
            },
        );
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["id"], "r1");
        assert_eq!(v["method"], "git_status");
        assert_eq!(v["params"]["path"], "/home/user/foo");
    }

    #[test]
    fn response_ok_roundtrip() {
        let resp = DaemonResponse::ok(
            "r1",
            GitStatus {
                branch: Some("main".to_string()),
                is_dirty: false,
                ahead: 0,
                behind: 0,
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        let back: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(back.is_ok());
    }

    #[test]
    fn response_error_roundtrip() {
        let resp = DaemonResponse::err("r1", "NOT_FOUND", "Path does not exist");
        let json = serde_json::to_string(&resp).unwrap();
        let back: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(!back.is_ok());
    }

    #[test]
    fn response_ok_omits_error_field() {
        let resp = DaemonResponse::ok("r1", json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn response_error_omits_result_field() {
        let resp = DaemonResponse::err("r1", "ERR", "oops");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn event_roundtrip() {
        let evt = DaemonEvent::new(
            event::GIT_CHANGED,
            GitChangedData {
                path: "/home/user/foo".to_string(),
            },
        );
        let json = serde_json::to_string(&evt).unwrap();
        let back: DaemonEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, back);
    }

    #[test]
    fn daemon_message_distinguishes_response_from_event() {
        let resp_json = r#"{"id": "r1", "result": {"ok": true}}"#;
        let msg: DaemonMessage = serde_json::from_str(resp_json).unwrap();
        assert!(matches!(msg, DaemonMessage::Response(_)));

        let evt_json = r#"{"event": "git_changed", "data": {"path": "/foo"}}"#;
        let msg: DaemonMessage = serde_json::from_str(evt_json).unwrap();
        assert!(matches!(msg, DaemonMessage::Event(_)));
    }

    #[test]
    fn ping_result_roundtrip() {
        // Regression: commits a53ad31 (removal added) and f9c1e89 (None => remove-all)
        // exposed that daemon pings did not identify their data root authority.
        let ping = PingResult {
            version: "0.1.0".to_string(),
            protocol_version: PROTOCOL_VERSION,
            uptime_secs: 120,
            data_root: "/tmp/taurhaus-data".to_string(),
        };
        let json = serde_json::to_string(&ping).unwrap();
        let roundtrip: PingResult = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip, ping);
    }

    #[test]
    fn ping_result_old_daemon_without_protocol_version() {
        // Old daemons won't include protocol_version — should default to 0
        let json = r#"{"version":"0.1.0","uptime_secs":60}"#;
        let r: PingResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.protocol_version, 0);
        assert!(r.data_root.is_empty());
    }

    #[test]
    fn git_log_params_defaults() {
        let json = r#"{"path": "/foo"}"#;
        let params: GitLogParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 50);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn git_log_params_explicit() {
        let json = r#"{"path": "/foo", "limit": 10, "offset": 5}"#;
        let params: GitLogParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 10);
        assert_eq!(params.offset, 5);
    }

    #[test]
    fn read_file_params_roundtrip() {
        let p = ReadFileParams {
            path: "/home/user/foo".to_string(),
            relative: "src/main.rs".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ReadFileParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn response_result_can_hold_git_status() {
        let status = GitStatus {
            branch: Some("main".to_string()),
            is_dirty: true,
            ahead: 2,
            behind: 0,
        };
        let resp = DaemonResponse::ok("r1", &status);
        let result = resp.result.unwrap();
        let back: GitStatus = serde_json::from_value(result).unwrap();
        assert_eq!(back, status);
    }

    #[test]
    fn response_result_can_hold_commits() {
        let commits = vec![Commit {
            hash: "abc12345".into(),
            message: "Initial".into(),
            body: None,
            author: "Me".into(),
            date: "2h".into(),
            timestamp: 1740000000,
        }];
        let resp = DaemonResponse::ok("r1", &commits);
        let result = resp.result.unwrap();
        let back: Vec<Commit> = serde_json::from_value(result).unwrap();
        assert_eq!(back, commits);
    }

    #[test]
    fn response_result_can_hold_file_tree() {
        let tree = vec![FileTreeNode {
            name: "src".into(),
            path: "src".into(),
            is_dir: true,
            children: vec![],
        }];
        let resp = DaemonResponse::ok("r1", &tree);
        let result = resp.result.unwrap();
        let back: Vec<FileTreeNode> = serde_json::from_value(result).unwrap();
        assert_eq!(back, tree);
    }

    #[test]
    fn file_changed_event_data_roundtrip() {
        let data = FileChangedData {
            path: "/home/user/foo".to_string(),
            files: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        };
        let evt = DaemonEvent::new(event::FILE_CHANGED, &data);
        let back: FileChangedData = serde_json::from_value(evt.data).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn session_file_created_event_data_roundtrip() {
        let data = SessionFileCreatedData {
            path: "/home/user/foo".to_string(),
            file: ".claude/handoffs/2025-01-15-session.md".to_string(),
        };
        let evt = DaemonEvent::new(event::SESSION_FILE_CREATED, &data);
        let back: SessionFileCreatedData = serde_json::from_value(evt.data).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn ping_request_has_no_params() {
        let req = DaemonRequest::ping("r1");
        assert_eq!(req.method, "ping");
        assert!(req.params.is_null());
    }

    #[test]
    fn latest_commit_time_result_with_value() {
        let r = LatestCommitTimeResult {
            timestamp: Some("2025-06-15T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: LatestCommitTimeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn latest_commit_time_result_null() {
        let r = LatestCommitTimeResult { timestamp: None };
        let json = serde_json::to_string(&r).unwrap();
        let back: LatestCommitTimeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn scan_sessions_result_roundtrip() {
        let r = ScanSessionsResult {
            paths: vec![
                "/foo/.claude/handoffs/a.md".into(),
                "/foo/.claude/handoffs/b.md".into(),
            ],
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: ScanSessionsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn watch_result_roundtrip() {
        let r = WatchResult { ok: true };
        let json = serde_json::to_string(&r).unwrap();
        let back: WatchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn launch_session_params_roundtrip() {
        let p = LaunchSessionParams {
            project_path: "/home/user/proj".to_string(),
            mode: LaunchMode::Continue,
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
            tmux_layout: "new_window".to_string(),
            command_override: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: LaunchSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn launch_session_params_with_command_override() {
        let p = LaunchSessionParams {
            project_path: "/proj".to_string(),
            mode: LaunchMode::Fresh,
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
            tmux_layout: "new_window".to_string(),
            command_override: Some("my-custom-claude --flag".to_string()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("command_override"));
        let back: LaunchSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.command_override,
            Some("my-custom-claude --flag".to_string())
        );
    }

    #[test]
    fn launch_session_params_defaults_to_claude() {
        // Old daemon protocol without cli_tool field should default to Claude
        let json = r#"{"project_path":"/proj","mode":"fresh"}"#;
        let p: LaunchSessionParams = serde_json::from_str(json).unwrap();
        assert_eq!(
            p.cli_tool,
            crate::session_scanner::cli_tool::CliTool::Claude
        );
        assert_eq!(p.command_override, None);
    }

    #[test]
    fn launch_session_result_roundtrip() {
        let r = LaunchSessionResult {
            tmux_session: Some("0".to_string()),
            tmux_window: "proj".to_string(),
            tmux_pane: "%5".to_string(),
            account_applied: Some(false),
            account_note: Some("team_default".to_string()),
            account_note_detail: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: LaunchSessionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn launch_session_result_backward_compat() {
        // Old daemons don't send tmux_session — should deserialize with None
        let json = r#"{"tmux_window":"proj","tmux_pane":"%5"}"#;
        let r: LaunchSessionResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.tmux_session, None);
        assert_eq!(r.tmux_window, "proj");
    }

    /// Additive: an app that speaks this method still pairs with a daemon that
    /// does not, so PROTOCOL_VERSION does not move for it.
    #[test]
    fn resolve_launch_base_roundtrips_without_a_protocol_bump() {
        let params = ResolveLaunchBaseParams {
            tool: crate::session_scanner::cli_tool::CliTool::Claude,
            base: "claude2 --dangerously-skip-permissions".to_string(),
            force: false,
        };
        let back: ResolveLaunchBaseParams =
            serde_json::from_str(&serde_json::to_string(&params).unwrap()).unwrap();
        assert_eq!(params, back);

        let result = crate::session_scanner::launch_base::ResolvedBase {
            command: "CLAUDE_CONFIG_DIR=~/.claude-account2 claude".to_string(),
            expansions: vec![crate::session_scanner::launch_base::AliasExpansion {
                name: "claude2".to_string(),
                body: "CLAUDE_CONFIG_DIR=~/.claude-account2 claude".to_string(),
            }],
            opaque_head: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"opaqueHead\""), "{json}");
        let back: crate::session_scanner::launch_base::ResolvedBase =
            serde_json::from_str(&json).unwrap();
        assert_eq!(result, back);
    }

    // The one equality pin: bumping the wire contract must be a deliberate
    // edit here, in ARCHITECTURE.md, and in docs/architecture/daemon-protocol.md.
    #[test]
    fn protocol_version_is_pinned() {
        assert_eq!(PROTOCOL_VERSION, 22);
    }

    #[test]
    fn protocol_22_pins_the_last_team_state_writer_intents() {
        assert_eq!(
            method::COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS,
            "coordination.publish_operational_snapshots"
        );
        assert_eq!(
            method::COORDINATION_RECONCILE_LIVE_PRESENCE,
            "coordination.reconcile_live_presence"
        );
        assert_eq!(
            method::COORDINATION_SET_ACTIVE_PROJECT_TEAM,
            "coordination.set_active_project_team"
        );

        let snapshots = CoordinationPublishOperationalSnapshotsParams {
            publications: Vec::new(),
        };
        let decoded: CoordinationPublishOperationalSnapshotsParams =
            serde_json::from_value(serde_json::to_value(&snapshots).unwrap()).unwrap();
        assert_eq!(decoded, snapshots);

        let presence = CoordinationReconcileLivePresenceParams {
            team_name: "architecture-final".to_string(),
            runtime_sessions: Vec::new(),
        };
        let decoded: CoordinationReconcileLivePresenceParams =
            serde_json::from_value(serde_json::to_value(&presence).unwrap()).unwrap();
        assert_eq!(decoded, presence);

        let mapping = CoordinationSetActiveProjectTeamParams {
            project_path: "/work/taurhaus".to_string(),
            team_name: Some("architecture-final".to_string()),
        };
        let decoded: CoordinationSetActiveProjectTeamParams =
            serde_json::from_value(serde_json::to_value(&mapping).unwrap()).unwrap();
        assert_eq!(decoded, mapping);
    }

    #[test]
    fn coordination_background_effort_method_contracts_roundtrip() {
        let mut cli_commands = crate::models::CliCommandSettings::default();
        cli_commands.claude.resume = "claude2 --resume".to_string();
        let launch_settings = CoordinationPutLaunchSettingsParams {
            version: 7,
            cli_commands: cli_commands.clone(),
            tmux_layout: "split".to_string(),
        };
        let decoded: CoordinationPutLaunchSettingsParams = serde_json::from_value(
            serde_json::to_value(&launch_settings).expect("serialize launch settings"),
        )
        .expect("decode launch settings");
        assert_eq!(decoded, launch_settings);
        assert_eq!(decoded.cli_commands.claude.resume, "claude2 --resume");

        let apply = CoordinationApplyTaskEffortParams {
            project_path: "/tmp/protocol-21".to_string(),
            cli_commands,
            tmux_layout: "new_window".to_string(),
        };
        let decoded: CoordinationApplyTaskEffortParams = serde_json::from_value(
            serde_json::to_value(&apply).expect("serialize task-effort intent"),
        )
        .expect("decode task-effort intent");
        assert_eq!(decoded, apply);

        for method in [
            method::COORDINATION_PUT_LAUNCH_SETTINGS,
            method::COORDINATION_APPLY_TASK_EFFORT,
            method::COORDINATION_APPLY_TASK_EFFORT_STATUS,
        ] {
            assert!(method.starts_with("coordination."));
        }
    }

    // Regression: 3c5b6cd9 invalidated only the Windows app's process-local
    // cache. The additive force bit must also decode as false for an older app.
    #[test]
    fn resolve_launch_base_force_is_additive_and_defaults_off() {
        let params: ResolveLaunchBaseParams = serde_json::from_value(serde_json::json!({
            "tool": "claude",
            "base": "claude2 --fresh"
        }))
        .expect("old app payload");

        assert!(!params.force);
    }

    #[test]
    fn coordination_initialize_method_contract_roundtrips() {
        let params = CoordinationInitializeParams {
            request: crate::coordination::requests::InitializeTeamRequest {
                team_name: "daemon-init".to_string(),
                team_description: Some("Runs in the daemon".to_string()),
                lead_mode: crate::coordination::requests::LeadMode::LaunchNew,
                lead: crate::coordination::requests::AgentDefinition {
                    name: "lead".to_string(),
                    cli_tool: "claude".to_string(),
                    model: "sonnet".to_string(),
                    reasoning_effort: None,
                    project_id: "/tmp/daemon-init".to_string(),
                    description: None,
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
                },
                agents: Vec::new(),
            },
            cli_commands: crate::models::CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
            operational_snapshots: Vec::new(),
        };
        let decoded: CoordinationInitializeParams =
            serde_json::from_str(&serde_json::to_string(&params).unwrap()).unwrap();
        assert_eq!(decoded, params);
        assert_eq!(
            method::COORDINATION_INITIALIZE_TEAM,
            "coordination.initialize_team"
        );
        assert_eq!(
            method::COORDINATION_INITIALIZE_STATUS,
            "coordination.initialize_status"
        );
    }

    #[test]
    fn coordination_member_operation_contracts_roundtrip() {
        let agent = crate::coordination::requests::AgentDefinition {
            name: "builder".to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: Some("high".to_string()),
            project_id: "/tmp/builder".to_string(),
            description: None,
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
        };
        let add = CoordinationAddAgentParams {
            request: crate::coordination::requests::AddAgentRequest {
                team_name: "arch".to_string(),
                agent,
            },
            cli_commands: crate::models::CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
            operational_snapshot: None,
            task_state_changed_at: None,
        };
        let resume = CoordinationResumeMemberParams {
            request: crate::coordination::requests::ResumeMemberRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            cli_commands: crate::models::CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
            operational_snapshot: None,
            task_state_changed_at: None,
        };
        assert_eq!(
            serde_json::from_str::<CoordinationAddAgentParams>(
                &serde_json::to_string(&add).unwrap()
            )
            .unwrap(),
            add
        );
        assert_eq!(
            serde_json::from_str::<CoordinationResumeMemberParams>(
                &serde_json::to_string(&resume).unwrap()
            )
            .unwrap(),
            resume
        );
        assert_eq!(method::COORDINATION_ADD_AGENT, "coordination.add_agent");
        assert_eq!(
            method::COORDINATION_RESUME_MEMBER,
            "coordination.resume_member"
        );
    }

    #[test]
    fn coordination_team_operation_contracts_roundtrip() {
        let resume = CoordinationResumeTeamParams {
            request: crate::coordination::requests::ResumeTeamRequest {
                team_name: "arch".to_string(),
            },
            cli_commands: crate::models::CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
        };
        let reonboard = CoordinationReonboardParams {
            request: crate::coordination::requests::ReonboardRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
            },
            cli_commands: crate::models::CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
            operational_snapshot: None,
            task_state_changed_at: None,
        };

        assert_eq!(
            serde_json::from_str::<CoordinationResumeTeamParams>(
                &serde_json::to_string(&resume).unwrap()
            )
            .unwrap(),
            resume
        );
        assert_eq!(
            serde_json::from_str::<CoordinationReonboardParams>(
                &serde_json::to_string(&reonboard).unwrap()
            )
            .unwrap(),
            reonboard
        );
        assert_eq!(method::COORDINATION_RESUME_TEAM, "coordination.resume_team");
        assert_eq!(method::COORDINATION_REONBOARD, "coordination.reonboard");
    }

    #[test]
    fn coordination_roster_operation_contracts_roundtrip() {
        let create = CoordinationCreateTeamParams {
            request: crate::coordination::requests::CreateTeamRequest {
                team_name: "arch".to_string(),
            },
        };
        let disband = CoordinationDisbandTeamParams {
            request: crate::coordination::requests::DisbandTeamRequest {
                team_name: "arch".to_string(),
            },
        };
        let add = CoordinationAddMemberParams {
            request: crate::coordination::requests::AddMemberRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
                backend_kind: "codex".to_string(),
                project_path: Some("/work/arch".to_string()),
            },
        };
        let remove = CoordinationRemoveMemberParams {
            request: crate::coordination::requests::RemoveMemberRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
            },
        };

        assert_eq!(
            serde_json::from_str::<CoordinationCreateTeamParams>(
                &serde_json::to_string(&create).unwrap()
            )
            .unwrap(),
            create
        );
        assert_eq!(
            serde_json::from_str::<CoordinationDisbandTeamParams>(
                &serde_json::to_string(&disband).unwrap()
            )
            .unwrap(),
            disband
        );
        assert_eq!(
            serde_json::from_str::<CoordinationAddMemberParams>(
                &serde_json::to_string(&add).unwrap()
            )
            .unwrap(),
            add
        );
        assert_eq!(
            serde_json::from_str::<CoordinationRemoveMemberParams>(
                &serde_json::to_string(&remove).unwrap()
            )
            .unwrap(),
            remove
        );
        assert_eq!(method::COORDINATION_CREATE_TEAM, "coordination.create_team");
        assert_eq!(
            method::COORDINATION_DISBAND_TEAM,
            "coordination.disband_team"
        );
        assert_eq!(method::COORDINATION_ADD_MEMBER, "coordination.add_member");
        assert_eq!(
            method::COORDINATION_REMOVE_MEMBER,
            "coordination.remove_member"
        );
    }

    #[test]
    fn stop_session_params_roundtrip() {
        let p = StopSessionParams {
            tmux_pane: "%3".to_string(),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: StopSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn stop_session_params_defaults_to_claude() {
        let json = r#"{"tmux_pane":"%3"}"#;
        let p: StopSessionParams = serde_json::from_str(json).unwrap();
        assert_eq!(
            p.cli_tool,
            crate::session_scanner::cli_tool::CliTool::Claude
        );
    }

    #[test]
    fn navigate_to_session_params_roundtrip() {
        let p = NavigateToSessionParams {
            tmux_session: "main".to_string(),
            tmux_window: "1".to_string(),
            tmux_pane: "%3".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: NavigateToSessionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn wait_session_updates_params_defaults() {
        let json = r#"{"since_version":42}"#;
        let p: WaitSessionUpdatesParams = serde_json::from_str(json).unwrap();
        assert_eq!(p.since_version, 42);
        assert_eq!(p.timeout_ms, 15_000);
        assert_eq!(
            p.since_degraded_revision, 0,
            "a client that does not track blackout edges asks as it always did"
        );
    }

    #[test]
    fn wait_session_updates_result_roundtrip() {
        // Regression: 967f956 let the scanner write SQLite directly. Moving
        // ownership to the app requires this credential-free observation to
        // survive the daemon protocol boundary instead.
        let r = WaitSessionUpdatesResult {
            version: 7,
            changed: true,
            sessions: vec![],
            account_observations: vec![crate::session_scanner::accounts::LiveAccountObservation {
                project_path: "/projects/taurhaus".to_string(),
                tool: crate::session_scanner::CliTool::Claude,
                account_id: "account-1".to_string(),
            }],
            focus: None,
            focus_project_path: None,
            degraded: false,
            degraded_revision: 0,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: WaitSessionUpdatesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    // Regression: the daemon hub kept its last good sessions across degraded
    // scanner cycles but the runtime-session protocol carried no degradation
    // status, so the Windows app read the cached snapshot as a fresh
    // observation. The flag travels on `get_runtime_session_snapshot`.
    #[test]
    fn runtime_session_snapshot_result_roundtrip_carries_degraded() {
        let r = RuntimeSessionSnapshotResult {
            version: 9,
            display_sessions: vec![],
            runtime_sessions: vec![],
            account_observations: vec![],
            focus: None,
            foreground_project_path: None,
            degraded: true,
            degraded_revision: 4,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"degraded\":true"));
        let back: RuntimeSessionSnapshotResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    // Regression companion: the field is additive — a daemon built before it
    // omits it and must decode as a healthy snapshot (its behavior so far).
    #[test]
    fn runtime_session_snapshot_result_old_daemon_without_degraded() {
        let json = r#"{"version":2,"display_sessions":[],"runtime_sessions":[],"focus":null,"foreground_project_path":null}"#;
        let r: RuntimeSessionSnapshotResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.version, 2);
        assert!(!r.degraded);
    }

    fn focus_fixture() -> crate::session_scanner::tmux::TmuxFocus {
        crate::session_scanner::tmux::TmuxFocus {
            session: "taurhaus".to_string(),
            window_index: "2".to_string(),
            pane_id: "%9".to_string(),
        }
    }

    // Regression: commit 07ab6c5 deleted the hook -> tmux-focus.json -> inotify
    // chain and made `wait_session_updates` the only live focus transport, but
    // left PROTOCOL_VERSION at 7. A v7 daemon omits the focus fields, the app
    // bridge reads that absence as "nothing is focused" and the sidebar
    // indicator goes dark with no other source to recover from. Startup must
    // refuse such a daemon on ping instead.
    #[test]
    fn protocol_version_excludes_daemons_without_hub_owned_focus() {
        // The last version whose daemon produced focus through the hook chain.
        let hook_chain_daemon = 7;
        assert!(
            PROTOCOL_VERSION > hook_chain_daemon,
            "hub-owned focus changed the wire contract: bump PROTOCOL_VERSION so \
             startup replaces a pre-PR8 daemon instead of trusting its empty focus"
        );
    }

    // Regression: commit 2b47b3b gave a blackout its own cursor
    // (`since_degraded_revision` in, `degraded_revision` out) but left
    // PROTOCOL_VERSION at 9, so both mixed pairs passed the exact-version gate.
    // A pre-PR10 app omits the cursor, it defaults to 0, and once a blackout has
    // ever happened the daemon's revision is permanently above 0 — every long
    // poll returns immediately and the bridge, which sleeps only between
    // failures, spins. The other direction is quieter and just as wrong: a
    // pre-PR10 daemon omits `degraded`/`degraded_revision`, so a new app decodes
    // a healthy snapshot and silently loses blackout reporting. Both are fixed
    // by making the version gate refuse the pair.
    #[test]
    fn protocol_version_excludes_daemons_without_degradation_cursor() {
        // The last version whose wire had no blackout cursor in either direction.
        let cursorless_daemon = 9;
        assert!(
            PROTOCOL_VERSION > cursorless_daemon,
            "the blackout cursor changed the wire contract in both directions: bump \
             PROTOCOL_VERSION so the exact-version gate refuses a pre-PR10 daemon \
             instead of losing degradation, and so a pre-PR10 app is refused \
             instead of spinning on immediate answers"
        );
    }

    #[test]
    fn protocol_version_excludes_daemons_with_claude_only_account_methods() {
        // Regression: commit d6839a3 added Claude-only account methods without
        // a protocol bump; replacing those wire names requires the exact-version
        // gate to reject both mixed app/daemon pairs.
        let last_protocol_with_claude_only_account_methods = 10;
        assert!(PROTOCOL_VERSION > last_protocol_with_claude_only_account_methods);
        assert_eq!(method::LIST_ACCOUNTS, "list_accounts");
        assert_eq!(method::PROJECT_TRANSCRIPT, "project_transcript");
        assert_eq!(method::RESOLVE_LAUNCH_BASE, "resolve_launch_base");
    }

    #[test]
    fn protocol_version_excludes_daemons_with_retired_cli_tool_vocabulary() {
        // Regression: commit 4cd067a replaced the daemon wire value for the
        // third harness while leaving protocol 11 pairs mutually incompatible.
        let last_protocol_with_retired_google_tool = 11;
        assert!(
            PROTOCOL_VERSION > last_protocol_with_retired_google_tool,
            "the CliTool vocabulary changed: bump PROTOCOL_VERSION so the exact-version gate refuses pre-18a daemons"
        );
    }

    #[test]
    fn protocol_version_excludes_daemons_without_the_grok_tool_value() {
        // Regression: commit bfecae9 shipped protocol 12 with a three-value
        // CliTool vocabulary. Adding `grok` is a wire vocabulary change in both
        // directions — a v12 daemon decodes `"grok"` as the retired-value
        // `Unknown`, and a v12 app does the same to a v13 daemon's sessions —
        // so the exact-version gate has to refuse the mixed pair.
        let last_protocol_without_grok = 12;
        assert!(
            PROTOCOL_VERSION > last_protocol_without_grok,
            "the CliTool vocabulary changed: bump PROTOCOL_VERSION so the exact-version gate refuses pre-18b daemons"
        );
        assert_eq!(
            serde_json::to_string(&crate::session_scanner::cli_tool::CliTool::Grok).unwrap(),
            "\"grok\""
        );
    }

    #[test]
    fn protocol_version_excludes_daemons_with_codex_compaction_mode() {
        // Regression: commit 6fe0aa3 added a daemon method for switching the
        // transcript owner. Retiring that method changes the paired app/daemon
        // vocabulary, so protocol 13 peers must be rejected.
        let last_protocol_with_codex_compaction_mode = 13;
        assert!(PROTOCOL_VERSION > last_protocol_with_codex_compaction_mode);
    }

    // Regression: 1bb8668e made the app the only deadline scheduler. Moving
    // that ownership without rejecting protocol 14 would permit a paired app
    // and daemon to execute the pass twice or not at all.
    #[test]
    fn protocol_version_excludes_daemons_without_the_deadline_scheduler() {
        let last_protocol_without_daemon_deadlines = 14;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_deadlines);
    }

    // Regression: 5cebfef8 let the app execute initialization locally. Once
    // that fallback is removed, a protocol-15 daemon cannot satisfy the only
    // remaining path and must be rejected by the exact-version gate.
    #[test]
    fn protocol_version_excludes_daemons_without_team_initialization() {
        let last_protocol_without_daemon_team_initialization = 15;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_team_initialization);
    }

    #[test]
    fn protocol_version_excludes_daemons_without_member_operations() {
        let last_protocol_without_daemon_member_operations = 16;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_member_operations);
    }

    #[test]
    fn protocol_version_excludes_daemons_without_team_resume_operations() {
        let last_protocol_without_daemon_team_resume_operations = 17;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_team_resume_operations);
    }

    // Regression: f8d08a21 added daemon-owned standalone team and roster
    // operations without pinning exclusion of protocol-18 daemons.
    #[test]
    fn protocol_version_excludes_daemons_without_roster_operations() {
        let last_protocol_without_daemon_roster_operations = 18;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_roster_operations);
    }

    #[test]
    fn protocol_version_excludes_daemons_with_stop_member_methods() {
        // Regression: 03eb3a2c made remove-member the app's roster-removal path
        // but left the superseded protocol-17 stop-member methods callable.
        let last_protocol_with_stop_member_methods = 19;
        assert!(PROTOCOL_VERSION > last_protocol_with_stop_member_methods);
    }

    // Regression: 25293092 made a background effort relaunch possible from
    // stock command defaults. Protocol 20 has no pushed-settings seam, so a
    // mixed pair could move a `claude2` member off its pinned account.
    #[test]
    fn protocol_version_excludes_daemons_without_background_pass_routing() {
        let last_protocol_without_daemon_background_passes = 20;
        assert!(PROTOCOL_VERSION > last_protocol_without_daemon_background_passes);
    }

    #[test]
    fn protocol_version_excludes_daemons_without_the_final_writer_intents() {
        let last_protocol_with_app_side_team_state_writers = 21;
        assert!(PROTOCOL_VERSION > last_protocol_with_app_side_team_state_writers);
    }

    #[test]
    fn wait_session_updates_result_roundtrips_focus() {
        let result = WaitSessionUpdatesResult {
            version: 7,
            changed: true,
            sessions: Vec::new(),
            account_observations: Vec::new(),
            focus: Some(focus_fixture()),
            focus_project_path: Some("/projects/mesh".to_string()),
            degraded: false,
            degraded_revision: 0,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serde_json::from_str::<WaitSessionUpdatesResult>(&json).unwrap(),
            result
        );
    }

    #[test]
    fn wait_session_updates_result_decodes_without_focus() {
        // Old daemons omit the focus fields entirely.
        let json = r#"{"version":3,"changed":false,"sessions":[]}"#;
        let result: WaitSessionUpdatesResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.focus, None);
        assert_eq!(result.focus_project_path, None);
        assert!(!result.degraded, "an omitted flag decodes as healthy");
        assert_eq!(
            result.degraded_revision, 0,
            "an omitted revision never advances, so it never claims a blind interval"
        );
    }

    // Regression: 6c6f1cb made the app present a degraded snapshot as
    // uncertain, but `wait_session_updates` — the transport the session bridge
    // actually lives on — carried no degradation status, so the retained
    // sessions arrived indistinguishable from a fresh observation. Additive
    // field: an older daemon omits it and decodes as healthy (above).
    #[test]
    fn wait_session_updates_result_roundtrips_degraded() {
        let result = WaitSessionUpdatesResult {
            version: 11,
            changed: false,
            sessions: Vec::new(),
            account_observations: Vec::new(),
            focus: None,
            focus_project_path: None,
            degraded: true,
            degraded_revision: 3,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"degraded\":true"));
        assert_eq!(
            serde_json::from_str::<WaitSessionUpdatesResult>(&json).unwrap(),
            result
        );
    }

    #[test]
    fn runtime_session_snapshot_result_roundtrips_focus() {
        let result = RuntimeSessionSnapshotResult {
            version: 4,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            account_observations: Vec::new(),
            focus: Some(focus_fixture()),
            foreground_project_path: Some("/projects/mesh".to_string()),
            degraded: false,
            degraded_revision: 0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["focus"]["session"], "taurhaus");
        assert_eq!(json["focus"]["window"], "2");
        assert_eq!(
            serde_json::from_value::<RuntimeSessionSnapshotResult>(json).unwrap(),
            result
        );
    }

    #[test]
    fn runtime_session_snapshot_result_decodes_legacy_focus_payload() {
        // A daemon built before the hub-owned probe sends the hook file shape.
        let json = r#"{"version":1,"display_sessions":[],"runtime_sessions":[],"focus":{"session":"taurhaus","window":"2","timestamp":123},"foreground_project_path":null}"#;
        let result: RuntimeSessionSnapshotResult = serde_json::from_str(json).unwrap();
        let focus = result.focus.expect("legacy focus decodes");
        assert_eq!(focus.session, "taurhaus");
        assert_eq!(focus.window_index, "2");
        assert_eq!(focus.pane_id, "");

        let detached = r#"{"version":1,"display_sessions":[],"runtime_sessions":[],"focus":{"session":null,"window":null,"timestamp":null},"foreground_project_path":null}"#;
        let result: RuntimeSessionSnapshotResult = serde_json::from_str(detached).unwrap();
        assert_eq!(result.focus.expect("detached focus decodes").session, "");
    }
}
