use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use tempfile::TempDir;

use crate::coordination::backend::fake::FakeBackend;
use crate::coordination::backend::{BackendCapabilities, BackendKind, CoordinationBackend};
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::{
    MemberActivationContext, MemberActivationDeliveryPolicy,
};
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentRequest, AgentSetupConfig, DeliveryRequest, DeliveryResult, InitializeTeamRequest,
    LaunchRequest, LaunchResult, LeadMode, ProbeRequest, ProbeResult, ResumeMemberRequest,
    StepStatus, TeardownRequest, TeardownResult,
};
use crate::coordination::runtime::{
    CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::templates::types::BehavioralContract;

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeliveryTimelineEvent {
    JoinMesh(String),
    SpawnDaemon(String),
    Deliver(String),
}

#[derive(Debug, Clone)]
struct SequencedBackend {
    inner: FakeBackend,
    events: Arc<Mutex<Vec<DeliveryTimelineEvent>>>,
}

impl SequencedBackend {
    fn new(events: Arc<Mutex<Vec<DeliveryTimelineEvent>>>) -> Self {
        Self {
            inner: FakeBackend::default(),
            events,
        }
    }

    fn push_event(&self, event: DeliveryTimelineEvent) {
        self.events
            .lock()
            .expect("sequenced backend events mutex poisoned")
            .push(event);
    }
}

impl CoordinationBackend for SequencedBackend {
    fn kind(&self) -> BackendKind {
        self.inner.kind()
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.inner.capabilities()
    }

    fn launch(&self, req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        self.inner.launch(req)
    }

    fn deliver(&self, req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        if let DeliveryRequest::OperatorNotice(payload) = &req {
            self.push_event(DeliveryTimelineEvent::Deliver(payload.member_name.clone()));
        }
        self.inner.deliver(req)
    }

    fn probe(&self, req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        self.inner.probe(req)
    }

    fn teardown(&self, req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        self.inner.teardown(req)
    }
}

#[derive(Debug)]
struct SequencedRuntime {
    inner: RecordingCoordinationRuntime,
    events: Arc<Mutex<Vec<DeliveryTimelineEvent>>>,
}

impl SequencedRuntime {
    fn new(events: Arc<Mutex<Vec<DeliveryTimelineEvent>>>) -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
            events,
        }
    }

    fn push_event(&self, event: DeliveryTimelineEvent) {
        self.events
            .lock()
            .expect("sequenced runtime events mutex poisoned")
            .push(event);
    }
}

impl CoordinationRuntime for SequencedRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn detect_runtime_session(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<crate::coordination::runtime::DetectedRuntimeSession, CoordinationError> {
        self.inner.detect_runtime_session(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.join_mesh(team_name, member_name, project_id)?;
        self.push_event(DeliveryTimelineEvent::JoinMesh(member_name.to_string()));
        Ok(())
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        let pid = self
            .inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)?;
        self.push_event(DeliveryTimelineEvent::SpawnDaemon(member_name.to_string()));
        Ok(pid)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.inner.pane_belongs_to_project(pane_id, project_id)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError> {
        self.inner.pane_current_command(pane_id)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }

    fn mesh_daemon_uses_current_binary(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.mesh_daemon_uses_current_binary(pid)
    }

    fn team_daemon_uses_current_binary(&self, team_name: &str) -> Result<bool, CoordinationError> {
        self.inner.team_daemon_uses_current_binary(team_name)
    }

    fn clear_mesh_daemon_pid_file(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        self.inner
            .clear_mesh_daemon_pid_file(team_name, member_name)
    }

    fn stop_team_daemon(&self, team_name: &str) -> Result<(), CoordinationError> {
        self.inner.stop_team_daemon(team_name)
    }
}

fn member(name: &str, role: MemberRole, cli_tool: CliTool, project: &str) -> Member {
    Member {
        name: name.to_string(),
        role,
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
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
        project_path: PathBuf::from(project),
        cli_tool,
    }
}

fn setup_config(name: &str, cli_tool: &str, model: &str, project_id: &str) -> AgentSetupConfig {
    AgentSetupConfig {
        name: name.to_string(),
        cli_tool: cli_tool.to_string(),
        model: model.to_string(),
        project_id: project_id.to_string(),
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
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    }
}

fn new_orchestrator(
    tmp: &TempDir,
    backend: Arc<FakeBackend>,
    runtime: Arc<RecordingCoordinationRuntime>,
) -> CoordinationOrchestrator {
    CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime)
}

fn timeline_index(events: &[DeliveryTimelineEvent], target: DeliveryTimelineEvent) -> usize {
    events
        .iter()
        .position(|event| *event == target)
        .expect("timeline event should exist")
}

fn mark_member_offline(
    tmp: &TempDir,
    team_name: &str,
    member_name: &str,
    pane_id: &str,
    daemon_pid: Option<u32>,
) {
    let mut runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("member runtime");
    runtime.pane_id = Some(pane_id.to_string());
    runtime.daemon_pid = daemon_pid;
    runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime)
        .expect("save offline runtime");
}

#[test]
fn staged_runtime_commit_merges_partial_updates_without_syncing_team_metadata() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let team_name = "architecture-final";
    let member_name = "builder";

    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            member(
                member_name,
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/builder",
            ),
        )
        .expect("add member");

    let member_config = setup_config(member_name, "codex", "gpt-5.4", "/tmp/builder");
    let context = MemberActivationContext::for_initialize_member(
        team_name,
        "team-lead",
        &member_config,
        MemberRole::Agent,
    )
    .expect("context");

    orchestrator
        .commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%11".to_string())),
                session_id: Some(None),
                jsonl_path: Some(None),
                daemon_pid: Some(None),
                attached_at: Some(Some(Utc::now())),
                health: Some(HealthState::Healthy),
            },
        )
        .expect("commit pane");
    orchestrator
        .commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                session_id: Some(Some("session-%11".to_string())),
                jsonl_path: Some(Some(PathBuf::from("/tmp/builder.jsonl"))),
                ..Default::default()
            },
        )
        .expect("commit session");
    orchestrator
        .commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                daemon_pid: Some(Some(4444)),
                ..Default::default()
            },
        )
        .expect("commit daemon");

    let runtime = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    assert_eq!(runtime.pane_id.as_deref(), Some("%11"));
    assert_eq!(runtime.session_id.as_deref(), Some("session-%11"));
    assert_eq!(
        runtime.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/builder.jsonl"))
    );
    assert_eq!(runtime.daemon_pid, Some(4444));
    assert_eq!(runtime.health, HealthState::Healthy);

    let raw_config =
        fs::read_to_string(tmp.path().join(team_name).join("config.json")).expect("read config");
    let config: serde_json::Value = serde_json::from_str(&raw_config).expect("parse config");
    let member = config["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|member| member["name"].as_str() == Some(member_name))
        .expect("member entry");
    assert_eq!(
        member["tmuxPaneId"].as_str(),
        None,
        "staged initialize commits should not sync config metadata yet"
    );
}

#[test]
fn finalized_runtime_commit_syncs_team_metadata() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let team_name = "architecture-final";
    let member_name = "builder";

    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            member(
                member_name,
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/builder",
            ),
        )
        .expect("add member");

    let member_config = setup_config(member_name, "codex", "gpt-5.4", "/tmp/builder");
    let context = MemberActivationContext::for_add_agent(team_name, "team-lead", &member_config)
        .expect("context");

    orchestrator
        .commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%21".to_string())),
                session_id: Some(Some("session-%21".to_string())),
                jsonl_path: Some(Some(PathBuf::from("/tmp/builder-final.jsonl"))),
                daemon_pid: Some(Some(9001)),
                attached_at: Some(Some(Utc::now())),
                health: Some(HealthState::Healthy),
            },
        )
        .expect("commit runtime");

    let runtime = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    assert_eq!(runtime.pane_id.as_deref(), Some("%21"));
    assert_eq!(runtime.session_id.as_deref(), Some("session-%21"));
    assert_eq!(runtime.daemon_pid, Some(9001));
    assert_eq!(runtime.health, HealthState::Healthy);

    let raw_config =
        fs::read_to_string(tmp.path().join(team_name).join("config.json")).expect("read config");
    let config: serde_json::Value = serde_json::from_str(&raw_config).expect("parse config");
    let member = config["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|member| member["name"].as_str() == Some(member_name))
        .expect("member entry");
    assert_eq!(
        member["tmuxPaneId"].as_str(),
        Some("%21"),
        "finalized runtime commits should refresh config metadata"
    );
}

#[test]
fn join_mesh_if_required_skips_claude_and_joins_mesh_sidecar_members() {
    let runtime = RecordingCoordinationRuntime::default();

    let claude_joined = join_mesh_if_required(
        &runtime,
        "architecture-final",
        "team-lead",
        "/tmp/lead",
        CliTool::Claude,
    )
    .expect("claude join result");
    let codex_joined = join_mesh_if_required(
        &runtime,
        "architecture-final",
        "builder",
        "/tmp/builder",
        CliTool::Codex,
    )
    .expect("codex join result");

    assert!(!claude_joined, "Claude should keep the no-op join behavior");
    assert!(codex_joined, "mesh-sidecar members should still join Mesh");

    let calls = runtime.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, RuntimeCall::JoinMesh { .. }))
            .count(),
        1,
        "only the mesh-sidecar member should issue join_mesh"
    );
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::JoinMesh {
            team_name,
            member_name,
            project_id,
        } if team_name == "architecture-final"
            && member_name == "builder"
            && project_id == "/tmp/builder"
    )));
}

#[test]
fn start_member_daemon_if_required_replaces_stale_pid_for_resume_policy() {
    let runtime = RecordingCoordinationRuntime::default();
    let mut warnings = Vec::new();

    let daemon_pid = start_member_daemon_if_required(
        &runtime,
        "architecture-final",
        "builder",
        "%11",
        CliTool::Codex,
        MemberDaemonStartPolicy::ReplaceStalePid {
            previous_daemon_pid: Some(55),
        },
        Some(&mut warnings),
    )
    .expect("daemon start result");

    assert_eq!(daemon_pid, Some(10000));
    assert!(
        warnings.is_empty(),
        "successful stale-pid replacement should stay quiet"
    );

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 55)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnDaemon {
            pane_id,
            team_name,
            member_name,
        } if pane_id == "%11"
            && team_name == "architecture-final"
            && member_name == "builder"
    )));
}

#[test]
fn build_cli_launch_command_uses_configured_fresh_command() {
    let mut cmds = crate::models::CliCommandSettings::default();
    cmds.gemini.fresh = "gemini --yolo --sandbox read-only".to_string();
    let agent = AgentSetupConfig {
        name: "reviewer".to_string(),
        cli_tool: "gemini".to_string(),
        model: "gemini-2.5-pro".to_string(),
        project_id: "/tmp/project".to_string(),
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
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    };
    assert_eq!(
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
            .expect("command"),
        "gemini --yolo --sandbox read-only"
    );
}

#[test]
fn build_cli_launch_command_for_codex_appends_model_when_missing() {
    let cmds = crate::models::CliCommandSettings::default();
    let agent = AgentSetupConfig {
        name: "builder".to_string(),
        cli_tool: "codex".to_string(),
        model: "gpt-5.4".to_string(),
        project_id: "/tmp/project".to_string(),
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
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    };
    assert_eq!(
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
            .expect("command"),
        "codex --yolo -m 'gpt-5.4'"
    );
}

#[test]
fn build_cli_launch_command_for_claude_appends_team_context() {
    let cmds = crate::models::CliCommandSettings::default();
    let agent = AgentSetupConfig {
        name: "team-lead".to_string(),
        cli_tool: "claude".to_string(),
        model: "claude-opus-4-6".to_string(),
        project_id: "/tmp/project".to_string(),
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
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    };
    let command =
        build_cli_launch_command(&agent, "ledger-team", MemberRole::Lead, &cmds).expect("command");
    assert!(command.contains("CLAUDECODE=1"));
    assert!(command.contains("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1"));
    assert!(command.contains("--team-name ledger-team"));
    assert!(command.contains("--agent-name team-lead"));
    assert!(command.contains("--agent-id team-lead@ledger-team"));
    assert!(command.contains("--agent-type orchestrator"));
}

// Resume always starts a fresh session — never uses --continue or resume --last.
// Multiple agents share the same project, so checkpoint-based resume would
// pick up another agent's checkpoint.
#[test]
fn build_resume_cli_launch_command_always_uses_fresh_session() {
    let cmds = crate::models::CliCommandSettings::default();
    let codex_agent = setup_config("builder", "codex", "gpt-5.3", "/tmp/project");

    let command = build_resume_cli_launch_command(
        &codex_agent,
        "architecture-final",
        MemberRole::Agent,
        &cmds,
    )
    .expect("command");
    assert_eq!(command, "codex --yolo -m 'gpt-5.3-codex'");
    assert!(!command.contains("resume"));
    assert!(!command.contains("--last"));

    let claude_agent = setup_config("team-lead", "claude", "opus", "/tmp/project");

    let command = build_resume_cli_launch_command(
        &claude_agent,
        "architecture-final",
        MemberRole::Lead,
        &cmds,
    )
    .expect("command");
    assert!(!command.contains("--continue"));
    assert!(command.contains("--agent-type orchestrator"));
    assert!(command.contains("--team-name architecture-final"));
}

#[test]
fn member_from_agent_setup_maps_role_template_context() {
    let mut setup = setup_config("codex-dev", "codex", "gpt-5.3", "/tmp/project");
    setup.description = Some("fallback instructions".to_string());
    setup.role_id = Some("codex-developer".to_string());
    setup.instructions = Some("template instructions".to_string());
    setup.behavioral_contract = Some(BehavioralContract {
        communication: vec!["post updates".to_string()],
        execution: vec!["ship patches".to_string()],
        escalation: vec!["raise blockers".to_string()],
    });
    setup.capabilities = Some(vec!["implementation".to_string()]);

    let member =
        member_from_agent_setup(&setup, MemberRole::Agent).expect("member mapping should work");

    assert_eq!(member.role_id.as_deref(), Some("codex-developer"));
    assert_eq!(
        member.instructions.as_deref(),
        Some("template instructions")
    );
    assert_eq!(
        member
            .behavioral_contract
            .as_ref()
            .map(|contract| contract.execution.clone())
            .unwrap_or_default(),
        vec!["ship patches".to_string()]
    );
    assert_eq!(
        member.capabilities.as_ref().cloned().unwrap_or_default(),
        vec!["implementation".to_string()]
    );
}

#[test]
fn initialize_pipeline_claude_template_agent_receives_role_context_message() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

    let mut claude_agent = setup_config("researcher", "claude", "claude-opus-4-6", "/tmp/research");
    claude_agent.role_id = Some("claude-researcher".to_string());
    claude_agent.instructions = Some("Investigate architecture tradeoffs.".to_string());
    claude_agent.behavioral_contract = Some(BehavioralContract {
        communication: vec!["post concise findings".to_string()],
        execution: vec!["run focused experiments".to_string()],
        escalation: vec!["escalate ambiguous requirements".to_string()],
    });
    claude_agent.capabilities = Some(vec!["analysis".to_string(), "research".to_string()]);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
        agents: vec![claude_agent],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let delivered = backend.delivered_requests();
    assert_eq!(
        delivered.len(),
        2,
        "lead + claude template agent should both receive onboarding"
    );
    // First delivery is to the lead (codex)
    match &delivered[0] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert_eq!(payload.member_name, "team-lead");
        }
        other => panic!("unexpected delivery payload for lead: {other:?}"),
    }
    // Second delivery is the claude agent with role context
    match &delivered[1] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert_eq!(payload.member_name, "researcher");
            assert!(payload.message.contains("[taurhaus] role_context"));
            assert!(payload.message.contains("Role: claude-researcher"));
            assert!(payload.message.contains("Capabilities:"));
            assert!(payload.message.contains("- analysis"));
            assert!(payload.message.contains("- research"));
            assert!(!payload.message.contains("mesh read --unread"));
        }
        other => panic!("unexpected delivery payload for agent: {other:?}"),
    }
}

#[test]
fn initialize_pipeline_claude_agent_without_role_context_stays_skipped() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
        agents: vec![setup_config(
            "researcher",
            "claude",
            "claude-opus-4-6",
            "/tmp/research",
        )],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);
    let delivered = backend.delivered_requests();
    assert_eq!(
        delivered.len(),
        1,
        "lead should receive onboarding even when claude agent has no role context"
    );
    match &delivered[0] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert_eq!(payload.member_name, "team-lead");
        }
        other => panic!("unexpected delivery payload: {other:?}"),
    }
}

#[test]
fn initialize_onboarding_entries_use_deferred_barrier_policy() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let orchestrator = new_orchestrator(&tmp, backend, runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
        agents: vec![setup_config("builder", "codex", "gpt-5.4", "/tmp/builder")],
    };

    let entries = orchestrator
        .prepare_initialize_onboarding_entries(&request)
        .expect("initialize onboarding entries");

    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|entry| { entry.policy == MemberActivationDeliveryPolicy::DeferredBarrier }));
}

// Regression: commit 3b17397 fixed a race where onboarding could reach a
// member before Mesh-sidecar activation had completed. Initialize must keep a
// full barrier: every member joins Mesh and starts its daemon before the first
// onboarding notice is delivered.
#[test]
fn initialize_onboarding_waits_for_member_activation_barrier() {
    let tmp = TempDir::new().expect("tempdir");
    let events = Arc::new(Mutex::new(Vec::new()));
    let backend = Arc::new(SequencedBackend::new(events.clone()));
    let runtime = Arc::new(SequencedRuntime::new(events.clone()));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.4", "/tmp/lead"),
        agents: vec![setup_config("builder", "codex", "gpt-5.4", "/tmp/builder")],
    };

    let report = orchestrator
        .initialize_team(&request)
        .expect("initialize report");
    assert!(report.failed_step.is_none(), "initialize should succeed");

    let events = events.lock().expect("timeline mutex").clone();
    let first_delivery = timeline_index(
        &events,
        DeliveryTimelineEvent::Deliver("team-lead".to_string()),
    );
    let last_activation = events
        .iter()
        .rposition(|event| {
            matches!(
                event,
                DeliveryTimelineEvent::JoinMesh(_) | DeliveryTimelineEvent::SpawnDaemon(_)
            )
        })
        .expect("activation events should exist");

    assert!(
        last_activation < first_delivery,
        "initialize should defer onboarding until all join/spawn work completes: {events:?}"
    );
    assert_eq!(
        events,
        vec![
            DeliveryTimelineEvent::JoinMesh("team-lead".to_string()),
            DeliveryTimelineEvent::JoinMesh("builder".to_string()),
            DeliveryTimelineEvent::SpawnDaemon("team-lead".to_string()),
            DeliveryTimelineEvent::SpawnDaemon("builder".to_string()),
            DeliveryTimelineEvent::Deliver("team-lead".to_string()),
            DeliveryTimelineEvent::Deliver("builder".to_string()),
        ]
    );
}

#[test]
fn initialize_pipeline_persists_codex_agent_session_id() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_detected_runtime_session(
        "test-pane-2",
        CliTool::Codex,
        Some("session-test-pane-2"),
        Some("/tmp/builder-session.jsonl"),
    );
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/lead"),
        agents: vec![setup_config("builder", "codex", "gpt-5.4", "/tmp/builder")],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let runtime_record =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
    assert_eq!(
        runtime_record.session_id.as_deref(),
        Some("session-test-pane-2")
    );
    assert_eq!(
        runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/builder-session.jsonl"))
    );
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "test-pane-2" && *cli_tool == CliTool::Codex
    )));
}

#[test]
fn initialize_pipeline_per_project_layout_reuses_anchor_pane() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/project"),
        agents: vec![
            setup_config("dev-1", "codex", "gpt-5.4", "/tmp/project"),
            setup_config("dev-2", "codex", "gpt-5.4", "/tmp/project"),
            setup_config("architect-1", "codex", "gpt-5.4", "/tmp/project"),
        ],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "per_project",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let pane_calls: Vec<_> = runtime
        .calls()
        .into_iter()
        .filter(|call| {
            matches!(
                call,
                RuntimeCall::CreatePane { .. } | RuntimeCall::CreatePaneInTarget { .. }
            )
        })
        .collect();
    assert_eq!(
        pane_calls,
        vec![
            RuntimeCall::CreatePane {
                project_id: "/tmp/project".to_string(),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: "/tmp/project".to_string(),
                target_pane: "test-pane-1".to_string(),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: "/tmp/project".to_string(),
                target_pane: "test-pane-1".to_string(),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: "/tmp/project".to_string(),
                target_pane: "test-pane-1".to_string(),
            },
        ]
    );
}

#[test]
fn initialize_pipeline_retries_transient_send_keys_failure_for_codex_agent() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_send_keys_failures(
        "test-pane-2",
        1,
        "tmux command failed (wsl -e tmux send-keys -t %141 -l codex --yolo -m 'gpt-5.4'): ",
    );
    runtime.set_detected_runtime_session(
        "test-pane-2",
        CliTool::Codex,
        Some("session-test-pane-2"),
        Some("/tmp/builder-session.jsonl"),
    );
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/lead"),
        agents: vec![setup_config("builder", "codex", "gpt-5.4", "/tmp/builder")],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let send_attempts = runtime
        .calls()
        .iter()
        .filter(|call| {
            matches!(
                call,
                RuntimeCall::SendKeys { pane_id, .. } if pane_id == "test-pane-2"
            )
        })
        .count();
    assert_eq!(send_attempts, 2, "codex launch should retry once");
}

#[test]
fn initialize_pipeline_reports_pane_diagnostics_after_send_keys_retries_exhaust() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_send_keys_failures(
        "test-pane-2",
        3,
        "tmux command failed (wsl -e tmux send-keys -t %141 -l codex --yolo -m 'gpt-5.4'): ",
    );
    runtime.set_pane_exists("test-pane-2", true);
    runtime.set_pane_dead("test-pane-2", false);
    runtime.set_pane_shell("test-pane-2", true);
    runtime.set_pane_current_command("test-pane-2", Some("zsh"));
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/lead"),
        agents: vec![setup_config("builder", "codex", "gpt-5.4", "/tmp/builder")],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step.as_deref(), Some("create_panes"));
    assert_eq!(
        report.succeeded_steps,
        vec!["validate_configuration", "create_team", "add_lead",]
    );
    assert!(report
        .message
        .contains("pane=test-pane-2 exists=true dead=false shell=false command=zsh"));
}

#[test]
fn initialize_pipeline_persists_claude_agent_session_id() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_detected_runtime_session(
        "test-pane-2",
        CliTool::Claude,
        Some("session-test-pane-2"),
        Some("/tmp/researcher-session.jsonl"),
    );
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
        agents: vec![setup_config(
            "researcher",
            "claude",
            "claude-opus-4-6",
            "/tmp/research",
        )],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let runtime_record =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "researcher").expect("runtime");
    assert_eq!(
        runtime_record.session_id.as_deref(),
        Some("session-test-pane-2")
    );
    assert_eq!(
        runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/researcher-session.jsonl"))
    );
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "test-pane-2" && *cli_tool == CliTool::Claude
    )));
}

#[test]
fn initialize_pipeline_seeds_full_roster_before_reload_dependent_steps() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Review pipeline".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.3", "/tmp/lead"),
        agents: vec![
            setup_config("builder", "codex", "gpt-5.4", "/tmp/builder"),
            setup_config("reviewer", "claude", "claude-opus-4-6", "/tmp/reviewer"),
        ],
    };

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &request,
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert_eq!(report.failed_step, None);

    let config = TeamConfigStore::load(tmp.path(), "architecture-final").expect("team config");
    assert_eq!(config.members.len(), 3);
    assert!(config
        .members
        .iter()
        .any(|member| member.name == "team-lead"));
    assert!(config.members.iter().any(|member| member.name == "builder"));
    assert!(config
        .members
        .iter()
        .any(|member| member.name == "reviewer"));

    let lead_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead").expect("runtime");
    assert_eq!(lead_runtime.cli_tool, Some(CliTool::Codex));
    assert_eq!(
        lead_runtime.project_path.as_deref(),
        Some(std::path::Path::new("/tmp/lead"))
    );

    let reviewer_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "reviewer").expect("runtime");
    assert_eq!(reviewer_runtime.cli_tool, Some(CliTool::Claude));
    assert_eq!(
        reviewer_runtime.project_path.as_deref(),
        Some(std::path::Path::new("/tmp/reviewer"))
    );
}

#[test]
fn load_resume_member_state_preserves_role_template_context() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "builder".to_string(),
                role: MemberRole::Agent,
                role_id: Some("codex-developer".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Implement safely".to_string()),
                behavioral_contract: Some(BehavioralContract {
                    communication: vec!["post updates".to_string()],
                    execution: vec!["ship patches".to_string()],
                    escalation: vec!["raise blockers".to_string()],
                }),
                quality_gates: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: Some(vec!["implementation".to_string(), "testing".to_string()]),
                project_path: PathBuf::from("/tmp/builder"),
                cli_tool: CliTool::Codex,
            },
        )
        .expect("add member");

    let request = ResumeMemberRequest {
        team_name: "architecture-final".to_string(),
        member_name: "builder".to_string(),
    };

    let (loaded_member, _runtime_record, lead_name) = orchestrator
        .load_resume_member_state(&request)
        .expect("resume state should load");

    assert_eq!(lead_name, "team-lead");
    assert_eq!(loaded_member.role_id.as_deref(), Some("codex-developer"));
    assert_eq!(
        loaded_member.instructions.as_deref(),
        Some("Implement safely")
    );
    assert_eq!(
        loaded_member
            .behavioral_contract
            .as_ref()
            .map(|contract| contract.execution.clone())
            .unwrap_or_default(),
        vec!["ship patches".to_string()]
    );
    assert_eq!(
        loaded_member
            .capabilities
            .as_ref()
            .cloned()
            .unwrap_or_default(),
        vec!["implementation".to_string(), "testing".to_string()]
    );
}

#[test]
fn resume_pipeline_claude_lead_skips_mesh_daemon_but_receives_onboarding() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");

    let mut lead_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead").expect("runtime");
    lead_runtime.pane_id = Some("%9".to_string());
    lead_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "team-lead", &lead_runtime)
        .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "team-lead")
        .expect("resume report");

    assert!(report.resumed);
    assert!(report.reused_pane);
    assert_eq!(report.failed_step, None);
    let join_step = report
        .steps
        .iter()
        .find(|step| step.step == "join_mesh")
        .expect("join step");
    assert_eq!(join_step.status, StepStatus::Succeeded);
    assert!(join_step
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("not required"));
    let daemon_step = report
        .steps
        .iter()
        .find(|step| step.step == "start_daemon")
        .expect("daemon step");
    assert!(daemon_step
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("not required"));
    let onboarding_step = report
        .steps
        .iter()
        .find(|step| step.step == "send_onboarding")
        .expect("onboarding step");
    assert_eq!(
        onboarding_step.status,
        StepStatus::Succeeded,
        "claude lead should receive onboarding"
    );

    let calls = runtime.calls();
    let launch = calls
        .iter()
        .find_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .expect("launch command");
    assert!(!launch.contains("--continue"));
    assert!(launch.contains("--agent-type orchestrator"));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert_eq!(
        backend.call_counts().1,
        1,
        "lead should receive onboarding delivery"
    );
}

#[test]
fn resume_pipeline_claude_member_sends_onboarding_and_skips_mesh_daemon() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "researcher",
                MemberRole::Agent,
                CliTool::Claude,
                "/tmp/research",
            ),
        )
        .expect("add member");

    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "researcher").expect("runtime");
    member_runtime.pane_id = Some("%10".to_string());
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "researcher",
        &member_runtime,
    )
    .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "researcher")
        .expect("resume report");

    assert!(report.resumed);
    let calls = runtime.calls();
    let launch = calls
        .iter()
        .find_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .expect("launch command");
    assert!(launch.contains("--agent-type general-purpose"));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert_eq!(backend.call_counts().1, 1, "onboarding should be delivered");
}

#[test]
fn resume_onboarding_entry_uses_immediate_policy() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "researcher".to_string(),
                role: MemberRole::Agent,
                role_id: Some("claude-researcher".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Investigate tradeoffs.".to_string()),
                behavioral_contract: None,
                quality_gates: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                project_path: PathBuf::from("/tmp/research"),
                cli_tool: CliTool::Claude,
            },
        )
        .expect("add member");

    let request = ResumeMemberRequest {
        team_name: "architecture-final".to_string(),
        member_name: "researcher".to_string(),
    };
    let (member, _runtime, lead_name) = orchestrator
        .load_resume_member_state(&request)
        .expect("load resume state");

    let entry = orchestrator
        .prepare_resume_onboarding_entry(&request, &member, &lead_name)
        .expect("resume onboarding entry");

    assert_eq!(entry.policy, MemberActivationDeliveryPolicy::Immediate);
}

// Regression: commit 3b17397 fixed the resume race by delivering onboarding as
// soon as each member is individually ready, rather than deferring delivery
// behind the full-team resume loop.
#[test]
fn resume_onboarding_delivers_immediately_per_member() {
    let tmp = TempDir::new().expect("tempdir");
    let events = Arc::new(Mutex::new(Vec::new()));
    let backend = Arc::new(SequencedBackend::new(events.clone()));
    let runtime = Arc::new(SequencedRuntime::new(events.clone()));
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add builder");
    mark_member_offline(&tmp, "architecture-final", "team-lead", "%11", None);
    mark_member_offline(&tmp, "architecture-final", "builder", "%12", Some(55));

    let lead_report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "architecture-final".to_string(),
                member_name: "team-lead".to_string(),
            },
            &CliCommandSettings::default(),
        )
        .expect("resume lead");
    assert!(
        lead_report.resumed,
        "lead resume should succeed: {lead_report:?}"
    );

    let builder_report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "architecture-final".to_string(),
                member_name: "builder".to_string(),
            },
            &CliCommandSettings::default(),
        )
        .expect("resume builder");
    assert!(
        builder_report.resumed,
        "builder resume should succeed: {builder_report:?}"
    );

    let events = events.lock().expect("timeline mutex").clone();
    let lead_delivery = timeline_index(
        &events,
        DeliveryTimelineEvent::Deliver("team-lead".to_string()),
    );
    let builder_join = timeline_index(
        &events,
        DeliveryTimelineEvent::JoinMesh("builder".to_string()),
    );
    let builder_spawn = timeline_index(
        &events,
        DeliveryTimelineEvent::SpawnDaemon("builder".to_string()),
    );
    let builder_delivery = timeline_index(
        &events,
        DeliveryTimelineEvent::Deliver("builder".to_string()),
    );

    assert!(
        lead_delivery < builder_join,
        "resume should deliver the first member onboarding before the next member activation starts: {events:?}"
    );
    assert!(
        builder_join < builder_spawn && builder_spawn < builder_delivery,
        "builder onboarding should follow builder activation, not precede it: {events:?}"
    );
}

#[test]
fn resume_pipeline_claude_member_with_role_context_sends_role_context_message() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "researcher".to_string(),
                role: MemberRole::Agent,
                role_id: Some("claude-researcher".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Investigate tradeoffs and summarize findings.".to_string()),
                behavioral_contract: Some(BehavioralContract {
                    communication: vec!["post concise updates".to_string()],
                    execution: vec!["run experiments".to_string()],
                    escalation: vec!["escalate blockers immediately".to_string()],
                }),
                quality_gates: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: Some(vec!["analysis".to_string()]),
                project_path: PathBuf::from("/tmp/research"),
                cli_tool: CliTool::Claude,
            },
        )
        .expect("add member");

    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "researcher").expect("runtime");
    member_runtime.pane_id = Some("%10".to_string());
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "researcher",
        &member_runtime,
    )
    .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "researcher")
        .expect("resume report");

    assert!(report.resumed);
    let delivered = backend.delivered_requests();
    assert_eq!(delivered.len(), 1);
    match &delivered[0] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert!(payload.message.contains("[taurhaus] role_context"));
            assert!(payload.message.contains("Role: claude-researcher"));
            assert!(payload.message.contains("Capabilities:"));
            assert!(payload.message.contains("- analysis"));
        }
        other => panic!("unexpected delivery payload: {other:?}"),
    }
}

#[test]
fn resume_pipeline_non_claude_reuses_pane_but_starts_fresh_session_and_updates_runtime() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_detected_runtime_session(
        "%11",
        CliTool::Codex,
        Some("session-%11"),
        Some("/tmp/builder-resume.jsonl"),
    );
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add member");

    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
    member_runtime.pane_id = Some("%11".to_string());
    member_runtime.daemon_pid = Some(55);
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
        .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "builder")
        .expect("resume report");
    assert!(report.resumed);
    assert!(report.reused_pane);

    let calls = runtime.calls();
    let launch = calls
        .iter()
        .find_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .expect("launch command");
    assert_eq!(launch, "codex --yolo");
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::JoinMesh { .. })));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 55)));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert_eq!(backend.call_counts().1, 1, "onboarding should be delivered");

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder")
        .expect("updated runtime");
    assert_eq!(updated.pane_id.as_deref(), Some("%11"));
    assert_eq!(updated.session_id.as_deref(), Some("session-%11"));
    assert_eq!(
        updated.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/builder-resume.jsonl"))
    );
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.daemon_pid, Some(10000));
    assert!(updated.attached_at.is_some());
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "%11" && *cli_tool == CliTool::Codex
    )));
}

#[test]
fn resume_pipeline_non_claude_lead_uses_sidecar_lifecycle_with_session_capture() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("session-%21"),
        Some("/tmp/team-lead-resume.jsonl"),
    );
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Codex,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");

    let mut lead_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead").expect("runtime");
    lead_runtime.pane_id = Some("%21".to_string());
    lead_runtime.daemon_pid = Some(91);
    lead_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "team-lead", &lead_runtime)
        .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "team-lead")
        .expect("resume report");
    assert!(report.resumed);
    assert!(report.reused_pane);

    let calls = runtime.calls();
    let launch = calls
        .iter()
        .find_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .expect("launch command");
    assert_eq!(launch, "codex --yolo");
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "team-lead")));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { member_name, .. } if member_name == "team-lead")));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 91)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "%21" && *cli_tool == CliTool::Codex
    )));
    assert_eq!(
        backend.call_counts().1,
        1,
        "lead should still receive onboarding"
    );

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead")
        .expect("updated runtime");
    assert_eq!(updated.pane_id.as_deref(), Some("%21"));
    assert_eq!(updated.session_id.as_deref(), Some("session-%21"));
    assert_eq!(
        updated.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/team-lead-resume.jsonl"))
    );
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.daemon_pid, Some(10000));
    assert!(updated.attached_at.is_some());
}

#[test]
fn resume_pipeline_recreates_mismatched_pane_and_syncs_config_tmux_pane_id() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add member");

    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
    member_runtime.pane_id = Some("%77".to_string());
    member_runtime.daemon_pid = Some(55);
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
        .expect("save runtime");

    runtime.set_pane_exists("%77", true);
    runtime.set_pane_dead("%77", false);
    runtime.set_pane_ownership("%77", false);

    let report = orchestrator
        .resume_member("architecture-final", "builder")
        .expect("resume report");
    assert!(report.resumed);
    assert!(!report.reused_pane);
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-1"));

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder")
        .expect("updated runtime");
    assert_eq!(updated.pane_id.as_deref(), Some("test-pane-1"));
    assert_eq!(updated.daemon_pid, Some(10000));

    let raw_config = fs::read_to_string(tmp.path().join("architecture-final").join("config.json"))
        .expect("read config");
    let config: serde_json::Value = serde_json::from_str(&raw_config).expect("parse config");
    let builder = config["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|member| member["name"].as_str() == Some("builder"))
        .expect("builder entry");
    assert_eq!(builder["tmuxPaneId"].as_str(), Some("test-pane-1"));
}

#[test]
fn resume_failure_cleans_created_resources_and_keeps_member_config() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    backend.set_deliver_error(CoordinationError::Backend("delivery failed".to_string()));
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add member");

    // Existing pane should be reused; rollback must not kill it.
    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
    member_runtime.pane_id = Some("%77".to_string());
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
        .expect("save runtime");

    let report = orchestrator
        .resume_member("architecture-final", "builder")
        .expect("resume report");
    assert!(!report.resumed);
    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));

    let config = TeamConfigStore::load(tmp.path(), "architecture-final").expect("team config");
    assert!(config.members.iter().any(|entry| entry.name == "builder"));

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 10000)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::ClearDaemonPidFile { team_name, member_name }
            if team_name == "architecture-final" && member_name == "builder"
    )));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%77")),
        "reused pane must not be killed during rollback"
    );
}

#[test]
fn add_agent_failure_clears_daemon_pid_file() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    backend.set_deliver_error(CoordinationError::Backend("delivery failed".to_string()));
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");

    let report = orchestrator
        .add_agent_to_team(&AddAgentRequest {
            team_name: "architecture-final".to_string(),
            agent: setup_config("builder", "codex", "gpt-5.4", "/tmp/builder"),
        })
        .expect("add-agent report");
    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 10000)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::ClearDaemonPidFile { team_name, member_name }
            if team_name == "architecture-final" && member_name == "builder"
    )));

    let config = TeamConfigStore::load(tmp.path(), "architecture-final").expect("team config");
    assert!(
        !config.members.iter().any(|entry| entry.name == "builder"),
        "failed hot-add should not leave the member in config"
    );
}

#[test]
fn add_agent_onboarding_entry_uses_immediate_policy() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    orchestrator
        .create_team("architecture-final", None)
        .expect("create team");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");

    let entry = orchestrator
        .prepare_add_agent_onboarding_entry(&AddAgentRequest {
            team_name: "architecture-final".to_string(),
            agent: setup_config("builder", "codex", "gpt-5.4", "/tmp/builder"),
        })
        .expect("prepare add-agent onboarding")
        .expect("add-agent onboarding entry");

    assert_eq!(entry.policy, MemberActivationDeliveryPolicy::Immediate);
}
