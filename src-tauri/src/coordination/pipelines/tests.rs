use super::*;
use fs2::FileExt;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use taurhaus_lib::logging::{install_global_sink, LogFileState};
use taurhaus_lib::session_scanner::launch_base::{
    AliasExpansion, LaunchAccountResult, ResolvedBase,
};
use tempfile::TempDir;

use crate::coordination::backend::fake::FakeBackend;
use crate::coordination::backend::{
    BackendCapabilities, BackendKind, CoordinationBackend, MeshBridgedBackend,
};
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::member_activation::{
    MemberActivationContext, MemberActivationDeliveryPolicy,
};
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentRequest, AgentSetupConfig, DeliveryRequest, DeliveryResult, InitializeTeamRequest,
    LaunchRequest, LaunchResult, LeadMode, ProbeRequest, ProbeResult, ResumeMemberRequest,
    StepStatus, TeardownRequest, TeardownResult, WakeDisposition,
};
use crate::coordination::runtime::{
    CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
};
use crate::coordination::stores::lock::TargetFileLock;
use crate::coordination::stores::{
    MemberRuntimeSnapshot, MemberRuntimeStore, MeshInboxStore, RuntimeCommitOutcome,
    TeamConfigStore,
};
use crate::coordination::task_effort::EffortPassScope;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::{spec, CliTool};
use crate::templates::storage::TemplateStore;
use crate::templates::types::BehavioralContract;

// Regression: 216e51e9 used a repairing hydration predicate to reject seats,
// emitting a catalog-substitution event even though validation refuses launch.
#[test]
fn wave2_review_validation_does_not_claim_model_substitution() {
    let _guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("validation.jsonl");
    let sink = LogFileState::new(log_path.clone()).unwrap();
    install_global_sink(&sink);
    let mut seat = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        tmp.path().to_str().unwrap(),
    );
    seat.model = Some("opus".into());
    let error = crate::coordination::validation::validate_member_configuration(&seat, tmp.path())
        .unwrap_err();
    assert!(matches!(error, CoordinationError::Validation(_)));
    sink.flush_for_test().unwrap();
    assert!(
        !fs::read_to_string(&log_path)
            .unwrap_or_default()
            .contains("launch.model.invalid"),
        "rejection must not claim that a default was substituted"
    );
    assert!(
        crate::coordination::member_activation::validated_role_model(
            CliTool::Codex,
            "opus",
            "builder",
            "resume_hydration"
        )
        .is_none()
    );
    sink.flush_for_test().unwrap();
    assert!(fs::read_to_string(&log_path)
        .unwrap()
        .contains("launch.model.invalid"));
}

// Regression: 216e51e9 put seat filesystem/template checks before the existing
// agent-name validation, masking empty and duplicate names with cwd errors.
#[test]
fn wave2_review_initialize_checks_names_before_seat_configuration() {
    for (name, expected) in [
        ("", "agent name must not be empty"),
        ("lead", "duplicate member name 'lead'"),
    ] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        let request = InitializeTeamRequest {
            messaging: None,
            team_name: "invalid-team".into(),
            team_description: None,
            lead: setup_config("lead", "claude", "opus", tmp.path().to_str().unwrap()),
            lead_mode: LeadMode::LaunchNew,
            agents: vec![setup_config(
                name,
                "codex",
                "gpt-6-astra",
                tmp.path().join("missing").to_str().unwrap(),
            )],
        };
        let report = orchestrator.initialize_team(&request).unwrap();
        assert!(format!("{report:?}").contains(expected), "{report:?}");
        assert!(runtime.calls().is_empty());
    }
}

// Regression: a79d392 allowed recreation/resume to retain dead cwd and
// unresolved models (F2/F13), and silently hydrated incoherent role/tool seats.
// Regression: 0f973a63 erased persisted `external` declarations on load,
// making an invalid model indistinguishable from an absent, defaultable model.
#[test]
fn wave2_member_validation_rejects_invalid_create_add_and_resume_before_launch() {
    for (field, value) in [
        ("cwd", "missing"),
        ("model", "external"),
        ("model", " ExTeRnAl "),
        ("model", "opus"),
        ("cli_tool", "mismatch"),
    ] {
        let tmp = TempDir::new().unwrap();
        let backend = Arc::new(FakeBackend::default());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, backend, runtime.clone());
        let project = tmp.path().to_str().unwrap();
        let mut agent = setup_config("invalid-seat", "codex", "gpt-6-astra", project);
        match field {
            "cwd" => agent.project_id = tmp.path().join(value).to_string_lossy().into_owned(),
            "model" => agent.model = value.to_string(),
            _ => {
                agent.role_id = Some("v3-lead-claude".to_string());
            }
        }
        let request = InitializeTeamRequest {
            messaging: None,
            team_name: "invalid-team".to_string(),
            team_description: None,
            lead: setup_config("lead", "claude", "opus", project),
            lead_mode: LeadMode::LaunchNew,
            agents: vec![agent.clone()],
        };
        let report = orchestrator.initialize_team(&request).unwrap();
        assert!(
            report.failed_step.is_some(),
            "create accepted invalid {field}"
        );
        let rendered = format!("{report:?}");
        assert!(
            rendered.contains("invalid-seat") && rendered.contains(field),
            "{rendered}"
        );
        assert!(!tmp.path().join("invalid-team/config.json").exists());

        orchestrator.create_team("existing-team", None).unwrap();
        let report = orchestrator
            .add_agent_to_team(&AddAgentRequest {
                team_name: "existing-team".to_string(),
                agent: agent.clone(),
            })
            .unwrap();
        assert!(report.failed_step.is_some(), "add accepted invalid {field}");

        // Recreate the archived config through the store, bypassing today's
        // create validation exactly as an older writer would.
        let member = member_from_agent_setup(&agent, MemberRole::Agent).unwrap();
        let mut config = TeamConfigStore::load(tmp.path(), "existing-team").unwrap();
        config.members.push(member);
        TeamConfigStore::save(tmp.path(), "existing-team", &config).unwrap();
        let report = orchestrator
            .resume_member("existing-team", "invalid-seat")
            .unwrap();
        assert!(
            report.failed_step.is_some(),
            "resume accepted invalid {field}"
        );
        let rendered = format!("{report:?}");
        assert!(
            rendered.contains("invalid-seat") && rendered.contains(field),
            "{rendered}"
        );
        assert!(
            runtime.calls().is_empty(),
            "invalid configuration reached runtime"
        );
    }
}

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
fn member_action_warning_surfaces_unread_not_attempted_dispositions() {
    // Regression: d4ebdf76 surfaced only `Failed` wake dispositions, hiding
    // durable notices that a confirmed dead or absent pane would not read.
    for reason in ["member pane is dead", "member pane not found"] {
        assert_eq!(
            members::onboarding_wake_warning(&WakeDisposition::NotAttempted {
                reason: reason.to_string(),
            }),
            Some(format!("onboarding wake not attempted: {reason}"))
        );
    }

    assert_eq!(
        members::onboarding_wake_warning(&WakeDisposition::NotAttempted {
            reason: "member uses a native inbox poller".to_string(),
        }),
        None,
        "native inbox polling is an expected no-wake disposition"
    );
}

#[test]
fn failed_add_agent_report_preserves_delivery_warnings() {
    // Regression: 7fdad577 added a frontend warning branch while the backend's
    // structured add-agent failure helper discarded warnings already collected.
    let mut steps = Vec::new();

    let report = helpers::failed_add_agent_report(
        "wake-warning-add",
        "builder",
        "update_roster",
        CoordinationError::StoreError("forced commit failure".to_string()),
        vec!["send_onboarding".to_string()],
        &mut steps,
        vec!["onboarding wake failed: forced wake failure".to_string()],
    );

    assert_eq!(
        report.warnings,
        vec!["onboarding wake failed: forced wake failure".to_string()]
    );
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

#[derive(Debug)]
struct DeliveryWakePipelineRuntime {
    inner: RecordingCoordinationRuntime,
    teams_dir: PathBuf,
    mesh_spawn_count: AtomicUsize,
}

impl DeliveryWakePipelineRuntime {
    fn new(teams_dir: &Path) -> Self {
        let inner = RecordingCoordinationRuntime::default();
        inner.set_mesh_join_teams_dir(teams_dir);
        Self {
            inner,
            teams_dir: teams_dir.to_path_buf(),
            mesh_spawn_count: AtomicUsize::new(0),
        }
    }
}

impl CoordinationRuntime for DeliveryWakePipelineRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn create_aitx_pane_and_launch_in_target(
        &self,
        project_id: &str,
        target_pane: &str,
        launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        self.inner
            .create_aitx_pane_and_launch_in_target(project_id, target_pane, launch_cmd)
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
        )
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        let spawn_index = self.mesh_spawn_count.fetch_add(1, Ordering::SeqCst);
        if spawn_index > 0 {
            return Err(CoordinationError::Backend(
                "forced onboarding wake spawn failure".to_string(),
            ));
        }

        let pid = self
            .inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)?;
        let mut runtime = MemberRuntimeStore::load(&self.teams_dir, team_name, member_name)?;
        runtime.pane_id = Some(pane_id.to_string());
        runtime.daemon_pid = None;
        MemberRuntimeStore::save(&self.teams_dir, team_name, member_name, &runtime)?;
        Ok(pid)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn find_existing_mesh_daemon_pids(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        self.inner
            .find_existing_mesh_daemon_pids(pane_id, team_name, member_name)
    }

    fn find_existing_mesh_daemon_pid_by_member(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<Option<u32>, CoordinationError> {
        self.inner
            .find_existing_mesh_daemon_pid_by_member(team_name, member_name)
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

    fn live_pane(
        &self,
        pane_id: &str,
    ) -> Result<Option<crate::coordination::runtime::LivePane>, CoordinationError> {
        self.inner.live_pane(pane_id)
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

use crate::coordination::state::test_support::fixture_project;

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
        model: crate::models::ModelCatalog::default_for(cli_tool).map(|entry| entry.id.clone()),
        reasoning_effort: None,
        account_id: None,
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
        delivery: None,
        account_id: None,
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
                fixture_project("builder").as_str(),
            ),
        )
        .expect("add member");

    let member_config = setup_config(
        member_name,
        "codex",
        "gpt-5.4",
        fixture_project("builder").as_str(),
    );
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
                fixture_project("builder").as_str(),
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

    let member_config = setup_config(
        member_name,
        "codex",
        "gpt-5.4",
        fixture_project("builder").as_str(),
    );
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
                fixture_project("builder").as_str(),
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
    let member_config = setup_config(
        member_name,
        "codex",
        "gpt-5.4",
        fixture_project("builder").as_str(),
    );
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
                fixture_project("builder").as_str(),
            ),
        )
        .expect("add member");

    let member_config = setup_config(
        member_name,
        "codex",
        "gpt-5.4",
        fixture_project("builder").as_str(),
    );
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
                fixture_project("builder").as_str(),
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

    let member_config = setup_config(
        member_name,
        "codex",
        "gpt-5.4",
        fixture_project("builder").as_str(),
    );
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
                messaging: None,
                team_name: "initialize-team".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(
                    "team-lead",
                    "codex",
                    "gpt-5.4",
                    fixture_project("lead").as_str(),
                ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    let add_agent_report = add_agent_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-team".to_string(),
                agent: setup_config(
                    "builder",
                    "codex",
                    "gpt-5.4",
                    fixture_project("builder").as_str(),
                ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    resume_orchestrator
        .add_member(
            "resume-team",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
                messaging: None,
                team_name: "initialize-claude".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(
                    "team-lead",
                    "claude",
                    "claude-opus-4-6",
                    fixture_project("lead").as_str(),
                ),
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
            && claude_dir == &crate::session_scanner::accounts::to_launch_namespace(
                initialize_tmp.path().parent().expect("Claude config dir")
            )
            .to_string_lossy()
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
                messaging: None,
                team_name: "initialize-sidecar".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(
                    "team-lead",
                    "codex",
                    "gpt-5.4",
                    fixture_project("lead").as_str(),
                ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    let add_agent_claude_report = add_agent_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-team".to_string(),
                agent: setup_config(
                    "researcher",
                    "claude",
                    "claude-opus-4-6",
                    fixture_project("research").as_str(),
                ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    let add_agent_sidecar_report = add_agent_sidecar_orchestrator
        .add_agent_to_team_with_cli_commands_and_layout(
            &AddAgentRequest {
                team_name: "add-agent-sidecar".to_string(),
                agent: setup_config(
                    "builder",
                    "codex",
                    "gpt-5.4",
                    fixture_project("builder").as_str(),
                ),
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
                fixture_project("lead-project").as_str(),
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
                fixture_project("research").as_str(),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    resume_sidecar_orchestrator
        .add_member(
            "resume-sidecar",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
        messaging: None,
        team_name: "initialize-team".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "init-builder",
            "codex",
            "gpt-5.4",
            fixture_project("init-builder").as_str(),
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
                fixture_project("lead-project").as_str(),
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
                fixture_project("init-builder").as_str(),
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
                fixture_project("add-builder").as_str(),
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
                fixture_project("resume-builder").as_str(),
            ),
        )
        .expect("add resume member");

    let add_agent_request = AddAgentRequest {
        team_name: "parity-team".to_string(),
        agent: setup_config(
            "add-builder",
            "codex",
            "gpt-5.4",
            fixture_project("add-builder").as_str(),
        ),
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
        &setup_config(
            "init-builder",
            "codex",
            "gpt-5.4",
            fixture_project("init-builder").as_str(),
        ),
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
        &setup_config(
            "add-builder",
            "codex",
            "gpt-5.4",
            fixture_project("add-builder").as_str(),
        ),
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
        fixture_project("lead").as_str(),
        MemberRole::Agent,
        CliTool::Claude,
        "opus",
        &std::path::PathBuf::from("/accounts/claude-work/teams"),
    )
    .expect("claude join result");
    let codex_joined = join_mesh_if_required(
        &runtime,
        "architecture-final",
        "builder",
        fixture_project("builder").as_str(),
        MemberRole::Agent,
        CliTool::Codex,
        "gpt-5.6-sol",
        &std::path::PathBuf::from("/accounts/claude-work/teams"),
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
            && project_id == fixture_project("builder").as_str()
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
        &std::path::PathBuf::from("/accounts/claude-work/teams"),
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
        project_id: fixture_project("project"),
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
        delivery: None,
        account_id: None,
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
        project_id: fixture_project("project"),
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
        delivery: None,
        account_id: None,
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
    let agent = setup_config(
        "builder",
        "claude",
        "opus",
        fixture_project("project").as_str(),
    );
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = format!("{variable}=low claude --dangerously-skip-permissions");

    let rendered =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("the launch still renders without the frozen level");
    assert_eq!(
        rendered,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions --model 'opus' ",
            "--team-name 'architecture-final' --agent-name 'builder' ",
            "--agent-id 'builder@architecture-final' --agent-type 'general-purpose' -n 'builder'"
        )
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
    let agent = setup_config(
        "builder",
        "claude",
        "opus",
        fixture_project("project").as_str(),
    );
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

    let agent = setup_config(
        "builder",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );
    let mut commands = crate::models::CliCommandSettings::default();
    let untrusted =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("untrusted command");
    commands.codex_bypass_hook_trust = true;
    let trusted =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("trusted command");
    assert_eq!(untrusted, "codex --yolo -m 'gpt-5.4'");
    assert_eq!(
        trusted,
        "codex --yolo --dangerously-bypass-hook-trust -m 'gpt-5.4'"
    );
}

#[test]
fn managed_codex_team_launch_carries_the_account_selector() {
    // Regression: 08c3961 registered CODEX_HOME for direct launches but left
    // coordination sidecars on the process-implicit account directory.
    let agent = setup_config(
        "builder",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );
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

#[test]
fn managed_codex_member_launch_resolves_its_persisted_account_id() {
    let mut agent = setup_config(
        "builder",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );
    agent.account_id = Some("codex-work".to_string());
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![crate::models::ManagedLaunchAccount {
            id: "codex-work".to_string(),
            label: "Work".to_string(),
            dir: std::path::PathBuf::from("/accounts/codex-work"),
            logged_in: true,
            is_default: false,
        }],
    );
    commands.account_selector_dirs.insert(
        "CODEX_HOME".to_string(),
        std::path::PathBuf::from("/accounts/codex-personal"),
    );

    let result = render_team_launch(
        &commands,
        CliTool::Codex,
        "gpt-5.4",
        None,
        "architecture-final",
        "builder",
        MemberRole::Agent,
        false,
        None,
        agent.account_id.as_deref(),
    )
    .expect("managed command");

    assert_eq!(
        result.command,
        "CODEX_HOME='/accounts/codex-work' codex --yolo -m 'gpt-5.4'"
    );
    assert_eq!(result.account.account_applied, Some(true));
    assert_eq!(result.account.account_id.as_deref(), Some("codex-work"));
    assert_eq!(result.account.account_label.as_deref(), Some("Work"));
    assert_eq!(result.account.fallback_from, None);
}

#[test]
fn managed_claude_team_launch_uses_the_selected_team_root() {
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Claude,
        vec![crate::models::ManagedLaunchAccount {
            id: "claude-work".to_string(),
            label: "Work".to_string(),
            dir: std::path::PathBuf::from("/accounts/claude-work"),
            logged_in: true,
            is_default: false,
        }],
    );
    commands.account_selector_dirs.insert(
        "CLAUDE_CONFIG_DIR".to_string(),
        std::path::PathBuf::from("/accounts/claude-work"),
    );

    let result = render_team_launch(
        &commands,
        CliTool::Claude,
        "opus",
        None,
        "architecture-final",
        "team-lead",
        MemberRole::Lead,
        false,
        None,
        Some("claude-work"),
    )
    .expect("team-scoped Claude account");

    assert!(result
        .command
        .starts_with("CLAUDE_CONFIG_DIR='/accounts/claude-work'"));
    assert_eq!(result.account.account_applied, Some(true));
    assert_eq!(result.account.account_id.as_deref(), Some("claude-work"));
}

#[test]
fn managed_claude_team_launch_rejects_a_member_account_outside_the_team_root() {
    // Regression: 18810949 removed the mixed-account Claude guard without
    // pinning hot-added members to the registry-resolved team account.
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Claude,
        vec![
            crate::models::ManagedLaunchAccount {
                id: "claude-team".to_string(),
                label: "Team".to_string(),
                dir: std::path::PathBuf::from("/accounts/claude-team"),
                logged_in: true,
                is_default: false,
            },
            crate::models::ManagedLaunchAccount {
                id: "claude-other".to_string(),
                label: "Other".to_string(),
                dir: std::path::PathBuf::from("/accounts/claude-other"),
                logged_in: true,
                is_default: false,
            },
        ],
    );
    commands.account_selector_dirs.insert(
        "CLAUDE_CONFIG_DIR".to_string(),
        std::path::PathBuf::from("/accounts/claude-team"),
    );

    let error = render_team_launch(
        &commands,
        CliTool::Claude,
        "opus",
        None,
        "architecture-final",
        "reviewer",
        MemberRole::Agent,
        false,
        None,
        Some("claude-other"),
    )
    .expect_err("mixed-account Claude member must be rejected");

    assert!(error.to_string().contains("one team account"), "{error}");
}

#[test]
fn unavailable_member_account_falls_back_loudly_to_the_registry_home() {
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("member-account-fallback.log.jsonl");
    let log_state = LogFileState::new(log_path.clone()).expect("log state");
    install_global_sink(&log_state);
    let mut commands = crate::models::CliCommandSettings::default();
    commands.account_selector_dirs.insert(
        "CODEX_HOME".to_string(),
        std::path::PathBuf::from("/accounts/codex-personal"),
    );
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![crate::models::ManagedLaunchAccount {
            id: "personal".to_string(),
            label: "Personal".to_string(),
            dir: std::path::PathBuf::from("/accounts/codex-personal"),
            logged_in: true,
            is_default: true,
        }],
    );

    let result = render_team_launch(
        &commands,
        CliTool::Codex,
        "gpt-5.4",
        None,
        "fallback-team",
        "builder",
        MemberRole::Agent,
        false,
        None,
        Some("missing-work"),
    )
    .expect("fallback remains launchable");

    assert!(result
        .command
        .starts_with("CODEX_HOME='/accounts/codex-personal'"));
    assert_eq!(result.account.account_applied, Some(false));
    assert_eq!(result.account.account_id.as_deref(), Some("personal"));
    assert_eq!(result.account.account_label.as_deref(), Some("Personal"));
    assert_eq!(
        result.account.fallback_from.as_deref(),
        Some("missing-work")
    );
    let contents =
        wait_for_pipeline_log_contains(&log_path, "\"event\":\"launch.account.fallback\"");
    assert!(contents.contains("\"member\":\"builder\""));
    assert!(contents.contains("\"requested_account_id\":\"missing-work\""));
}

// Regression: cba5b9941 described a missing managed-account detection result
// as a signed-out account even though detection had never run.
#[test]
fn unavailable_account_without_detection_reports_detection_unavailable() {
    let mut commands = crate::models::CliCommandSettings::default();
    commands.account_selector_dirs.insert(
        "CODEX_HOME".to_string(),
        std::path::PathBuf::from("/accounts/codex-personal"),
    );

    let result = render_team_launch(
        &commands,
        CliTool::Codex,
        "gpt-5.4",
        None,
        "fallback-team",
        "builder",
        MemberRole::Agent,
        false,
        None,
        Some("missing-work"),
    )
    .expect("fallback remains launchable without account detection");

    assert_eq!(
        result.account.account_note_detail.as_deref(),
        Some("account detection unavailable")
    );
}

#[test]
fn signed_out_member_account_fallback_uses_its_human_label() {
    let mut commands = crate::models::CliCommandSettings::default();
    commands.account_selector_dirs.insert(
        "CODEX_HOME".to_string(),
        std::path::PathBuf::from("/accounts/codex-personal"),
    );
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![
            crate::models::ManagedLaunchAccount {
                id: "personal-id".to_string(),
                label: "Personal".to_string(),
                dir: std::path::PathBuf::from("/accounts/codex-personal"),
                logged_in: true,
                is_default: true,
            },
            crate::models::ManagedLaunchAccount {
                id: "3f2a1b8c-0000-0000-0000-000000000000".to_string(),
                label: "Work".to_string(),
                dir: std::path::PathBuf::from("/accounts/codex-work"),
                logged_in: false,
                is_default: false,
            },
        ],
    );

    let result = render_team_launch(
        &commands,
        CliTool::Codex,
        "gpt-5.4",
        None,
        "fallback-team",
        "builder",
        MemberRole::Agent,
        false,
        None,
        Some("3f2a1b8c-0000-0000-0000-000000000000"),
    )
    .expect("signed-out selection falls back");

    assert_eq!(result.account.fallback_from.as_deref(), Some("Work"));
    assert_eq!(result.account.account_label.as_deref(), Some("Personal"));
}

#[test]
fn selectorless_member_without_detected_account_has_no_fabricated_account_result() {
    let result = render_team_launch(
        &crate::models::CliCommandSettings::default(),
        CliTool::Agy,
        "gemini-3.1-pro-high",
        None,
        "architecture-final",
        "researcher",
        MemberRole::Agent,
        false,
        None,
        None,
    )
    .expect("selectorless launch");

    assert_eq!(result.account, LaunchAccountResult::default());
}

#[test]
fn managed_team_launch_defeats_a_base_alias_account_selector() {
    // Regression: commit 0f2bfbb0 resolved aliases only on the app-launch path,
    // so a managed member still ran on the account selected inside `claude2`.
    let agent = setup_config(
        "builder",
        "claude",
        "opus",
        fixture_project("project").as_str(),
    );
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = "claude2 --dangerously-skip-permissions".to_string();
    commands.account_selector_dirs.insert(
        "CLAUDE_CONFIG_DIR".to_string(),
        std::path::PathBuf::from("/home/user/.claude"),
    );
    commands.resolved_bases.insert(
        (CliTool::Claude, crate::daemon::protocol::LaunchMode::Fresh),
        ResolvedBase {
            command: "CLAUDE_CONFIG_DIR=/home/user/.claude-account2 claude --dangerously-skip-permissions"
                .to_string(),
            expansions: vec![AliasExpansion {
                name: "claude2".to_string(),
                body: "CLAUDE_CONFIG_DIR=/home/user/.claude-account2 claude".to_string(),
            }],
            opaque_head: None,
        },
    );

    let command =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("managed command");

    assert_eq!(
        command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "CLAUDE_CONFIG_DIR='/home/user/.claude' claude --dangerously-skip-permissions ",
            "--model 'opus' --team-name 'architecture-final' --agent-name 'builder' ",
            "--agent-id 'builder@architecture-final' --agent-type 'general-purpose' -n 'builder'"
        )
    );
}

#[test]
fn unavailable_team_base_resolution_launches_the_literal_and_logs_once() {
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("team-base-unresolved.log.jsonl");
    let log_state = LogFileState::new(log_path.clone()).expect("log state");
    install_global_sink(&log_state);
    let agent = setup_config(
        "base-unresolved-member",
        "claude",
        "opus",
        fixture_project("project").as_str(),
    );
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = "claude2 --dangerously-skip-permissions".to_string();

    let command =
        build_cli_launch_command(&agent, "base-unresolved-team", MemberRole::Agent, &commands)
            .expect("resolution failure must not block the launch");

    assert_eq!(
        command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude2 --dangerously-skip-permissions --model 'opus' ",
            "--team-name 'base-unresolved-team' --agent-name 'base-unresolved-member' ",
            "--agent-id 'base-unresolved-member@base-unresolved-team' ",
            "--agent-type 'general-purpose' -n 'base-unresolved-member'"
        )
    );
    let contents =
        wait_for_pipeline_log_contains(&log_path, "\"event\":\"launch.base.unresolved\"");
    assert_eq!(
        contents
            .lines()
            .filter(|line| {
                line.contains("\"event\":\"launch.base.unresolved\"")
                    && line.contains("\"member\":\"base-unresolved-member\"")
                    && line.contains("\"team\":\"base-unresolved-team\"")
            })
            .count(),
        1,
        "one unresolved resolution should produce one event: {contents}"
    );
}

#[test]
fn opaque_team_base_reports_the_account_note_and_logs_once() {
    // Regression: commit 0f2bfbb0 surfaced opaque launch bases only for app
    // launches, leaving a managed member's account selection silently unclear.
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("team-base-opaque.log.jsonl");
    let log_state = LogFileState::new(log_path.clone()).expect("log state");
    install_global_sink(&log_state);
    let mut commands = crate::models::CliCommandSettings::default();
    commands.resolved_bases.insert(
        (CliTool::Claude, crate::daemon::protocol::LaunchMode::Fresh),
        ResolvedBase {
            command: "team-wrapper claude --dangerously-skip-permissions".to_string(),
            expansions: Vec::new(),
            opaque_head: Some("team-wrapper".to_string()),
        },
    );

    let result = render_team_launch(
        &commands,
        CliTool::Claude,
        "opus",
        None,
        "opaque-base-team",
        "opaque-base-member",
        MemberRole::Agent,
        false,
        None,
        None,
    )
    .expect("opaque wrapper must remain launchable");

    assert_eq!(
        result.command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "team-wrapper claude --dangerously-skip-permissions --model 'opus' ",
            "--team-name 'opaque-base-team' --agent-name 'opaque-base-member' ",
            "--agent-id 'opaque-base-member@opaque-base-team' ",
            "--agent-type 'general-purpose' -n 'opaque-base-member'"
        )
    );
    assert_eq!(result.account.account_applied, Some(false));
    assert_eq!(
        result.account.account_note.as_deref(),
        Some("opaque_base_command")
    );
    assert_eq!(
        result.account.account_note_detail.as_deref(),
        Some("team-wrapper")
    );
    let contents = wait_for_pipeline_log_contains(&log_path, "\"event\":\"launch.base.opaque\"");
    assert_eq!(
        contents
            .lines()
            .filter(|line| {
                line.contains("\"event\":\"launch.base.opaque\"")
                    && line.contains("\"member\":\"opaque-base-member\"")
                    && line.contains("\"team\":\"opaque-base-team\"")
            })
            .count(),
        1,
        "one opaque resolution should produce one event: {contents}"
    );
}

#[test]
fn initialized_member_persists_the_opaque_base_account_note() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let mut commands = crate::models::CliCommandSettings::default();
    commands.resolved_bases.insert(
        (CliTool::Codex, crate::daemon::protocol::LaunchMode::Fresh),
        ResolvedBase {
            command: "team-wrapper codex --yolo".to_string(),
            expansions: Vec::new(),
            opaque_head: Some("team-wrapper".to_string()),
        },
    );

    let report = orchestrator
        .initialize_team_with_cli_commands_and_layout(
            &InitializeTeamRequest {
                messaging: None,
                team_name: "opaque-runtime-team".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(
                    "team-lead",
                    "codex",
                    "gpt-5.4",
                    fixture_project("lead").as_str(),
                ),
                agents: Vec::new(),
            },
            &commands,
            "new_window",
        )
        .expect("initialize report");
    assert!(
        report.failed_step.is_none(),
        "initialize failed: {report:?}"
    );

    let runtime = MemberRuntimeStore::load(tmp.path(), "opaque-runtime-team", "team-lead")
        .expect("persisted member runtime");
    let account = runtime.launch_account;
    assert_eq!(account.account_applied, Some(false));
    assert_eq!(account.account_note.as_deref(), Some("opaque_base_command"));
    assert_eq!(account.account_note_detail.as_deref(), Some("team-wrapper"));
}

// Regression: 791f6be centralized team launch rendering without a managed
// Codex notify input, so the pipeline could not opt into native idle edges.
#[test]
fn managed_codex_team_launch_includes_native_notify_sink() {
    let agent = setup_config(
        "builder",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );
    let commands = crate::models::CliCommandSettings {
        codex_notify_executable: Some(std::path::PathBuf::from(
            "/home/test/.local/bin/taurhaus-daemon",
        )),
        ..Default::default()
    };

    let command =
        build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &commands)
            .expect("managed command");

    assert_eq!(
        command,
        concat!(
            "codex --yolo -c 'notify=[\"/home/test/.local/bin/taurhaus-daemon\",",
            "\"codex-notify\"]' -m 'gpt-5.4'"
        )
    );
}

// Regression: a79d392 forced the catalog's low effort onto declarations that omitted it,
// changing the command after activation instead of preserving the CLI's configured effort.
#[test]
fn initialize_and_resume_leave_undeclared_effort_to_the_cli() {
    let mut agent = setup_config("builder", "codex", "", fixture_project("project").as_str());
    agent.role_id = Some("v3-developer-codex".to_string());
    let initialize = MemberActivationContext::for_initialize_member(
        "architecture-final",
        "team-lead",
        &agent,
        MemberRole::Agent,
    )
    .expect("initialize context");

    let mut persisted = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        fixture_project("project").as_str(),
    );
    persisted.role_id = Some("v3-developer-codex".to_string());
    let resume =
        MemberActivationContext::for_resume_member("architecture-final", "team-lead", &persisted);

    let initialize_command = build_member_activation_launch_command(
        Path::new("/scratch/teams"),
        &initialize,
        &CliCommandSettings::default(),
    )
    .expect("initialize command")
    .command;
    let resume_command = build_member_activation_launch_command(
        Path::new("/scratch/teams"),
        &resume,
        &CliCommandSettings::default(),
    )
    .expect("resume command")
    .command;

    assert_eq!(initialize_command, resume_command);
    assert_eq!(
        initialize_command,
        "CLAUDE_DIR='/scratch' codex --yolo -m 'gpt-5.6-sol'"
    );
}

// Regression: 13111833 dropped a rendered launch when the member had no task
// in its operational snapshot yet, so a later assignment could never be
// attributed and routing reports stayed empty for launch-once team waves.
#[test]
fn launch_then_task_snapshot_attributes_the_rendered_launch() {
    use crate::coordination::stores::{
        telemetry::{read_task_telemetry, RoutingTelemetryEvent},
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalOwnershipSnapshot, OperationalTaskSnapshot, OperationalWorkingSetSnapshot,
    };

    let tmp = TempDir::new().expect("tempdir");
    let runtime = RecordingCoordinationRuntime::default();
    let agent = setup_config(
        "builder",
        "codex",
        "gpt-5.6-sol",
        fixture_project("project").as_str(),
    );
    let context = MemberActivationContext::for_initialize_member(
        "routing-team",
        "team-lead",
        &agent,
        MemberRole::Agent,
    )
    .expect("initialize context");
    let mut runtime_state = MemberActivationRuntimeState::default();

    run_member_session_phase(
        &runtime,
        tmp.path(),
        &context,
        "%1",
        MemberSessionPhase::LaunchOnly(&CliCommandSettings::default()),
        &mut runtime_state,
    )
    .expect("launch member");

    let unattributed = tmp
        .path()
        .join("routing-team/state/telemetry/_unattributed.jsonl");
    assert!(matches!(
        read_task_telemetry(&unattributed).as_slice(),
        [RoutingTelemetryEvent::LaunchRendered {
            task_id: None,
            model: Some(model),
            ..
        }] if model == "gpt-5.6-sol"
    ));

    crate::coordination::operational_context::publish_member_operation_snapshot(
        tmp.path(),
        &OperationalContextSnapshot {
            recovery_card: None,
            version: 1,
            team_name: "routing-team".to_string(),
            member_name: "builder".to_string(),
            updated_at: Utc::now(),
            task: OperationalTaskSnapshot {
                id: "42".to_string(),
                subject: "Implement routing telemetry".to_string(),
                status: "in_progress".to_string(),
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot::default(),
            ownership: OperationalOwnershipSnapshot::default(),
            working_set: OperationalWorkingSetSnapshot {
                project_path: fixture_project("project"),
                focal_files: Vec::new(),
            },
        },
        None,
    )
    .expect("publish task snapshot");

    let attributed = tmp.path().join("routing-team/state/telemetry/42.jsonl");
    assert!(matches!(
        read_task_telemetry(&attributed).as_slice(),
        [RoutingTelemetryEvent::LaunchRendered {
            task_id: Some(task_id),
            member,
            model: Some(model),
            ..
        }] if task_id == "42" && member == "builder" && model == "gpt-5.6-sol"
    ));
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
        project_id: fixture_project("project"),
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
        delivery: None,
        account_id: None,
    };

    let command = build_cli_launch_command(&agent, "architecture-final", MemberRole::Agent, &cmds)
        .expect("command");
    assert_eq!(
        command,
        "codex --yolo -m 'gpt-5.4' -c 'model_reasoning_effort=\"high\"'"
    );
}

/// A team member config, with everything but the tool left at its default.
fn team_agent(cli_tool: &str) -> AgentSetupConfig {
    AgentSetupConfig {
        name: "team-lead".to_string(),
        cli_tool: cli_tool.to_string(),
        model: String::new(),
        project_id: fixture_project("project"),
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
        delivery: None,
        account_id: None,
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
    assert_eq!(
        claude,
        format!(
            concat!(
                "CLAUDE_CONFIG_DIR='{}' CLAUDECODE=1 ",
                "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
                "claude --dangerously-skip-permissions --team-name 'ledger-team' ",
                "--agent-name 'team-lead' --agent-id 'team-lead@ledger-team' ",
                "--agent-type 'orchestrator' -n 'team-lead'"
            ),
            override_dir.path().display()
        )
    );

    let codex = codex.expect("codex command");
    assert_eq!(codex, "codex --yolo");
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

    assert_eq!(
        command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions --team-name 'ledger-team' ",
            "--agent-name 'team-lead' --agent-id 'team-lead@ledger-team' ",
            "--agent-type 'orchestrator' -n 'team-lead'"
        )
    );
}

#[test]
fn build_cli_launch_command_for_claude_appends_team_context() {
    let cmds = crate::models::CliCommandSettings::default();
    let agent = AgentSetupConfig {
        name: "team-lead".to_string(),
        cli_tool: "claude".to_string(),
        model: "claude-opus-4-6".to_string(),
        project_id: fixture_project("project"),
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
        delivery: None,
        account_id: None,
    };
    let command =
        build_cli_launch_command(&agent, "ledger-team", MemberRole::Lead, &cmds).expect("command");
    assert_eq!(
        command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions --model 'claude-opus-4-6' ",
            "--team-name 'ledger-team' --agent-name 'team-lead' ",
            "--agent-id 'team-lead@ledger-team' --agent-type 'orchestrator' -n 'team-lead'"
        )
    );
}

// Resume always starts a fresh session — never uses --continue or resume --last.
// Multiple agents share the same project, so checkpoint-based resume would
// pick up another agent's checkpoint.
#[test]
fn build_resume_cli_launch_command_always_uses_fresh_session() {
    // This render reads the tool home (TAURHAUS_CLAUDE_DIR): hold the shared
    // env guard so a concurrent env-mutating test cannot leak its tempdir
    // into the rendered selector.
    let _env = taurhaus_lib::test_support::acquire_env_test_guard();
    let cmds = crate::models::CliCommandSettings::default();
    let codex_agent = setup_config(
        "builder",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );

    let command = build_resume_cli_launch_command(
        &codex_agent,
        "architecture-final",
        MemberRole::Agent,
        &cmds,
    )
    .expect("command");
    assert_eq!(command, "codex --yolo -m 'gpt-5.4'");

    let claude_agent = setup_config(
        "team-lead",
        "claude",
        "opus",
        fixture_project("project").as_str(),
    );

    let command = build_resume_cli_launch_command(
        &claude_agent,
        "architecture-final",
        MemberRole::Lead,
        &cmds,
    )
    .expect("command");
    assert_eq!(
        command,
        concat!(
            "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 ",
            "claude --dangerously-skip-permissions --model 'opus' ",
            "--team-name 'architecture-final' --agent-name 'team-lead' ",
            "--agent-id 'team-lead@architecture-final' ",
            "--agent-type 'orchestrator' -n 'team-lead'"
        )
    );
}

#[test]
fn member_from_agent_setup_maps_role_template_context() {
    let mut setup = setup_config(
        "codex-dev",
        "codex",
        "gpt-5.4",
        fixture_project("project").as_str(),
    );
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

    let mut claude_agent = setup_config(
        "researcher",
        "claude",
        "claude-opus-4-6",
        fixture_project("research").as_str(),
    );
    claude_agent.role_id = Some("adversarial-reviewer-claude".to_string());
    claude_agent.instructions = Some("Investigate architecture tradeoffs.".to_string());
    claude_agent.behavioral_contract = Some(BehavioralContract {
        communication: vec!["post concise findings".to_string()],
        execution: vec!["run focused experiments".to_string()],
        escalation: vec!["escalate ambiguous requirements".to_string()],
    });
    claude_agent.capabilities = Some(vec!["analysis".to_string(), "research".to_string()]);

    let request = InitializeTeamRequest {
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
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
            assert!(payload.message.contains("[taurhaus] recovery_card"));
            assert!(payload
                .message
                .contains("Role: adversarial-reviewer-claude"));
            assert!(!payload.message.contains("Capabilities:"));
            assert!(!payload
                .message
                .contains("HOLD: minimal role steering unavailable"));
            assert!(payload.message.contains("Investigate"));
            assert!(!payload.message.contains("mesh read --unread"));
        }
        other => panic!("unexpected delivery payload for agent: {other:?}"),
    }
}

#[test]
fn initialize_pipeline_claude_agent_without_role_context_receives_unassigned_card() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

    let request = InitializeTeamRequest {
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "researcher",
            "claude",
            "claude-opus-4-6",
            fixture_project("research").as_str(),
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
        2,
        "lead and unassigned Claude seat each receive a baseline"
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "codex",
            "gpt-5.4",
            fixture_project("builder").as_str(),
        )],
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "codex",
            "gpt-5.4",
            fixture_project("builder").as_str(),
        )],
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "claude",
            "claude-opus-4-6",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "codex",
            "gpt-5.4",
            fixture_project("builder").as_str(),
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "claude",
            "claude-opus-4-6",
            fixture_project("project").as_str(),
        ),
        agents: vec![
            setup_config(
                "dev-1",
                "codex",
                "gpt-5.4",
                fixture_project("project").as_str(),
            ),
            setup_config(
                "dev-2",
                "codex",
                "gpt-5.4",
                fixture_project("project").as_str(),
            ),
            setup_config(
                "architect-1",
                "codex",
                "gpt-5.4",
                fixture_project("project").as_str(),
            ),
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
                project_id: fixture_project("project"),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: fixture_project("project"),
                target_pane: "test-pane-1".to_string(),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: fixture_project("project"),
                target_pane: "test-pane-1".to_string(),
            },
            RuntimeCall::CreatePaneInTarget {
                project_id: fixture_project("project"),
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "claude",
            "claude-opus-4-6",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "codex",
            "gpt-5.4",
            fixture_project("builder").as_str(),
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "claude",
            "claude-opus-4-6",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "codex",
            "gpt-5.4",
            fixture_project("builder").as_str(),
        )],
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "researcher",
            "claude",
            "claude-opus-4-6",
            fixture_project("research").as_str(),
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
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: Some("Review pipeline".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![
            setup_config(
                "builder",
                "codex",
                "gpt-5.4",
                fixture_project("builder").as_str(),
            ),
            setup_config(
                "reviewer",
                "claude",
                "claude-opus-4-6",
                fixture_project("reviewer").as_str(),
            ),
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
        Some(std::path::Path::new(fixture_project("lead").as_str()))
    );

    let reviewer_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "reviewer").expect("runtime");
    assert_eq!(reviewer_runtime.cli_tool, Some(CliTool::Claude));
    assert_eq!(
        reviewer_runtime.project_path.as_deref(),
        Some(std::path::Path::new(fixture_project("reviewer").as_str()))
    );
}

#[test]
fn initialize_pipeline_progress_callback_preserves_batch_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);

    let request = InitializeTeamRequest {
        messaging: None,
        team_name: "architecture-final".to_string(),
        team_description: None,
        lead_mode: LeadMode::LaunchNew,
        lead: setup_config(
            "team-lead",
            "codex",
            "gpt-5.4",
            fixture_project("lead").as_str(),
        ),
        agents: vec![setup_config(
            "builder",
            "claude",
            "claude-opus-4-6",
            fixture_project("builder").as_str(),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "builder".to_string(),
                role: MemberRole::Agent,
                role_id: Some("v4-developer-codex".to_string()),
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
                model: Some("gpt-5.6-sol".to_string()),
                reasoning_effort: None,
                account_id: None,
                project_path: PathBuf::from(fixture_project("builder")),
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
    assert_eq!(loaded_member.role_id.as_deref(), Some("v4-developer-codex"));
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
    assert_eq!(loaded_member.model.as_deref(), Some("gpt-5.6-sol"));
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
            if member_name == "builder" && model == "gpt-5.6-sol"
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SendKeys { keys, .. }
            if keys.contains("-m 'gpt-5.6-sol'")
                && keys.contains("model_reasoning_effort=\"high\"")
    )));
}

#[test]
fn rollback_resume_with_deleted_home_preserves_thread_fence() {
    // Regression: 06f76b01 excluded app_server but not host_rollback from the
    // missing-rollout fallback, losing the thread before rollback could resume it.
    for detected in ["replacement-thread", "owned-thread"] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        orchestrator.create_team("team", None).unwrap();
        orchestrator
            .add_member(
                "team",
                member(
                    "seat",
                    MemberRole::Lead,
                    CliTool::Codex,
                    tmp.path().to_str().unwrap(),
                ),
            )
            .unwrap();
        let home = tmp.path().join("deleted-home");
        fs::create_dir(&home).unwrap();
        let rollout = home.join("rollout.jsonl");
        fs::write(&rollout, "").unwrap();
        fs::remove_dir_all(&home).unwrap();
        let mut record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        record.health = HealthState::SessionDead;
        record.session_id = Some("owned-thread".into());
        record.jsonl_path = Some(rollout);
        record.host_rollback = Some(serde_json::json!({"attachment": {
            "contract": 1, "socketPath": tmp.path().join("stopped.sock"),
            "threadId": "owned-thread", "memberId": "seat", "accountRoot": tmp.path(),
            "processId": 0, "processStart": "0", "hostGeneration": "old",
            "build": "fixture", "host": "fixture", "configuration": "fixture",
            "trust": "fixture", "transport": "unix", "state": "stopped"
        }}));
        assert!(record.app_server.is_none());
        MemberRuntimeStore::save(tmp.path(), "team", "seat", &record).unwrap();
        runtime.set_detected_runtime_session("test-pane-1", CliTool::Codex, Some(detected), None);
        let mut commands = CliCommandSettings::default();
        commands
            .account_selector_dirs
            .insert("CODEX_HOME".into(), tmp.path().into());
        let report = orchestrator
            .resume_member_with_cli_commands(
                &ResumeMemberRequest {
                    team_name: "team".into(),
                    member_name: "seat".into(),
                    reasoning_effort_override: None,
                },
                &commands,
            )
            .unwrap();
        assert!(runtime.calls().iter().any(|call| matches!(call,
            RuntimeCall::SendKeys { keys, .. } if keys.contains("resume") && keys.contains("owned-thread")
        )), "rollback must launch the recorded thread: {report:?}");
        if detected == "owned-thread" {
            assert!(report.resumed, "{report:?}");
        } else {
            assert!(!report.resumed);
            assert!(
                report
                    .message
                    .contains("rollback did not recover the named thread"),
                "{report:?}"
            );
            assert!(runtime.calls().iter().any(|call| matches!(call,
                RuntimeCall::KillPane { pane_id } if pane_id == "test-pane-1"
            )));
        }
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .session_id
                .as_deref(),
            Some("owned-thread")
        );
    }
}

#[test]
fn operator_resume_renders_recorded_session_for_capturing_harnesses() {
    // Regression: 4994b243 limited recorded-session resume to effort switches;
    // e2e lane 4 run 7 observed an operator resume lose the tmux conversation.
    let _guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let logs = TempDir::new().unwrap();
    let log_path = logs.path().join("resume.jsonl");
    let sink = taurhaus_lib::logging::LogFileState::new(log_path.clone()).unwrap();
    taurhaus_lib::logging::install_global_sink(&sink);
    for tool in [CliTool::Codex, CliTool::Claude, CliTool::Grok, CliTool::Agy] {
        for (session_id, effort, rollout_state) in [
            (Some("  recorded-session  "), None, "present"),
            (Some("recorded-session"), None, "deleted_home"),
            (None, None, "present"),
            (Some(""), None, "present"),
            (Some(" \t "), None, "present"),
            (Some("recorded-session"), Some("high"), "present"),
            (Some("recorded-session"), None, "missing_file"),
            (Some("recorded-session"), None, "compressed"),
        ] {
            let tmp = TempDir::new().unwrap();
            let runtime = Arc::new(RecordingCoordinationRuntime::default());
            let mut orchestrator =
                new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
            orchestrator.create_team("resume-recorded", None).unwrap();
            orchestrator
                .add_member(
                    "resume-recorded",
                    member("seat", MemberRole::Lead, tool, tmp.path().to_str().unwrap()),
                )
                .unwrap();
            let mut record =
                MemberRuntimeStore::load(tmp.path(), "resume-recorded", "seat").unwrap();
            record.session_id = session_id.map(str::to_string);
            // Regression: 106f06c7 resumed missing Codex rollouts without fallback.
            // Regression: 06f76b01 discarded ids when Codex compressed or relocated a rollout.
            let home = tmp.path().join("codex-home");
            fs::create_dir(&home).unwrap();
            let rollout = home.join("rollout.jsonl");
            fs::write(&rollout, "").unwrap();
            match rollout_state {
                "deleted_home" => fs::remove_dir_all(&home).unwrap(),
                "missing_file" => fs::remove_file(&rollout).unwrap(),
                "compressed" => fs::rename(&rollout, home.join("rollout.jsonl.zst")).unwrap(),
                _ => {}
            }
            record.jsonl_path = Some(rollout);
            // Regression: 39eeb33a / e2e lane 4 run 9 (106f06c7): the
            // supported stop's offline reconciliation discarded the conversation.
            record.health = HealthState::Healthy;
            record.pane_id = Some("%stopped".into());
            MemberRuntimeStore::save(tmp.path(), "resume-recorded", "seat", &record).unwrap();

            runtime.set_pane_exists("%stopped", false);
            orchestrator
                .reconcile_team_liveness("resume-recorded")
                .unwrap();
            let report = orchestrator
                .resume_member_with_cli_commands(
                    &ResumeMemberRequest {
                        team_name: "resume-recorded".into(),
                        member_name: "seat".into(),
                        reasoning_effort_override: effort.map(str::to_string),
                    },
                    &CliCommandSettings::default(),
                )
                .unwrap();
            assert!(report.resumed, "{report:?}");
            let calls = runtime.calls();
            let launch = calls
                .iter()
                .find_map(|call| match call {
                    RuntimeCall::SendKeys { keys, .. } => Some(keys),
                    _ => None,
                })
                .unwrap();
            let expected = match tool {
                CliTool::Codex => Some("codex resume 'recorded-session' --yolo"),
                CliTool::Claude => {
                    Some("claude --dangerously-skip-permissions --resume 'recorded-session'")
                }
                CliTool::Grok => Some("grok --always-approve --resume 'recorded-session'"),
                // Antigravity has no resume base: every resume launches fresh.
                CliTool::Agy => None,
                _ => unreachable!(),
            };
            let resumes = expected.is_some()
                && session_id.is_some_and(|id| !id.trim().is_empty())
                && !(rollout_state == "deleted_home" && tool == CliTool::Codex);
            if resumes {
                assert!(launch.contains(expected.unwrap()), "{tool}: {launch}");
            } else {
                if tool == CliTool::Agy {
                    assert!(
                        launch.contains("agy --dangerously-skip-permissions --model"),
                        "{launch}"
                    );
                }
                assert!(!launch.contains("recorded-session"), "{launch}");
                assert!(
                    !launch.contains(" resume ")
                        && !launch.contains(" --resume")
                        && !launch.contains("--conversation"),
                    "{launch}"
                );
            }
            if let Some(effort) = effort {
                let updated =
                    MemberRuntimeStore::load(tmp.path(), "resume-recorded", "seat").unwrap();
                assert_eq!(updated.applied_effort.as_deref(), Some(effort), "{launch}");
            }
            if effort.is_none() {
                MemberRuntimeStore::update(tmp.path(), "resume-recorded", "seat", |r| {
                    r.health = HealthState::Healthy;
                    r.pane_id = record.pane_id.clone();
                    r.session_id = record.session_id.clone();
                    r.jsonl_path = record.jsonl_path.clone();
                })
                .unwrap();
                // Exercise team resume from the same supported offline stop as member resume.
                orchestrator
                    .reconcile_team_liveness("resume-recorded")
                    .unwrap();
                assert_eq!(
                    MemberRuntimeStore::load(tmp.path(), "resume-recorded", "seat")
                        .unwrap()
                        .health,
                    HealthState::SessionDead
                );
                let offset = runtime.calls().len();
                let report = orchestrator
                    .resume_team_with_cli_commands_and_layout(
                        &crate::coordination::requests::ResumeTeamRequest {
                            team_name: "resume-recorded".into(),
                        },
                        &CliCommandSettings::default(),
                        "new_window",
                    )
                    .unwrap();
                assert!(report.resumed, "{report:?}");
                assert!(runtime.calls()[offset..].iter().any(|call| matches!(
                    call, RuntimeCall::SendKeys { keys, .. } if keys == launch
                )));
            }
        }
    }
    sink.flush_for_test().unwrap();
    assert!(fs::read_to_string(log_path).unwrap().lines().any(|line| {
        let event: serde_json::Value = serde_json::from_str(line).unwrap();
        event["event"] == "launch.resume.fallback" && event["level"] == "WARN"
    }));
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
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "minimal-runtime",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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

// Regression: 216e51e9 rejected persisted null/empty models before the
// existing role -> catalog hydration, and turned catalog suggestions into an allowlist.
#[test]
fn wave2_resolved_seats_preserve_defaults_and_custom_models() {
    for declared in [None, Some(""), Some("custom-model-2027")] {
        for role_model in [None, Some("custom-role-model-2027")] {
            let tmp = TempDir::new().unwrap();
            let mut orchestrator = new_orchestrator(
                &tmp,
                Arc::new(FakeBackend::default()),
                Arc::new(RecordingCoordinationRuntime::default()),
            );
            let mut seat = member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                tmp.path().to_str().unwrap(),
            );
            seat.model = declared.map(str::to_string);
            if let Some(model) = role_model {
                let store = TemplateStore::new(orchestrator.template_root.clone());
                let mut role = store.get_role("v4-developer-codex").unwrap().template;
                role.role_id = "custom-builder".into();
                role.defaults.model = model.into();
                store.create_role(&role).unwrap();
                seat.role_id = Some(role.role_id);
            }
            // The same preflight is used for create/add seats.
            crate::coordination::validation::validate_member_configuration(
                &seat,
                &orchestrator.template_root,
            )
            .expect("resolvable seat");
            orchestrator.create_team("resolved-seat", None).unwrap();
            orchestrator.add_member("resolved-seat", seat).unwrap();
            let (resolved, _, _) = orchestrator
                .load_resume_member_state(&ResumeMemberRequest {
                    team_name: "resolved-seat".into(),
                    member_name: "builder".into(),
                    reasoning_effort_override: Some("high".into()),
                })
                .expect("persisted seat must resume, including effort relaunch");
            let expected = declared
                .filter(|model| !model.is_empty())
                .or(role_model)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    crate::models::ModelCatalog::default_for(CliTool::Codex)
                        .unwrap()
                        .id
                        .clone()
                });
            assert_eq!(resolved.model.as_deref(), Some(expected.as_str()));
        }
    }
}

// Regression: 0f973a63 silently repaired the external placeholder on load,
// hiding the invalid persisted seat observed in F13.
#[test]
fn resume_external_placeholder_is_rejected_before_hydration() {
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
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead").as_str(),
            ),
        )
        .expect("add lead");
    let mut builder = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        fixture_project("builder").as_str(),
    );
    builder.role_id = Some("v4-developer-codex".to_string());
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
    assert!(!report.resumed);
    assert!(report.message.contains("builder") && report.message.contains("model"));
    assert!(runtime.calls().is_empty());
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
        .get_role("v4-developer-codex")
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
    let mut builder = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        fixture_project("builder").as_str(),
    );
    builder.role_id = Some("user-root-builder".to_string());
    builder.model = Some("gpt-5.5".to_string());
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
    let mut builder = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        fixture_project("builder").as_str(),
    );
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
                fixture_project("lead-project").as_str(),
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
                messaging: None,
                team_name: "lead-join-initialize".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(
                    "team-lead",
                    "claude",
                    "claude-opus-4-6",
                    fixture_project("lead").as_str(),
                ),
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
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead").as_str(),
            ),
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
                fixture_project("lead-project").as_str(),
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
                fixture_project("research").as_str(),
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
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);

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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "researcher".to_string(),
                role: MemberRole::Agent,
                role_id: Some("adversarial-reviewer-claude".to_string()),
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
                model: Some("opus".to_string()),
                reasoning_effort: None,
                account_id: None,
                project_path: PathBuf::from(fixture_project("research")),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add member");

    let leases_dir = tmp
        .path()
        .join("architecture-final")
        .join("state")
        .join("leases");
    fs::create_dir_all(&leases_dir).expect("create leases dir");
    fs::write(
        leases_dir.join("delivery-renderer.json"),
        r#"{"name":"delivery-renderer","state":"held","holder":"researcher","waiters":[]}"#,
    )
    .expect("write held lease");

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
    orchestrator
        .deliver_onboarding_entries(vec![entry])
        .unwrap();
    let delivered = backend.delivered_requests();
    let DeliveryRequest::OperatorNotice(notice) = &delivered[0] else {
        panic!("notice")
    };
    assert!(notice.message.contains("Leases: held delivery-renderer."));
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            Member {
                name: "researcher".to_string(),
                role: MemberRole::Agent,
                role_id: Some("adversarial-reviewer-claude".to_string()),
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
                model: Some("opus".to_string()),
                reasoning_effort: None,
                account_id: None,
                project_path: PathBuf::from(fixture_project("research")),
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
            assert!(payload.message.contains("[taurhaus] recovery_card"));
            assert!(payload
                .message
                .contains("Role: adversarial-reviewer-claude"));
            assert!(!payload.message.contains("Capabilities:"));
            assert!(!payload
                .message
                .contains("HOLD: minimal role steering unavailable"));
            assert!(payload.message.contains("Investigate"));
        }
        other => panic!("unexpected delivery payload: {other:?}"),
    }
}

#[test]
fn resume_pipeline_codex_accepts_scanner_rebound_identity() {
    // Regression: 4994b243 omitted the operator's recorded session (e2e lane 4
    // run 7). A named Codex resume may rebind to a new rollout, as in PR #172.
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
        )
        .expect("add member");

    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "builder").expect("runtime");
    member_runtime.pane_id = Some("%11".to_string());
    member_runtime.daemon_pid = Some(55);
    member_runtime.session_id = Some("recorded-session".into());
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
    assert_eq!(
        launch,
        format!(
            "CLAUDE_DIR={} codex resume 'recorded-session' --yolo -m 'gpt-5.6-sol'",
            crate::session_scanner::launch::shell_escape(
                &tmp.path().parent().unwrap().to_string_lossy()
            )
        )
    );
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
fn resume_report_carries_onboarding_wake_failure() {
    // Regression: 7fdad577 surfaced warnings from a mocked frontend payload
    // without proving the member pipeline carried the real wake disposition.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(DeliveryWakePipelineRuntime::new(tmp.path()));
    runtime.inner.set_pane_exists("%11", true);
    runtime.inner.set_pane_dead("%11", false);
    runtime.inner.set_pane_current_command("%11", Some("codex"));
    runtime
        .inner
        .set_pane_current_path("%11", Some(fixture_project("builder").as_str()));
    let backend: Arc<dyn CoordinationBackend> = Arc::new(MeshBridgedBackend::new_with_teams_dir(
        tmp.path().to_path_buf(),
    ));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);

    orchestrator
        .create_team("wake-warning-resume", None)
        .expect("create team");
    orchestrator
        .add_member(
            "wake-warning-resume",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "wake-warning-resume",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
        )
        .expect("add member");
    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), "wake-warning-resume", "builder").expect("runtime");
    member_runtime.pane_id = Some("%11".to_string());
    member_runtime.health = HealthState::SessionDead;
    MemberRuntimeStore::save(
        tmp.path(),
        "wake-warning-resume",
        "builder",
        &member_runtime,
    )
    .expect("save runtime");

    let report = orchestrator
        .resume_member("wake-warning-resume", "builder")
        .expect("resume report");

    assert!(
        report.resumed,
        "resume should remain successful: {report:?}"
    );
    assert_eq!(
        report.warnings,
        vec![
            "onboarding wake failed: daemon spawn failed: Backend error: forced onboarding wake spawn failure"
                .to_string()
        ]
    );
    assert_eq!(
        MeshInboxStore::load(tmp.path(), "wake-warning-resume", "builder")
            .expect("inbox")
            .len(),
        1,
        "report mapping must not retry the durable onboarding append"
    );
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
                fixture_project("lead-project").as_str(),
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
    assert_eq!(
        launch,
        format!(
            "CLAUDE_DIR={} codex --yolo -m 'gpt-5.6-sol'",
            crate::session_scanner::launch::shell_escape(
                &tmp.path().parent().unwrap().to_string_lossy()
            )
        )
    );
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "new-pane-identity",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
                fixture_project("builder").as_str(),
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
                fixture_project("builder").as_str(),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            "architecture-final",
            member(
                "builder",
                MemberRole::Agent,
                CliTool::Codex,
                fixture_project("builder").as_str(),
            ),
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");

    let report = orchestrator
        .add_agent_to_team(&AddAgentRequest {
            team_name: "architecture-final".to_string(),
            agent: setup_config(
                "builder",
                "codex",
                "gpt-5.4",
                fixture_project("builder").as_str(),
            ),
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
fn add_agent_report_carries_onboarding_wake_failure() {
    // Regression: 7fdad577 taught the frontend to read `warnings`, but the
    // add-agent domain and IPC reports dropped the pipeline's real warnings.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(DeliveryWakePipelineRuntime::new(tmp.path()));
    let backend: Arc<dyn CoordinationBackend> = Arc::new(MeshBridgedBackend::new_with_teams_dir(
        tmp.path().to_path_buf(),
    ));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    orchestrator
        .create_team("wake-warning-add", None)
        .expect("create team");
    orchestrator
        .add_member(
            "wake-warning-add",
            member(
                "team-lead",
                MemberRole::Lead,
                CliTool::Claude,
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");

    let report = orchestrator
        .add_agent_to_team(&AddAgentRequest {
            team_name: "wake-warning-add".to_string(),
            agent: setup_config(
                "builder",
                "codex",
                "gpt-5.4",
                fixture_project("builder").as_str(),
            ),
        })
        .expect("add-agent report");

    assert!(
        report.failed_step.is_none(),
        "wake failure is a warning after durable delivery: {report:?}"
    );
    let serialized = serde_json::to_value(&report).expect("serialize add-agent report");
    assert_eq!(
        serialized["warnings"],
        serde_json::json!([
            "onboarding wake failed: daemon spawn failed: Backend error: forced onboarding wake spawn failure"
        ])
    );
    assert_eq!(
        MeshInboxStore::load(tmp.path(), "wake-warning-add", "builder")
            .expect("inbox")
            .len(),
        1,
        "report mapping must not retry the durable onboarding append"
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");

    let entry = orchestrator
        .prepare_add_agent_onboarding_entry(&AddAgentRequest {
            team_name: "architecture-final".to_string(),
            agent: setup_config(
                "builder",
                "codex",
                "gpt-5.4",
                fixture_project("builder").as_str(),
            ),
        })
        .expect("prepare add-agent onboarding")
        .expect("add-agent onboarding entry");

    assert_eq!(entry.policy, MemberActivationDeliveryPolicy::Immediate);
}

// ---------------------------------------------------------------------------
// Task-level effort: the resume path taurhaus owns for Codex
// ---------------------------------------------------------------------------

#[test]
fn effort_attempt_budget_is_shared_by_every_pass_scope() {
    let cases = [
        ("a new switch is allowed", 0, true),
        ("a failed switch is retried", 1, true),
        ("the third failure spends the budget", 3, false),
    ];

    for (name, failed_attempts, expected) in cases {
        assert_eq!(
            super::effort::attempt_is_allowed(failed_attempts),
            expected,
            "{name}"
        );
    }
}

fn effort_team(
    tmp: &TempDir,
    runtime: Arc<RecordingCoordinationRuntime>,
    cli_tool: CliTool,
    launch_effort: Option<&str>,
) -> CoordinationOrchestrator {
    let rollout = tmp.path().join("effort.jsonl");
    fs::write(&rollout, "").unwrap();
    runtime.set_detected_runtime_session("%21", cli_tool, Some("session-effort"), rollout.to_str());
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    let mut builder = member(
        "builder",
        MemberRole::Agent,
        cli_tool,
        fixture_project("builder").as_str(),
    );
    builder.reasoning_effort = launch_effort.map(ToString::to_string);
    orchestrator
        .add_member("effort-team", builder)
        .expect("add builder");
    orchestrator
}

fn canonical_effort_team(
    root: &TempDir,
    runtime: Arc<RecordingCoordinationRuntime>,
) -> (PathBuf, CoordinationOrchestrator) {
    let teams_dir = root.path().join("teams");
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("session-effort"),
        Some("/tmp/effort.jsonl"),
    );
    runtime.set_pane_identity("%21", Some(2021), Some(1_755_000_021));
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        teams_dir.clone(),
        Arc::new(FakeBackend::default()),
        runtime,
    );
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
                fixture_project("lead-project").as_str(),
            ),
        )
        .expect("add lead");
    let mut builder = member(
        "builder",
        MemberRole::Agent,
        CliTool::Codex,
        fixture_project("builder").as_str(),
    );
    builder.reasoning_effort = Some("low".to_string());
    orchestrator
        .add_member("effort-team", builder)
        .expect("add builder");
    (teams_dir, orchestrator)
}

fn seed_running_canonical_codex_member(
    teams_dir: &Path,
    orchestrator: &mut CoordinationOrchestrator,
) {
    let mut record =
        MemberRuntimeStore::load(teams_dir, "effort-team", "builder").expect("member runtime");
    record.pane_id = Some("%21".to_string());
    record.health = HealthState::SessionDead;
    MemberRuntimeStore::save(teams_dir, "effort-team", "builder", &record)
        .expect("save offline runtime");
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
}

fn write_mesh_task(root: &TempDir, task_id: &str, status: &str, level: &str, why: &str) {
    let task_dir = root.path().join("tasks/effort-team");
    fs::create_dir_all(&task_dir).expect("create mesh task dir");
    fs::write(
        task_dir.join(format!("{task_id}.json")),
        serde_json::to_vec_pretty(&serde_json::json!({
            "id": task_id,
            "subject": format!("Task {task_id}"),
            "description": "isolated assignment fixture",
            "status": status,
            "owner": "builder",
            "activeForm": null,
            "blocks": [],
            "blockedBy": [],
            "metadata": {
                "effort": level,
                "effortWhy": why,
            },
        }))
        .expect("serialize mesh task"),
    )
    .expect("write mesh task");
}

fn block_mesh_task(root: &TempDir, task_id: &str, blocker_id: &str) {
    let path = root
        .path()
        .join("tasks/effort-team")
        .join(format!("{task_id}.json"));
    let mut task: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read mesh task")).expect("parse mesh task");
    task["blockedBy"] = serde_json::json!([blocker_id]);
    fs::write(
        path,
        serde_json::to_vec_pretty(&task).expect("serialize blocked mesh task"),
    )
    .expect("write blocked mesh task");
}

fn hold_mesh_assignment(root: &TempDir, task_id: &str) {
    let attention_dir = root
        .path()
        .join("teams/effort-team/state/projections/attention");
    fs::create_dir_all(&attention_dir).expect("create attention projection dir");
    fs::write(
        attention_dir.join(format!("{task_id}.json")),
        serde_json::to_vec_pretty(&serde_json::json!({
            "taskId": task_id,
            "assignmentId": format!("assignment-{task_id}"),
            "assignedTo": "builder",
            "assignedAt": "2026-08-30T09:00:00Z",
            "deliveryState": "pending",
            "deliveredAt": null,
        }))
        .expect("serialize attention projection"),
    )
    .expect("write attention projection");
}

fn write_mesh_attention(
    root: &TempDir,
    task_id: &str,
    delivery_state: &str,
    attention_state: &str,
    delivered_at: Option<&str>,
) {
    let attention_dir = root
        .path()
        .join("teams/effort-team/state/projections/attention");
    fs::create_dir_all(&attention_dir).expect("create attention projection dir");
    fs::write(
        attention_dir.join(format!("{task_id}.json")),
        serde_json::to_vec_pretty(&serde_json::json!({
            "taskId": task_id,
            "assignmentId": format!("assignment-{task_id}"),
            "assignedTo": "builder",
            "assignedAt": "2026-08-30T09:00:00Z",
            "deliveryState": delivery_state,
            "attentionState": attention_state,
            "deliveredAt": delivered_at,
        }))
        .expect("serialize attention projection"),
    )
    .expect("write attention projection");
}

fn write_member_snapshot_at(
    teams_dir: &Path,
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
        teams_dir,
        &OperationalContextSnapshot {
            recovery_card: None,
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
                project_path: fixture_project("builder"),
                focal_files: vec![],
            },
        },
    )
    .expect("write operational snapshot");
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
    write_member_snapshot_at(tmp.path(), member_name, task, level, why);
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

// Regression: 7bf0405e paired effort only with the one task taurhaus ranked as
// active. With two open assignments, mesh could hold the newer notice for a
// different task forever because taurhaus compared the applied level with the
// wrong assignment and saw no switch pending.
#[test]
fn two_assignments_converge_on_the_task_whose_notice_mesh_holds() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("41", "Already in progress")),
        "high",
        "the active task is risky",
    );
    write_mesh_task(
        &root,
        "41",
        "in_progress",
        "high",
        "the active task is risky",
    );
    write_mesh_task(&root, "42", "pending", "low", "the held task is mechanical");
    hold_mesh_assignment(&root, "42");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "the held task wins even though another open assignment requests more effort"
    );
}

// Regression: 9e288c56 accepted only mesh's `pending` delivery projection, so
// an unknown or failed held notice fell back to the other task's higher effort
// and could wait forever for its own requested level.
#[test]
fn held_unknown_and_failed_notices_keep_their_task_identity() {
    for (delivery_state, attention_state) in [
        ("unknown", "assigned_pending_delivery"),
        ("failed", "delivery_failed"),
    ] {
        let root = TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
        seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
        MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
            record.applied_effort = Some("high".to_string());
        })
        .expect("seed applied effort");
        write_mesh_task(
            &root,
            "41",
            "in_progress",
            "high",
            "the active task is risky",
        );
        write_mesh_task(&root, "42", "pending", "low", "the held task is mechanical");
        write_mesh_attention(&root, "42", delivery_state, attention_state, None);

        let resumed = orchestrator
            .apply_pending_task_effort(
                "effort-team",
                &CliCommandSettings::default(),
                "new_window",
                EffortPassScope::TaskChanged,
            )
            .expect("effort pass");

        assert_eq!(
            resumed,
            vec!["builder".to_string()],
            "{delivery_state} must select the held task"
        );
        let record =
            MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
        assert_eq!(
            record.applied_effort.as_deref(),
            Some("low"),
            "{delivery_state} must apply the held task's effort"
        );
    }
}

// Regression: 9e288c56 forgot the task identity behind `appliedEffort` after
// mesh delivered the notice, so the next task scan switched the running task
// to another queued assignment's higher level.
#[test]
fn delivered_notice_does_not_flip_the_running_task_to_another_assignment() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_mesh_task(
        &root,
        "41",
        "in_progress",
        "high",
        "the other task is risky",
    );
    write_mesh_task(&root, "42", "pending", "low", "the held task is mechanical");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("41", "Other task")),
        "high",
        "the other task is risky",
    );
    hold_mesh_assignment(&root, "42");

    let first = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("held effort pass");
    assert_eq!(first, vec!["builder".to_string()]);

    write_mesh_task(
        &root,
        "42",
        "in_progress",
        "low",
        "the running task is mechanical",
    );
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("42", "Running task")),
        "low",
        "the running task is mechanical",
    );
    write_mesh_attention(
        &root,
        "42",
        "delivered",
        "active",
        Some("2026-08-30T09:00:10Z"),
    );

    let second = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("post-delivery effort pass");

    assert!(
        second.is_empty(),
        "the running task already has its requested effort"
    );
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("low"));
}

#[test]
fn without_a_held_projection_the_highest_open_requested_effort_wins() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("medium".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("41", "Already in progress")),
        "low",
        "the active task is mechanical",
    );
    write_mesh_task(
        &root,
        "41",
        "in_progress",
        "low",
        "the active task is mechanical",
    );
    write_mesh_task(&root, "42", "pending", "high", "the queued task is risky");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// Policy pin: blocked mesh task records are outside the open candidate set, so
// they cannot outrank lower-effort work that is actually in progress.
#[test]
fn a_blocked_high_assignment_does_not_anchor_over_in_progress_low_work() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("medium".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("41", "Active mechanical work")),
        "low",
        "the active task is mechanical",
    );
    write_mesh_task(&root, "41", "in_progress", "low", "mechanical work");
    write_mesh_task(&root, "62", "blocked", "high", "risky when unblocked");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("low"));
}

// Regression: 2e2b52f0 treated mesh's append-only `blockedBy` history as live
// blocking even after the blocker completed, so active high-effort work could
// be silently run at a lower pending task's level.
#[test]
fn completed_blocker_history_does_not_disqualify_in_progress_high_work() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("medium".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("62", "Risky active work")),
        "high",
        "the migration is irreversible",
    );
    write_mesh_task(&root, "41", "pending", "low", "mechanical follow-up");
    write_mesh_task(&root, "61", "completed", "low", "finished prerequisite");
    write_mesh_task(&root, "62", "in_progress", "high", "risky active work");
    // Effort policy intentionally ignores append-only `blockedBy` history;
    // the task's current status is the eligibility authority.
    block_mesh_task(&root, "62", "61");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("effort pass");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// Regression: 9e288c56 let the operational-snapshot compatibility fallback
// override a blocked mesh task's authoritative status, so the widened daemon
// sweep could relaunch a live member whose only work was explicitly stood down.
#[test]
fn a_background_sweep_does_not_relaunch_blocked_only_work() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("low".to_string());
    })
    .expect("seed applied effort");
    write_mesh_task(&root, "62", "blocked", "high", "risky when unblocked");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("62", "Risky blocked work")),
        "high",
        "risky when unblocked",
    );

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .expect("effort pass");

    assert!(resumed.is_empty(), "blocked work must not start a switch");
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("low"));
}

// Regression: 4344edb4 let liveness adopt a hand-restarted session id while
// retaining the previous session's applied effort, suppressing every later
// sweep even though the foreign session could be running another level.
#[test]
fn adopting_a_foreign_session_clears_applied_effort_and_the_sweep_refires() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime.clone());
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("42", "Risky active work")),
        "high",
        "the migration is irreversible",
    );
    write_mesh_task(
        &root,
        "42",
        "in_progress",
        "high",
        "the migration is irreversible",
    );
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("hand-restarted-session"),
        Some("/tmp/hand-restarted-session.jsonl"),
    );

    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("liveness reconcile");

    let adopted =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(
        adopted.session_id.as_deref(),
        Some("hand-restarted-session")
    );
    assert_eq!(
        adopted.applied_effort, None,
        "a session taurhaus did not launch has unknown applied effort"
    );

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .expect("daemon-style sweep");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// Regression: 51b1b3be treated the first session-id capture after taurhaus's
// own launch as a foreign adoption, cleared the level that launch applied, and
// made the background sweep relaunch an already-correct member.
#[test]
fn capturing_the_session_id_after_our_launch_preserves_applied_effort() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime.clone());
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.session_id = None;
        record.jsonl_path = None;
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed launch record without captured session id");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("42", "Risky active work")),
        "high",
        "the migration is irreversible",
    );
    write_mesh_task(
        &root,
        "42",
        "in_progress",
        "high",
        "the migration is irreversible",
    );
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("launched-session"),
        Some("/tmp/launched-session.jsonl"),
    );

    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("liveness reconcile");

    let captured =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(captured.session_id.as_deref(), Some("launched-session"));
    assert_eq!(
        captured.applied_effort.as_deref(),
        Some("high"),
        "the first identity capture belongs to the level taurhaus launched"
    );

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .expect("daemon-style sweep");

    assert!(resumed.is_empty(), "the correct session must not relaunch");
}

// Regression: f02b0b1d required a recorded session id before an adopted one
// counted as foreign, so a member hand-restarted after liveness had already
// cleared that id kept asserting the dead session's level and the sweep stayed
// suppressed.
#[test]
fn adopting_a_session_after_an_offline_pass_clears_applied_effort() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime.clone());
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("42", "Risky active work")),
        "high",
        "the migration is irreversible",
    );
    write_mesh_task(
        &root,
        "42",
        "in_progress",
        "high",
        "the migration is irreversible",
    );

    // The member's session exits: liveness sees a bare shell and retains the
    // pane binding, conversation and its applied level.
    runtime.set_pane_current_command("%21", Some("zsh"));
    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("offline liveness reconcile");
    let offline =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(offline.health, HealthState::SessionDead);
    assert!(offline.session_id.is_some());

    // The operator restarts `codex` by hand in the same pane.
    // Regression: a2e07d0c dropped the pane binding, making hand-restart revival unreachable.
    runtime.set_pane_current_command("%21", Some("codex"));
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("hand-restarted-session"),
        Some("/tmp/hand-restarted-session.jsonl"),
    );

    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("liveness reconcile");

    let adopted =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(
        adopted.session_id.as_deref(),
        Some("hand-restarted-session")
    );
    assert_eq!(
        adopted.applied_effort, None,
        "a session that appeared on a dead record is not one taurhaus launched"
    );

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .expect("daemon-style sweep");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

// Regression (round 7): the hand-restarted CLI can become visible BEFORE its
// session identity is detectable. The first reconcile then promoted the dead
// record to Healthy with no id, and the id arriving one pass later was not
// classified as foreign — the stale applied level survived and the sweep
// stayed suppressed. Reviving a dead record with no detectable id is itself
// the foreign adoption; the level clears at promotion time.
#[test]
fn a_hand_restart_seen_before_its_identity_still_clears_applied_effort() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime.clone());
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("42", "Risky active work")),
        "high",
        "the migration is irreversible",
    );
    write_mesh_task(
        &root,
        "42",
        "in_progress",
        "high",
        "the migration is irreversible",
    );

    // Regression: 39eeb33a erased stopped ids, masking stale effort on revival.
    // Session exits; liveness retains the pane binding and the id.
    runtime.set_pane_current_command("%21", Some("zsh"));
    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("offline liveness reconcile");

    // Pass 1: the hand-restarted CLI is visible, but its identity is not yet
    // detectable (no registry entry written) — detection returns no id.
    // Regression: a2e07d0c dropped the pane binding, making hand-restart revival unreachable.
    runtime.set_pane_current_command("%21", Some("codex"));
    runtime.set_detected_runtime_session("%21", CliTool::Codex, None, None);
    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("pre-identity liveness reconcile");
    let revived =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(revived.health, HealthState::Healthy);
    assert_eq!(
        revived.applied_effort, None,
        "a dead record revived with no detectable id is a foreign session"
    );

    // Pass 2: the identity lands; the record must not resurrect the level.
    runtime.set_detected_runtime_session(
        "%21",
        CliTool::Codex,
        Some("late-identity-session"),
        Some("/tmp/late-identity-session.jsonl"),
    );
    orchestrator
        .reconcile_team_liveness("effort-team")
        .expect("post-identity liveness reconcile");

    let resumed = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .expect("daemon-style sweep");

    assert_eq!(resumed, vec!["builder".to_string()]);
    let record =
        MemberRuntimeStore::load(&teams_dir, "effort-team", "builder").expect("runtime record");
    assert_eq!(record.applied_effort.as_deref(), Some("high"));
}

#[test]
fn pending_effort_carries_the_held_task_requested_and_applied_identity() {
    let root = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let (teams_dir, mut orchestrator) = canonical_effort_team(&root, runtime);
    seed_running_canonical_codex_member(&teams_dir, &mut orchestrator);
    MemberRuntimeStore::update(&teams_dir, "effort-team", "builder", |record| {
        record.applied_effort = Some("high".to_string());
    })
    .expect("seed applied effort");
    write_member_snapshot_at(
        &teams_dir,
        "builder",
        Some(("41", "Already in progress")),
        "high",
        "the active task is risky",
    );
    write_mesh_task(
        &root,
        "41",
        "in_progress",
        "high",
        "the active task is risky",
    );
    write_mesh_task(&root, "42", "pending", "low", "the held task is mechanical");
    hold_mesh_assignment(&root, "42");
    let config = TeamConfigStore::load(&teams_dir, "effort-team").expect("team config");
    let builder = config
        .members
        .iter()
        .find(|member| member.name == "builder")
        .expect("builder");

    let pending = super::effort::pending_member_effort(
        &orchestrator,
        "effort-team",
        builder,
        EffortPassScope::TaskChanged,
    )
    .expect("pending effort");

    assert_eq!(pending.task_id, "42");
    assert_eq!(pending.requested, "low");
    assert_eq!(pending.applied.as_deref(), Some("high"));
}

// Regression: 2529309 emitted and persisted an effort refusal but returned an
// empty switched list, making the caller treat the pass as nominal success.
#[test]
fn an_effort_refusal_is_returned_in_the_typed_outcome() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = seed_running_codex_member(&tmp, runtime, &CliCommandSettings::default());
    assign_task(&tmp, "builder", "high", "the migration is irreversible");
    MemberRuntimeStore::update(tmp.path(), "effort-team", "builder", |record| {
        record.session_id = None;
    })
    .expect("clear session id");

    let mut cli_commands = CliCommandSettings::default();
    let outcome = orchestrator
        .apply_pending_task_effort_outcome(
            "effort-team",
            &mut cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
            &mut |_, _| {},
        )
        .expect("effort pass");

    assert!(outcome.switched.is_empty());
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, "builder");
    assert!(outcome.failed[0].1.contains("no recorded session"));
    assert!(outcome.skipped_teams.is_empty());
}

// Regression: 135c6f54 added `skipped_teams` to the typed effort result but
// returned team config failures as an outer error, leaving the producer's
// team-skip branch permanently empty.
#[test]
fn an_unreadable_team_is_returned_in_the_typed_effort_outcome() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(
        &tmp,
        Arc::new(FakeBackend::default()),
        Arc::new(RecordingCoordinationRuntime::default()),
    );
    let broken_team = tmp.path().join("broken-team");
    fs::create_dir_all(&broken_team).expect("create broken team");
    fs::write(broken_team.join("config.json"), b"{not valid json").expect("write broken config");

    let mut cli_commands = CliCommandSettings::default();
    let outcome = orchestrator
        .apply_pending_task_effort_outcome(
            "broken-team",
            &mut cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
            &mut |_, _| {},
        )
        .expect("typed effort outcome");

    assert!(outcome.switched.is_empty());
    assert!(outcome.failed.is_empty());
    assert_eq!(outcome.skipped_teams.len(), 1);
    assert_eq!(outcome.skipped_teams[0].0, "broken-team");
    assert!(outcome.skipped_teams[0].1.contains("failed to parse"));
}

#[test]
fn a_launch_records_the_effort_the_session_actually_runs_at() {
    // mesh reads this before it types `/effort`, so it has to start from the
    // level the launch put into effect rather than from nothing.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime, CliTool::Codex, Some("low"));
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
fn a_case_mismatched_configured_effort_rejected_by_the_renderer_is_not_recorded() {
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
    assert_eq!(
        record.applied_effort, None,
        "the renderer dropped the invalid case spelling, so the applied level is unknown"
    );
}

#[test]
fn an_invalid_requested_effort_is_not_recorded_as_applied() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime, CliTool::Codex, Some("not-an-effort-level"));
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
    assert_eq!(record.applied_effort, None);
}

// Regression: f9716c83 committed `applied_effort = None` for a level the
// renderer dropped, while the same successful launch cleared the attempt
// budget. With ada4cbc6's widened sweep, a level the member's model does not
// accept therefore stopped and resumed the member on every 30s cycle, forever:
// nothing recorded an attempt and nothing bounded the next pass.
#[test]
fn an_effort_the_model_rejects_is_refused_within_the_attempt_budget() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    // `gpt-5.6-luna` publishes efforts up to `max`, so an `ultra` assignment is
    // a level the launch renderer will refuse to write into the command.
    let mut config = TeamConfigStore::load(tmp.path(), "effort-team").expect("team config");
    config
        .members
        .iter_mut()
        .find(|member| member.name == "builder")
        .expect("builder member")
        .model = Some("gpt-5.6-luna".to_string());
    TeamConfigStore::save(tmp.path(), "effort-team", &config).expect("save team config");
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
    assign_task(&tmp, "builder", "ultra", "the migration is irreversible");

    let before = codex_launch_attempts(&runtime);
    for _ in 0..6 {
        orchestrator
            .apply_pending_task_effort(
                "effort-team",
                &CliCommandSettings::default(),
                "new_window",
                EffortPassScope::BackgroundSweep,
            )
            .expect("background sweep");
    }

    assert!(
        codex_launch_attempts(&runtime) - before <= 3,
        "a level the rendered command cannot carry must not relaunch the member \
         more than the attempt budget allows"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "the member keeps the level its session actually runs at"
    );
    let failure = record
        .effort_resume_failure
        .expect("the refused switch is recorded so the sweep converges");
    assert_eq!(failure.level, "ultra");
    assert_eq!(failure.attempts, 3);
    assert_eq!(failure.reason.as_deref(), Some("budget_exhausted"));
}

// Regression: 25293092 committed a requested effort even when the launch
// renderer dropped it in favor of the operator's effort-pinning base command,
// making the runtime record disagree with the session it described.
#[test]
fn a_pinning_resume_base_does_not_claim_the_ignored_requested_effort() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut cli_commands = CliCommandSettings::default();
    cli_commands.codex.resume =
        "codex resume --last -c model_reasoning_effort=\"low\" --yolo".to_string();
    let mut orchestrator =
        seed_running_codex_member(&tmp, runtime.clone(), &CliCommandSettings::default());
    MemberRuntimeStore::update(tmp.path(), "effort-team", "builder", |record| {
        record.health = HealthState::SessionDead;
    })
    .expect("stop member before operator resume");

    let report = orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                reasoning_effort_override: Some("high".to_string()),
            },
            &cli_commands,
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
        .rfind(|keys| keys.contains("codex resume"))
        .expect("resume command");
    assert!(
        launch.contains("model_reasoning_effort=\\\"low\\\"")
            || launch.contains("model_reasoning_effort=\"low\"")
    );
    assert!(
        !launch.contains("model_reasoning_effort=\\\"high\\\"")
            && !launch.contains("model_reasoning_effort=\"high\"")
    );
    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("low"),
        "applied effort comes from the rendered command, not the request"
    );
}

// Regression: 216e51e9 added resume seat validation after the effort pass
// stopped the member; 535badc5 preserved invalid external models on load.
#[test]
fn wave2_effort_validation_keeps_invalid_persisted_members_running() {
    for model in ["external", "opus"] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
        mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
        orchestrator
            .resume_member_with_cli_commands(
                &ResumeMemberRequest {
                    team_name: "effort-team".into(),
                    member_name: "builder".into(),
                    reasoning_effort_override: None,
                },
                &CliCommandSettings::default(),
            )
            .unwrap();
        assign_task(&tmp, "builder", "high", "migration requires review");
        let mut config = TeamConfigStore::load(tmp.path(), "effort-team").unwrap();
        config
            .members
            .iter_mut()
            .find(|member| member.name == "builder")
            .unwrap()
            .model = Some(model.into());
        TeamConfigStore::save(tmp.path(), "effort-team", &config).unwrap();
        let before = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").unwrap();
        let calls_before = runtime.calls().len();
        let outcome = orchestrator
            .apply_pending_task_effort_outcome(
                "effort-team",
                &mut CliCommandSettings::default(),
                "new_window",
                EffortPassScope::TaskChanged,
                &mut |_, _| {},
            )
            .unwrap();
        assert!(
            runtime.calls()[calls_before..]
                .iter()
                .all(|call| !matches!(call, RuntimeCall::KillPane { .. })),
            "invalid {model} must never stop the pane"
        );
        assert!(outcome.switched.is_empty());
        assert!(
            outcome
                .failed
                .iter()
                .any(|(member, reason)| member == "builder" && reason.contains("model")),
            "{outcome:?}"
        );
        let after = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").unwrap();
        assert_eq!(after.pane_id, before.pane_id);
        assert_eq!(after.health, before.health);
        assert_eq!(after.applied_effort, before.applied_effort);
        assert_eq!(after.effort_resume_failure.unwrap().attempts, 1);
    }
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

fn member_relaunch_attempts(runtime: &RecordingCoordinationRuntime) -> usize {
    runtime
        .calls()
        .into_iter()
        .filter(|call| {
            matches!(
                call,
                RuntimeCall::CreatePane { .. } | RuntimeCall::CreatePaneInTarget { .. }
            )
        })
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

// Regression: 2529309 stopped retrying after the bounded effort budget but
// left no durable reason and emitted no terminal event, so every later sweep
// looked nominal while the member stayed at the wrong level.
#[test]
fn three_attempts_emit_one_budget_exhausted_event_and_later_passes_are_silent() {
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("effort-budget.log.jsonl");
    let log_state = LogFileState::new(log_path.clone()).expect("log state");
    install_global_sink(&log_state);
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
    write_member_snapshot(
        &tmp,
        "builder",
        Some(("budget-exhaustion-task", "Run the migration")),
        "high",
        "the migration is irreversible",
    );

    runtime.set_send_keys_failures("%21", usize::MAX, "launch failed");
    for index in 1..=8 {
        runtime.set_send_keys_failures(&format!("test-pane-{index}"), usize::MAX, "launch failed");
    }

    let before = member_relaunch_attempts(&runtime);
    for _ in 0..3 {
        orchestrator
            .apply_pending_task_effort(
                "effort-team",
                &CliCommandSettings::default(),
                "new_window",
                EffortPassScope::TaskChanged,
            )
            .expect("budgeted attempt");
    }
    assert_eq!(
        member_relaunch_attempts(&runtime) - before,
        3,
        "the retry budget is exactly three launch attempts"
    );

    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("budget exhaustion pass");
    orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::TaskChanged,
        )
        .expect("silent pass after exhaustion");
    log_state.flush_for_test().expect("flush effort events");

    let events: Vec<serde_json::Value> = fs::read_to_string(&log_path)
        .expect("read effort events")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid log record"))
        .filter(|event| {
            event["event"] == "effort.resume.failed"
                && event["reason"] == "budget_exhausted"
                && event["task_id"] == "budget-exhaustion-task"
        })
        .collect();
    assert_eq!(events.len(), 1, "exhaustion is emitted exactly once");
    assert_eq!(events[0]["attempts"], 3);
    assert_eq!(events[0]["task_id"], "budget-exhaustion-task");
    assert_eq!(member_relaunch_attempts(&runtime) - before, 3);

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    let failure = record
        .effort_resume_failure
        .expect("exhaustion remains visible in runtime state");
    assert_eq!(failure.task_id, "budget-exhaustion-task");
    assert_eq!(failure.attempts, 3);
    assert_eq!(failure.reason.as_deref(), Some("budget_exhausted"));
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

// Regression: e2f9745b made team rendering prefer a resolved base, but the
// effort rewrite still changed only the configured base. Production therefore
// relaunched at the resolved base's old pin and recorded the assignment's level
// as applied even though it never reached the command the renderer used.
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
        .apply_pending_task_effort_outcome(
            "effort-team",
            &mut cli_commands,
            "new_window",
            EffortPassScope::TaskChanged,
            &mut |tool, commands| {
                commands.resolved_bases.insert(
                    (tool, crate::daemon::protocol::LaunchMode::Resume),
                    ResolvedBase {
                        command: commands.codex.resume.clone(),
                        expansions: Vec::new(),
                        opaque_head: None,
                    },
                );
            },
        )
        .expect("effort pass");

    assert_eq!(resumed.switched, vec!["builder".to_string()]);
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

// Regression: 4463736e reported a base-pinned Codex level as the first
// `model_reasoning_effort` override in the command, while Codex applies the
// last. A base — here one an alias expands to — carrying two overrides ran at
// one level and had the other committed as applied.
#[test]
fn a_base_pinning_two_levels_records_the_one_the_launch_runs_at() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut cli_commands = CliCommandSettings::default();
    cli_commands.codex.fresh = "codex2 -c model_reasoning_effort=\"high\"".to_string();
    cli_commands.resolved_bases.insert(
        (CliTool::Codex, crate::daemon::protocol::LaunchMode::Fresh),
        ResolvedBase {
            command: concat!(
                "codex -c model_reasoning_effort=\"low\" ",
                "-c model_reasoning_effort=\"high\""
            )
            .to_string(),
            expansions: vec![AliasExpansion {
                name: "codex2".to_string(),
                body: "codex -c model_reasoning_effort=\"low\"".to_string(),
            }],
            opaque_head: None,
        },
    );

    let _orchestrator = seed_running_codex_member(&tmp, runtime, &cli_commands);

    let record = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").expect("runtime");
    assert_eq!(
        record.applied_effort.as_deref(),
        Some("high"),
        "the record has to name the override the launched command actually applies"
    );
}

// Regression: word_spans split on whitespace alone, so a quoted assignment
// ahead of the frozen variable ('A="b c"') broke the env prefix early and the
// frozen effort level survived into the managed launch.
#[test]
fn a_quoted_assignment_before_the_frozen_effort_env_is_still_stripped() {
    let cleaned = without_frozen_effort_env(
        "A='b c' CLAUDE_CODE_EFFORT_LEVEL=high claude",
        CliTool::Claude,
        "quoted-env-team",
        "builder",
    )
    .expect("the frozen variable is removable");
    assert_eq!(cleaned.as_ref(), "A='b c' claude");
}

// Regression: 18810949 moved teams to selected Claude roots, but member launches
// passed only harness account selectors. Mesh 0.2.29 reads CLAUDE_DIR, not
// CLAUDE_CONFIG_DIR, so every harness's onboarded mesh commands missed the team.
#[cfg(unix)]
#[test]
fn nondefault_team_root_reaches_onboarded_mesh_commands_for_every_harness() {
    use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
    use crate::session_scanner::launch::shell_escape;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let tmp = TempDir::new().unwrap();
    // Exercise shell quoting as well as account/root separation.
    let root = tmp.path().join("account 2's $root");
    let teams_dir = root.join("teams");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        teams_dir.clone(),
        Arc::new(FakeBackend::default()),
        runtime.clone(),
    );
    orchestrator.create_team("root-team", None).unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let mesh = bin.join("mesh");
    // A scratch-only mesh double matching src/cli.rs + main.rs + paths.rs in
    // mesh 0.2.29: CLAUDE_DIR selects the root; CLAUDE_CONFIG_DIR is ignored.
    // No installed mesh or harness CLI is ever executed.
    fs::write(
        &mesh,
        r#"#!/bin/sh
set -eu
while [ "$#" -gt 0 ]; do
    case "$1" in
        --team) team="$2"; shift ;;
        --name) member="$2"; shift ;;
    esac
    shift
done
root="${CLAUDE_DIR:-$HOME/.claude}"
test -f "$root/teams/$team/config.json"
cat "$root/teams/$team/inboxes/$member.json"
"#,
    )
    .unwrap();
    fs::set_permissions(&mesh, fs::Permissions::from_mode(0o755)).unwrap();
    let harness = tmp.path().join("harness");
    fs::write(
        &harness,
        "#!/bin/sh\nset -eu\n/bin/sh -c \"$ONBOARDED_COMMAND\"\n",
    )
    .unwrap();

    for tool in [CliTool::Codex, CliTool::Grok, CliTool::Claude, CliTool::Agy] {
        let name = tool.to_string();
        let seat = member(&name, MemberRole::Agent, tool, tmp.path().to_str().unwrap());
        orchestrator.add_member("root-team", seat.clone()).unwrap();
        let notice = format!("notice for {name}");
        MeshInboxStore::append(
            &teams_dir,
            "root-team",
            &name,
            &crate::coordination::stores::MeshInboxMessage::new(
                "team-lead",
                notice.clone(),
                None,
                Utc::now(),
            ),
        )
        .unwrap();
        let onboarding = DeliveryRenderer::render_onboarding(
            "root-team",
            &name,
            "team-lead",
            RoleContext::default(),
        );
        let read_command = onboarding
            .lines()
            .find(|line| line.starts_with("mesh read "))
            .unwrap();
        let mut commands = CliCommandSettings::default();
        let account = tmp.path().join(format!("{name}-account"));
        if let Some(selector) = spec(tool).capabilities.account_selector {
            commands
                .account_selector_dirs
                .insert(selector.into(), account.clone());
        }
        // No Claude selector is supplied for non-Claude seats: a single-member
        // resume need not include a Claude member in its launch inputs.
        for stale_base in [false, true] {
            let base = format!(
                "{} /bin/sh {}",
                if stale_base {
                    "env CLAUDE_DIR=/wrong CLAUDE_DIR=/also-wrong"
                } else {
                    "env"
                },
                shell_escape(harness.to_str().unwrap())
            );
            let tool_commands = commands.get_mut(tool).unwrap();
            tool_commands.fresh = base.clone();
            tool_commands.resume = base;
            for resumed in [false, true] {
                let mut context =
                    MemberActivationContext::for_resume_member("root-team", "team-lead", &seat);
                context.resume_session_id = resumed.then(|| "scratch-session".into());
                let mut state = MemberActivationRuntimeState::default();
                if resumed {
                    run_member_session_phase(
                        runtime.as_ref(),
                        &teams_dir,
                        &context,
                        "%scratch",
                        MemberSessionPhase::LaunchOnly(&commands),
                        &mut state,
                    )
                    .unwrap();
                } else {
                    orchestrator
                        .acquire_initialize_member_pane(
                            &context,
                            &commands,
                            "new_window",
                            &mut Default::default(),
                            &mut state,
                        )
                        .unwrap();
                }
                let launch = runtime
                    .calls()
                    .into_iter()
                    .rev()
                    .find_map(|call| match call {
                        RuntimeCall::SendKeys { keys, .. } => Some(keys),
                        _ => None,
                    })
                    .unwrap();
                let output = Command::new("/bin/sh")
                    .args(["-c", &launch])
                    .env_clear()
                    .env("HOME", tmp.path())
                    .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                    .env("CLAUDE_DIR", tmp.path().join("inherited-wrong-root"))
                    .env("ONBOARDED_COMMAND", read_command)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{tool}, resume={resumed}, stale={stale_base}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let inbox: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(inbox[0]["text"], notice);
                if let Some(selector) = spec(tool).capabilities.account_selector {
                    assert!(
                        launch.contains(&format!(
                            "{selector}={}",
                            shell_escape(account.to_str().unwrap())
                        )),
                        "mesh root must not replace the harness account: {launch}"
                    );
                }
            }
        }
    }
}

#[test]
fn recovery_managed_onboarding_retry_uses_one_baseline() {
    let tmp = TempDir::new().unwrap();
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime);
    orchestrator.create_team("team", None).unwrap();
    let seat = member(
        "seat",
        MemberRole::Agent,
        CliTool::Codex,
        tmp.path().to_str().unwrap(),
    );
    orchestrator.add_member("team", seat.clone()).unwrap();
    crate::coordination::recovery_delivery::reserve_activation(
        tmp.path(),
        "team",
        "seat",
        "attachment-1",
    )
    .unwrap();
    let request = ResumeMemberRequest {
        team_name: "team".into(),
        member_name: "seat".into(),
        reasoning_effort_override: None,
    };
    for _ in 0..3 {
        let entry = orchestrator
            .prepare_resume_onboarding_entry(&request, &seat, "lead")
            .unwrap();
        orchestrator
            .deliver_onboarding_entries(vec![entry])
            .unwrap();
    }
    assert_eq!(backend.call_counts().1, 1);
    let records = backend.delivered_requests();
    let DeliveryRequest::OperatorNotice(notice) = &records[0] else {
        panic!("notice")
    };
    crate::coordination::recovery_card::assert_control_golden(&notice.message);
}

#[test]
fn recovery_actual_launch_changes_attachment_stamp_even_for_the_same_session() {
    // Regression: 25ba6532 reused a generation when a preserved-session relaunch kept its old stamp.
    let temp = TempDir::new().unwrap();
    let runtime = RecordingCoordinationRuntime::default();
    let agent = setup_config(
        "seat",
        "codex",
        "gpt-6-astra",
        temp.path().to_str().unwrap(),
    );
    let context =
        MemberActivationContext::for_initialize_member("team", "lead", &agent, MemberRole::Agent)
            .unwrap();
    let mut pending = MemberActivationRuntimeState::default();
    for _ in 0..2 {
        let previous = pending.attached_at;
        run_member_session_phase(
            &runtime,
            temp.path(),
            &context,
            "%1",
            MemberSessionPhase::LaunchOnly(&CliCommandSettings::default()),
            &mut pending,
        )
        .unwrap();
        assert_ne!(pending.attached_at, previous);
    }
}

#[test]
fn recovery_launch_captures_the_selected_harness_root_separately() {
    let temp = TempDir::new().unwrap();
    let runtime = RecordingCoordinationRuntime::default();
    let agent = setup_config(
        "seat",
        "codex",
        "gpt-6-astra",
        temp.path().to_str().unwrap(),
    );
    let context =
        MemberActivationContext::for_initialize_member("team", "lead", &agent, MemberRole::Agent)
            .unwrap();
    let mut settings = CliCommandSettings::default();
    let account = temp.path().join("selected-account");
    settings
        .account_selector_dirs
        .insert("CODEX_HOME".into(), account.clone());
    let mut pending = MemberActivationRuntimeState::default();
    run_member_session_phase(
        &runtime,
        temp.path(),
        &context,
        "%1",
        MemberSessionPhase::LaunchOnly(&settings),
        &mut pending,
    )
    .unwrap();
    assert_eq!(
        pending.harness_account_root.as_deref(),
        Some(account.as_path())
    );
}

#[test]
fn recovery_team_recreation_and_seat_replacement_mint_distinct_recipients() {
    let tmp = TempDir::new().unwrap();
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = new_orchestrator(&tmp, backend, runtime);
    let mut recipients = Vec::new();
    for generation in 0..3 {
        if generation != 1 {
            orchestrator.create_team("team", None).unwrap();
        }
        orchestrator
            .add_member(
                "team",
                member(
                    "seat",
                    MemberRole::Agent,
                    CliTool::Codex,
                    tmp.path().to_str().unwrap(),
                ),
            )
            .unwrap();
        crate::coordination::recovery_delivery::reserve_activation(
            tmp.path(),
            "team",
            "seat",
            "activation",
        )
        .unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        let team = TeamConfigStore::load(tmp.path(), "team").unwrap();
        recipients.push((
            team.team_incarnation_id.unwrap(),
            record.recovery.member_incarnation_id.unwrap(),
        ));
        if generation == 0 {
            orchestrator.remove_member("team", "seat", None).unwrap();
        }
        if generation == 1 {
            orchestrator.disband_team("team", None).unwrap();
        }
    }
    assert_eq!(recipients[0].0, recipients[1].0);
    assert_ne!(recipients[0].1, recipients[1].1);
    assert_ne!(recipients[1].0, recipients[2].0);
}

#[test]
fn recovery_delivered_onboarding_uses_the_common_compiler() {
    // Regression: 25ba6532 left full-role replay in the onboarding path.
    let tmp = TempDir::new().unwrap();
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = new_orchestrator(
        &tmp,
        backend.clone(),
        Arc::new(RecordingCoordinationRuntime::default()),
    );
    let request = AddAgentRequest {
        team_name: "team".into(),
        agent: setup_config("seat", "codex", "gpt-6-astra", tmp.path().to_str().unwrap()),
    };
    orchestrator.create_team("team", None).unwrap();
    orchestrator
        .add_member(
            "team",
            member(
                "seat",
                MemberRole::Agent,
                CliTool::Codex,
                tmp.path().to_str().unwrap(),
            ),
        )
        .unwrap();
    let entry = orchestrator
        .prepare_add_agent_onboarding_entry(&request)
        .unwrap()
        .unwrap();
    orchestrator
        .deliver_onboarding_entries(vec![entry])
        .unwrap();
    let delivered = backend.delivered_requests();
    let DeliveryRequest::OperatorNotice(notice) = &delivered[0] else {
        panic!("notice")
    };
    assert!(notice.message.starts_with("[taurhaus] recovery_card"));
    assert!(!notice.message.contains("mesh read"));
}

#[test]
fn recovery_submission_recomposes_a_changed_view_without_spending_a_retry() {
    use crate::coordination::requests::OperatorNoticeDelivery;
    // Regression: 25ba6532 submitted prepared content without rechecking current operative facts.
    let tmp = TempDir::new().unwrap();
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = new_orchestrator(
        &tmp,
        backend.clone(),
        Arc::new(RecordingCoordinationRuntime::default()),
    );
    orchestrator.create_team("team", None).unwrap();
    orchestrator
        .add_member(
            "team",
            member(
                "seat",
                MemberRole::Agent,
                CliTool::Codex,
                tmp.path().to_str().unwrap(),
            ),
        )
        .unwrap();
    crate::coordination::recovery_delivery::reserve_activation(
        tmp.path(),
        "team",
        "seat",
        "activation",
    )
    .unwrap();
    let first = crate::coordination::recovery_delivery::prepare(
        &orchestrator.root_registry,
        tmp.path(),
        "team",
        "seat",
        "inbox",
    )
    .unwrap()
    .unwrap();
    let mut snapshot = crate::coordination::stores::OperationalContextSnapshotStore::load(
        tmp.path(),
        "team",
        "seat",
    )
    .unwrap()
    .unwrap();
    snapshot.assignment_footer.validation_expectation = "CURRENT-VALIDATION".into();
    crate::coordination::stores::OperationalContextSnapshotStore::save(tmp.path(), &snapshot)
        .unwrap();
    orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            journal_links: None,
            team_name: "team".into(),
            member_name: "seat".into(),
            sender_name: None,
            message: first.text,
            recovery_card: Some(first.receipt.clone()),
            operational_context: None,
        }))
        .unwrap();
    let records = backend.delivered_requests();
    let DeliveryRequest::OperatorNotice(notice) = &records[0] else {
        panic!("notice")
    };
    assert!(notice.message.contains("CURRENT-VALIDATION"));
    let receipt = notice.recovery_card.as_ref().unwrap();
    assert_eq!(receipt.delivery_id, first.receipt.delivery_id);
    assert_eq!(receipt.attempt, 1);
    assert_ne!(receipt.content_revision, first.receipt.content_revision);
}

#[test]
fn terminal_launch_defers_behind_holder_and_retries_after_release() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let path = root.join("team/state/terminal/seat.lock");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let holder = File::create(path).unwrap();
    holder.lock_exclusive().unwrap();
    let agent = setup_config("seat", "codex", "gpt-5.4", root.to_str().unwrap());
    let context =
        MemberActivationContext::for_initialize_member("team", "lead", &agent, MemberRole::Agent)
            .unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    orchestrator.create_team("team", None).unwrap();
    orchestrator
        .add_member(
            "team",
            member_from_agent_setup(&agent, MemberRole::Agent).unwrap(),
        )
        .unwrap();
    let mut state = PendingRuntimeState::default();
    let settings = CliCommandSettings::default();
    assert!(run_member_session_phase(
        runtime.as_ref(),
        root,
        &context,
        "%1",
        MemberSessionPhase::LaunchOnly(&settings),
        &mut state
    )
    .is_err());
    assert!(runtime.calls().is_empty());
    assert!(orchestrator
        .acquire_initialize_member_pane(
            &context,
            &settings,
            "new_window",
            &mut Default::default(),
            &mut state
        )
        .is_err());
    assert!(runtime.calls().is_empty());
    holder.unlock().unwrap();
    run_member_session_phase(
        runtime.as_ref(),
        root,
        &context,
        "%1",
        MemberSessionPhase::LaunchOnly(&settings),
        &mut state,
    )
    .unwrap();
    assert!(!runtime.calls().is_empty());
    orchestrator
        .acquire_initialize_member_pane(
            &context,
            &settings,
            "new_window",
            &mut Default::default(),
            &mut state,
        )
        .unwrap();
    let record = MemberRuntimeStore::load(root, "team", "seat").unwrap();
    // Regression: 80a83d08 certified launches even with null socket/session facts.
    assert_eq!(record.tmux_socket, Some(root.join("recording-tmux.sock")));
    assert_eq!(record.tmux_session_id.as_deref(), Some("$1"));
    assert!(record.pane_pid.is_some());
    assert!(record.pane_start_time.is_some());
    assert!(record.harness.is_some());
    assert!(record
        .activity_snapshot_path
        .as_ref()
        .unwrap()
        .is_absolute());
    assert_eq!(record.terminal_contract, 1);
    assert_eq!(record.attachment_generation, 1);
    assert_eq!(record.pane_id, state.pane_id);
    assert_eq!(
        record.launch_root.unwrap().teams_dir,
        root.canonicalize().unwrap()
    );
}

#[test]
fn team_owned_delivery_never_starts_or_heals_member_daemon() {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, None);
    let mut config = TeamConfigStore::load(tmp.path(), "effort-team").unwrap();
    config.extra.insert("delivery_owner".into(), "team".into());
    TeamConfigStore::save(tmp.path(), "effort-team", &config).unwrap();
    let pid = start_member_daemon_if_required(
        runtime.as_ref(),
        "effort-team",
        "builder",
        "%21",
        CliTool::Codex,
        tmp.path(),
        MemberDaemonStartPolicy::ReplaceStalePid {
            previous_daemon_pid: None,
        },
        None,
    )
    .unwrap();
    assert_eq!(pid, None);
    MemberRuntimeStore::update(tmp.path(), "effort-team", "builder", |r| {
        r.pane_id = Some("%21".into());
        r.health = HealthState::Healthy;
    })
    .unwrap();
    orchestrator.reconcile_team_liveness("effort-team").unwrap();
    assert!(!runtime
        .calls()
        .iter()
        .any(|c| matches!(c, RuntimeCall::SpawnDaemon { .. })));
}

#[test]
fn terminal_effort_marks_dead_before_wait_and_retries_without_spending_budget() {
    // Regression: 3ca169ed tore the pane down before invalidating its record.
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = effort_team(&tmp, runtime.clone(), CliTool::Codex, Some("low"));
    mark_member_offline(&tmp, "effort-team", "builder", "%21", None);
    orchestrator
        .resume_member_with_cli_commands(
            &ResumeMemberRequest {
                team_name: "effort-team".into(),
                member_name: "builder".into(),
                reasoning_effort_override: None,
            },
            &CliCommandSettings::default(),
        )
        .unwrap();
    assign_task(&tmp, "builder", "high", "review");
    let before = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").unwrap();
    let lock = File::options()
        .write(true)
        .open(tmp.path().join("effort-team/state/terminal/builder.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let offset = runtime.calls().len();
    let result = orchestrator
        .apply_pending_task_effort(
            "effort-team",
            &CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
        )
        .unwrap();
    assert!(result.is_empty());
    assert!(!runtime.calls()[offset..].iter().any(|c| matches!(
        c,
        RuntimeCall::KillPane { .. } | RuntimeCall::SendKeys { .. }
    )));
    let dead = MemberRuntimeStore::load(tmp.path(), "effort-team", "builder").unwrap();
    assert_eq!(dead.health, HealthState::SessionDead);
    assert_eq!(dead.attachment_generation, before.attachment_generation + 1);
    assert_eq!(dead.effort_resume_failure.as_ref().unwrap().attempts, 0);
    lock.unlock().unwrap();
    let retried = orchestrator
        .apply_pending_task_effort_outcome(
            "effort-team",
            &mut CliCommandSettings::default(),
            "new_window",
            EffortPassScope::BackgroundSweep,
            &mut |_, _| {},
        )
        .unwrap();
    assert_eq!(retried.switched, ["builder"], "{retried:?}");
}

#[test]
fn reinitialize_resets_attachment_without_rewinding_generation() {
    // Regression: b643834d required an existing config even on the legacy path.
    // Regression: 80a83d08 merged a fresh seed behind the on-disk generation,
    // silently retaining its old healthy pane throughout reinitialization.
    let tmp = TempDir::new().unwrap();
    let mut orchestrator = new_orchestrator(
        &tmp,
        Arc::new(FakeBackend::default()),
        Arc::new(RecordingCoordinationRuntime::default()),
    );
    let lead = member(
        "lead",
        MemberRole::Lead,
        CliTool::Codex,
        tmp.path().to_str().unwrap(),
    );
    MemberRuntimeStore::save(
        tmp.path(),
        "team",
        "lead",
        &crate::coordination::stores::MemberRuntimeRecord {
            attachment_generation: 7,
            pane_id: Some("%old".into()),
            health: HealthState::Healthy,
            ..Default::default()
        },
    )
    .unwrap();
    orchestrator
        .seed_initialize_roster("team", None, lead, &[], false)
        .unwrap();
    let record = MemberRuntimeStore::load(tmp.path(), "team", "lead").unwrap();
    assert_eq!(record.health, HealthState::SessionDead);
    assert!(record.pane_id.is_none());
    assert!(record.attachment_generation >= 7);
    assert_eq!(record.terminal_contract, 0);
}

#[cfg(target_os = "linux")]
#[test]
fn hosted_member_liveness_effort_and_attached_pane_restart_preserve_thread() {
    let tmp = TempDir::new().unwrap();
    let registry = crate::coordination::hosted::tests::seat(tmp.path());
    let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    orchestrator
        .hosted
        .launch(&registry, "team", "seat", &launch)
        .unwrap();
    orchestrator.reconcile_team_liveness("team").unwrap();
    orchestrator
        .reconcile_team_presence_for_live_status_with_runtime_sessions("team", &[])
        .unwrap();
    let before = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(before.health, HealthState::Healthy);
    let roster =
        crate::coordination::roster::get_team_roster_with_runtime_sessions(tmp.path(), "team", &[])
            .unwrap();
    assert_eq!(roster[0].session_id.as_deref(), Some("owned-thread"));
    // Regression: fa18910c lost appServer when roster teardown reconstructed runtime.
    let attachment = roster[0].runtime_record().unwrap();
    assert!(attachment.app_server.is_some());
    // Regression: 1db4f9bf, L4 run 4: closing the TUI left its host running.
    orchestrator.hosted.stop(&registry, "team", "seat").unwrap();
    let stopped = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(stopped.health, HealthState::SessionDead);
    assert_eq!(stopped.app_server.as_ref().unwrap().state, "stopped");
    assert_eq!(
        stopped.attachment_generation,
        before.attachment_generation + 1
    );
    let mut commands = CliCommandSettings::default();
    let command = format!(
        "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
        tmp.path().display(),
        launch.program.display()
    );
    // Same resolved alias path as ordinary launches; selected scratch account wins.
    commands.codex.fresh = "codex-seat".into();
    commands.codex.resume = "codex-seat resume {session_id}".into();
    for mode in [
        crate::daemon::protocol::LaunchMode::Fresh,
        crate::daemon::protocol::LaunchMode::Resume,
    ] {
        commands.resolved_bases.insert(
            (CliTool::Codex, mode),
            ResolvedBase {
                command: if mode == crate::daemon::protocol::LaunchMode::Fresh {
                    command.clone()
                } else {
                    format!("{command} resume {{session_id}}")
                },
                expansions: vec![AliasExpansion {
                    name: "codex-seat".into(),
                    body: command.clone(),
                }],
                opaque_head: None,
            },
        );
    }
    commands.codex_bypass_hook_trust = false;
    commands
        .account_selector_dirs
        .insert("CODEX_HOME".into(), tmp.path().into());
    let request = ResumeMemberRequest {
        team_name: "team".into(),
        member_name: "seat".into(),
        reasoning_effort_override: Some("high".into()),
    };
    let report = orchestrator
        .resume_member_with_cli_commands_and_layout(&request, &commands, "new_window")
        .unwrap();
    assert!(report.resumed, "{}", report.message);
    // Regression: cadd533e returned before opening the operator's attached TUI.
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-1"));
    let after = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(before.session_id, after.session_id);
    assert!(after.attachment_generation > stopped.attachment_generation);
    let requests = || {
        fs::read_to_string(tmp.path().join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>()
    };
    let recovery_count = || {
        requests()
            .iter()
            .filter(|r| r["method"] == "turn/start")
            .count()
    };
    assert!(requests()
        .iter()
        .any(|r| r["method"] == "thread/resume" && r["params"]["threadId"] == "owned-thread"));

    assert_eq!(after.applied_effort.as_deref(), Some("high"));
    let wire = serde_json::to_value(&after).unwrap();
    let host = &wire["appServer"];
    assert_eq!(
        host["attachArgv"],
        serde_json::json!([
            launch.program.to_str().unwrap(),
            "--remote",
            format!("unix://{}", host["socketPath"].as_str().unwrap()),
            "resume",
            "owned-thread",
            "--no-alt-screen",
            "--strict-config"
        ])
    );
    assert!(runtime.calls().iter().any(|c| matches!(c,
        RuntimeCall::SendKeys { keys, .. } if keys.contains("env -u TMUX")
            && keys.contains("--remote") && keys.contains("owned-thread")
            && keys.contains(tmp.path().to_str().unwrap()))));
    let generation = after.attachment_generation;
    let pid = host["processId"].clone();
    let before_calls = runtime.calls().len();
    let report = orchestrator
        .resume_member_with_cli_commands_and_layout(&request, &commands, "new_window")
        .unwrap();
    assert!(report.resumed, "{}", report.message);
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-1"));
    assert!(report.reused_pane);
    assert!(!runtime.calls()[before_calls..]
        .iter()
        .any(|c| matches!(c, RuntimeCall::SendKeys { .. })));
    runtime.set_pane_exists("test-pane-1", false);
    let report = orchestrator
        .resume_member_with_cli_commands_and_layout(&request, &commands, "new_window")
        .unwrap();
    assert!(report.resumed, "{}", report.message);
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-2"));
    let reattached = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(reattached.attachment_generation, generation);
    assert_eq!(
        serde_json::to_value(&reattached).unwrap()["appServer"]["processId"],
        pid
    );
    assert_eq!(reattached.session_id.as_deref(), Some("owned-thread"));
    assert_eq!(reattached.pane_pid, Some(1002));
    assert!(reattached.pane_start_time.is_some());
    assert!(reattached.tmux_socket.is_some());
    assert_eq!(
        recovery_count(),
        2,
        "one recovery card for each of the initial and resumed generations"
    );
    let result = orchestrator.teardown_member_resources_best_effort(
        "team",
        "seat",
        Some(tmp.path()),
        Some(&reattached),
    );
    assert!(result
        .steps
        .iter()
        .any(|s| s.step == "stop_host" && s.success));
    // Regression: b4a4b2dd added the attached pane but inherited host-only teardown.
    assert!(runtime
        .calls()
        .iter()
        .any(|c| matches!(c, RuntimeCall::KillPane { pane_id } if pane_id == "test-pane-2")));
    // Regression: 1db4f9bf, L4 run 4: a day-close must leave both delivery modes resumable.
    let mut config = TeamConfigStore::load(tmp.path(), "team").unwrap();
    config.members.push(member(
        "plain",
        MemberRole::Lead,
        CliTool::Claude,
        tmp.path().to_str().unwrap(),
    ));
    TeamConfigStore::save(tmp.path(), "team", &config).unwrap();
    let plain = crate::coordination::stores::MemberRuntimeRecord {
        health: HealthState::SessionDead,
        session_id: Some("plain-thread".into()),
        ..Default::default()
    };
    MemberRuntimeStore::save(tmp.path(), "team", "plain", &plain).unwrap();
    let stopped = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    let report = orchestrator
        .resume_team_with_cli_commands_and_layout(
            &crate::coordination::requests::ResumeTeamRequest {
                team_name: "team".into(),
            },
            &commands,
            "new_window",
        )
        .unwrap();
    assert!(report.resumed, "{:?}", report.failed_members);
    assert_eq!(report.resumed_members.len(), 2);
    for name in ["seat", "plain"] {
        let resumed = MemberRuntimeStore::load(tmp.path(), "team", name).unwrap();
        assert!(resumed.pane_id.is_some());
    }
    let resumed = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(resumed.session_id.as_deref(), Some("owned-thread"));
    assert!(resumed.attachment_generation > stopped.attachment_generation);
    assert_eq!(recovery_count(), 3);
}

#[cfg(target_os = "linux")]
#[test]
fn hosted_teardown_reports_already_closed_pane() {
    // Regression: efb1ddb8 reported a successful pane kill even when the TUI was absent.
    let tmp = TempDir::new().unwrap();
    let registry = crate::coordination::hosted::tests::seat(tmp.path());
    let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    orchestrator
        .hosted
        .launch(&registry, "team", "seat", &launch)
        .unwrap();
    orchestrator
        .hosted
        .attach_pane(&registry, "team", "seat", runtime.as_ref(), "new_window")
        .unwrap();
    let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    runtime.set_pane_exists(record.pane_id.as_deref().unwrap(), false);
    let result = orchestrator.teardown_member_resources_best_effort(
        "team",
        "seat",
        Some(tmp.path()),
        Some(&record),
    );
    assert!(result.steps.iter().any(|step| step.step == "kill_pane"
        && step.success
        && step.message.as_deref() == Some("attached TUI already closed")));
    assert!(!runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::KillPane { .. })));
}

#[cfg(target_os = "linux")]
#[test]
fn hosted_member_controlled_rollback_resumes_the_same_thread_in_a_new_pane() {
    check_hosted_rollback(false);
}

#[cfg(target_os = "linux")]
#[test]
fn hosted_member_controlled_rollback_skips_reused_foreign_pane() {
    // Regression: ef8f6ce9 made a stale, reused TUI pane identity wedge rollback forever.
    check_hosted_rollback(true);
}

#[cfg(target_os = "linux")]
fn check_hosted_rollback(foreign_pane: bool) {
    // Regression: fa18910c made hosted-to-pane rollback permanently refuse.
    let tmp = TempDir::new().unwrap();
    let registry = crate::coordination::hosted::tests::seat(tmp.path());
    let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    orchestrator
        .hosted
        .launch(&registry, "team", "seat", &launch)
        .unwrap();
    // Regression: efb1ddb8 retained the attached TUI identity into plain-pane rollback.
    orchestrator
        .hosted
        .attach_pane(&registry, "team", "seat", runtime.as_ref(), "new_window")
        .unwrap();
    let before = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    runtime.set_pane_shell(before.pane_id.as_deref().unwrap(), false);
    if foreign_pane {
        runtime.set_pane_identity(before.pane_id.as_deref().unwrap(), Some(9999), Some(9999));
    }
    orchestrator.hosted.stop(&registry, "team", "seat").unwrap();
    let mut config = TeamConfigStore::load(tmp.path(), "team").unwrap();
    config.members[0].extra.remove("adapter_mode");
    std::fs::write(
        tmp.path().join("team/config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    runtime.set_detected_runtime_session("test-pane-2", CliTool::Codex, Some("owned-thread"), None);
    let mut commands = CliCommandSettings::default();
    let command = format!(
        "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
        tmp.path().display(),
        launch.program.display()
    );
    commands.codex.fresh = command.clone();
    commands.codex.resume = format!("{command} resume {{session_id}}");
    commands.codex_bypass_hook_trust = false;
    commands
        .account_selector_dirs
        .insert("CODEX_HOME".into(), tmp.path().into());
    let request = ResumeMemberRequest {
        team_name: "team".into(),
        member_name: "seat".into(),
        reasoning_effort_override: None,
    };
    let report = orchestrator
        .resume_member_with_cli_commands_and_layout(&request, &commands, "new_window")
        .unwrap();
    assert!(report.resumed, "{}", report.message);
    assert_eq!(report.pane_id.as_deref(), Some("test-pane-2"));
    assert_ne!(report.pane_id, before.pane_id);
    // Regression: 9d358935 cleared the retained TUI identity without closing its owned pane.
    assert_eq!(
        runtime.calls().iter().any(|c| matches!(c,
        RuntimeCall::KillPane { pane_id } if Some(pane_id) == before.pane_id.as_ref())),
        !foreign_pane
    );
    assert!(runtime.calls().iter().all(|c| !matches!(c,
        RuntimeCall::SendKeys { pane_id, keys, .. } if Some(pane_id) == before.pane_id.as_ref() && !keys.contains("--remote"))));
    let after = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
    assert_eq!(before.session_id, after.session_id);
    assert!(after.app_server.is_none());
    assert!(after.attachment_generation > before.attachment_generation);
    assert!(runtime.calls().iter().any(|c| matches!(c,
        RuntimeCall::SendKeys { keys, .. } if (keys.contains("resume") && keys.contains("owned-thread")) && keys.contains(tmp.path().to_str().unwrap()))));
    assert!(
        taurhaus_lib::platform::process_start_ticks(before.app_server.unwrap().process_id)
            .is_none()
    );
}

// Regression: 50a07ab6 (#151) added team-owned runtime exclusion, but initialize still
// created legacy teams and could not activate the canonical delivery owner.
#[test]
fn canonical_initialize_adopts_mesh_config_and_launches_before_delivery() {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    let project = tmp.path().to_str().unwrap();
    let request: InitializeTeamRequest = serde_json::from_value(serde_json::json!({
        "team_name": "canonical", "team_description": "Disposable trial",
        "lead_mode": "launch_new",
        "lead": setup_config("lead", "codex", "gpt-6-astra", project),
        "agents": [setup_config("builder", "codex", "gpt-6-astra", project)],
        "messaging": {"mode": "canonical", "retentionPolicy": {"synthetic_disposable": true}}
    }))
    .unwrap();
    let report = orchestrator.initialize_team(&request).unwrap();
    assert!(report.failed_step.is_none(), "{report:?}");
    let config = TeamConfigStore::load(tmp.path(), "canonical").unwrap();
    assert_eq!(
        config.extra.get("messaging_format"),
        Some(&serde_json::json!(2))
    );
    assert_eq!(config.members.len(), 2);
    assert_eq!(
        config.members.iter().filter(|m| m.name == "lead").count(),
        1
    );
    assert!(!runtime.calls().iter().any(|c| matches!(
        c,
        RuntimeCall::SpawnDaemon { .. } | RuntimeCall::SpawnDaemonAtRoot { .. }
    )));
    let launch = report
        .succeeded_steps
        .iter()
        .position(|s| s == "launch_sessions")
        .unwrap();
    let opt_in = report
        .succeeded_steps
        .iter()
        .position(|s| s == "opt_in_delivery")
        .unwrap();
    let calls = runtime.calls();
    let RuntimeCall::CreateCanonicalTeam { args } = &calls[0] else {
        panic!("{calls:?}")
    };
    let policy_path = Path::new(&args[5]);
    assert_eq!(policy_path.parent(), Some(tmp.path()));
    assert!(policy_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with(".canonical-policy-"));
    assert!(!policy_path.exists());
    let root = tmp.path().parent().unwrap().to_str().unwrap();
    assert_eq!(
        args,
        &vec![
            "team",
            "create",
            "--messaging-canonical",
            "--isolated",
            "--retention-policy",
            policy_path.to_str().unwrap(),
            "--claude-dir",
            root,
            "--team",
            "canonical",
            "--name",
            "lead"
        ]
    );
    let delivery = calls
        .iter()
        .position(|call| matches!(call, RuntimeCall::OptInTeamDelivery { .. }))
        .unwrap();
    let daemon = calls
        .iter()
        .position(|call| matches!(call, RuntimeCall::SpawnTeamDaemon { .. }))
        .unwrap();
    assert!(delivery < daemon);
    assert!(
        calls[..delivery]
            .iter()
            .filter(|call| matches!(call, RuntimeCall::DetectSessionId { .. }))
            .count()
            >= 2
    );
    assert_eq!(
        calls[delivery],
        RuntimeCall::OptInTeamDelivery {
            args: vec![
                "team",
                "delivery",
                "--owner",
                "team",
                "--claude-dir",
                root,
                "--team",
                "canonical",
                "--name",
                "lead"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        }
    );
    let lead = config.members.iter().find(|m| m.name == "lead").unwrap();
    assert_eq!(lead.role, MemberRole::Lead);
    assert_eq!(lead.cli_tool, CliTool::Codex);
    assert_eq!(lead.model.as_deref(), Some("gpt-6-astra"));
    assert_eq!(lead.project_path, tmp.path());
    assert_eq!(lead.extra["controlAuthTokenHash"], "recording-only-hash");
    assert_eq!(
        config.team_incarnation_id.as_deref(),
        Some("recorded-incarnation")
    );
    assert_eq!(config.extra["minimum_writer"], "mesh-journal/2");
    assert_eq!(config.extra["delivery_owner"], "team");
    assert_eq!(
        config.extra["messaging_policy"],
        serde_json::json!({"synthetic_disposable": true})
    );
    for name in ["lead", "builder"] {
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), "canonical", name)
                .unwrap()
                .terminal_contract,
            1
        );
    }
    assert!(launch < opt_in);
}

#[test]
fn canonical_initialize_refusals_preserve_unowned_or_retryable_teams() {
    for creation_failure in [true, false] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        let project = tmp.path().to_str().unwrap();
        let mut request: InitializeTeamRequest = serde_json::from_value(serde_json::json!({
            "team_name": "canonical", "team_description": "Disposable trial",
            "lead_mode": "launch_new",
            "lead": setup_config("lead", "codex", "gpt-6-astra", project),
            "agents": [setup_config("builder", "codex", "gpt-6-astra", project)],
            "messaging": {"mode": "canonical", "retentionPolicy": {"synthetic_disposable": true}}
        }))
        .unwrap();

        if creation_failure {
            runtime.set_canonical_create_failure(Some("mesh: policy refused"));
        } else {
            runtime.set_delivery_opt_in_failure(Some(
                "mesh: OLD executor lead: terminalContract: 1 required",
            ));
        }
        let report = orchestrator.initialize_team(&request).unwrap();
        assert_eq!(
            report.failed_step.as_deref(),
            Some(if creation_failure {
                "create_team"
            } else {
                "opt_in_delivery"
            })
        );
        assert!(report.retryable);
        assert!(
            report.message.contains(if creation_failure {
                "mesh: policy refused"
            } else {
                "mesh: OLD executor lead"
            }),
            "{report:?}"
        );
        // The fake publishes a team before refusing, modeling another creator.
        // Only a successful Mesh create transfers cleanup ownership to Taurhaus.
        assert!(tmp.path().join("canonical/config.json").exists());
        assert!(!runtime
            .calls()
            .iter()
            .any(|c| matches!(c, RuntimeCall::SpawnTeamDaemon { .. })));
        assert!(!fs::read_dir(tmp.path()).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".canonical-policy-")));
        // An absent field follows the original path (covered by the unchanged
        // batch-order and launch goldens). A conflict must never delete it.
        if !creation_failure {
            request.messaging = None;
            let original = fs::read(tmp.path().join("canonical/config.json")).unwrap();
            let retry = orchestrator.initialize_team(&request).unwrap();
            assert_eq!(retry.failed_step.as_deref(), Some("create_team"));
            assert_eq!(
                fs::read(tmp.path().join("canonical/config.json")).unwrap(),
                original
            );
            let messaging = serde_json::from_value(serde_json::json!({"mode": "canonical", "retentionPolicy": {"synthetic_disposable": true}})).unwrap();
            request.messaging = Some(messaging);
            runtime.set_delivery_opt_in_failure(None);
            let before = runtime.calls().len();
            let retried = orchestrator.initialize_team(&request).unwrap();
            assert!(retried.failed_step.is_none(), "{retried:?}");
            assert!(!runtime.calls()[before..].iter().any(|c| matches!(
                c,
                RuntimeCall::CreateCanonicalTeam { .. }
                    | RuntimeCall::CreatePane { .. }
                    | RuntimeCall::SendKeys { .. }
            )));
            assert!(!tmp
                .path()
                .join(".taurhaus-initialize-pending/canonical.json")
                .exists());
        }
    }
}

fn canonical_review_request(tmp: &TempDir) -> InitializeTeamRequest {
    serde_json::from_value(serde_json::json!({
        "team_name": "canonical", "lead_mode": "launch_new",
        "lead": setup_config("lead", "codex", "gpt-6-astra", tmp.path().to_str().unwrap()),
        "agents": [],
        "messaging": {"mode": "canonical", "retentionPolicy": {"synthetic_disposable": true}}
    }))
    .unwrap()
}

// Regression: b643834d surfaced clap usage when the installed Mesh lacked team create.
#[test]
fn canonical_review_unsupported_mesh_has_actionable_error() {
    for refusal in [
        "error: unrecognized subcommand 'team'",
        "error: unexpected argument '--messaging-canonical' found",
    ] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_canonical_create_failure(Some(refusal));
        let mut orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
        let report = orchestrator
            .initialize_team(&canonical_review_request(&tmp))
            .unwrap();
        assert_eq!(report.failed_step.as_deref(), Some("create_team"));
        assert!(
            report
                .message
                .contains("installed Mesh does not support canonical teams"),
            "{report:?}"
        );
        assert!(
            report.message.contains("disable Canonical messaging"),
            "{report:?}"
        );
    }
}

// Regression: 796bba0e retained launched steps without checking stale pane identities.
#[test]
fn canonical_review_retry_refuses_missing_dead_reused_or_unprobeable_seats() {
    for problem in ["missing", "dead", "reused", "probe"] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_delivery_opt_in_failure(Some("mesh: runtime pending"));
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        let request = canonical_review_request(&tmp);
        assert_eq!(
            orchestrator
                .initialize_team(&request)
                .unwrap()
                .failed_step
                .as_deref(),
            Some("opt_in_delivery")
        );
        let record = MemberRuntimeStore::load(tmp.path(), "canonical", "lead").unwrap();
        let pane = record.pane_id.unwrap();
        match problem {
            "missing" => runtime.set_pane_exists(&pane, false),
            "dead" => runtime.set_pane_dead(&pane, true),
            "reused" => runtime.set_pane_identity(&pane, Some(999999), Some(999999)),
            _ => runtime.set_live_pane_failure(&pane, "probe failed"),
        }
        runtime.set_delivery_opt_in_failure(None);
        let before = runtime.calls().len();
        let report = orchestrator.initialize_team(&request).unwrap();
        assert_eq!(
            report.failed_step.as_deref(),
            Some("launch_sessions"),
            "{problem}: {report:?}"
        );
        assert!(
            report.message.contains("disband and re-initialize"),
            "{report:?}"
        );
        assert!(!runtime.calls()[before..].iter().any(|c| matches!(
            c,
            RuntimeCall::OptInTeamDelivery { .. } | RuntimeCall::SpawnTeamDaemon { .. }
        )));
        assert!(tmp.path().join("canonical/config.json").exists());
    }
}

// Regression: 796bba0e placed Taurhaus retry bookkeeping inside Mesh-owned state.
#[test]
fn canonical_review_checkpoint_is_outside_the_team_tree() {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_delivery_opt_in_failure(Some("mesh: runtime pending"));
    let mut orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
    orchestrator
        .initialize_team(&canonical_review_request(&tmp))
        .unwrap();
    assert!(!tmp
        .path()
        .join("canonical/state/taurhaus-initialize-pending.json")
        .exists());
    assert!(tmp
        .path()
        .join(".taurhaus-initialize-pending/canonical.json")
        .exists());
}

// Regression: 13beff81 moved retry checkpoints into a hidden teams-root directory,
// which list() exposed as a team and startup hook reconciliation could not load.
fn assert_canonical_checkpoint_does_not_pollute_discovery(refusal: Option<&str>) {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_delivery_opt_in_failure(refusal);
    let mut orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
    let report = orchestrator
        .initialize_team(&canonical_review_request(&tmp))
        .unwrap();
    assert_eq!(
        report.failed_step.as_deref(),
        refusal.map(|_| "opt_in_delivery")
    );
    let checkpoint_dir = tmp.path().join(".taurhaus-initialize-pending");
    assert!(checkpoint_dir.is_dir());
    assert_eq!(
        checkpoint_dir.join("canonical.json").exists(),
        refusal.is_some()
    );

    // This Codex-only roster requires no Claude hook installation: the startup
    // scan must succeed without ever resolving or touching a real account home.
    let hook_scan = crate::coordination::state::ensure_startup_claude_compact_hook(tmp.path());
    assert!(matches!(hook_scan, Ok(false)), "{hook_scan:?}");
    assert_eq!(
        TeamConfigStore::list(tmp.path()).unwrap(),
        vec!["canonical"]
    );
}

#[test]
fn canonical_review_success_checkpoint_does_not_pollute_discovery() {
    assert_canonical_checkpoint_does_not_pollute_discovery(None);
}

#[test]
fn canonical_review_refused_checkpoint_does_not_pollute_discovery() {
    assert_canonical_checkpoint_does_not_pollute_discovery(Some("mesh: runtime pending"));
}

// Regression: 796bba0e propagated checkpoint unlink errors after successful launch.
#[test]
fn canonical_review_checkpoint_cleanup_is_best_effort() {
    for replace_with_directory in [false, true] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
        let report = orchestrator
            .initialize_team_with_cli_commands_and_layout_and_progress(
                &canonical_review_request(&tmp),
                &CliCommandSettings::default(),
                "new_window",
                Some(&mut |step, status, _| {
                    if step == "send_onboarding" && status == StepStatus::Succeeded {
                        for relative in [
                            "canonical/state/taurhaus-initialize-pending.json",
                            ".taurhaus-initialize-pending/canonical.json",
                        ] {
                            let path = tmp.path().join(relative);
                            if path.is_file() {
                                fs::remove_file(&path).unwrap();
                                if replace_with_directory {
                                    fs::create_dir(path).unwrap();
                                }
                            }
                        }
                    }
                }),
            )
            .expect("bookkeeping cannot discard a successful report");
        assert!(report.failed_step.is_none(), "{report:?}");
    }
}

// Regression: b643834d disbanded a concurrently published team on Mesh create refusal.
#[test]
fn canonical_review_create_refusal_never_removes_a_concurrently_published_team() {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    // This fake publishes Mesh-shaped config before refusing: a competing creator
    // won the creation lock after Taurhaus's unlocked directory pre-check.
    runtime.set_canonical_create_failure(Some("mesh: new canonical team required"));
    let mut orchestrator = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
    let report = orchestrator
        .initialize_team(&canonical_review_request(&tmp))
        .unwrap();
    assert_eq!(report.failed_step.as_deref(), Some("create_team"));
    assert!(tmp.path().join("canonical/config.json").exists());
}

#[test]
fn seat_delivery_validation_names_unsupported_harness_and_unknown_choice() {
    for (tool, delivery, expected) in [
        ("claude", "app_server", "app_server_unsupported_harness"),
        ("agy", "app_server", "app_server_unsupported_harness"),
        ("grok", "app_server", "app_server_unsupported_harness"),
        ("codex", "typo", "unsupported seat delivery"),
    ] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        let mut agent =
            serde_json::to_value(setup_config("seat", tool, "", tmp.path().to_str().unwrap()))
                .unwrap();
        agent["delivery"] = serde_json::json!(delivery);
        let request: InitializeTeamRequest = serde_json::from_value(serde_json::json!({
            "team_name": "team", "lead_mode": "launch_new",
            "lead": setup_config("lead", "claude", "opus", tmp.path().to_str().unwrap()),
            "agents": [agent]
        }))
        .unwrap();
        let report = orchestrator.initialize_team(&request).unwrap();
        assert!(report.failed_step.is_some(), "{tool} {report:?}");
        assert!(
            report.message.contains("seat") && report.message.contains(expected),
            "{report:?}"
        );
        assert!(runtime.calls().is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn seat_delivery_canonical_creation_and_operational_rollback() {
    let tmp = TempDir::new().unwrap();
    let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
    // A real project has instructions; fake transport proves they survive into the record.
    let script = fs::read_to_string(&launch.program).unwrap().replace(
        "'instructionSources':[]",
        "'instructionSources':['AGENTS.md']",
    );
    fs::write(&launch.program, script).unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    let mut commands = CliCommandSettings::default();
    commands.codex.fresh = format!(
        "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
        tmp.path().display(),
        launch.program.display()
    );
    // Regression: 6f61f611 also forwarded the managed notify override to the host.
    commands.codex_notify_executable = Some(tmp.path().join("fake-notify"));
    // Regression: 3d3a0f83 rejected the default managed hook flag before spawning a host.
    commands.codex_bypass_hook_trust = true;
    commands
        .account_selector_dirs
        .insert("CODEX_HOME".into(), tmp.path().into());
    let mut request = canonical_review_request(&tmp);
    request.agents.push(setup_config(
        "seat",
        "codex",
        "gpt-6-astra",
        tmp.path().to_str().unwrap(),
    ));
    let mut agent = serde_json::to_value(&request.agents[0]).unwrap();
    agent["delivery"] = serde_json::json!("app_server");
    request.agents[0] = serde_json::from_value(agent).unwrap();
    let name = request.agents[0].name.clone();
    let report = orchestrator
        .initialize_team_with_cli_commands(&request, &commands)
        .unwrap();
    assert!(report.failed_step.is_none(), "{report:?}");
    // Regression: 1b19edd2 tested an unused renderer, leaving production notify suppression unguarded.
    let host_argv: Vec<String> =
        serde_json::from_str(&fs::read_to_string(tmp.path().join("host-argv.json")).unwrap())
            .unwrap();
    assert!(
        host_argv.iter().all(|arg| !arg.contains("notify=")),
        "managed hosted launch must suppress notify overrides"
    );
    let record = MemberRuntimeStore::load(tmp.path(), "canonical", &name).unwrap();
    let wire = serde_json::to_value(&record).unwrap();
    assert_eq!(wire["appServer"]["host"], "taurhaus-daemon-owned-thread/1");
    assert_eq!(wire["appServer"]["configuration"], "strict-config/1");
    assert_eq!(wire["appServer"]["trust"], "daemon-owned/1");
    assert_eq!(
        wire["appServer"]["instructionSources"],
        serde_json::json!(["AGENTS.md"])
    );
    assert!(
        tmp.path().join("start.json").exists(),
        "fake thread/start must run"
    );
    assert!(runtime.calls().iter().any(|c| matches!(c,
        RuntimeCall::SendKeys { keys, .. } if keys.contains("--remote") && keys.contains("--strict-config"))));
    let refusal = orchestrator
        .hosted
        .rollback_to_pane(
            &orchestrator.root_registry,
            "canonical",
            &name,
            runtime.as_ref(),
        )
        .unwrap_err();
    assert_eq!(refusal, "app_server_rollback_on_team_owned_team: stop the seat, remove it, re-add it with delivery tmux");

    let stopped = orchestrator.teardown_member_resources_best_effort(
        "canonical",
        &name,
        Some(tmp.path()),
        Some(&record),
    );
    assert!(stopped.steps.iter().all(|s| s.success), "{stopped:?}");
    let removed = orchestrator
        .remove_member("canonical", &name, None)
        .unwrap();
    assert!(removed.removed, "{removed:?}");
    let before_add = runtime.calls().len();
    let mut agent = serde_json::to_value(&request.agents[0]).unwrap();
    agent["delivery"] = serde_json::json!("tmux");
    let added = orchestrator
        .add_agent_to_team_with_cli_commands(
            &AddAgentRequest {
                team_name: "canonical".into(),
                agent: serde_json::from_value(agent).unwrap(),
            },
            &commands,
        )
        .unwrap();
    assert!(added.failed_step.is_none(), "{added:?}");
    let plain = MemberRuntimeStore::load(tmp.path(), "canonical", &name).unwrap();
    assert!(plain.app_server.is_none());
    assert_eq!(plain.terminal_contract, 1);
    assert!(plain.pane_id.is_some());
    assert!(runtime.calls()[before_add..].iter().any(|c| matches!(c,
        RuntimeCall::SendKeys { keys, .. } if !keys.contains("--remote") && keys.contains(launch.program.to_str().unwrap()))));
    assert!(!runtime.calls().iter().any(|c| matches!(
        c,
        RuntimeCall::SpawnDaemon { .. } | RuntimeCall::SpawnDaemonAtRoot { .. }
    )));
    assert!(plain.daemon_pid.is_none());

    orchestrator
        .remove_member("canonical", &name, None)
        .unwrap();
    let before_hosted_add = runtime.calls().len();
    // The fake owns one thread per account; a fresh fixture models the replacement seat.
    let account = tmp.path().join("added-account");
    fs::create_dir(&account).unwrap();
    let added_launch = crate::coordination::hosted_process::tests::fixture(&account);
    commands.codex.fresh = format!(
        "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
        account.display(),
        added_launch.program.display()
    );
    commands
        .account_selector_dirs
        .insert("CODEX_HOME".into(), account);
    let added = orchestrator
        .add_agent_to_team_with_cli_commands(
            &AddAgentRequest {
                team_name: "canonical".into(),
                agent: request.agents[0].clone(),
            },
            &commands,
        )
        .unwrap();
    assert!(added.failed_step.is_none(), "{added:?}");
    let hosted = MemberRuntimeStore::load(tmp.path(), "canonical", &name).unwrap();
    assert!(hosted.app_server.is_some());
    assert!(hosted.daemon_pid.is_none());
    assert!(!runtime.calls()[before_hosted_add..]
        .iter()
        .any(|c| matches!(
            c,
            RuntimeCall::SpawnDaemon { .. } | RuntimeCall::SpawnDaemonAtRoot { .. }
        )));
}

#[cfg(target_os = "linux")]
#[test]
fn seat_delivery_hosted_lead_preserves_canonical_identity_and_legacy_default() {
    for canonical in [true, false] {
        let tmp = TempDir::new().unwrap();
        let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        let mut commands = CliCommandSettings::default();
        commands.codex.fresh = format!(
            "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
            tmp.path().display(),
            launch.program.display()
        );
        commands.codex_bypass_hook_trust = false;
        commands
            .account_selector_dirs
            .insert("CODEX_HOME".into(), tmp.path().into());
        let mut request = canonical_review_request(&tmp);
        if !canonical {
            request.messaging = None;
        }
        request.lead.delivery = Some("app_server".into());
        let report = orchestrator
            .initialize_team_with_cli_commands(&request, &commands)
            .unwrap();
        if !canonical {
            // Regression: 90f89257 — mesh admits hosted delivery only on a team-owned
            // canonical team (`native_mode_or_authority`), so a legacy request with a
            // hosted seat is refused at validation instead of creating an undeliverable seat.
            assert_eq!(
                report.failed_step.as_deref(),
                Some("validate_configuration")
            );
            assert!(
                format!("{report:?}").contains("app_server_requires_canonical_messaging"),
                "{report:?}"
            );
            assert!(runtime.calls().is_empty());
            continue;
        }
        assert!(report.failed_step.is_none(), "{report:?}");
        let config = TeamConfigStore::load(tmp.path(), "canonical").unwrap();
        assert_eq!(config.members[0].extra["adapter_mode"], "app_server");
        let record = MemberRuntimeStore::load(tmp.path(), "canonical", "lead").unwrap();
        assert!(record.app_server.is_some());
        assert!(record.daemon_pid.is_none());
        assert!(!runtime.calls().iter().any(|c| matches!(
            c,
            RuntimeCall::SpawnDaemon { .. } | RuntimeCall::SpawnDaemonAtRoot { .. }
        )));
    }
}

#[test]
fn seat_delivery_tmux_and_omission_produce_identical_member_config() {
    let setup = setup_config("seat", "codex", "gpt-6-astra", "/scratch");
    let before = super::helpers::member_from_agent_setup(&setup, MemberRole::Agent).unwrap();
    let mut explicit = setup;
    explicit.delivery = Some("tmux".into());
    let after = super::helpers::member_from_agent_setup(&explicit, MemberRole::Agent).unwrap();
    assert_eq!(
        serde_json::to_vec(&before).unwrap(),
        serde_json::to_vec(&after).unwrap()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn seat_delivery_failed_add_cleans_unpublished_seat_but_retains_attachment() {
    // Regression: 3d3a0f83 skipped add rollback, stranding pre-submission refusals.
    for failure in ["build", "join", "attach"] {
        let tmp = TempDir::new().unwrap();
        let launch = crate::coordination::hosted_process::tests::fixture(tmp.path());
        let script = fs::read_to_string(&launch.program).unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let backend = Arc::new(FakeBackend::default());
        let mut orchestrator = new_orchestrator(&tmp, backend.clone(), runtime.clone());
        let request = canonical_review_request(&tmp);
        assert!(orchestrator
            .initialize_team(&request)
            .unwrap()
            .failed_step
            .is_none());
        let mut commands = CliCommandSettings::default();
        commands.codex.fresh = format!(
            "CODEX_HOME='{}' '{}' --sandbox read-only --ask-for-approval never",
            tmp.path().display(),
            launch.program.display()
        );
        commands.codex_bypass_hook_trust = false;
        commands
            .account_selector_dirs
            .insert("CODEX_HOME".into(), tmp.path().into());
        let mut agent = setup_config("seat", "codex", "gpt-6-astra", tmp.path().to_str().unwrap());
        agent.delivery = Some("app_server".into());
        let add = AddAgentRequest {
            team_name: request.team_name,
            agent,
        };
        match failure {
            "build" => fs::write(
                &launch.program,
                script.replace("'FAKE_BUILD','0.153.4'", "'FAKE_BUILD','wrong-build'"),
            )
            .unwrap(),
            "join" => runtime.set_join_mesh_failure("join refused"),
            _ => runtime.set_send_keys_failures("test-pane-2", 10, "attach refused"),
        }
        let report = orchestrator
            .add_agent_to_team_with_cli_commands(&add, &commands)
            .unwrap();
        assert_eq!(
            report.failed_step.as_deref(),
            Some(match failure {
                "build" => "launch_host",
                "join" => "join_mesh",
                _ => "attach_tui",
            }),
            "{report:?}"
        );
        let config = TeamConfigStore::load(tmp.path(), "canonical").unwrap();
        assert_eq!(
            config.members.iter().any(|m| m.name == "seat"),
            failure == "attach"
        );
        if failure == "attach" {
            assert!(MemberRuntimeStore::load(tmp.path(), "canonical", "seat")
                .unwrap()
                .app_server
                .is_some());
        } else {
            assert!(!tmp.path().join("canonical/runtime/seat.json").exists());
        }
        if failure == "build" {
            assert_eq!(backend.call_counts().3, 1, "Mesh join must be rolled back");
            fs::write(&launch.program, &script).unwrap();
            let retried = orchestrator
                .add_agent_to_team_with_cli_commands(&add, &commands)
                .unwrap();
            assert!(retried.failed_step.is_none(), "{retried:?}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn seat_delivery_attach_existing_lead_never_launches_host() {
    // Regression: 3d3a0f83 added hosted dispatch; validation must still guard attach-existing.
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    let mut request = canonical_review_request(&tmp);
    request.lead_mode = LeadMode::AttachExisting;
    request.lead.delivery = Some("app_server".into());
    let report = orchestrator.initialize_team(&request).unwrap();
    assert_eq!(
        report.failed_step.as_deref(),
        Some("validate_configuration")
    );
    assert!(report
        .message
        .contains("attach-existing is not supported yet"));
    assert!(!report
        .succeeded_steps
        .iter()
        .any(|step| step == "launch_host"));
    assert!(runtime.calls().is_empty());
}

#[test]
fn seat_delivery_seed_preserves_created_incarnation() {
    // Regression: 3d3a0f83 redundantly minted an incarnation; preserve creation authority.
    let tmp = TempDir::new().unwrap();
    let mut orchestrator = new_orchestrator(
        &tmp,
        Arc::new(FakeBackend::default()),
        Arc::new(RecordingCoordinationRuntime::default()),
    );
    orchestrator.create_team("team", None).unwrap();
    let before = TeamConfigStore::load(tmp.path(), "team").unwrap();
    let mut lead = member(
        "lead",
        MemberRole::Lead,
        CliTool::Codex,
        tmp.path().to_str().unwrap(),
    );
    lead.extra
        .insert("adapter_mode".into(), serde_json::json!("app_server"));
    orchestrator
        .seed_initialize_roster("team", None, lead, &[], false)
        .unwrap();
    assert_eq!(
        TeamConfigStore::load(tmp.path(), "team")
            .unwrap()
            .team_incarnation_id,
        before.team_incarnation_id
    );
}

// Regression: 06d1267b (observed at 6398bfa3), integration attempt 10 and L4 run 3: self-heal acquired
// the canonical owner during initialize; retained Retry seats remained exposed.
#[test]
fn initialize_owner_race_guard_lifecycle_and_interleaved_self_heal() {
    let _lock = taurhaus_lib::test_support::acquire_global_log_test_guard();
    for canonical in [true, false] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let backend = Arc::new(FakeBackend::default());
        let mut orch = new_orchestrator(&tmp, backend.clone(), runtime.clone());
        let state = crate::coordination::state::CoordinationState::with_components_and_runtime(
            tmp.path().into(),
            crate::coordination::backend::BackendSelector::m0(),
            Arc::new(move |_, _| Ok(backend.clone())),
            Arc::new({
                let runtime = runtime.clone();
                move || runtime.clone()
            }),
        );
        let log_path = tmp.path().join("race.jsonl");
        let sink = LogFileState::new(log_path.clone()).unwrap();
        install_global_sink(&sink);
        let guard = tmp
            .path()
            .join(".taurhaus-initialize-pending/canonical.guard");
        let mut request = canonical_review_request(&tmp);
        if !canonical {
            request.messaging = None;
        }
        let owner_count = || {
            runtime
                .calls()
                .iter()
                .filter(|c| matches!(c, RuntimeCall::SpawnTeamDaemonAtRoot { .. }))
                .count()
        };
        let mut observe = |step: &str, status: StepStatus, _message: Option<String>| {
            if step == "create_team" && status == StepStatus::Succeeded {
                assert!(guard.exists());
            }
            if canonical && step == "send_onboarding" {
                assert_eq!(owner_count(), 1);
            }
            if step == "send_onboarding" || step == "opt_in_delivery" {
                assert!(guard.exists());
                let before = runtime.calls().len();
                let pass = state.run_background_self_heal_core_pass().unwrap();
                assert_eq!(pass.teams_skipped, 1);
                assert_eq!(pass.team_daemons_ensured, 0);
                assert_eq!(runtime.calls().len(), before);
            }
        };
        for refusal in [Some("mesh: runtime pending"), None] {
            if !canonical && refusal.is_some() {
                continue;
            }
            runtime.set_delivery_opt_in_failure(refusal);
            let report = orch
                .initialize_team_with_cli_commands_and_layout_and_progress(
                    &request,
                    &CliCommandSettings::default(),
                    "new_window",
                    Some(&mut observe),
                )
                .unwrap();
            assert_eq!(
                report.failed_step.as_deref(),
                refusal.map(|_| "opt_in_delivery")
            );
            assert_eq!(guard.exists(), refusal.is_some());
        }
        if canonical {
            assert_eq!(owner_count(), 1);
        }
        sink.flush_for_test().unwrap();
        assert!(fs::read_to_string(log_path)
            .unwrap()
            .contains("self_heal.team.skipped_initializing"));
    }
}

// Regression: 77616a34 rejected idle owner health records from attempt 10 / L4 run 3.
#[test]
fn initialize_owner_race_resets_only_a_validated_idle_owner() {
    let _lock = taurhaus_lib::test_support::acquire_global_log_test_guard();
    for case in [
        "empty",
        "health_pending",
        "health_completed",
        "health_error",
        "health_corrupt",
        "pending",
        "corrupt",
        "history",
        "invalid_pid",
        "other",
    ] {
        let tmp = TempDir::new().unwrap();
        let runtime = RecordingCoordinationRuntime::default();
        let refusal = if case == "other" {
            "another refusal"
        } else {
            "quiescent required before opt-in: team owner already holds lifetime lock"
        };
        runtime.set_delivery_opt_in_failure_once(refusal);
        runtime.set_pid_running(4242, case != "invalid_pid");
        let pending = match case {
            "pending" => "[{}]",
            "corrupt" => "invalid",
            _ => "[]",
        };
        let state_file = if case == "history" {
            "attempt.json"
        } else {
            "pending-test.json"
        };
        for (path, bytes) in [
            ("daemons/team.pid".into(), "4242"),
            (format!("state/delivery/{state_file}"), pending),
        ] {
            let path = tmp.path().join("race").join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        let mut health = serde_json::json!({
            "member": "lead", "root": tmp.path(), "incarnation": "attempt-10",
            "epoch": 1, "heartbeat": "2026-09-10T12:39:02.352Z",
            "pending_since": null, "completed": 0, "failures": 0, "error": null
        });
        match case {
            "health_pending" => health["pending_since"] = "2026-09-10T12:39:00Z".into(),
            "health_completed" => health["completed"] = 1.into(),
            "health_error" => health["error"] = "delivery failed".into(),
            "health_corrupt" => health = serde_json::json!({}),
            _ => {}
        }
        fs::write(
            tmp.path().join("race/state/delivery/health-lead.json"),
            health.to_string(),
        )
        .unwrap();
        let log = tmp.path().join("reset.jsonl");
        let sink = LogFileState::new(log.clone()).unwrap();
        install_global_sink(&sink);
        let result = crate::coordination::runtime::team_activation::opt_in_with_owner_reset(
            &runtime,
            "race",
            "lead",
            tmp.path(),
        );
        let recovered = case == "empty";
        assert_eq!(result.is_ok(), recovered, "{case}: {result:?}");
        let calls = runtime.calls();
        let stops = calls
            .iter()
            .filter(|c| matches!(c, RuntimeCall::StopTeamDaemon { .. }))
            .count();
        let attempts = calls
            .iter()
            .filter(|c| matches!(c, RuntimeCall::OptInTeamDelivery { .. }))
            .count();
        assert_eq!(stops, usize::from(recovered));
        assert_eq!(attempts, 1 + usize::from(recovered));
        sink.flush_for_test().unwrap();
        let records = fs::read_to_string(log).unwrap();
        assert_eq!(
            records.contains("coordination.opt_in.owner_reset"),
            recovered
        );
    }
}

// Regression: c9e18117 made best-effort ensure and guard cleanup fatal after opt-in
// while fixing attempt 10 / L4 run 3; neither should misreport a successful opt-in.
#[test]
fn initialize_owner_race_success_tolerates_guard_cleanup_and_owner_skip() {
    for case in ["missing_guard", "unremovable_guard", "owner_skip"] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orch = new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime);
        let request = canonical_review_request(&tmp);
        let guard = crate::coordination::initialize_guard::path(tmp.path(), "canonical");
        let mut observe = |step: &str, status: StepStatus, _: Option<String>| {
            if case == "owner_skip" && step == "opt_in_delivery" && status == StepStatus::Running {
                fs::remove_file(tmp.path().join("canonical/state/control_auth/lead.json")).unwrap();
            }
            if case != "owner_skip" && step == "send_onboarding" && status == StepStatus::Succeeded
            {
                fs::remove_file(&guard).unwrap();
                if case == "unremovable_guard" {
                    fs::create_dir(&guard).unwrap();
                }
            }
        };
        let report = orch
            .initialize_team_with_cli_commands_and_layout_and_progress(
                &request,
                &CliCommandSettings::default(),
                "new_window",
                Some(&mut observe),
            )
            .expect("successful onboarding must return a report");
        assert!(report.failed_step.is_none(), "{case}: {report:?}");
    }
}

// Regression: 90f89257 let an explicit hosted seat through a legacy (no `messaging`)
// initialize; mesh refuses delivery to a hosted seat on a format-1 team.
#[test]
fn initialize_refuses_hosted_seat_without_canonical_messaging() {
    let tmp = TempDir::new().unwrap();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator =
        new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
    let project = tmp.path().to_str().unwrap();
    let mut agent = setup_config("seat", "codex", "gpt-6-astra", project);
    agent.delivery = Some("app_server".into());
    let request = InitializeTeamRequest {
        messaging: None,
        team_name: "legacy-hosted".into(),
        team_description: None,
        lead: setup_config("lead", "claude", "opus", project),
        lead_mode: LeadMode::LaunchNew,
        agents: vec![agent],
    };
    let report = orchestrator.initialize_team(&request).unwrap();
    assert_eq!(
        report.failed_step.as_deref(),
        Some("validate_configuration")
    );
    assert!(
        format!("{report:?}").contains("app_server_requires_canonical_messaging"),
        "{report:?}"
    );
    assert!(runtime.calls().is_empty());
}

// Regression: 90f89257 defaulted legacy adds to hosted delivery without a target-team guard.
#[cfg(target_os = "linux")]
#[test]
fn hosted_add_requires_target_team_canonical_messaging() {
    for format in [None, Some(1), Some(2)] {
        let tmp = TempDir::new().unwrap();
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mut orchestrator =
            new_orchestrator(&tmp, Arc::new(FakeBackend::default()), runtime.clone());
        orchestrator.create_team("team", None).unwrap();
        let mut config = TeamConfigStore::load(tmp.path(), "team").unwrap();
        if let Some(format) = format {
            config
                .extra
                .insert("messaging_format".into(), serde_json::json!(format));
        }
        TeamConfigStore::save(tmp.path(), "team", &config).unwrap();
        for delivery in [None, Some("tmux"), Some("app_server")] {
            let mut agent =
                setup_config("seat", "codex", "gpt-6-astra", tmp.path().to_str().unwrap());
            agent.delivery = delivery.map(str::to_string);
            let result = orchestrator.validate_add_agent_request(&AddAgentRequest {
                team_name: "team".into(),
                agent,
            });
            if delivery == Some("app_server") && format != Some(2) {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("app_server_requires_canonical_messaging"));
            } else {
                result.unwrap();
            }
        }
        assert!(runtime.calls().is_empty());
        assert_eq!(TeamConfigStore::load(tmp.path(), "team").unwrap(), config);
    }
}
