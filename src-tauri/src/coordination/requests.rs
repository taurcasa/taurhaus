//! Backend-agnostic coordination requests and responses.

use serde::{Deserialize, Serialize};

use crate::coordination::domain::{HealthState, Member};
use crate::session_scanner::cli_tool::CliTool;
use crate::templates::types::{BehavioralContract, RuntimeCompactSummary};

/// Launch-time policy controls for a member session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPermissions {
    Standard,
    Restricted,
    Elevated,
}

/// Request to launch or re-attach a managed team member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub member: Member,
    pub team_name: String,
    pub pane_target: Option<String>,
    pub permissions: LaunchPermissions,
}

/// Result of a backend launch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchResult {
    pub pane_id: String,
    pub process_id: Option<u32>,
}

/// Typed payload for first-contact delivery after launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapDelivery {
    pub member_name: String,
    pub team_name: String,
    pub message: String,
}

/// Typed payload for recovery nudges when health degrades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryNudgeDelivery {
    pub member_name: String,
    pub team_name: String,
    pub reason: String,
}

/// Typed payload for operator-authored notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OperationalTaskContext {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OperationalAssignmentFooter {
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub file_ownership_boundary: Vec<String>,
    #[serde(default)]
    pub adjacent_fix_policy: String,
    #[serde(default)]
    pub validation_expectation: String,
    #[serde(default)]
    pub response_expectation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OperationalOwnershipContext {
    #[serde(default)]
    pub override_allowed: bool,
    #[serde(default)]
    pub active_override_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OperationalWorkingSetContext {
    #[serde(default)]
    pub project_path: String,
    #[serde(default)]
    pub focal_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OperationalContextUpdate {
    #[serde(default)]
    pub task: Option<OperationalTaskContext>,
    #[serde(default)]
    pub assignment_footer: Option<OperationalAssignmentFooter>,
    #[serde(default)]
    pub ownership: Option<OperationalOwnershipContext>,
    #[serde(default)]
    pub working_set: Option<OperationalWorkingSetContext>,
}

/// Typed payload for operator-authored notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorNoticeDelivery {
    pub member_name: String,
    pub team_name: String,
    pub message: String,
    #[serde(default)]
    pub sender_name: Option<String>,
    #[serde(default)]
    pub operational_context: Option<OperationalContextUpdate>,
}

/// Typed delivery request variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "payload")]
pub enum DeliveryRequest {
    Bootstrap(BootstrapDelivery),
    RecoveryNudge(RecoveryNudgeDelivery),
    OperatorNotice(Box<OperatorNoticeDelivery>),
}

impl DeliveryRequest {
    pub fn operator_notice(payload: OperatorNoticeDelivery) -> Self {
        Self::OperatorNotice(Box::new(payload))
    }
}

/// Mechanism used by backend to deliver a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMethod {
    InboxFile,
    TmuxInjection,
    NativeMessageApi,
}

/// Delivery completion response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryResult {
    pub delivered: bool,
    pub method: DeliveryMethod,
}

/// Request to probe a member's process and interaction health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeRequest {
    pub member_name: String,
    pub team_name: String,
}

/// Signal quality produced by probe execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeEvidence {
    None,
    WeakIo,
    StrongInbox,
}

/// Probe response used by health monitoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub alive: bool,
    pub health: HealthState,
    pub evidence: ProbeEvidence,
}

/// Teardown mode for stopping member sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeardownMode {
    Graceful,
    Force,
}

/// Request to tear down a member session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeardownRequest {
    pub member_name: String,
    pub team_name: String,
    pub mode: TeardownMode,
}

/// Result of a teardown attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeardownResult {
    pub success: bool,
}

/// Team-lead startup mode for initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadMode {
    AttachExisting,
    LaunchNew,
}

/// Agent role in a live team roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Lead,
    Member,
}

/// Runtime session status shown in the live team roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Active,
    Idle,
    Offline,
}

/// Operation kind used by streamed step-progress events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    InitializeTeam,
    AddAgent,
    ReOnboard,
}

/// Status for a single step in a long-running operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

/// Canonical member-activation stages shared by initialize, resume, and add-agent.
///
/// These stages intentionally describe member-scoped activation work only.
/// Wrapper-scoped steps such as `create_team` and `add_lead` remain outside this
/// vocabulary and map to an empty canonical stage set in the legacy mapping
/// table below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberActivationStage {
    PrepareMember,
    AcquirePane,
    LaunchSession,
    CaptureSessionIdentity,
    JoinMesh,
    StartMemberDaemon,
    CommitRuntime,
    DeliverOnboarding,
}

impl MemberActivationStage {
    pub const ALL: [Self; 8] = [
        Self::PrepareMember,
        Self::AcquirePane,
        Self::LaunchSession,
        Self::CaptureSessionIdentity,
        Self::JoinMesh,
        Self::StartMemberDaemon,
        Self::CommitRuntime,
        Self::DeliverOnboarding,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PrepareMember => "prepare_member",
            Self::AcquirePane => "acquire_pane",
            Self::LaunchSession => "launch_session",
            Self::CaptureSessionIdentity => "capture_session_identity",
            Self::JoinMesh => "join_mesh",
            Self::StartMemberDaemon => "start_member_daemon",
            Self::CommitRuntime => "commit_runtime",
            Self::DeliverOnboarding => "deliver_onboarding",
        }
    }
}

impl std::fmt::Display for MemberActivationStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Maps existing wrapper-local stage names onto the canonical member-activation
/// vocabulary.
///
/// An empty `canonical_stages` slice means the legacy step is wrapper-scoped
/// and intentionally remains outside the shared member-activation stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyMemberActivationStageMapping {
    pub wrapper: &'static str,
    pub legacy_stage: &'static str,
    pub canonical_stages: &'static [MemberActivationStage],
    pub note: &'static str,
}

pub const LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS: &[LegacyMemberActivationStageMapping] = &[
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "validate_configuration",
        canonical_stages: &[MemberActivationStage::PrepareMember],
        note: "Team-wide/member-wide preparation before activation begins.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "create_team",
        canonical_stages: &[],
        note: "Wrapper-scoped team creation; not part of member activation.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "add_lead",
        canonical_stages: &[],
        note: "Wrapper-scoped roster seeding; not part of member activation.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "create_panes",
        canonical_stages: &[
            MemberActivationStage::AcquirePane,
            MemberActivationStage::LaunchSession,
        ],
        note: "Initialize currently opens panes and launches sessions in one batch stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "launch_sessions",
        canonical_stages: &[MemberActivationStage::CaptureSessionIdentity],
        note: "Initialize captures runtime session identity after launch.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "join_mesh",
        canonical_stages: &[MemberActivationStage::JoinMesh],
        note: "Canonical member mesh-join stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "start_daemons",
        canonical_stages: &[MemberActivationStage::StartMemberDaemon],
        note: "Canonical member daemon-start stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "initialize",
        legacy_stage: "send_onboarding",
        canonical_stages: &[MemberActivationStage::DeliverOnboarding],
        note: "Canonical onboarding delivery stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "validate",
        canonical_stages: &[MemberActivationStage::PrepareMember],
        note: "Part of resume member preparation.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "load_member",
        canonical_stages: &[MemberActivationStage::PrepareMember],
        note: "Part of resume member preparation.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "resolve_pane",
        canonical_stages: &[MemberActivationStage::AcquirePane],
        note: "Canonical pane acquisition stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "launch_session",
        canonical_stages: &[
            MemberActivationStage::LaunchSession,
            MemberActivationStage::CaptureSessionIdentity,
        ],
        note: "Legacy resume step spans both launch and runtime session capture.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "join_mesh",
        canonical_stages: &[MemberActivationStage::JoinMesh],
        note: "Canonical member mesh-join stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "start_daemon",
        canonical_stages: &[MemberActivationStage::StartMemberDaemon],
        note: "Canonical member daemon-start stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "send_onboarding",
        canonical_stages: &[MemberActivationStage::DeliverOnboarding],
        note: "Canonical onboarding delivery stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "resume",
        legacy_stage: "update_runtime",
        canonical_stages: &[MemberActivationStage::CommitRuntime],
        note: "Canonical runtime commit stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "validate",
        canonical_stages: &[MemberActivationStage::PrepareMember],
        note: "Part of add-agent member preparation.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "create_pane",
        canonical_stages: &[
            MemberActivationStage::AcquirePane,
            MemberActivationStage::LaunchSession,
        ],
        note: "Add-agent currently opens the pane and launches the CLI in one step.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "launch_session",
        canonical_stages: &[MemberActivationStage::CaptureSessionIdentity],
        note: "Add-agent uses this step for runtime session identity capture.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "join_mesh",
        canonical_stages: &[MemberActivationStage::JoinMesh],
        note: "Canonical member mesh-join stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "start_daemon",
        canonical_stages: &[MemberActivationStage::StartMemberDaemon],
        note: "Canonical member daemon-start stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "send_onboarding",
        canonical_stages: &[MemberActivationStage::DeliverOnboarding],
        note: "Canonical onboarding delivery stage.",
    },
    LegacyMemberActivationStageMapping {
        wrapper: "add_agent",
        legacy_stage: "update_roster",
        canonical_stages: &[MemberActivationStage::CommitRuntime],
        note: "Add-agent commits member/runtime state into team metadata here.",
    },
];

pub fn canonical_member_activation_stages(
    wrapper: &str,
    legacy_stage: &str,
) -> &'static [MemberActivationStage] {
    LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS
        .iter()
        .find(|entry| entry.wrapper == wrapper && entry.legacy_stage == legacy_stage)
        .map(|entry| entry.canonical_stages)
        .unwrap_or(&[])
}

/// Progress metadata for one operation step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProgress {
    pub step: String,
    pub status: StepStatus,
    pub message: Option<String>,
}

/// Shared agent definition for initialize/hot-add requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub name: String,
    pub cli_tool: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub project_id: String,
    pub description: Option<String>,
    #[serde(default)]
    pub role_id: Option<String>,
    #[serde(default)]
    pub role_name: Option<String>,
    #[serde(default)]
    pub focus_area: Option<String>,
    #[serde(default)]
    pub context_summary: Option<String>,
    #[serde(default)]
    pub behavior_summary: Option<String>,
    #[serde(default)]
    pub communication_style: Option<String>,
    #[serde(default)]
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub behavioral_contract: Option<BehavioralContract>,
    #[serde(default)]
    pub quality_gates: Option<Vec<String>>,
    #[serde(default)]
    pub handoff_expectations: Option<Vec<String>>,
    #[serde(default)]
    pub definition_of_done: Option<Vec<String>>,
    #[serde(default)]
    pub phase_scope: Option<Vec<String>>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub required_artifacts: Option<Vec<String>>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
}

/// Domain contract for full team initialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializeTeam {
    pub team_name: String,
    pub team_description: Option<String>,
    pub lead_mode: LeadMode,
    pub lead: AgentDefinition,
    pub agents: Vec<AgentDefinition>,
}

/// Domain contract for adding a member to a running team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddAgent {
    pub team_name: String,
    pub agent: AgentDefinition,
}

/// Shared report fields for initialize/hot-add flows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializeResult {
    pub team_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// Result contract for adding one agent to a running team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddAgentResult {
    pub team_name: String,
    pub member_name: String,
    pub succeeded_steps: Vec<String>,
    pub failed_step: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub steps: Vec<StepProgress>,
}

/// Request contract for resuming a team member session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeMemberRequest {
    pub team_name: String,
    pub member_name: String,
}

/// Request contract for resuming all members in a team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeTeamRequest {
    pub team_name: String,
}

/// Result contract for resuming an agent in a running team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Failure entry for one member during team resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeTeamMemberFailure {
    pub member_name: String,
    pub message: String,
    pub retryable: bool,
}

/// Aggregated result contract for resuming a persisted team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

pub type AgentSetupConfig = AgentDefinition;
pub type InitializeTeamRequest = InitializeTeam;
pub type AddAgentRequest = AddAgent;
pub type InitializeReport = InitializeResult;
pub type AddAgentReport = AddAgentResult;
pub type LiveAgentStatus = LiveAgent;

/// Live roster row rendered by the frontend mesh view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveAgent {
    pub name: String,
    pub role: AgentRole,
    pub cli_tool: CliTool,
    pub model: String,
    pub project_id: String,
    pub description: Option<String>,
    pub session_status: SessionStatus,
    pub pane_id: Option<String>,
}

/// Live team payload returned to frontend for runtime view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveTeamStatus {
    pub team_name: String,
    pub lead_name: String,
    pub members: Vec<LiveAgent>,
}

/// Streamed progress event payload emitted during long operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProgressEvent {
    pub team_name: String,
    pub operation: OperationKind,
    pub progress: StepProgress,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::coordination::domain::MemberRole;
    use crate::session_scanner::cli_tool::CliTool;

    #[test]
    fn launch_request_round_trip() {
        let req = LaunchRequest {
            member: Member {
                name: "agent-1".to_string(),
                role: MemberRole::Agent,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Focus on implementation".to_string()),
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
            },
            team_name: "architecture-final".to_string(),
            pane_target: Some("main.%0".to_string()),
            permissions: LaunchPermissions::Standard,
        };

        let encoded = serde_json::to_string(&req).expect("request should serialize");
        let decoded: LaunchRequest =
            serde_json::from_str(&encoded).expect("request should deserialize");
        assert_eq!(decoded, req);
    }

    #[test]
    fn delivery_request_round_trip() {
        let req = DeliveryRequest::OperatorNotice(Box::new(OperatorNoticeDelivery {
            member_name: "agent-1".to_string(),
            team_name: "architecture-final".to_string(),
            message: "Check your inbox".to_string(),
            sender_name: Some("team-lead".to_string()),
            operational_context: None,
        }));

        let encoded = serde_json::to_value(&req).expect("request should serialize");
        let decoded: DeliveryRequest =
            serde_json::from_value(encoded).expect("request should deserialize");
        assert_eq!(decoded, req);
    }

    #[test]
    fn initialize_contract_round_trip() {
        let req = InitializeTeam {
            team_name: "architecture-final".to_string(),
            team_description: Some("Cross-project team".to_string()),
            lead_mode: LeadMode::AttachExisting,
            lead: AgentDefinition {
                name: "team-lead".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                project_id: "proj-core".to_string(),
                description: Some("Lead".to_string()),
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
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
            },
            agents: vec![AgentDefinition {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                project_id: "proj-web".to_string(),
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
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
            }],
        };

        let json = serde_json::to_string(&req).expect("serialize initialize contract");
        let decoded: InitializeTeam =
            serde_json::from_str(&json).expect("deserialize initialize contract");
        assert_eq!(decoded, req);
    }

    #[test]
    fn add_agent_request_round_trip() {
        let req = AddAgent {
            team_name: "architecture-final".to_string(),
            agent: AgentDefinition {
                name: "backend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                project_id: "proj-api".to_string(),
                description: Some("Own API work".to_string()),
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
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
            },
        };

        let json = serde_json::to_string(&req).expect("serialize add-agent request");
        let decoded: AddAgent = serde_json::from_str(&json).expect("deserialize add-agent request");
        assert_eq!(decoded, req);
    }

    #[test]
    fn resume_team_request_and_report_round_trip() {
        let req = ResumeTeamRequest {
            team_name: "architecture-final".to_string(),
        };

        let req_json = serde_json::to_string(&req).expect("serialize resume-team request");
        let req_decoded: ResumeTeamRequest =
            serde_json::from_str(&req_json).expect("deserialize resume-team request");
        assert_eq!(req_decoded, req);

        let report = ResumeTeamReport {
            team_name: "architecture-final".to_string(),
            resumed: true,
            total_members: 3,
            resumed_members: vec!["team-lead".to_string(), "reviewer".to_string()],
            failed_members: vec![ResumeTeamMemberFailure {
                member_name: "builder".to_string(),
                message: "mesh join failed".to_string(),
                retryable: true,
            }],
            warnings: vec!["builder: created a replacement pane".to_string()],
            started_team_daemon: false,
            team_daemon_warning: Some("team daemon start not implemented".to_string()),
        };

        let report_json = serde_json::to_string(&report).expect("serialize resume-team report");
        let report_decoded: ResumeTeamReport =
            serde_json::from_str(&report_json).expect("deserialize resume-team report");
        assert_eq!(report_decoded, report);
    }

    #[test]
    fn initialize_and_add_agent_reports_round_trip() {
        let init_report = InitializeResult {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec![
                "validate_configuration".to_string(),
                "create_team".to_string(),
            ],
            failed_step: Some("launch_sessions".to_string()),
            retryable: true,
            message: "one launch failed".to_string(),
            steps: vec![
                StepProgress {
                    step: "create_team".to_string(),
                    status: StepStatus::Succeeded,
                    message: None,
                },
                StepProgress {
                    step: "launch_sessions".to_string(),
                    status: StepStatus::Failed,
                    message: Some("codex missing".to_string()),
                },
            ],
        };
        let init_json = serde_json::to_string(&init_report).expect("serialize init report");
        let init_decoded: InitializeResult =
            serde_json::from_str(&init_json).expect("deserialize init report");
        assert_eq!(init_decoded, init_report);

        let add_report = AddAgentResult {
            team_name: "architecture-final".to_string(),
            member_name: "backend-dev".to_string(),
            succeeded_steps: vec![
                "create_pane".to_string(),
                "launch_session".to_string(),
                "join_mesh".to_string(),
            ],
            failed_step: None,
            retryable: false,
            message: "added".to_string(),
            steps: vec![StepProgress {
                step: "join_mesh".to_string(),
                status: StepStatus::Succeeded,
                message: Some("ok".to_string()),
            }],
        };
        let add_json = serde_json::to_string(&add_report).expect("serialize add report");
        let add_decoded: AddAgentResult =
            serde_json::from_str(&add_json).expect("deserialize add report");
        assert_eq!(add_decoded, add_report);
    }

    #[test]
    fn live_team_status_and_progress_event_round_trip() {
        let status = LiveTeamStatus {
            team_name: "architecture-final".to_string(),
            lead_name: "team-lead".to_string(),
            members: vec![
                LiveAgent {
                    name: "team-lead".to_string(),
                    role: AgentRole::Lead,
                    cli_tool: CliTool::Claude,
                    model: "opus".to_string(),
                    project_id: "proj-core".to_string(),
                    description: Some("Lead".to_string()),
                    session_status: SessionStatus::Active,
                    pane_id: Some("%1".to_string()),
                },
                LiveAgent {
                    name: "frontend-dev".to_string(),
                    role: AgentRole::Member,
                    cli_tool: CliTool::Codex,
                    model: "gpt-5.4".to_string(),
                    project_id: "proj-web".to_string(),
                    description: None,
                    session_status: SessionStatus::Idle,
                    pane_id: Some("%2".to_string()),
                },
            ],
        };
        let status_json = serde_json::to_string(&status).expect("serialize live team");
        let status_decoded: LiveTeamStatus =
            serde_json::from_str(&status_json).expect("deserialize live team");
        assert_eq!(status_decoded, status);

        let event = StepProgressEvent {
            team_name: "architecture-final".to_string(),
            operation: OperationKind::AddAgent,
            progress: StepProgress {
                step: "send_onboarding".to_string(),
                status: StepStatus::Running,
                message: Some("sending".to_string()),
            },
        };
        let event_json = serde_json::to_string(&event).expect("serialize event");
        let event_decoded: StepProgressEvent =
            serde_json::from_str(&event_json).expect("deserialize event");
        assert_eq!(event_decoded, event);
    }

    #[test]
    fn member_activation_stage_round_trip() {
        let stage = MemberActivationStage::CommitRuntime;
        let json = serde_json::to_string(&stage).expect("serialize member activation stage");
        assert_eq!(json, "\"commit_runtime\"");

        let decoded: MemberActivationStage =
            serde_json::from_str(&json).expect("deserialize member activation stage");
        assert_eq!(decoded, stage);
        assert_eq!(stage.to_string(), "commit_runtime");
    }

    #[test]
    fn legacy_member_activation_stage_mapping_covers_all_wrappers() {
        assert!(LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS
            .iter()
            .any(|entry| entry.wrapper == "initialize"));
        assert!(LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS
            .iter()
            .any(|entry| entry.wrapper == "resume"));
        assert!(LEGACY_MEMBER_ACTIVATION_STAGE_MAPPINGS
            .iter()
            .any(|entry| entry.wrapper == "add_agent"));
    }

    #[test]
    fn canonical_member_activation_stages_look_up_legacy_mapping() {
        assert_eq!(
            canonical_member_activation_stages("resume", "update_runtime"),
            &[MemberActivationStage::CommitRuntime]
        );
        assert_eq!(
            canonical_member_activation_stages("initialize", "create_team"),
            &[]
        );
        assert_eq!(
            canonical_member_activation_stages("missing", "missing"),
            &[]
        );
    }
}
