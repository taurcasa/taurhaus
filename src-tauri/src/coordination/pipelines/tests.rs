use super::*;
use fs2::FileExt;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use taurhaus_lib::logging::{install_global_sink, LogFileState};
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
use crate::coordination::stores::lock::TargetFileLock;
use crate::coordination::stores::{
    MemberRuntimeSnapshot, MemberRuntimeStore, RuntimeCommitOutcome, TeamConfigStore,
};
use crate::coordination::task_effort::EffortPassScope;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::{spec, CliTool};
use crate::templates::storage::TemplateStore;
use crate::templates::types::BehavioralContract;

#[test]
fn optional_pane_identity_capture_failure_does_not_abort_activation() {
    // Regression: aecc8ac made optional pane identity capture fatal after the
    // member CLI had already launched, causing cleanup to kill a working pane.
    let runtime = RecordingCoordinationRuntime::default();
    runtime.set_pane_exists("%gone", false);
    let mut state = MemberActivationRuntimeState::default();

    capture_member_pane_identity(&runtime, "%gone", &mut state)
        .expect("optional identity capture should fail soft");

    assert_eq!(state.pane_pid, None);
    assert_eq!(state.pane_start_time, None);
}

#[test]
fn dead_pane_identity_capture_erases_previous_identity() {
    // Regression: aecc8ac made identity capture fail-soft but collapsed a
    // confirmed dead pane and a transient probe failure into the same state.
    let runtime = RecordingCoordinationRuntime::default();
    runtime.set_pane_exists("%dead", true);
    runtime.set_pane_dead("%dead", true);
    let mut state = MemberActivationRuntimeState {
        pane_pid: Some(7001),
        pane_start_time: Some(1_755_000_007),
        ..Default::default()
    };

    capture_member_pane_identity(&runtime, "%dead", &mut state)
        .expect("dead-pane identity capture should fail soft");

    assert_eq!(state.pane_pid, None);
    assert_eq!(state.pane_start_time, None);
}

#[test]
fn pane_identity_probe_failure_preserves_previous_identity_and_logs() {
    // Regression: aecc8ac erased durable pane identity on a transient tmux
    // probe error, permanently weakening later ownership checks to path-only.
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("pane-probe.log.jsonl");
    let log_state = LogFileState::new(log_path.clone()).expect("log state");
    install_global_sink(&log_state);
    let runtime = RecordingCoordinationRuntime::default();
    runtime.set_live_pane_failure("%reused", "transient tmux failure");
    let mut state = MemberActivationRuntimeState {
        pane_pid: Some(7001),
        pane_start_time: Some(1_755_000_007),
        ..Default::default()
    };

    capture_member_pane_identity(&runtime, "%reused", &mut state)
        .expect("probe failure should fail soft");

    assert_eq!(state.pane_pid, Some(7001));
    assert_eq!(state.pane_start_time, Some(1_755_000_007));
    let contents =
        wait_for_pipeline_log_contains(&log_path, "\"event\":\"coordination.pane.probe_failed\"");
    assert!(contents.contains("\"pane_id\":\"%reused\""));
}

#[test]
fn only_a_reused_pane_inherits_the_previous_runtime_identity() {
    // Regression: aecc8ac did not distinguish a reused pane from a newly
    // created pane when deciding which identity a failed capture may retain.
    let mut previous = default_runtime_record("builder");
    previous.pane_pid = Some(7001);
    previous.pane_start_time = Some(1_755_000_007);
    let reused = crate::coordination::runtime::PaneResolution {
        pane_id: "%reused".to_string(),
        reused_pane: true,
        created_new_pane: false,
        foreign_pane_reason: None,
    };
    let mut state = MemberActivationRuntimeState::default();

    seed_member_pane_identity_for_resolution(&mut state, &previous, &reused);
    assert_eq!(state.pane_pid, previous.pane_pid);
    assert_eq!(state.pane_start_time, previous.pane_start_time);

    let created = crate::coordination::runtime::PaneResolution {
        pane_id: "%new".to_string(),
        reused_pane: false,
        created_new_pane: true,
        foreign_pane_reason: None,
    };
    seed_member_pane_identity_for_resolution(&mut state, &previous, &created);
    assert_eq!(state.pane_pid, None);
    assert_eq!(state.pane_start_time, None);
}

fn wait_for_pipeline_log_contains(path: &std::path::Path, needle: &str) -> String {
    for _ in 0..50 {
        if let Ok(contents) = fs::read_to_string(path) {
            if contents.contains(needle) {
                return contents;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    fs::read_to_string(path).unwrap_or_default()
}

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
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )?;
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
        handoff_expectations: None,
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
        model: None,
        reasoning_effort: None,
        project_path: PathBuf::from(project),
        cli_tool,
        extra: Default::default(),
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
        reasoning_effort: None,
        handoff_expectations: None,
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

fn config_member_tmux_pane_id(tmp: &TempDir, team_name: &str, member_name: &str) -> Option<String> {
    let raw_config =
        fs::read_to_string(tmp.path().join(team_name).join("config.json")).expect("read config");
    let config: serde_json::Value = serde_json::from_str(&raw_config).expect("parse config");
    config["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|member| member["name"].as_str() == Some(member_name))
        .and_then(|member| member["tmuxPaneId"].as_str())
        .map(str::to_string)
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
                ..Default::default()
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
fn activation_runtime_commit_skips_a_stale_dependency_snapshot() {
    // Regression: 366f4b7 left activation's load-to-save window outside any
    // shared critical section, so a concurrent liveness writer could be
    // overwritten by a patch based on the runtime record from before it.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let team_name = "activation-stale-snapshot";
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

    let original = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    let expected = MemberRuntimeSnapshot::capture(&original);
    let mut concurrent = original;
    concurrent.pane_id = Some("%winner".to_string());
    concurrent.pane_pid = Some(9001);
    concurrent.pane_start_time = Some(1_755_000_009);
    concurrent.session_id = Some("session-winner".to_string());
    concurrent.daemon_pid = Some(9002);
    concurrent.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &concurrent)
        .expect("concurrent liveness save");

    let member_config = setup_config(member_name, "codex", "gpt-5.4", "/tmp/builder");
    let context = MemberActivationContext::for_initialize_member(
        team_name,
        "team-lead",
        &member_config,
        MemberRole::Agent,
    )
    .expect("context");
    let outcome = orchestrator
        .commit_member_runtime_if_unchanged(
            &context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%stale".to_string())),
                session_id: Some(Some("session-stale".to_string())),
                daemon_pid: Some(Some(8000)),
                health: Some(HealthState::SessionDead),
                ..Default::default()
            },
            &expected,
        )
        .expect("stale activation commit is handled");

    assert!(matches!(outcome, RuntimeCommitOutcome::Skipped { .. }));
    assert_eq!(
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("winning runtime"),
        concurrent
    );
}

#[test]
fn skipped_activation_runtime_commit_is_reported_as_a_conflict() {
    // Regression: 0dc5fcae swallowed RuntimeCommitOutcome::Skipped in
    // commit_member_runtime, so resume reported a launch whose runtime state
    // was never recorded.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let team_name = "activation-skipped-conflict";
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

    let original = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    let mut concurrent = original;
    concurrent.pane_id = Some("%winner".to_string());
    concurrent.pane_pid = Some(9001);
    concurrent.pane_start_time = Some(1_755_000_009);
    concurrent.session_id = Some("session-winner".to_string());
    concurrent.daemon_pid = Some(9002);
    concurrent.health = HealthState::Healthy;

    let runtime_path = tmp
        .path()
        .join(team_name)
        .join("runtime")
        .join(format!("{member_name}.json"));
    let target_lock = TargetFileLock::acquire_if_exists(&runtime_path)
        .expect("acquire target lock")
        .expect("runtime target exists");
    let member_config = setup_config(member_name, "codex", "gpt-5.4", "/tmp/builder");
    let context = MemberActivationContext::for_initialize_member(
        team_name,
        "team-lead",
        &member_config,
        MemberRole::Agent,
    )
    .expect("context");
    let commit = std::thread::spawn(move || {
        orchestrator.commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%stale".to_string())),
                session_id: Some(Some("session-stale".to_string())),
                daemon_pid: Some(Some(8000)),
                health: Some(HealthState::SessionDead),
                ..Default::default()
            },
        )
    });

    let team_lock_path = tmp.path().join(team_name).join(".lock");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let lock_file = File::open(&team_lock_path).expect("open team lock");
        if lock_file.try_lock_exclusive().is_err() {
            break;
        }
        FileExt::unlock(&lock_file).expect("release probe lock");
        assert!(
            std::time::Instant::now() < deadline,
            "activation commit did not reach its target-file wait"
        );
        std::thread::yield_now();
    }

    fs::write(
        &runtime_path,
        serde_json::to_string_pretty(&concurrent).expect("serialize concurrent runtime"),
    )
    .expect("write concurrent runtime");
    drop(target_lock);

    let error = commit
        .join()
        .expect("activation commit thread")
        .expect_err("a skipped activation commit must not report success");
    assert!(matches!(error, CoordinationError::Conflict(_)));
    assert_eq!(
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("winning runtime"),
        concurrent
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
                ..Default::default()
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
fn finalized_runtime_commit_preserves_mesh_owned_member_fields() {
    // Regression: mesh-findings P1; sync_team_config_metadata erased the live
    // controlAuthTokenHash written by mesh while refreshing tmux metadata.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let team_name = "metadata-round-trip";
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

    let path = tmp.path().join(team_name).join("config.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read config"))
            .expect("parse config");
    value["members"][0]["controlAuthTokenHash"] =
        serde_json::Value::String("sha256:mesh-token".to_string());
    value["members"][0]["statusState"] = serde_json::Value::String("working".to_string());
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).expect("serialize injected config"),
    )
    .expect("write injected config");

    let member_config = setup_config(member_name, "codex", "gpt-5.4", "/tmp/builder");
    let context = MemberActivationContext::for_add_agent(team_name, "team-lead", &member_config)
        .expect("context");
    orchestrator
        .commit_member_runtime(
            &context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%22".to_string())),
                health: Some(HealthState::Healthy),
                ..Default::default()
            },
        )
        .expect("finalized runtime commit");

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read saved config"))
            .expect("parse saved config");
    assert_eq!(
        saved["members"][0]["controlAuthTokenHash"],
        "sha256:mesh-token"
    );
    assert_eq!(saved["members"][0]["statusState"], "working");
}

#[test]
fn shared_stage_session_capture_persists_runtime_identity_across_wrappers() {
    // Regression: mesh-findings P3, tmux reused pane ids; daemons for
    // taurrust/gotaurus/espn pointed at claude panes.
    let cli_commands = CliCommandSettings::default();

    let initialize_tmp = TempDir::new().expect("tempdir");
    let initialize_backend = Arc::new(FakeBackend::default());
    let initialize_runtime = Arc::new(RecordingCoordinationRuntime::default());
    initialize_runtime.set_detected_runtime_session(
        "test-pane-1",
        CliTool::Codex,
        Some("session-initialize"),
        Some("/tmp/initialize.jsonl"),
    );
    let mut initialize_orchestrator =
        new_orchestrator(&initialize_tmp, initialize_backend, initialize_runtime);
    let initialize_report = initialize_orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &InitializeTeamRequest {
                team_name: "initialize-team".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config("team-lead", "codex", "gpt-5.4", "/tmp/lead"),
                agents: vec![],
            },
            &cli_commands,
            "new_window",
        )
        .expect("initialize report");
    assert!(
        initialize_report.failed_step.is_none(),
        "initialize should succeed: {initialize_report:?}"
    );
    let initialize_runtime_record =
        MemberRuntimeStore::load(initialize_tmp.path(), "initialize-team", "team-lead")
            .expect("initialize runtime");
    assert_eq!(
        initialize_runtime_record.session_id.as_deref(),
        Some("session-initialize")
    );
    assert_eq!(
        initialize_runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/initialize.jsonl"))
    );
    assert_eq!(initialize_runtime_record.health, HealthState::Healthy);
    assert_eq!(initialize_runtime_record.pane_pid, Some(1001));
    assert_eq!(
        initialize_runtime_record.pane_start_time,
        Some(1_755_000_001)
    );

    let add_agent_tmp = TempDir::new().expect("tempdir");
    let add_agent_backend = Arc::new(FakeBackend::default());
    let add_agent_runtime = Arc::new(RecordingCoordinationRuntime::default());
    add_agent_runtime.set_detected_runtime_session(
        "test-pane-1",
        CliTool::Codex,
        Some("session-add-agent"),
        Some("/tmp/add-agent.jsonl"),
    );
    let mut add_agent_orchestrator =
        new_orchestrator(&add_agent_tmp, add_agent_backend, add_agent_runtime);
    add_agent_orchestrator
        .create_team("add-agent-team", None)
        .expect("create team");
    add_agent_orchestrator
        .add_member(
            "add-agent-team",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    let add_agent_report = add_agent_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-team".to_string(),
                agent: setup_config("builder", "codex", "gpt-5.4", "/tmp/builder"),
            },
            &cli_commands,
            "new_window",
        )
        .expect("add-agent report");
    assert!(
        add_agent_report.failed_step.is_none(),
        "add-agent should succeed: {add_agent_report:?}"
    );
    let add_agent_runtime_record =
        MemberRuntimeStore::load(add_agent_tmp.path(), "add-agent-team", "builder")
            .expect("add-agent runtime");
    assert_eq!(
        add_agent_runtime_record.session_id.as_deref(),
        Some("session-add-agent")
    );
    assert_eq!(
        add_agent_runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/add-agent.jsonl"))
    );
    assert_eq!(add_agent_runtime_record.health, HealthState::Healthy);
    assert_eq!(add_agent_runtime_record.pane_pid, Some(1001));
    assert_eq!(
        add_agent_runtime_record.pane_start_time,
        Some(1_755_000_001)
    );

    let resume_tmp = TempDir::new().expect("tempdir");
    let resume_backend = Arc::new(FakeBackend::default());
    let resume_runtime = Arc::new(RecordingCoordinationRuntime::default());
    resume_runtime.set_detected_runtime_session(
        "%11",
        CliTool::Codex,
        Some("session-resume"),
        Some("/tmp/resume.jsonl"),
    );
    resume_runtime.set_pane_identity("%11", Some(2011), Some(1_755_000_011));
    let mut resume_orchestrator = new_orchestrator(&resume_tmp, resume_backend, resume_runtime);
    resume_orchestrator
        .create_team("resume-team", None)
        .expect("create team");
    resume_orchestrator
        .add_member(
            "resume-team",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    resume_orchestrator
        .add_member(
            "resume-team",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add builder");
    mark_member_offline(&resume_tmp, "resume-team", "builder", "%11", Some(55));
    let resume_report = resume_orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "resume-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &cli_commands,
        )
        .expect("resume report");
    assert!(
        resume_report.resumed,
        "resume should succeed: {resume_report:?}"
    );
    let resume_runtime_record =
        MemberRuntimeStore::load(resume_tmp.path(), "resume-team", "builder")
            .expect("resume runtime");
    assert_eq!(
        resume_runtime_record.session_id.as_deref(),
        Some("session-resume")
    );
    assert_eq!(
        resume_runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/resume.jsonl"))
    );
    assert_eq!(resume_runtime_record.health, HealthState::Healthy);
    assert_eq!(resume_runtime_record.pane_pid, Some(2011));
    assert_eq!(resume_runtime_record.pane_start_time, Some(1_755_000_011));
}

#[test]
fn shared_stage_mesh_join_and_daemon_rules_match_expected_wrapper_differences() {
    let initialize_tmp = TempDir::new().expect("tempdir");
    let initialize_backend = Arc::new(FakeBackend::default());
    let initialize_runtime = Arc::new(RecordingCoordinationRuntime::default());
    initialize_runtime.set_mesh_join_teams_dir(initialize_tmp.path());
    let credential_dir = initialize_tmp
        .path()
        .join("initialize-claude")
        .join("state")
        .join("control_auth");
    fs::create_dir_all(&credential_dir).expect("credential dir");
    fs::write(
        credential_dir.join("team-lead.json"),
        r#"{"name":"team-lead","token":"test-token"}"#,
    )
    .expect("lead credential");
    let mut initialize_orchestrator = new_orchestrator(
        &initialize_tmp,
        initialize_backend,
        initialize_runtime.clone(),
    );
    let initialize_claude_report = initialize_orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &InitializeTeamRequest {
                team_name: "initialize-claude".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/lead"),
                agents: vec![],
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize claude report");
    assert!(
        initialize_claude_report.failed_step.is_none(),
        "initialize Claude should succeed: {initialize_claude_report:?}"
    );
    // Regression: commit 76c284e made Claude members inbox-native but also
    // skipped the Claude lead's mesh join, so team-daemon auth never existed.
    let initialize_claude_calls = initialize_runtime.calls();
    let lead_join_indexes = initialize_claude_calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "team-lead")
                .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lead_join_indexes.len(),
        1,
        "Claude lead joins mesh exactly once"
    );
    let lead_join = initialize_claude_calls
        .iter()
        .find(|call| matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "team-lead"))
        .expect("Claude lead join call");
    assert!(matches!(
        lead_join,
        RuntimeCall::JoinMesh {
            member_type,
            model,
            claude_dir,
            ..
        } if member_type == "lead"
            && model == "claude-opus-4-6"
            && claude_dir == &crate::coordination::runtime::resolve_mesh_cli_claude_dir_arg()
                .expect("Claude config dir")
    ));
    assert!(
        initialize_claude_calls
            .iter()
            .all(|call| !matches!(call, RuntimeCall::SpawnDaemon { .. })),
        "Claude members stay member-daemon-less"
    );
    let team_daemon_index = initialize_claude_calls
        .iter()
        .position(|call| matches!(call, RuntimeCall::SpawnTeamDaemon { operator_name, .. } if operator_name == "team-lead"))
        .expect("team daemon start");
    assert!(lead_join_indexes[0] < team_daemon_index);

    let initialize_sidecar_tmp = TempDir::new().expect("tempdir");
    let initialize_sidecar_backend = Arc::new(FakeBackend::default());
    let initialize_sidecar_runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut initialize_sidecar_orchestrator = new_orchestrator(
        &initialize_sidecar_tmp,
        initialize_sidecar_backend,
        initialize_sidecar_runtime.clone(),
    );
    let initialize_sidecar_report = initialize_sidecar_orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &InitializeTeamRequest {
                team_name: "initialize-sidecar".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config("team-lead", "codex", "gpt-5.4", "/tmp/lead"),
                agents: vec![],
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize sidecar report");
    assert!(
        initialize_sidecar_report.failed_step.is_none(),
        "initialize Codex should succeed: {initialize_sidecar_report:?}"
    );
    let initialize_sidecar_calls = initialize_sidecar_runtime.calls();
    assert!(initialize_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "team-lead")
    ));
    assert!(initialize_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::SpawnDaemon { member_name, .. } if member_name == "team-lead")
    ));
    assert!(!initialize_sidecar_calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { .. })));

    let add_agent_tmp = TempDir::new().expect("tempdir");
    let add_agent_backend = Arc::new(FakeBackend::default());
    let add_agent_runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut add_agent_orchestrator =
        new_orchestrator(&add_agent_tmp, add_agent_backend, add_agent_runtime.clone());
    add_agent_orchestrator
        .create_team("add-agent-team", None)
        .expect("create team");
    add_agent_orchestrator
        .add_member(
            "add-agent-team",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    let add_agent_claude_report = add_agent_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-team".to_string(),
                agent: setup_config("researcher", "claude", "claude-opus-4-6", "/tmp/research"),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("add-agent claude report");
    assert!(
        add_agent_claude_report.failed_step.is_none(),
        "add-agent Claude should succeed: {add_agent_claude_report:?}"
    );
    assert!(
        add_agent_runtime.calls().iter().all(|call| !matches!(
            call,
            RuntimeCall::JoinMesh { .. } | RuntimeCall::SpawnDaemon { .. }
        )),
        "add-agent Claude members should skip mesh join and daemon start"
    );

    let add_agent_sidecar_tmp = TempDir::new().expect("tempdir");
    let add_agent_sidecar_backend = Arc::new(FakeBackend::default());
    let add_agent_sidecar_runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut add_agent_sidecar_orchestrator = new_orchestrator(
        &add_agent_sidecar_tmp,
        add_agent_sidecar_backend,
        add_agent_sidecar_runtime.clone(),
    );
    add_agent_sidecar_orchestrator
        .create_team("add-agent-sidecar", None)
        .expect("create team");
    add_agent_sidecar_orchestrator
        .add_member(
            "add-agent-sidecar",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    let add_agent_sidecar_report = add_agent_sidecar_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-sidecar".to_string(),
                agent: setup_config("builder", "codex", "gpt-5.4", "/tmp/builder"),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("add-agent sidecar report");
    assert!(
        add_agent_sidecar_report.failed_step.is_none(),
        "add-agent Codex should succeed: {add_agent_sidecar_report:?}"
    );
    let add_agent_sidecar_calls = add_agent_sidecar_runtime.calls();
    assert!(add_agent_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "builder")
    ));
    assert!(add_agent_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::SpawnDaemon { member_name, .. } if member_name == "builder")
    ));
    assert!(!add_agent_sidecar_calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { .. })));

    let resume_tmp = TempDir::new().expect("tempdir");
    let resume_backend = Arc::new(FakeBackend::default());
    let resume_runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut resume_orchestrator =
        new_orchestrator(&resume_tmp, resume_backend, resume_runtime.clone());
    resume_orchestrator
        .create_team("resume-team", None)
        .expect("create team");
    resume_orchestrator
        .add_member(
            "resume-team",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    resume_orchestrator
        .add_member(
            "resume-team",
            member(
                "researcher",
                MemberRole::Agent,
                CliTool::Claude,
                "/tmp/research",
            ),
        )
        .expect("add member");
    mark_member_offline(&resume_tmp, "resume-team", "researcher", "%31", None);
    let resume_claude_report = resume_orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "resume-team".to_string(),
                member_name: "researcher".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume claude report");
    assert!(
        resume_claude_report.resumed,
        "resume Claude should succeed: {resume_claude_report:?}"
    );
    assert!(
        resume_runtime.calls().iter().all(|call| !matches!(
            call,
            RuntimeCall::JoinMesh { .. } | RuntimeCall::SpawnDaemon { .. }
        )),
        "resume Claude members should skip mesh join and daemon start"
    );

    let resume_sidecar_tmp = TempDir::new().expect("tempdir");
    let resume_sidecar_backend = Arc::new(FakeBackend::default());
    let resume_sidecar_runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut resume_sidecar_orchestrator = new_orchestrator(
        &resume_sidecar_tmp,
        resume_sidecar_backend,
        resume_sidecar_runtime.clone(),
    );
    resume_sidecar_orchestrator
        .create_team("resume-sidecar", None)
        .expect("create team");
    resume_sidecar_orchestrator
        .add_member(
            "resume-sidecar",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    resume_sidecar_orchestrator
        .add_member(
            "resume-sidecar",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add member");
    mark_member_offline(
        &resume_sidecar_tmp,
        "resume-sidecar",
        "builder",
        "%41",
        Some(55),
    );
    let resume_sidecar_report = resume_sidecar_orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "resume-sidecar".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume sidecar report");
    assert!(
        resume_sidecar_report.resumed,
        "resume Codex should succeed: {resume_sidecar_report:?}"
    );
    let resume_sidecar_calls = resume_sidecar_runtime.calls();
    assert!(resume_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::JoinMesh { member_name, .. } if member_name == "builder")
    ));
    assert!(resume_sidecar_calls.iter().any(
        |call| matches!(call, RuntimeCall::SpawnDaemon { member_name, .. } if member_name == "builder")
    ));
    assert!(resume_sidecar_calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 55)));
}

#[test]
fn shared_stage_onboarding_and_runtime_commit_policies_assert_wrapper_differences() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    let initialize_request = InitializeTeamRequest {
        team_name: "initialize-team".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.4", "/tmp/lead"),
        agents: vec![setup_config(
            "init-builder",
            "codex",
            "gpt-5.4",
            "/tmp/init-builder",
        )],
    };
    let initialize_entries = orchestrator
        .prepare_initialize_onboarding_entries(&initialize_request)
        .expect("initialize onboarding entries");
    assert!(initialize_entries
        .iter()
        .all(|entry| entry.policy == MemberActivationDeliveryPolicy::DeferredBarrier));

    orchestrator
        .create_team("parity-team", None)
        .expect("create team");
    orchestrator
        .add_member(
            "parity-team",
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
            "parity-team",
            member(
                "init-builder",
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/init-builder",
            ),
        )
        .expect("add initialize member");
    orchestrator
        .add_member(
            "parity-team",
            member(
                "add-builder",
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/add-builder",
            ),
        )
        .expect("add add-agent member");
    orchestrator
        .add_member(
            "parity-team",
            member(
                "resume-builder",
                MemberRole::Agent,
                CliTool::Codex,
                "/tmp/resume-builder",
            ),
        )
        .expect("add resume member");

    let add_agent_request = AddAgentRequest {
        team_name: "parity-team".to_string(),
        agent: setup_config("add-builder", "codex", "gpt-5.4", "/tmp/add-builder"),
    };
    let add_agent_entry = orchestrator
        .prepare_add_agent_onboarding_entry(&add_agent_request)
        .expect("add-agent onboarding entry")
        .expect("add-agent should have onboarding");
    assert_eq!(
        add_agent_entry.policy,
        MemberActivationDeliveryPolicy::Immediate
    );

    let resume_request = ResumeMemberRequest {
        team_name: "parity-team".to_string(),
        member_name: "resume-builder".to_string(),
        reasoning_effort_override: None,
    };
    let (resume_member, _runtime_record, lead_name) = orchestrator
        .load_resume_member_state(&resume_request)
        .expect("resume member state");
    let resume_entry = orchestrator
        .prepare_resume_onboarding_entry(&resume_request, &resume_member, &lead_name)
        .expect("resume should have onboarding");
    assert_eq!(
        resume_entry.policy,
        MemberActivationDeliveryPolicy::Immediate
    );

    let initialize_context = MemberActivationContext::for_initialize_member(
        "parity-team",
        "team-lead",
        &setup_config("init-builder", "codex", "gpt-5.4", "/tmp/init-builder"),
        MemberRole::Agent,
    )
    .expect("initialize context");
    orchestrator
        .commit_member_runtime(
            &initialize_context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%11".to_string())),
                session_id: Some(Some("session-init".to_string())),
                jsonl_path: Some(Some(PathBuf::from("/tmp/init.jsonl"))),
                attached_at: Some(Some(Utc::now())),
                health: Some(HealthState::Healthy),
                ..Default::default()
            },
        )
        .expect("initialize commit");
    assert_eq!(
        config_member_tmux_pane_id(&tmp, "parity-team", "init-builder"),
        None,
        "initialize keeps runtime commits staged until wrapper finalization"
    );

    let add_context = MemberActivationContext::for_add_agent(
        "parity-team",
        "team-lead",
        &setup_config("add-builder", "codex", "gpt-5.4", "/tmp/add-builder"),
    )
    .expect("add-agent context");
    orchestrator
        .commit_member_runtime(
            &add_context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%21".to_string())),
                session_id: Some(Some("session-add".to_string())),
                jsonl_path: Some(Some(PathBuf::from("/tmp/add.jsonl"))),
                attached_at: Some(Some(Utc::now())),
                health: Some(HealthState::Healthy),
                ..Default::default()
            },
        )
        .expect("add-agent commit");
    assert_eq!(
        config_member_tmux_pane_id(&tmp, "parity-team", "add-builder").as_deref(),
        Some("%21"),
        "add-agent finalizes runtime metadata immediately"
    );

    let resume_context =
        MemberActivationContext::for_resume_member("parity-team", "team-lead", &resume_member);
    orchestrator
        .commit_member_runtime(
            &resume_context,
            RuntimeCommitPatch {
                pane_id: Some(Some("%31".to_string())),
                session_id: Some(Some("session-resume".to_string())),
                jsonl_path: Some(Some(PathBuf::from("/tmp/resume.jsonl"))),
                attached_at: Some(Some(Utc::now())),
                health: Some(HealthState::Healthy),
                ..Default::default()
            },
        )
        .expect("resume commit");
    assert_eq!(
        config_member_tmux_pane_id(&tmp, "parity-team", "resume-builder").as_deref(),
        Some("%31"),
        "resume finalizes runtime metadata immediately"
    );
}

#[test]
fn join_mesh_if_required_skips_non_lead_claude_and_joins_required_members() {
    let runtime = RecordingCoordinationRuntime::default();

    let claude_joined = join_mesh_if_required(
        &runtime,
        "architecture-final",
        "team-lead",
        "/tmp/lead",
        MemberRole::Agent,
        CliTool::Claude,
        "opus",
    )
    .expect("claude join result");
    let codex_joined = join_mesh_if_required(
        &runtime,
        "architecture-final",
        "builder",
        "/tmp/builder",
        MemberRole::Agent,
        CliTool::Codex,
        "gpt-5.6-sol",
    )
    .expect("codex join result");

    assert!(!claude_joined, "non-lead Claude members do not mesh join");
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
            model,
            ..
        } if team_name == "architecture-final"
            && member_name == "builder"
            && project_id == "/tmp/builder"
            && model == "gpt-5.6-sol"
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
    // Regression: commit efcd7d2 appended the permission-bypass flag after the
    // free-form Settings command, preventing an operator from removing it.
    let mut cmds = crate::models::CliCommandSettings::default();
    cmds.agy.fresh = "agy --sandbox read-only".to_string();
    let agent = AgentSetupConfig {
        name: "reviewer".to_string(),
        cli_tool: "agy".to_string(),
        model: "gemini-3.7-flash-high".to_string(),
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
        reasoning_effort: None,
        handoff_expectations: None,
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
        "agy --sandbox read-only --model 'gemini-3.7-flash-high'"
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
        reasoning_effort: None,
        handoff_expectations: None,
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

// Regression: W5b shipped the Claude effort side-effect capture while the
// launch renderer still preserved a configured base verbatim, so a base such
// as `CLAUDE_CODE_EFFORT_LEVEL=low claude` froze a managed member at that
// level for the session's whole life and silently discarded every assignment's
// `/effort`.
#[test]
fn a_managed_launch_never_carries_the_frozen_effort_variable() {
    let variable = spec(CliTool::Claude)
        .capabilities
        .runtime_effort_frozen_env
        .expect("Claude freezes its effort through an environment variable");
    let agent = setup_config("builder", "claude", "opus", "/tmp/project");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = format!("{variable}=low claude --dangerously-skip-permissions");

    let rendered =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("the launch still renders without the frozen level");
    assert!(
        !rendered.contains(variable),
        "a managed launch must not freeze the level: {rendered}"
    );
    assert!(
        rendered.contains("claude --dangerously-skip-permissions"),
        "the rest of the operator's own command is kept: {rendered}"
    );
}

// A spelling this renderer cannot rewrite safely is refused instead, so the
// frozen level can never reach a managed pane by another route.
#[test]
fn a_frozen_effort_variable_the_renderer_cannot_strip_is_refused() {
    let variable = spec(CliTool::Claude)
        .capabilities
        .runtime_effort_frozen_env
        .expect("Claude freezes its effort through an environment variable");
    let agent = setup_config("builder", "claude", "opus", "/tmp/project");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = format!("export {variable}=low && claude");

    let error =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect_err("the frozen level must not reach a managed launch");
    assert!(
        error.to_string().contains(variable),
        "the error names the variable: {error}"
    );
}

#[test]
fn team_launch_rendering_does_not_probe_ambient_codex_home() {
    // Regression: 6fe0aa3 made pure launch rendering stat the developer's real
    // CODEX_HOME, changing six command snapshots after the managed hook existed.
    let helpers_source = include_str!("helpers.rs");
    assert!(!helpers_source.contains("codex_compact_hook_is_installed"));

    let agent = setup_config("builder", "codex", "gpt-5.4", "/tmp/project");
    let mut commands = crate::models::CliCommandSettings::default();
    let untrusted =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("untrusted command");
    commands.codex_bypass_hook_trust = true;
    let trusted =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("trusted command");
    assert!(!untrusted.contains("--dangerously-bypass-hook-trust"));
    assert!(trusted.contains("--dangerously-bypass-hook-trust"));
}

#[test]
fn managed_codex_team_launch_carries_the_account_selector() {
    // Regression: 08c3961 registered CODEX_HOME for direct launches but left
    // coordination sidecars on the process-implicit account directory.
    let agent = setup_config("builder", "codex", "gpt-5.4", "/tmp/project");
    let mut commands = crate::models::CliCommandSettings::default();
    let selector = spec(CliTool::Codex)
        .capabilities
        .account_selector
        .expect("Codex selector capability");
    commands.account_selector_dirs.insert(
        selector.to_string(),
        std::path::PathBuf::from("/accounts/codex-work"),
    );

    let command =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("managed command");

    assert_eq!(
        command,
        "CODEX_HOME='/accounts/codex-work' codex --yolo -m 'gpt-5.4'"
    );
}

// Regression: 791f6be centralized team launch rendering without a managed
// Codex notify input, so the pipeline could not opt into native idle edges.
#[test]
fn managed_codex_team_launch_includes_native_notify_sink() {
    let agent = setup_config("builder", "codex", "gpt-5.4", "/tmp/project");
    let commands = crate::models::CliCommandSettings {
        codex_notify_executable: Some(std::path::PathBuf::from(
            "/home/test/.local/bin/taurhaus-daemon",
        )),
        ..Default::default()
    };

    let command =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("managed command");

    assert!(command.contains(concat!(
        "-c 'notify=[\"/home/test/.local/bin/taurhaus-daemon\",",
        "\"codex-notify\"]'"
    )));
}

// Regression: a79d392 forced the catalog's low effort onto declarations that omitted it,
// changing the command after activation instead of preserving the CLI's configured effort.
#[test]
fn initialize_and_resume_leave_undeclared_effort_to_the_cli() {
    let mut agent = setup_config("builder", "codex", "", "/tmp/project");
    agent.role_id = Some("v3-developer-codex".to_string());
    let initialize = MemberActivationContext::for_initialize_member(
        "architecture-final",
        "team-lead",
        &agent,
        MemberRole::Agent,
    )
    .expect("initialize context");

    let mut persisted = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/project");
    persisted.role_id = Some("v3-developer-codex".to_string());
    let resume =
        MemberActivationContext::for_resume_member("architecture-final", "team-lead", &persisted);

    let initialize_command =
        build_member_activation_launch_command(&initialize, &CliCommandSettings::default())
            .expect("initialize command");
    let resume_command =
        build_member_activation_launch_command(&resume, &CliCommandSettings::default())
            .expect("resume command");

    assert_eq!(initialize_command, resume_command);
    assert_eq!(initialize_command, "codex --yolo -m 'gpt-5.6-sol'");
}

// Regression: ff40911 stripped the suffix and 5d2ce27 aliased gpt-5.3;
// roles declaring "gpt-5.4 high" ran at the user's global xhigh.
#[test]
fn build_cli_launch_command_for_codex_emits_legacy_reasoning_effort() {
    let cmds = crate::models::CliCommandSettings::default();
    let agent = AgentSetupConfig {
        name: "builder".to_string(),
        cli_tool: "codex".to_string(),
        model: "gpt-5.4 high".to_string(),
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
        reasoning_effort: None,
        handoff_expectations: None,
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    };

    let command = build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
        .expect("command");
    assert!(command.contains("-m 'gpt-5.4'"));
    assert!(command.contains("-c 'model_reasoning_effort=\"high\"'"));
}

/// A team member config, with everything but the tool left at its default.
fn team_agent(cli_tool: &str) -> AgentSetupConfig {
    AgentSetupConfig {
        name: "team-lead".to_string(),
        cli_tool: cli_tool.to_string(),
        model: String::new(),
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
        reasoning_effort: None,
        handoff_expectations: None,
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    }
}

// Regression: 760f776 rendered every team member launch with no
// `CLAUDE_CONFIG_DIR` at all. Agent inboxes live under
// `PlatformPaths::teams_dir()`, which `TAURHAUS_CLAUDE_DIR` moves — and Claude
// Code has never heard of that variable, so a member launched without the
// assignment ran against its physical `~/.claude` and wrote its inbox where
// the team that started it never looks.
#[test]
fn build_cli_launch_command_names_a_configured_claude_root() {
    let _guard = taurhaus_lib::test_support::acquire_env_test_guard();
    let cmds = CliCommandSettings::default();
    let override_dir = TempDir::new().expect("tempdir");
    std::env::set_var("TAURHAUS_CLAUDE_DIR", override_dir.path());

    let claude = build_cli_launch_command(
        &team_agent("claude"),
        "ledger-team",
        MemberRole::Lead,
        &cmds,
    );
    let codex = build_cli_launch_command(
        &team_agent("codex"),
        "ledger-team",
        MemberRole::Agent,
        &cmds,
    );

    std::env::remove_var("TAURHAUS_CLAUDE_DIR");

    let claude = claude.expect("claude command");
    assert!(
        claude.starts_with(&format!(
            "CLAUDE_CONFIG_DIR='{}' ",
            override_dir.path().display()
        )),
        "{claude}"
    );
    // The team environment still lands in front of the binary.
    assert!(claude.contains("CLAUDECODE=1"), "{claude}");

    let codex = codex.expect("codex command");
    assert!(!codex.contains("CLAUDE_CONFIG_DIR"), "{codex}");
}

#[test]
fn build_cli_launch_command_leaves_an_unmoved_claude_root_implicit() {
    let _guard = taurhaus_lib::test_support::acquire_env_test_guard();
    let cmds = CliCommandSettings::default();
    std::env::remove_var("TAURHAUS_CLAUDE_DIR");

    let command = build_cli_launch_command(
        &team_agent("claude"),
        "ledger-team",
        MemberRole::Lead,
        &cmds,
    )
    .expect("command");

    assert!(!command.contains("CLAUDE_CONFIG_DIR"), "{command}");
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
        reasoning_effort: None,
        handoff_expectations: None,
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
    assert!(command.contains("--model 'claude-opus-4-6'"));
    assert!(command.contains("--team-name 'ledger-team'"));
    assert!(command.contains("--agent-name 'team-lead'"));
    assert!(command.contains("--agent-id 'team-lead@ledger-team'"));
    assert!(command.contains("--agent-type 'orchestrator'"));
    assert!(command.contains("-n 'team-lead'"));
    for flag in [
        "--team-name",
        "--agent-name",
        "--agent-id",
        "--agent-type",
        "-n",
    ] {
        assert_eq!(
            command
                .split_whitespace()
                .filter(|token| *token == flag)
                .count(),
            1,
            "{flag} must be rendered exactly once: {command}"
        );
    }
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
    assert_eq!(command, "codex --yolo -m 'gpt-5.3'");
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
    assert!(command.contains("--agent-type 'orchestrator'"));
    assert!(command.contains("--team-name 'architecture-final'"));
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
fn initialize_pipeline_progress_callback_preserves_batch_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    let request = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config("team-lead", "codex", "gpt-5.4", "/tmp/lead"),
        agents: vec![setup_config(
            "builder",
            "claude",
            "claude-opus-4-6",
            "/tmp/builder",
        )],
    };

    let mut emitted = Vec::new();
    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout_and_progress(
            &request,
            &CliCommandSettings::default(),
            "new_window",
            Some(&mut |step, status, message| {
                emitted.push((step.to_string(), status, message));
            }),
        )
        .expect("initialize report");

    assert_eq!(emitted.len(), report.steps.len() * 2);
    for (idx, step) in report.steps.iter().enumerate() {
        let running = &emitted[idx * 2];
        let completed = &emitted[idx * 2 + 1];
        assert_eq!(running.0, step.step);
        assert_eq!(running.1, StepStatus::Running);
        assert_eq!(running.2, None);
        assert_eq!(completed.0, step.step);
        assert_eq!(completed.1, step.status);
        assert_eq!(completed.2.as_deref(), step.message.as_deref());
    }
}

#[test]
fn load_resume_member_state_preserves_role_template_context() {
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
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: Some(vec!["implementation".to_string(), "testing".to_string()]),
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from("/tmp/builder"),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            },
        )
        .expect("add member");

    let request = ResumeMemberRequest {
        team_name: "architecture-final".to_string(),
        member_name: "builder".to_string(),
        reasoning_effort_override: None,
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
    // Regression: ff40911 discarded the role effort during relaunch, while
    // resume also replaced the model with an empty string.
    assert_eq!(loaded_member.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(loaded_member.reasoning_effort.as_deref(), Some("high"));

    mark_member_offline(&tmp, "architecture-final", "builder", "%61", Some(55));
    let report = orchestrator
        .resume_member_with_cli_commands(&request, &CliCommandSettings::default())
        .expect("resume role-backed member");
    assert!(report.resumed, "resume should succeed: {report:?}");

    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::JoinMesh { member_name, model, .. }
            if member_name == "builder" && model == "gpt-5.4"
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SendKeys { keys, .. }
            if keys.contains("-m 'gpt-5.4'")
                && keys.contains("model_reasoning_effort=\"high\"")
    )));
}

#[test]
fn resume_accepts_a_minimal_runtime_record_written_by_mesh() {
    // Regression: 50fc736 made a mesh-owned applied-effort record fatal to
    // activation because taurhaus required its own health field to be present.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    orchestrator
        .create_team("minimal-runtime", None)
        .expect("create team");
    orchestrator
        .add_member(
            "minimal-runtime",
            member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "minimal-runtime",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add builder");
    fs::write(
        tmp.path()
            .join("minimal-runtime")
            .join("runtime")
            .join("builder.json"),
        r#"{"appliedEffort":"medium"}"#,
    )
    .expect("write minimal mesh runtime");

    let report = orchestrator
        .resume_member("minimal-runtime", "builder")
        .expect("resume report");

    assert!(
        report.resumed,
        "partial runtime should activate: {report:?}"
    );
    assert_eq!(report.failed_step, None);
}

// Regression: a79d392 treated mesh's pre-existing `external` placeholder as a model
// declaration, so resume rendered `-m 'external'` instead of the member role's model.
#[test]
fn resume_external_placeholder_hydrates_the_role_model() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());

    orchestrator
        .create_team("external-placeholder", None)
        .expect("create team");
    orchestrator
        .add_member(
            "external-placeholder",
            member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead");
    let mut builder = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder");
    builder.role_id = Some("v3-developer-codex".to_string());
    builder.model = Some("external".to_string());
    orchestrator
        .add_member("external-placeholder", builder)
        .expect("add builder");
    mark_member_offline(&tmp, "external-placeholder", "builder", "%71", None);

    let report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "external-placeholder".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume member");
    assert!(report.resumed, "resume should succeed: {report:?}");

    let calls = runtime.calls();
    let launch = calls
        .iter()
        .find_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.as_str()),
            _ => None,
        })
        .expect("launch command");
    assert_eq!(launch, "codex --yolo -m 'gpt-5.4'");
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::JoinMesh { model, .. } if model == "gpt-5.4"
    )));
}

// Regression: a79d392 derived the template root from the Claude-owned teams path,
// making app-data user roles invisible during resume hydration.
#[test]
fn resume_hydrates_user_role_from_app_data_template_root() {
    let tmp = TempDir::new().expect("tempdir");
    let app_data_dir = tmp.path().join("app-data");
    let claude_dir = tmp.path().join("claude");

    let store = TemplateStore::new(app_data_dir.clone());
    let mut role = store
        .get_role("v3-developer-codex")
        .expect("bundled role")
        .template;
    role.role_id = "user-root-builder".to_string();
    role.name = "User Root Builder".to_string();
    role.defaults.model = "gpt-5.5".to_string();
    role.defaults.reasoning_effort = Some("high".to_string());
    store.create_role(&role).expect("create user role");

    let teams_dir = claude_dir.join("teams");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime_and_template_root(
        teams_dir,
        app_data_dir,
        backend,
        runtime,
    );
    orchestrator
        .create_team("user-root", None)
        .expect("create team");
    let mut builder = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder");
    builder.role_id = Some("user-root-builder".to_string());
    orchestrator
        .add_member("user-root", builder)
        .expect("add builder");

    let (loaded, _, _) = orchestrator
        .load_resume_member_state(&ResumeMemberRequest {
            team_name: "user-root".to_string(),
            member_name: "builder".to_string(),
            reasoning_effort_override: None,
        })
        .expect("load resume state");

    assert_eq!(loaded.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(loaded.reasoning_effort.as_deref(), Some("high"));
}

// Regression: a79d392 made a corrupt user role fatal to resume even though the
// pre-existing resume path did not require template storage to be healthy.
#[test]
fn resume_falls_back_when_user_role_is_corrupt() {
    let tmp = TempDir::new().expect("tempdir");
    let roles_dir = tmp.path().join("templates").join("roles");
    fs::create_dir_all(&roles_dir).expect("create roles dir");
    fs::write(
        roles_dir.join("v3-developer-codex.yaml"),
        "schema: [invalid\n",
    )
    .expect("write corrupt role");

    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    orchestrator
        .create_team("corrupt-role", None)
        .expect("create team");
    let mut builder = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder");
    builder.role_id = Some("v3-developer-codex".to_string());
    orchestrator
        .add_member("corrupt-role", builder)
        .expect("add builder");

    let (loaded, _, _) = orchestrator
        .load_resume_member_state(&ResumeMemberRequest {
            team_name: "corrupt-role".to_string(),
            member_name: "builder".to_string(),
            reasoning_effort_override: None,
        })
        .expect("corrupt role should degrade to catalog defaults");

    assert_eq!(loaded.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(loaded.reasoning_effort, None);
}

#[test]
fn resume_pipeline_claude_lead_joins_mesh_but_skips_member_daemon() {
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
    assert_eq!(join_step.message.as_deref(), Some("mesh joined"));
    assert_eq!(
        runtime
            .calls()
            .iter()
            .filter(|call| matches!(call, RuntimeCall::JoinMesh { member_name, member_type, .. } if member_name == "team-lead" && member_type == "lead"))
            .count(),
        1
    );
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
    assert!(launch.contains("--agent-type 'orchestrator'"));
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
fn claude_lead_join_failure_is_nonfatal_after_activation_commit() {
    // Regression: 694b130 deferred the Claude-lead join until after commit but
    // kept the fatal cleanup path, killing an already-persisted activation.
    let initialize_tmp = TempDir::new().expect("tempdir");
    let initialize_backend = Arc::new(FakeBackend::default());
    let initialize_runtime = Arc::new(RecordingCoordinationRuntime::default());
    initialize_runtime.set_join_mesh_failure("simulated lead credential failure");
    let mut initialize_orchestrator = new_orchestrator(
        &initialize_tmp,
        initialize_backend,
        initialize_runtime.clone(),
    );

    let initialize_report = initialize_orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &InitializeTeamRequest {
                team_name: "lead-join-initialize".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config("team-lead", "claude", "claude-opus-4-6", "/tmp/lead"),
                agents: vec![],
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("initialize report");
    assert!(
        initialize_report.failed_step.is_none(),
        "credential refresh must not fail initialization: {initialize_report:?}"
    );
    let initialize_join_step = initialize_report
        .steps
        .iter()
        .find(|step| step.step == "join_mesh")
        .expect("initialize join step");
    assert_eq!(initialize_join_step.status, StepStatus::Succeeded);
    assert!(initialize_join_step
        .message
        .as_deref()
        .unwrap_or_default()
        .contains("simulated lead credential failure"));
    assert!(
        TeamConfigStore::load(initialize_tmp.path(), "lead-join-initialize").is_ok(),
        "the committed team remains usable"
    );

    let resume_tmp = TempDir::new().expect("tempdir");
    let resume_backend = Arc::new(FakeBackend::default());
    let resume_runtime = Arc::new(RecordingCoordinationRuntime::default());
    resume_runtime.set_join_mesh_failure("simulated lead credential failure");
    let mut resume_orchestrator =
        new_orchestrator(&resume_tmp, resume_backend, resume_runtime.clone());
    resume_orchestrator
        .create_team("lead-join-resume", None)
        .expect("create team");
    resume_orchestrator
        .add_member(
            "lead-join-resume",
            member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead");
    mark_member_offline(&resume_tmp, "lead-join-resume", "team-lead", "%stale", None);
    resume_runtime.set_pane_ownership("%stale", false);

    let resume_report = resume_orchestrator
        .resume_member("lead-join-resume", "team-lead")
        .expect("resume report");
    assert!(
        resume_report.resumed,
        "credential refresh must not roll back resume: {resume_report:?}"
    );
    assert!(resume_report
        .warnings
        .iter()
        .any(|warning| { warning.contains("simulated lead credential failure") }));
    assert!(
        resume_runtime
            .calls()
            .iter()
            .all(|call| !matches!(call, RuntimeCall::KillPane { .. })),
        "best-effort credential refresh must not kill the committed pane"
    );
    let persisted = MemberRuntimeStore::load(resume_tmp.path(), "lead-join-resume", "team-lead")
        .expect("persisted runtime");
    assert_eq!(persisted.health, HealthState::Healthy);
    assert_eq!(
        persisted.pane_id.as_deref(),
        resume_report.pane_id.as_deref()
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
    assert!(launch.contains("--agent-type 'general-purpose'"));
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
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from("/tmp/research"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add member");

    let request = ResumeMemberRequest {
        team_name: "architecture-final".to_string(),
        member_name: "researcher".to_string(),
        reasoning_effort_override: None,
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
                reasoning_effort_override: None,
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
                reasoning_effort_override: None,
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
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: Some(vec!["analysis".to_string()]),
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from("/tmp/research"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
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
    assert_eq!(launch, "codex --yolo -m 'gpt-5.6-sol'");
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
    assert_eq!(launch, "codex --yolo -m 'gpt-5.6-sol'");
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
    // Regression: mesh-findings P3, tmux reused pane ids; daemons for
    // taurrust/gotaurus/espn pointed at claude panes.
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
    runtime.set_pane_current_command("%77", Some("claude"));

    let report = orchestrator
        .resume_member("architecture-final", "builder")
        .expect("resume report");
    assert!(report.resumed);
    assert!(!report.reused_pane);
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-1"));

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder")
        .expect("updated runtime");
    assert_eq!(updated.pane_id.as_deref(), Some("test-pane-1"));
    assert_eq!(updated.pane_pid, Some(1001));
    assert_eq!(updated.pane_start_time, Some(1_755_000_001));
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
fn newly_created_pane_does_not_inherit_identity_when_capture_probe_fails() {
    // Regression: aecc8ac requires probe failures to preserve identity only
    // for reuse; carrying it onto a newly created pane would fabricate owner
    // evidence for the wrong tmux process.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());
    orchestrator
        .create_team("new-pane-identity", None)
        .expect("create team");
    orchestrator
        .add_member(
            "new-pane-identity",
            member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "new-pane-identity",
            member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/builder"),
        )
        .expect("add builder");

    let mut previous =
        MemberRuntimeStore::load(tmp.path(), "new-pane-identity", "builder").expect("runtime");
    previous.pane_id = Some("%gone".to_string());
    previous.pane_pid = Some(7001);
    previous.pane_start_time = Some(1_755_000_007);
    previous.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), "new-pane-identity", "builder", &previous)
        .expect("save runtime");
    runtime.set_pane_exists("%gone", false);
    runtime.set_live_pane_failure("test-pane-1", "transient capture failure");

    let report = orchestrator
        .resume_member("new-pane-identity", "builder")
        .expect("resume report");
    assert!(report.resumed, "capture remains fail-soft: {report:?}");
    assert!(!report.reused_pane);

    let updated = MemberRuntimeStore::load(tmp.path(), "new-pane-identity", "builder")
        .expect("updated runtime");
    assert_eq!(updated.pane_id.as_deref(), Some("test-pane-1"));
    assert_eq!(updated.pane_pid, None);
    assert_eq!(updated.pane_start_time, None);
}

#[test]
fn resume_foreign_pane_launch_failure_leaves_runtime_dead_without_daemon() {
    // Regression: mesh-findings P3, tmux reused pane ids; daemons for
    // taurrust/gotaurus/espn pointed at claude panes.
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
    member_runtime.session_id = Some("stale-session".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "builder", &member_runtime)
        .expect("save runtime");

    runtime.set_pane_exists("%77", true);
    runtime.set_pane_current_command("%77", Some("claude"));
    runtime.set_pid_running(55, true);
    runtime.set_send_keys_failures("test-pane-1", usize::MAX, "launch failed");

    let report = orchestrator
        .resume_member("architecture-final", "builder")
        .expect("resume report");
    assert!(!report.resumed);
    assert_eq!(report.failed_step.as_deref(), Some("launch_session"));

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder")
        .expect("updated runtime");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert_eq!(updated.daemon_pid, None);
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid: 55 })));
    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::SpawnDaemon { .. })));
}

#[test]
fn stale_foreign_pane_decision_cannot_overwrite_a_concurrent_runtime_commit() {
    // Regression: 366f4b7 left the resume foreign-pane cleanup as another
    // load/probe/save writer, so its stale cleanup record could replace a new
    // owner while the identity probe was still in flight.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());
    let team_name = "foreign-pane-interleave";
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

    let mut stale = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    stale.pane_id = Some("%foreign".to_string());
    stale.pane_pid = Some(7001);
    stale.pane_start_time = Some(1_755_000_007);
    stale.session_id = Some("session-stale".to_string());
    stale.daemon_pid = Some(7100);
    stale.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &stale)
        .expect("save stale binding");
    runtime.set_pane_exists("%foreign", true);
    runtime.set_pane_current_command("%foreign", Some("claude"));
    runtime.set_pid_running(7100, true);
    runtime.set_send_keys_failures("test-pane-1", usize::MAX, "launch failed");
    let probe_gate = runtime.pause_live_pane_probe("%foreign");

    let resume = std::thread::spawn(move || {
        orchestrator
            .resume_member(team_name, member_name)
            .expect("resume report")
    });
    probe_gate.wait_until_blocked();

    let mut concurrent = stale;
    concurrent.pane_id = Some("%winner".to_string());
    concurrent.pane_pid = Some(8001);
    concurrent.pane_start_time = Some(1_755_000_008);
    concurrent.session_id = Some("session-winner".to_string());
    concurrent.daemon_pid = Some(8100);
    concurrent.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &concurrent)
        .expect("concurrent runtime commit");
    probe_gate.release();

    let report = resume.join().expect("resume thread");
    assert!(!report.resumed);
    assert_eq!(report.failed_step.as_deref(), Some("resolve_pane"));
    assert_eq!(
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("final runtime"),
        concurrent,
        "foreign cleanup based on the old pane must be dropped"
    );
}

#[test]
fn foreign_pane_commit_error_cleans_the_new_resume_pane() {
    // Regression: 731dc539 added fallible lock and compare-and-commit calls
    // whose `?` returns bypassed cleanup_failure after a replacement pane had
    // already been created.
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());
    let team_name = "foreign-pane-commit-error";
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

    let mut stale = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    stale.pane_id = Some("%foreign".to_string());
    stale.pane_pid = Some(7001);
    stale.pane_start_time = Some(1_755_000_007);
    stale.session_id = Some("session-stale".to_string());
    stale.daemon_pid = Some(7100);
    stale.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &stale)
        .expect("save stale binding");
    runtime.set_pane_exists("%foreign", true);
    runtime.set_pane_current_command("%foreign", Some("claude"));
    runtime.set_pid_running(7100, true);
    let probe_gate = runtime.pause_live_pane_probe("%foreign");

    let resume = std::thread::spawn(move || {
        orchestrator
            .resume_member(team_name, member_name)
            .expect("resume report")
    });
    probe_gate.wait_until_blocked();
    fs::write(
        tmp.path()
            .join(team_name)
            .join("runtime")
            .join(format!("{member_name}.json")),
        "{ malformed runtime",
    )
    .expect("corrupt runtime while foreign-pane probe is in flight");
    probe_gate.release();

    let report = resume.join().expect("resume thread");
    assert!(!report.resumed);
    assert_eq!(report.failed_step.as_deref(), Some("resolve_pane"));
    assert!(
        runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::KillPane { pane_id } if pane_id == "test-pane-1"
        )),
        "the replacement pane must be rolled back on a store error"
    );
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

// ---------------------------------------------------------------------------
// Task-level effort: the resume path taurhaus owns for Codex
// ---------------------------------------------------------------------------

fn effort_team(
    tmp: &TempDir,
    runtime: Arc<RecordingCoordinationRuntime>,
    cli_tool: CliTool,
    launch_effort: Option<&str>,
) -> CoordinationOrchestrator {
    runtime.set_detected_runtime_session(
        "%21",
        cli_tool,
        Some("session-effort"),
        Some("/tmp/effort.jsonl"),
    );
    runtime.set_pane_identity("%21", Some(2021), Some(1_755_000_021));
    let mut orchestrator = new_orchestrator(tmp, Arc::new(FakeBackend::default()), runtime);
    orchestrator
        .create_team("effort-team", None)
        .expect("create team");
    orchestrator
        .add_member(
            "effort-team",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                "/tmp/lead-project",
            ),
        )
        .expect("add lead");
    let mut builder = member("builder", MemberRole::Agent, cli_tool, "/tmp/builder");
    builder.reasoning_effort = launch_effort.map(ToString::to_string);
    orchestrator
        .add_member("effort-team", builder)
        .expect("add builder");
    orchestrator
}

/// Put the member on an active task carrying `level`, the way the operational
/// snapshot sync does after mesh writes the assignment onto the task record.
fn assign_task(tmp: &TempDir, member_name: &str, level: &str, why: &str) {
    write_member_snapshot(
        tmp,
        member_name,
        Some(("42", "Run the migration")),
        level,
        why,
    );
}

/// The member has nothing assigned: its last task is finished, so the snapshot
/// carries neither a task nor a level.
fn clear_assignment(tmp: &TempDir, member_name: &str) {
    write_member_snapshot(tmp, member_name, None, "", "");
}

fn write_member_snapshot(
    tmp: &TempDir,
    member_name: &str,
    task: Option<(&str, &str)>,
    level: &str,
    why: &str,
) {
    use crate::coordination::stores::{
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };

    OperationalContextSnapshotStore::save(
        tmp.path(),
        &OperationalContextSnapshot {
            version: 1,
            team_name: "effort-team".to_string(),
            member_name: member_name.to_string(),
            updated_at: Utc::now(),
            task: task
                .map(|(id, subject)| OperationalTaskSnapshot {
                    id: id.to_string(),
                    subject: subject.to_string(),
                    status: "in_progress".to_string(),
                    ..Default::default()
                })
                .unwrap_or_default(),
            assignment_footer: OperationalAssignmentFooterSnapshot {
                task_effort: level.to_string(),
                task_effort_why: why.to_string(),
                ..Default::default()
            },
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/tmp/builder".to_string(),
                focal_files: vec![],
            },
        },
    )
    .expect("write operational snapshot");
}

/// An assignment mesh delivered to the member's inbox at some point. Kept only
/// so the tests can prove the switch does *not* read it.
fn append_inbox_assignment(tmp: &TempDir, member_name: &str, level: &str, why: &str) {
    let mut message = crate::coordination::stores::MeshInboxMessage::new(
        "team-lead",
        format!("Effort: {level} — {why}\nStart on the migration."),
        None,
        Utc::now(),
    );
    message
        .extra
        .insert("effort".to_string(), serde_json::json!(level));
    message
        .extra
        .insert("effortWhy".to_string(), serde_json::json!(why));
    crate::coordination::stores::MeshInboxStore::append(
        tmp.path(),
        "effort-team",
        member_name,
        &message,
    )
    .expect("append assignment");
}

#[test]
fn a_launch_records_the_effort_the_session_actually_runs_at() {
    // mesh reads this before it types `/effort`, so it has to start from the
    // level the launch put into effect rather than from nothing.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime, CliTool::Codex, Some("Low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);

    let report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume report");
    assert!(report.resumed, "resume should succeed: {report:?}");

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.applied_effort.as_deref(), Some("low"));
}

#[test]
fn a_codex_member_is_relaunched_with_the_assignment_effort() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("seed resume");

    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");
    assert_eq!(resumed, vec!["builder".to_string()]);

    let launch = runtime
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys),
            _ => None,
        })
        .rfind(|keys| keys.contains("codex"))
        .expect("a codex launch was sent to the pane");
    assert!(
        launch.contains("model_reasoning_effort=\\\"high\\\"")
            || launch.contains("model_reasoning_effort=\"high\""),
        "codex resume must carry the assignment effort, got: {launch}"
    );

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// Regression: d055165 bounded a pending effort switch to assignments delivered
// at or after the running session's `attached_at`, and an operator's own resume
// resets that stamp while carrying no level of its own. A task assigned while
// the member was stopped therefore came back at the launch effort, and its
// older timestamp excluded it from every later pass.
#[test]
fn a_resume_carries_the_effort_of_an_assignment_made_while_the_member_was_down() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);

    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume report");
    assert!(report.resumed, "resume should succeed: {report:?}");

    let launch = runtime
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys),
            _ => None,
        })
        .rfind(|keys| keys.contains("codex"))
        .expect("a codex launch was sent to the pane");
    assert!(
        launch.contains("high"),
        "the resume must carry the open assignment's level, got: {launch}"
    );

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("high"),
        "the member is back at the level the open assignment asked for"
    );

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");
    assert!(
        resumed.is_empty(),
        "the resume already put the level into force; nothing is taken down again"
    );
}

// Regression: 063e74a had taurhaus type `/effort <level>` into a member's own
// pane from a background pass. mesh 0.2.22 submits that command itself, before
// it delivers the assignment notice, so taurhaus's copy was a second owner
// writing into the same pane — and it landed after the member could already
// read the assignment. The submission is mesh's alone.
#[test]
fn taurhaus_never_types_an_effort_command_into_a_members_pane() {
    for tool in [CliTool::Claude, CliTool::Agy, CliTool::Grok] {
        let tmp = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = effort_team(&tmp, runtime.clone(), tool, Some("low"));
        mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
        orchestrator
            .resume_member_with_cli_commands(
                &ResumeMemberRequest {
                    team_name: "effort-team".to_string(),
                    member_name: "builder".to_string(),
                    reasoning_effort_override: None,
                },
                &CliCommandSettings::default(),
            )
            .expect("seed resume");
        assign_task(&tmp, "builder", "high", "the migration is irreversible");

        let handled = orchestrator
            .apply_pending_task_effort(
                "effort-team",
                &CliCommandSettings::default(),
                "new_window",
                EffortPassScope::TaskChanged,
            )
            .expect("effort pass");

        assert!(
            handled.is_empty(),
            "{tool} takes the level from mesh, so taurhaus has nothing to do"
        );
        assert!(
            !runtime.calls().into_iter().any(|call| matches!(
                call,
                RuntimeCall::SendKeys { keys, .. } if keys.starts_with("/effort")
            )),
            "{tool}: mesh owns the slash command; taurhaus must not send it"
        );
        assert!(
            !runtime
                .calls()
                .into_iter()
                .any(|call| matches!(call, RuntimeCall::KillPane { .. })),
            "{tool}: a harness mesh can reach is never relaunched for effort"
        );
    }
}

#[test]
fn a_resume_base_is_pointed_at_the_named_conversation() {
    use super::helpers::resume_base_for_session;

    assert_eq!(
        resume_base_for_session("codex resume --last --yolo", "abc-123"),
        "codex resume 'abc-123' --yolo",
        "--last resumes whoever touched the account last, not this member"
    );
    assert_eq!(
        resume_base_for_session("codex resume {session_id} --yolo", "abc-123"),
        "codex resume 'abc-123' --yolo",
        "an operator's own placeholder wins"
    );
    assert_eq!(
        resume_base_for_session("codex resume", "abc-123"),
        "codex resume 'abc-123'",
        "a resume verb with no conversation would open the interactive picker"
    );
}

// Regression: 2529309 routed the Codex effort switch through the generic resume
// pipeline, which always renders `LaunchMode::Fresh`. The member lost its
// conversation, its persisted session id never reached the command, and the
// settings the operator launched it with were replaced by defaults.
#[test]
fn a_codex_effort_relaunch_resumes_the_members_own_session() {
    let tmp = TempDir::new().expect("tempdir");
    let codex_home = TempDir::new().expect("codex home");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    let mut cli_commands = CliCommandSettings::default();
    cli_commands
        .account_selector_dirs
        .insert("CODEX_HOME".to_string(), codex_home.path().to_path_buf());
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &cli_commands,
        )
        .expect("seed resume");
    let seeded = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(seeded.session_id.as_deref(), Some("session-effort"));

    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    let launch = runtime
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys),
            _ => None,
        })
        .rfind(|keys| keys.contains("codex"))
        .expect("a codex launch was sent to the pane");

    assert!(
        launch.contains("codex resume") && launch.contains("session-effort"),
        "the effort relaunch must resume the member's own conversation, got: {launch}"
    );
    assert!(
        !launch.contains("--last"),
        "resuming by id must not fall back to whatever conversation ran last, got: {launch}"
    );
    assert!(
        launch.contains("model_reasoning_effort=\\\"high\\\"")
            || launch.contains("model_reasoning_effort=\"high\""),
        "the resume must carry the assignment effort, got: {launch}"
    );
    assert!(
        launch.contains("CODEX_HOME=") && launch.contains(&codex_home.path().display().to_string()),
        "the relaunch must keep the account the member was launched on, got: {launch}"
    );
}

#[test]
fn a_second_pass_over_the_same_assignment_does_not_relaunch_again() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime, CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("seed resume");
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let first = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("first pass");
    let second = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("second pass");

    assert_eq!(first, vec!["builder".to_string()]);
    assert!(
        second.is_empty(),
        "the member is already at the assigned level"
    );
}

fn codex_launch_attempts(runtime: &RecordingCoordinationRuntime) -> usize {
    runtime
        .calls()
        .into_iter()
        .filter(|call| matches!(call, RuntimeCall::SendKeys { keys, .. } if keys.contains("codex")))
        .count()
}

// Regression: 2529309 recorded the requested level as `applied_effort` on the
// failure branch too. The member was already stopped, the level had never
// taken effect, and every later pass compared requested against applied and
// saw nothing pending — so the stopped member was never brought back.
#[test]
fn a_failed_effort_relaunch_stays_retryable_within_a_budget() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("seed resume");
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    runtime.set_send_keys_failures("%21", usize::MAX, "launch failed");
    for index in 1..=8 {
        runtime.set_send_keys_failures(&format!("test-pane-{index}"), usize::MAX, "launch failed");
    }

    let before = codex_launch_attempts(&runtime);
    let first = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("first pass");
    assert!(first.is_empty(), "the relaunch failed, so nothing resumed");

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "a level that never took effect is not what the session is running at"
    );

    let after_first = codex_launch_attempts(&runtime);
    assert!(after_first > before, "the first pass attempted a launch");

    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("second pass");
    let after_second = codex_launch_attempts(&runtime);
    assert!(
        after_second > after_first,
        "a transient failure has to stay retryable"
    );

    // Bounded: the budget stops a stopped member being restarted forever.
    for _ in 0..4 {
        orchestrator
            .apply_pending_task_effort(
                "effort-team",
                &CliCommandSettings::default(),
                "new_window",
                EffortPassScope::TaskChanged,
            )
            .expect("later pass");
    }
    let after_budget = codex_launch_attempts(&runtime);
    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("pass past the budget");
    assert_eq!(
        codex_launch_attempts(&runtime),
        after_budget,
        "a level that keeps failing must not restart the pane on every pass"
    );
}

// Regression: the same failure branch left a member that later came back
// unable to reach a level it had failed once, because the budget was never
// cleared by a successful launch.
#[test]
fn a_successful_launch_clears_the_failed_effort_budget() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("seed resume");
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    runtime.set_send_keys_failures("%21", usize::MAX, "launch failed");
    for index in 1..=8 {
        runtime.set_send_keys_failures(&format!("test-pane-{index}"), usize::MAX, "launch failed");
    }
    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("failing pass");

    for index in 1..=16 {
        runtime.set_send_keys_failures(&format!("test-pane-{index}"), 0, "");
    }
    runtime.set_send_keys_failures("%21", 0, "");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("recovered pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
    assert_eq!(record.effort_resume_failure, None);
}

// Regression: 4994b24 read the switch's level out of the member's inbox, and
// an inbox keeps every assignment ever delivered. The newest effort-bearing
// message therefore outlived the task it was asked for, so a member whose work
// was long finished still counted as owing that level. The task the member is
// on is the only thing the level may be read from.
#[test]
fn a_finished_assignment_leaves_the_running_pane_alone() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    append_inbox_assignment(&tmp, "builder", "high", "the migration is irreversible");
    clear_assignment(&tmp, "builder");

    let before = codex_launch_attempts(&runtime);
    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(resumed.is_empty(), "nothing is assigned to act on");
    assert_eq!(
        codex_launch_attempts(&runtime),
        before,
        "the level of finished work is not what the member is working under now"
    );
}

// The other side of the same rule: the task the member is on is what the pass
// exists for.
#[test]
fn an_assignment_on_the_members_active_task_relaunches_it() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
}

// Regression: 4994b24 resolved an operator's own resume from the newest
// effort-bearing message in the inbox with no lower bound, so a member coming
// back came back at the level of the last assignment it had *ever* been sent —
// work that was finished hours earlier.
#[test]
fn a_resume_ignores_an_assignment_the_member_has_already_finished() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    append_inbox_assignment(&tmp, "builder", "high", "the migration is irreversible");
    clear_assignment(&tmp, "builder");

    let report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .expect("resume report");
    assert!(report.resumed, "resume should succeed: {report:?}");

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "the member comes back at its own launch effort, not a finished task's level"
    );
}

#[test]
fn a_member_that_never_started_is_left_alone() {
    // No runtime record means no session to switch; a resume here would start
    // a member the operator never launched.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime, CliTool::Codex, Some("low"));
    MemberRuntimeStore::delete(tmp.path(), "effort-team", "builder").expect("delete runtime");
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(resumed.is_empty());
}

fn seed_running_codex_member(
    tmp: &TempDir,
    runtime: Arc<RecordingCoordinationRuntime>,
    cli_commands: &CliCommandSettings,
) -> CoordinationOrchestrator {
    let mut orchestrator = effort_team(tmp, runtime, CliTool::Codex, Some("low"));
    mark_member_offline(tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: None,
            },
            cli_commands,
        )
        .expect("seed resume");
    orchestrator
}

// Regression: 6128bd1 pointed the relaunch at the member's own conversation
// but left a record with no session id eligible for the pass. The member was
// stopped anyway, the resume rendered `LaunchMode::Fresh`, and an effort
// switch threw away the conversation the assignment was building on.
#[test]
fn an_effort_switch_without_a_session_id_leaves_the_running_pane_alone() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    assign_task(&tmp, "builder", "high", "the migration is irreversible");
    // What an older record — or a session capture that never landed — leaves.
    MemberRuntimeStore::update(tmp.path(), "effort-team", "builder", |record| {
        record.session_id = None;
    })
    .expect("clear session id");

    let before = codex_launch_attempts(&runtime);
    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(resumed.is_empty(), "the switch has to be deferred");
    assert_eq!(
        codex_launch_attempts(&runtime),
        before,
        "an effort switch must never start a fresh conversation"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.health,
        HealthState::Healthy,
        "the member keeps running at its previous level"
    );
    assert_eq!(
        record
            .effort_resume_failure
            .as_ref()
            .map(|failure| failure.level.as_str()),
        Some("high"),
        "a switch that cannot be made is reported, not silently deferred forever"
    );
}

// Regression: 2529309 handed the member to `teardown_member_resources_best_effort`
// and relaunched whatever came back. A teardown that could not terminate the
// pane — an ownership check that fails, a kill that errors — reported its
// failure only in its diagnostics, so the pass went on to resume a member whose
// session was still running and rendered a second one beside it.
#[test]
fn a_stop_that_failed_aborts_the_effort_resume() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    assign_task(&tmp, "builder", "high", "the migration is irreversible");
    // The pane id has been reused by another process, so the identity-aware
    // teardown refuses to kill it even though its project path still matches.
    runtime.set_pane_identity("%21", Some(4040), Some(1_755_000_040));

    let before = codex_launch_attempts(&runtime);
    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(resumed.is_empty(), "a stop that failed is not a switch");
    assert_eq!(
        codex_launch_attempts(&runtime),
        before,
        "a member whose session is still running must never be launched again"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "the level never took effect"
    );
    assert_eq!(
        record
            .effort_resume_failure
            .as_ref()
            .map(|failure| failure.level.as_str()),
        Some("high"),
        "the failure is recorded so the retry is bounded and reported"
    );
}

// Regression: 2529309 accepted any runtime record, so an assignment that
// arrived after the operator stopped a member started the member again from a
// background pass the operator never asked for.
#[test]
fn an_operator_stopped_member_is_not_restarted_by_the_effort_pass() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    assign_task(&tmp, "builder", "high", "the migration is irreversible");
    // The operator's own Stop.
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);

    let before = codex_launch_attempts(&runtime);
    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(resumed.is_empty(), "a stopped member stays stopped");
    assert_eq!(
        codex_launch_attempts(&runtime),
        before,
        "an assignment is not a reason to start a member the operator stopped"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.health, HealthState::SessionDead);
}

// Regression: d055165 skipped the relaunch outright when the configured
// resume base already pinned `model_reasoning_effort`, so the member stayed
// at the operator's configured level and the lead's assignment never took
// effect. The override belongs in the command the relaunch renders.
#[test]
fn a_base_command_that_pins_the_effort_is_relaunched_at_the_assignments_level() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut cli_commands = CliCommandSettings::default();
    cli_commands.codex.resume =
        "codex resume --last -c model_reasoning_effort=\"low\" --yolo".to_string();
    let mut orchestrator = seed_running_codex_member(&tmp, runtime.clone(), &cli_commands);
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let launch = runtime
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys),
            _ => None,
        })
        .rfind(|keys| keys.contains("codex"))
        .expect("a codex launch was sent to the pane");
    assert!(
        launch.contains("model_reasoning_effort=\\\"high\\\"")
            || launch.contains("model_reasoning_effort=\"high\""),
        "the pinned value is replaced by the assignment's level, got: {launch}"
    );
    assert!(
        !launch.contains("model_reasoning_effort=\\\"low\\\"")
            && !launch.contains("model_reasoning_effort=\"low\""),
        "the level the assignment replaced must not survive, got: {launch}"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// The other half of the same rule: a pin the rewrite cannot read leaves the
// command unable to carry the level, and stopping a working member for a
// switch that cannot land buys nothing.
#[test]
fn a_pin_the_rewrite_cannot_read_leaves_the_member_running() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut cli_commands = CliCommandSettings::default();
    // A trailing bare key: there is no value token to replace.
    cli_commands.codex.resume = "codex resume --last --yolo -c model_reasoning_effort".to_string();
    let mut orchestrator = seed_running_codex_member(&tmp, runtime.clone(), &cli_commands);
    assign_task(&tmp, "builder", "high", "the migration is irreversible");

    let before = codex_launch_attempts(&runtime);
    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert!(
        resumed.is_empty(),
        "a relaunch that cannot carry the level is not a switch"
    );
    assert_eq!(
        codex_launch_attempts(&runtime),
        before,
        "the member is not stopped for a command that cannot carry the level"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(record.health, HealthState::Healthy);
}
