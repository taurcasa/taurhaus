use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use chrono::{DateTime, Utc};
use tempfile::{NamedTempFile, TempDir};

use super::*;
use crate::commands::projects::DbState;
use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{
    ActiveProjectTeamStore, MemberRuntimeStore, OperationalContextSnapshotStore, TeamConfigStore,
};
use taurhaus_lib::daemon_api::protocol;
use taurhaus_lib::ProviderState;

#[test]
fn codex_hook_reconcile_failure_is_degraded_for_managed_launches() {
    // Regression: 6fe0aa3 made Codex hook filesystem errors abort initialize,
    // add, and resume before the otherwise valid coordination pipeline ran.
    let source = include_str!("../coordination.rs");
    assert!(source.contains("compaction.codex_hook.degraded"));
}

// Regression: 6128bd1 collapsed managed-Codex discovery errors to `false`, so
// task-arrival and self-heal callers silently rendered settings that omitted
// the launch inputs a managed Codex member requires.
#[test]
fn background_launch_settings_reports_managed_codex_discovery_failure() {
    let teams = TempDir::new().expect("teams dir");
    let broken_team = teams.path().join("broken-team");
    std::fs::create_dir_all(&broken_team).expect("create broken team");
    std::fs::write(broken_team.join("config.json"), b"{not valid json")
        .expect("write broken config");
    let (db, _db_file) = test_db_state();

    let error = background_launch_settings(&db, teams.path())
        .expect_err("managed-Codex discovery failure must reach the caller");

    assert!(error.to_string().contains("failed to parse"));
}

#[test]
fn successful_team_commands_do_not_reconcile_the_codex_hook_twice() {
    // Regression: 6fe0aa3 reconciled Codex both before launch and again after a
    // successful pipeline, doubling writes and structured reconciliation events.
    let source = include_str!("../coordination.rs");
    let helper = source
        .split("fn maybe_ensure_compact_hooks_for_team")
        .nth(1)
        .expect("post-pipeline hook helper")
        .split("fn emit_initialize_pipeline_result")
        .next()
        .expect("post-pipeline hook helper body");
    assert!(!helper.contains("reconcile_codex_before_managed_launch"));
}

#[test]
fn every_registered_roster_mutation_reconciles_the_global_harness_hooks() {
    // Regression: commit 86601a2 reconciled grok's one global hook from the
    // add-agent, remove-member and disband paths but left the registered
    // `coordination_add_member` mutation alone, so adding the first grok member
    // through that IPC left grok without its hook until some later trigger.
    const ROSTER_MUTATIONS: &[&str] = &[
        "coordination_initialize_team",
        "coordination_add_agent",
        "coordination_add_member",
        "coordination_remove_member",
        "coordination_disband_team",
    ];
    let source = include_str!("../coordination.rs");

    let unreconciled = source
        .split("#[tauri::command")
        .skip(1)
        .filter_map(|command| {
            let name = command
                .split_once("pub fn ")
                .or_else(|| command.split_once("pub async fn "))
                .and_then(|(_, rest)| rest.split_once('('))
                .map(|(name, _)| name)?;
            let mutates_roster = ROSTER_MUTATIONS.contains(&name);
            let reconciles = command.contains("reconcile_global_harness_hooks")
                || command.contains("maybe_ensure_compact_hooks_for_team");
            (mutates_roster && !reconciles).then_some(name)
        })
        .collect::<Vec<_>>();

    assert!(
        unreconciled.is_empty(),
        "roster mutations that never reconcile the global harness hooks: {unreconciled:?}"
    );
}

#[derive(Debug, Default)]
struct MockBinaryLookup {
    available: HashSet<String>,
}

impl MockBinaryLookup {
    fn with_available(names: &[&str]) -> Self {
        Self {
            available: names.iter().map(|name| (*name).to_string()).collect(),
        }
    }
}

impl BinaryLookup for MockBinaryLookup {
    fn is_available(&self, binary_name: &str) -> bool {
        self.available.contains(binary_name)
    }
}

fn test_state(teams_dir: PathBuf) -> CoordinationState {
    CoordinationState::with_components_and_runtime(
        teams_dir,
        BackendSelector::m0(),
        Arc::new(|_kind, _teams_dir| {
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    )
}

fn test_state_with_runtime(
    teams_dir: PathBuf,
    runtime: Arc<RecordingCoordinationRuntime>,
) -> CoordinationState {
    CoordinationState::with_components_and_runtime(
        teams_dir,
        BackendSelector::m0(),
        Arc::new(|_kind, _teams_dir| {
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(move || runtime.clone()),
    )
}

#[allow(clippy::useless_conversion)]
fn test_db_state() -> (DbState, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("temp db");
    let conn = taurhaus_lib::db::init_db(tmp.path()).expect("init db");
    (DbState(Mutex::new(conn).into()), tmp)
}

fn test_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid timestamp")
        .with_timezone(&Utc)
}

struct StubDaemon {
    addr: String,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for StubDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn start_stub_daemon(response: serde_json::Value) -> StubDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
    let addr = listener.local_addr().expect("stub daemon addr");
    let addr_string = format!("127.0.0.1:{}", addr.port());

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept daemon client");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request");

        let request: protocol::DaemonRequest =
            serde_json::from_str(&line).expect("parse daemon request");
        let mut response = response;
        if let Some(map) = response.as_object_mut() {
            map.insert("id".to_string(), serde_json::Value::String(request.id));
        }
        let response_line = format!(
            "{}\n",
            serde_json::to_string(&response).expect("serialize daemon response")
        );
        let mut writer = stream;
        writer
            .write_all(response_line.as_bytes())
            .expect("write daemon response");
        writer.flush().expect("flush daemon response");
    });

    StubDaemon {
        addr: addr_string,
        handle: Some(handle),
    }
}

fn sample_preflight_request() -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            role_id: Some("claude-orchestrator".to_string()),
            role_name: Some("Claude Orchestrator".to_string()),
            focus_area: Some("Team sequencing and escalation".to_string()),
            context_summary: Some(
                "Keeps the full delivery plan and blocker state in view.".to_string(),
            ),
            behavior_summary: Some("Coordinates specialists and escalates blockers.".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
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
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: Some("codex-architect".to_string()),
                role_name: Some("Codex Architect".to_string()),
                focus_area: Some("Architecture decisions and structural review".to_string()),
                context_summary: Some(
                    "Carries long-lived context around module boundaries and reviews.".to_string(),
                ),
                behavior_summary: Some(
                    "Handles pattern choices and escalates direction changes.".to_string(),
                ),
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
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
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "agy".to_string(),
                model: "pro".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-api".to_string(),
                description: None,
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
        ],
    }
}

fn sample_add_agent_request(team_name: &str, member_name: &str) -> AddAgentRequest {
    AddAgentRequest {
        team_name: team_name.to_string(),
        agent: AgentSetupConfig {
            name: member_name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
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
    }
}

fn sample_resume_request(team_name: &str, member_name: &str) -> ResumeMemberRequest {
    ResumeMemberRequest {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    }
}

#[test]
fn team_summary_serialization_round_trip() {
    let value = TeamSummary {
        team_name: "architecture-final".to_string(),
        lead_project_path: Some("/tmp/taurhaus".to_string()),
    };
    let json = serde_json::to_string(&value).expect("serialize team summary");
    let decoded: TeamSummary = serde_json::from_str(&json).expect("deserialize team summary");
    assert_eq!(decoded, value);
}

#[test]
fn team_discovery_response_serialization_round_trip() {
    let value = TeamDiscoveryResponse {
        teams: vec![TeamSummary {
            team_name: "architecture-final".to_string(),
            lead_project_path: Some("/tmp/taurhaus".to_string()),
        }],
        warnings: vec!["skipped team folder 'broken-team'".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize team discovery response");
    let decoded: TeamDiscoveryResponse =
        serde_json::from_str(&json).expect("deserialize team discovery response");
    assert_eq!(decoded, value);
}

#[test]
fn team_status_serialization_round_trip() {
    let value = TeamStatus {
        team_name: "architecture-final".to_string(),
        members: vec!["team-lead".to_string(), "codex-reviewer".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize team status");
    let decoded: TeamStatus = serde_json::from_str(&json).expect("deserialize team status");
    assert_eq!(decoded, value);
}

#[test]
fn disband_team_response_serialization_round_trip() {
    let value = DisbandTeamResponse {
        team_name: "architecture-final".to_string(),
        disbanded: false,
        already_disbanded: true,
        message: "team already disbanded".to_string(),
    };
    let json = serde_json::to_string(&value).expect("serialize disband response");
    let decoded: DisbandTeamResponse =
        serde_json::from_str(&json).expect("deserialize disband response");
    assert_eq!(decoded, value);
}

#[test]
fn create_team_rejects_empty_or_whitespace_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err = coordination_create_team_impl(&state, "".to_string()).expect_err("empty should fail");
    assert!(err.contains("team_name"));

    let err = coordination_create_team_impl(&state, "   \n\t  ".to_string())
        .expect_err("whitespace should fail");
    assert!(err.contains("team_name"));
}

#[test]
fn member_commands_validate_all_required_fields() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err = coordination_disband_team_impl(&state, "   ".to_string())
        .expect_err("blank team should fail");
    assert!(err.contains("team_name"));

    let err = coordination_add_member_impl(
        &state,
        "team".to_string(),
        "".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("empty member should fail");
    assert!(err.contains("member_name"));

    let err = coordination_add_member_impl(
        &state,
        "team".to_string(),
        "alice".to_string(),
        "".to_string(),
        None,
    )
    .expect_err("empty backend should fail");
    assert!(err.contains("backend_kind"));

    let err = coordination_remove_member_impl(&state, "".to_string(), "alice".to_string())
        .expect_err("empty team should fail");
    assert!(err.contains("team_name"));

    let err = coordination_remove_member_impl(&state, "team".to_string(), "  ".to_string())
        .expect_err("whitespace member should fail");
    assert!(err.contains("member_name"));
}

#[test]
fn get_team_status_validates_non_empty_team_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err =
        coordination_get_team_status_impl(&state, " ".to_string()).expect_err("whitespace invalid");
    assert!(err.contains("team_name"));
}

#[test]
fn preflight_all_tools_present_returns_clean_report() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(report.can_initialize);
    assert!(report.blocking_errors.is_empty());
    assert!(report.agent_warnings.is_empty());
}

#[test]
fn preflight_mesh_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["tmux", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report.blocking_errors.contains(
        &"Mesh CLI not found. Install it to enable multi-agent collaboration.".to_string()
    ));
}

#[test]
fn preflight_tmux_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report
        .blocking_errors
        .contains(&"tmux is required for multi-agent sessions.".to_string()));
}

#[test]
fn preflight_agent_tool_missing_returns_warning() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(report.can_initialize);
    assert!(report.blocking_errors.is_empty());
    assert_eq!(report.agent_warnings.len(), 1);
    assert_eq!(report.agent_warnings[0].agent_name, "frontend-dev");
    assert_eq!(report.agent_warnings[0].cli_tool, "codex");
    assert!(report.agent_warnings[0]
        .message
        .contains("Codex CLI not found"));
}

#[test]
fn preflight_multiple_issues_reports_all() {
    let lookup = MockBinaryLookup::with_available(&["codex"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert_eq!(report.blocking_errors.len(), 2);
    assert_eq!(report.agent_warnings.len(), 2);
    assert!(report
        .agent_warnings
        .iter()
        .any(|w| w.agent_name == "team-lead" && w.message.contains("Claude CLI not found")));
    assert!(report
        .agent_warnings
        .iter()
        .any(|w| w.agent_name == "reviewer" && w.message.contains("Antigravity CLI not found")));
}

#[test]
fn feature_availability_reports_missing_mesh_and_tmux() {
    let lookup = MockBinaryLookup::with_available(&["claude"]);
    let report = coordination_get_feature_availability_with_lookup(&lookup);
    assert!(!report.can_initialize);
    assert!(!report.mesh_available);
    assert!(!report.tmux_available);
    assert_eq!(report.blocking_errors.len(), 2);
}

#[test]
fn feature_availability_reports_ready_when_required_tools_exist() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);
    let report = coordination_get_feature_availability_with_lookup(&lookup);
    assert!(report.can_initialize);
    assert!(report.mesh_available);
    assert!(report.tmux_available);
    assert!(report.blocking_errors.is_empty());
}

#[test]
fn initialize_ipc_delegates_to_orchestrator_and_returns_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let (db, _db_file) = test_db_state();
    let request = sample_preflight_request();

    let report = coordination_initialize_team_internal(
        &state,
        Some(&db),
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert!(report.failed_step.is_none());
    assert!(!report.steps.is_empty());
    assert_eq!(report.steps[0].step, "validate_configuration");
}

#[test]
fn initialize_writes_initial_operational_snapshots_for_members() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let (db, _db_file) = test_db_state();

    coordination_initialize_team_internal(
        &state,
        Some(&db),
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot =
        OperationalContextSnapshotStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("load snapshot")
            .expect("snapshot exists");

    assert_eq!(snapshot.task.id, "");
    assert_eq!(snapshot.task.subject, "");
    assert_eq!(snapshot.task.status, "");
    assert_eq!(snapshot.assignment_footer.execution_mode, "");
    assert_eq!(snapshot.working_set.project_path, "proj-web");
    assert!(snapshot.working_set.focal_files.is_empty());
}

#[test]
fn initialize_progress_events_are_emitted_in_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let request = sample_preflight_request();

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| {
        emitted.push(event.clone());
    };
    let report = coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("initialize should return a report");

    assert_eq!(emitted.len(), report.steps.len() * 2);
    for (idx, step) in report.steps.iter().enumerate() {
        let running = &emitted[idx * 2];
        let completed = &emitted[idx * 2 + 1];
        assert_eq!(running.operation, "initialize_team");
        assert_eq!(running.progress.step, step.step);
        assert_eq!(running.progress.status, StepStatus::Running);
        assert_eq!(completed.progress.step, step.step);
        assert_eq!(completed.progress.status, step.status);
    }
}

#[test]
fn initialize_error_case_returns_structured_failed_step_report() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let mut request = sample_preflight_request();
    request.agents[0].name = request.lead.name.clone(); // duplicate name -> validation step failure

    let report = coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return structured failure report");
    assert_eq!(
        report.failed_step.as_deref(),
        Some("validate_configuration")
    );
    assert!(report.retryable);
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].status, StepStatus::Failed);
}

#[test]
fn initialize_failure_after_team_creation_does_not_get_rewritten_to_config_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding delivery failure".to_string(),
    ));
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    let (db, _db_file) = test_db_state();

    // Regression: when initialize failed after create_team, the command layer
    // still ran post-initialize config sync and overwrote the real failed step
    // with a raw "team config not found" error after cleanup deleted the team.
    let report = coordination_initialize_team_internal(
        &state,
        Some(&db),
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return structured failure report");

    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(report.retryable);
    assert!(
        report
            .message
            .contains("simulated onboarding delivery failure"),
        "expected original onboarding failure, got: {}",
        report.message
    );
}

#[test]
fn add_agent_ipc_returns_add_agent_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let report = coordination_add_agent_internal(
        &state,
        None,
        sample_add_agent_request("arch", "bob"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return report");

    assert_eq!(report.team_name, "arch");
    assert_eq!(report.member_name, "bob");
    assert!(report.failed_step.is_none());
    assert!(report
        .succeeded_steps
        .contains(&"update_roster".to_string()));
    assert!(!report.steps.is_empty());
}

#[test]
fn add_agent_progress_events_are_emitted_in_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| emitted.push(event.clone());
    let report = coordination_add_agent_internal(
        &state,
        None,
        sample_add_agent_request("arch", "bob"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("add-agent should return report");

    assert_eq!(emitted.len(), report.steps.len() * 2);
    for (idx, step) in report.steps.iter().enumerate() {
        let running = &emitted[idx * 2];
        let completed = &emitted[idx * 2 + 1];
        assert_eq!(running.operation, "add_agent");
        assert_eq!(running.progress.step, step.step);
        assert_eq!(running.progress.status, StepStatus::Running);
        assert_eq!(completed.progress.step, step.step);
        assert_eq!(completed.progress.status, step.status);
    }
}

#[test]
fn reonboard_succeeds_for_existing_member() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let result = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "architecture-final".to_string(),
            member_name: "team-lead".to_string(),
        },
    )
    .expect("reonboard should succeed");

    assert!(result.delivered);
    let requests = fake.delivered_requests();
    let DeliveryRequest::OperatorNotice(delivery) = requests.last().expect("reonboard delivery")
    else {
        panic!("expected operator notice")
    };
    // Regression: commit efcd7d2 silently replaced Claude re-onboarding with
    // the lifecycle-only role-context block, dropping the explicit mesh loop.
    assert!(delivery.message.starts_with("[taurhaus] onboarding"));
    assert!(delivery.message.contains("mesh read --unread --mark-read"));
}

#[test]
fn reonboard_fails_for_nonexistent_team_or_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let missing_team = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "missing".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("missing team should fail");
    assert!(missing_team.contains("Not found"));

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let missing_member = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "arch".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("missing member should fail");
    assert!(missing_member.contains("Not found"));
}

#[test]
fn add_agent_and_reonboard_validate_empty_strings() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let add_agent_err = coordination_add_agent_internal(
        &state,
        None,
        AddAgentRequest {
            team_name: " ".to_string(),
            agent: AgentSetupConfig {
                name: "".to_string(),
                cli_tool: "".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "".to_string(),
                description: None,
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
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty add-agent fields should fail");
    assert!(add_agent_err.contains("team_name"));

    let reonboard_team_err = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("empty team_name should fail");
    assert!(reonboard_team_err.contains("team_name"));

    let reonboard_member_err = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "arch".to_string(),
            member_name: "  ".to_string(),
        },
    )
    .expect_err("empty member_name should fail");
    assert!(reonboard_member_err.contains("member_name"));
}

#[test]
fn resume_member_validates_empty_fields() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = coordination_resume_member_internal(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "".to_string(),
            member_name: "agent".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty team_name should fail");
    assert!(err.contains("team_name"));

    let err = coordination_resume_member_internal(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "arch".to_string(),
            member_name: " ".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("blank member_name should fail");
    assert!(err.contains("member_name"));
}

#[test]
fn resume_member_ipc_returns_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
    )
    .expect("member runtime");
    runtime.health = crate::coordination::domain::HealthState::SessionDead;
    crate::coordination::stores::MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &runtime,
    )
    .expect("save runtime");

    let report = coordination_resume_member_internal(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("resume should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert_eq!(report.member_name, "frontend-dev");
    assert!(report.resumed);
    assert_eq!(report.failed_step, None);
    assert!(!report.steps.is_empty());
}

#[test]
fn resume_member_progress_events_are_emitted_from_canonical_member_stages() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
    )
    .expect("member runtime");
    runtime.health = crate::coordination::domain::HealthState::SessionDead;
    runtime.pane_id = None;
    runtime.daemon_pid = None;
    crate::coordination::stores::MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &runtime,
    )
    .expect("save runtime");

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| emitted.push(event.clone());
    let report = coordination_resume_member_internal(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("resume should return a report");

    assert!(report.resumed);
    assert!(!emitted.is_empty());
    assert!(emitted
        .iter()
        .all(|event| event.operation == "resume_member"));
    assert!(emitted.iter().any(|event| {
        event.progress.step == "prepare_member"
            && event.progress.status == StepStatus::Running
            && event.canonical_stages == vec![MemberActivationStage::PrepareMember]
    }));
    assert!(emitted.iter().any(|event| {
        event.progress.step == "commit_runtime"
            && event.progress.status == StepStatus::Succeeded
            && event.canonical_stages == vec![MemberActivationStage::CommitRuntime]
    }));
}

#[test]
fn resume_team_validates_empty_team_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty team_name should fail");
    assert!(err.contains("team_name"));
}

#[test]
fn resume_team_ipc_returns_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "architecture-final",
            member_name,
        )
        .expect("member runtime");
        runtime.health = crate::coordination::domain::HealthState::SessionDead;
        runtime.pane_id = None;
        runtime.daemon_pid = None;
        crate::coordination::stores::MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime,
        )
        .expect("save runtime");
    }

    let report = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "architecture-final".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("resume should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert!(report.resumed);
    assert_eq!(report.total_members, 3);
    assert_eq!(report.resumed_members.len(), 3);
    assert!(report.failed_members.is_empty());
}

#[test]
fn resume_team_progress_events_are_emitted_per_member_stage() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "architecture-final",
            member_name,
        )
        .expect("member runtime");
        runtime.health = crate::coordination::domain::HealthState::SessionDead;
        runtime.pane_id = None;
        runtime.daemon_pid = None;
        crate::coordination::stores::MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime,
        )
        .expect("save runtime");
    }

    let mut emitted = Vec::new();
    let mut emit = |event: &ResumeTeamProgressEvent| emitted.push(event.clone());
    let report = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "architecture-final".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("resume should return a report");

    assert!(report.resumed);
    assert!(!emitted.is_empty());
    assert!(emitted.iter().all(|event| event.operation == "resume_team"));
    assert!(emitted.iter().all(|event| event.member_count == 3));
    assert_eq!(
        emitted
            .iter()
            .filter(|event| {
                event.status == StepStatus::Running
                    && event.stage == MemberActivationStage::PrepareMember
            })
            .map(|event| (event.member_name.clone(), event.member_index))
            .collect::<Vec<_>>(),
        vec![
            ("team-lead".to_string(), 1),
            ("frontend-dev".to_string(), 2),
            ("reviewer".to_string(), 3),
        ]
    );
    assert!(emitted.iter().any(|event| {
        event.member_name == "reviewer"
            && event.stage == MemberActivationStage::CommitRuntime
            && event.status == StepStatus::Succeeded
    }));
}

#[test]
fn initialize_success_surfaces_terminal_with_taurhaus_tmux_session() {
    let (db, _db_file) = test_db_state();
    let expected_emulator =
        crate::commands::terminal_settings::load_terminal_settings(&db).emulator;
    let _ = crate::terminal::take_recorded_terminal_intents();

    // Regression: before this fix, successful team initialization returned its
    // report without surfacing the terminal; no single breaking commit was
    // identified, but the command layer omitted the shared EnsureOpen path.
    maybe_surface_terminal_after_initialize(
        &db,
        Some("Ubuntu".to_string()),
        &InitializeReport {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec!["launch_sessions".to_string()],
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps: Vec::new(),
        },
    );

    assert_eq!(
        crate::terminal::take_recorded_terminal_intents(),
        vec![crate::terminal::TerminalIntent::EnsureOpen {
            distro: Some("Ubuntu".to_string()),
            tmux_session: "taurhaus".to_string(),
            emulator: expected_emulator,
            custom_command: String::new(),
        }]
    );
}

#[test]
fn initialize_failure_does_not_surface_terminal() {
    let (db, _db_file) = test_db_state();
    let _ = crate::terminal::take_recorded_terminal_intents();

    maybe_surface_terminal_after_initialize(
        &db,
        Some("Ubuntu".to_string()),
        &InitializeReport {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec!["create_team".to_string()],
            failed_step: Some("launch_sessions".to_string()),
            retryable: true,
            message: "launch failed".to_string(),
            steps: Vec::new(),
        },
    );

    assert!(crate::terminal::take_recorded_terminal_intents().is_empty());
}

#[test]
fn resume_team_surfaces_terminal_only_when_members_resumed() {
    let (db, _db_file) = test_db_state();
    let expected_emulator =
        crate::commands::terminal_settings::load_terminal_settings(&db).emulator;
    let _ = crate::terminal::take_recorded_terminal_intents();

    // Regression: before this fix, successful team resume rebuilt panes and
    // sessions but left the terminal hidden; no single breaking commit was
    // identified, but the command layer never called the shared terminal path.
    maybe_surface_terminal_after_resume_team(
        &db,
        Some("Ubuntu".to_string()),
        &ResumeTeamReport {
            team_name: "architecture-final".to_string(),
            resumed: true,
            total_members: 3,
            resumed_members: vec![
                "team-lead".to_string(),
                "frontend-dev".to_string(),
                "reviewer".to_string(),
            ],
            failed_members: Vec::new(),
            warnings: Vec::new(),
            started_team_daemon: true,
            team_daemon_warning: None,
        },
    );

    assert_eq!(
        crate::terminal::take_recorded_terminal_intents(),
        vec![crate::terminal::TerminalIntent::EnsureOpen {
            distro: Some("Ubuntu".to_string()),
            tmux_session: "taurhaus".to_string(),
            emulator: expected_emulator,
            custom_command: String::new(),
        }]
    );

    maybe_surface_terminal_after_resume_team(
        &db,
        Some("Ubuntu".to_string()),
        &ResumeTeamReport {
            team_name: "architecture-final".to_string(),
            resumed: false,
            total_members: 3,
            resumed_members: Vec::new(),
            failed_members: vec![ResumeTeamMemberFailure {
                member_name: "team-lead".to_string(),
                message: "resume failed".to_string(),
                retryable: true,
            }],
            warnings: Vec::new(),
            started_team_daemon: false,
            team_daemon_warning: Some("not started".to_string()),
        },
    );

    assert!(crate::terminal::take_recorded_terminal_intents().is_empty());
}

#[test]
fn create_team_happy_path_persists_team() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create should succeed");
    let discovery = coordination_list_teams_impl(&state).expect("list should succeed");
    assert_eq!(
        discovery.teams,
        vec![TeamSummary {
            team_name: "arch".to_string(),
            lead_project_path: None,
        }]
    );
    assert!(discovery.warnings.is_empty());
}

#[test]
fn create_team_error_mapping_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("first create");
    let err = coordination_create_team_impl(&state, "arch".to_string())
        .expect_err("duplicate should fail");
    assert!(err.contains("Conflict"));
}

#[test]
fn disband_team_happy_path_removes_team() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let result = coordination_disband_team_impl(&state, "arch".to_string()).expect("disband");
    assert!(result.disbanded);
    assert!(!result.already_disbanded);
    assert_eq!(result.message, "team disbanded");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert!(discovery.teams.is_empty());
    assert!(discovery.warnings.is_empty());
}

#[test]
fn disband_team_is_idempotent_and_reports_already_disbanded() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let result = coordination_disband_team_impl(&state, "missing".to_string())
        .expect("idempotent disband should succeed");
    assert!(!result.disbanded);
    assert!(result.already_disbanded);
    assert_eq!(result.message, "team already disbanded");
}

#[test]
fn add_member_happy_path_persists_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add member");
    let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
    assert_eq!(status.members, vec!["alice".to_string()]);
}

#[test]
fn the_first_grok_member_added_through_this_mutation_earns_the_hook() {
    // Regression: commit 86601a2 left `coordination_add_member` outside hook
    // reconciliation, so the grok member it persists was invisible to the hook
    // installer until an unrelated roster change ran.
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let grok_home = tmp.path().join("grok-home");
    let exe = tmp.path().join("taurhaus");
    std::fs::write(&exe, b"fixture").expect("executable fixture");

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "grok".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add grok member");

    // What the reconciliation this command now runs sees on that roster.
    assert!(
        crate::coordination::compact_hook::any_managed_grok_member(state.teams_dir())
            .expect("managed grok discovery"),
        "the member this mutation persists is a managed grok member"
    );
    crate::coordination::compact_hook::ensure_grok_compact_hook_installed_at(&grok_home, &exe)
        .expect("install grok hook");
    assert!(
        crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home),
        "the first grok member on the roster earns grok's one global hook"
    );
}

#[test]
fn add_member_error_mapping_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = coordination_add_member_impl(
        &state,
        "missing".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("missing team");
    assert!(err.contains("Not found"));
}

#[test]
fn add_member_requires_project_path_when_team_has_no_members() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let err = coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("missing project path should fail for empty team");
    assert!(err.contains("project_path must be provided"));
}

#[test]
fn add_member_defaults_to_lead_project_path_instead_of_process_cwd() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let request = sample_preflight_request();

    coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    coordination_add_member_impl(
        &state,
        "architecture-final".to_string(),
        "legacy-dev".to_string(),
        "codex".to_string(),
        None,
    )
    .expect("add member");

    let member_project_path = state
        .with_orchestrator(|orchestrator| {
            let status = orchestrator.get_team_status("architecture-final")?;
            let member = status
                .config
                .members
                .iter()
                .find(|member| member.name == "legacy-dev")
                .ok_or_else(|| {
                    CoordinationError::NotFound("member 'legacy-dev' not found".to_string())
                })?;
            Ok(member.project_path.clone())
        })
        .expect("member should be persisted");

    assert_eq!(member_project_path, PathBuf::from("proj-core"));
}

#[test]
fn derive_cross_project_status_returns_false_for_same_normalized_path() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/taurhaus"),
        Path::new("/home/user/projects/taurhaus/"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_returns_true_for_different_project_path() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/taurhaus"),
        Path::new("/home/user/projects/mesh"),
    );

    assert!(status.is_cross_project);
    assert_eq!(status.project_label, "mesh");
}

#[test]
fn derive_cross_project_status_matches_windows_and_linux_forms() {
    let status = derive_cross_project_status(
        Path::new("/mnt/c/Users/me/code/taurhaus"),
        Path::new(r"C:\Users\me\code\taurhaus"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_matches_case_variant_windows_paths() {
    let status = derive_cross_project_status(
        Path::new(r"C:\Users\Me\Code\Taurhaus"),
        Path::new(r"c:\users\me\code\taurhaus"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_matches_wsl_unc_and_linux_forms() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/mesh"),
        Path::new(r"\\wsl.localhost\Ubuntu\home\user\projects\mesh"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn remove_member_happy_path_removes_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add");
    let report = coordination_remove_member_impl(&state, "arch".to_string(), "alice".to_string())
        .expect("remove");
    assert!(report.removed);
    assert_eq!(report.team_name, "arch");
    assert_eq!(report.member_name, "alice");
    assert!(!report.steps.is_empty());
    let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
    assert!(status.members.is_empty());
}

#[test]
fn remove_member_error_mapping_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let err = coordination_remove_member_impl(&state, "arch".to_string(), "missing".to_string())
        .expect_err("missing member");
    assert!(err.contains("Not found"));
}

#[test]
fn list_teams_happy_path_returns_sorted_summaries() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "zeta".to_string()).expect("create zeta");
    coordination_create_team_impl(&state, "alpha".to_string()).expect("create alpha");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(
        discovery.teams,
        vec![
            TeamSummary {
                team_name: "alpha".to_string(),
                lead_project_path: None,
            },
            TeamSummary {
                team_name: "zeta".to_string(),
                lead_project_path: None,
            }
        ]
    );
    assert!(discovery.warnings.is_empty());
}

#[test]
fn list_teams_error_mapping_io() {
    let tmp = TempDir::new().expect("tempdir");
    let file_path = tmp.path().join("teams-file");
    std::fs::write(&file_path, "not a directory").expect("write marker");
    let state = test_state(file_path);

    let err = coordination_list_teams_impl(&state).expect_err("list should fail");
    assert!(err.contains("IO error"));
}

#[test]
fn list_teams_includes_lead_project_anchor() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let request = sample_preflight_request();
    coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, "architecture-final");
    assert_eq!(
        discovery.teams[0].lead_project_path.as_deref(),
        Some("proj-core")
    );
}

#[test]
fn list_teams_skips_corrupt_folders_with_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "good".to_string()).expect("create");
    let broken_dir = tmp.path().join("broken-team");
    std::fs::create_dir_all(&broken_dir).expect("create broken dir");
    std::fs::write(broken_dir.join("config.json"), "{ invalid").expect("write broken");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, "good");
    assert_eq!(discovery.warnings.len(), 1);
    assert!(discovery.warnings[0].contains("broken-team"));
}

#[test]
fn get_team_status_happy_path_returns_team_and_members() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add");
    let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
    assert_eq!(status.team_name, "arch");
    assert_eq!(status.members, vec!["alice".to_string()]);
}

#[test]
fn get_team_status_error_mapping_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err =
        coordination_get_team_status_impl(&state, "missing".to_string()).expect_err("missing team");
    assert!(err.contains("Not found"));
}

#[test]
fn project_mesh_snapshot_returns_null_team_when_project_has_no_match() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "/projects/missing".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert!(snapshot.mesh_available);
    assert!(snapshot.tmux_available);
    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::None);
    assert_eq!(snapshot.team_name, None);
    assert_eq!(snapshot.team_status, None);
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn project_mesh_snapshot_classifies_active_when_all_members_are_live() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Active);
}

#[test]
fn project_mesh_snapshot_classifies_degraded_when_live_and_offline_members_mix() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let offline_pane = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime")
        .pane_id
        .expect("pane id");
    runtime.set_pane_exists(&offline_pane, false);
    let mut offline_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("reload runtime");
    offline_runtime.health = HealthState::SessionDead;
    offline_runtime.session_id = None;
    offline_runtime.daemon_pid = None;
    MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &offline_runtime,
    )
    .expect("persist offline runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Degraded);
    let team_status = snapshot.team_status.expect("team status");
    let frontend_dev = team_status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);
}

#[test]
fn project_mesh_snapshot_classifies_cold_resume_when_all_members_are_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", member_name)
                .expect("load runtime");
        let pane_id = runtime_record.pane_id.clone().expect("pane id");
        runtime.set_pane_exists(&pane_id, false);
        runtime_record.health = HealthState::SessionDead;
        runtime_record.session_id = None;
        runtime_record.daemon_pid = None;
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime_record,
        )
        .expect("persist offline runtime");
    }

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::ColdResume);
    let team_status = snapshot.team_status.expect("team status");
    assert!(team_status
        .members
        .iter()
        .all(|member| member.session_status == SessionStatus::Offline));
}

#[test]
fn project_mesh_snapshot_uses_fast_snapshot_without_runtime_reconcile_calls() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime");
    let pane_id = record.pane_id.clone().expect("pane id");
    record.health = HealthState::SessionDead;
    record.session_id = None;
    record.daemon_pid = None;
    record.last_seen_at = None;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    runtime.set_pane_exists(&pane_id, true);
    runtime.set_pane_dead(&pane_id, false);
    runtime.set_pane_shell(&pane_id, false);
    runtime.set_matching_daemon_pids(&pane_id, "architecture-final", "frontend-dev", &[5555]);
    let call_count_before = runtime.calls().len();

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let frontend_dev = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);
    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("reload runtime");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.daemon_pid, None);
    let calls = runtime.calls();
    let snapshot_calls = &calls[call_count_before..];
    assert!(
        !snapshot_calls.iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPaneExists { .. }
                | RuntimeCall::CheckPaneDead { .. }
                | RuntimeCall::CheckPaneShell { .. }
                | RuntimeCall::FindDaemon { .. }
                | RuntimeCall::SpawnDaemon { .. }
        )),
        "snapshot path should not execute runtime liveness probes"
    );
}

#[test]
fn project_mesh_snapshot_returns_fast_team_snapshot_for_matching_project() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Active);
    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(snapshot.warnings.is_empty());

    let team_status = snapshot.team_status.expect("team status should be present");
    assert_eq!(team_status.lead_name, "team-lead");
    assert_eq!(team_status.members.len(), 3);

    let frontend_dev = team_status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should be present");
    assert_eq!(frontend_dev.role, AgentRole::Member);
    assert_eq!(frontend_dev.cli_tool, "codex");
    assert_eq!(frontend_dev.project_id, "proj-web");
    assert!(frontend_dev.is_cross_project);
    assert_eq!(frontend_dev.project_label, "proj-web");
    assert_eq!(frontend_dev.session_status, SessionStatus::Active);
    assert_eq!(frontend_dev.pane_id.as_deref(), Some("%9"));
    assert_eq!(frontend_dev.role_id.as_deref(), Some("codex-architect"));
    assert_eq!(frontend_dev.role_name.as_deref(), Some("Codex Architect"));
    assert_eq!(
        frontend_dev.focus_area.as_deref(),
        Some("Architecture decisions and structural review")
    );
    assert_eq!(
        frontend_dev.context_summary.as_deref(),
        Some("Carries long-lived context around module boundaries and reviews.")
    );
    assert_eq!(
        frontend_dev.behavior_summary.as_deref(),
        Some("Handles pattern choices and escalates direction changes.")
    );
}

#[test]
fn project_mesh_snapshot_prefers_persisted_active_team_when_multiple_teams_match_project() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut old_request = sample_preflight_request();
    old_request.team_name = "towerhouse-product-team".to_string();
    coordination_initialize_team_internal(
        &state,
        None,
        old_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize old team");

    let mut active_request = sample_preflight_request();
    active_request.team_name = "taurhaus-team".to_string();
    coordination_initialize_team_internal(
        &state,
        None,
        active_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize active team");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("taurhaus-team"));
    assert!(snapshot.team_status.is_some());
}

#[test]
fn project_mesh_snapshot_recovers_missing_active_team_mapping_from_runtime_signal() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut old_request = sample_preflight_request();
    old_request.team_name = "towerhouse-product-team".to_string();
    coordination_initialize_team_internal(
        &state,
        None,
        old_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize old team");

    let mut active_request = sample_preflight_request();
    active_request.team_name = "taurhaus-team".to_string();
    coordination_initialize_team_internal(
        &state,
        None,
        active_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize active team");

    ActiveProjectTeamStore::clear_project(tmp.path(), "proj-web").expect("clear active mapping");
    assert_eq!(
        ActiveProjectTeamStore::load_active_team(tmp.path(), "proj-web").expect("load active team"),
        None
    );

    let stale_seen_at = test_timestamp("2026-03-01T10:00:00Z");
    let live_seen_at = test_timestamp("2026-03-11T05:30:00Z");
    for (index, member_name) in ["team-lead", "frontend-dev", "reviewer"]
        .into_iter()
        .enumerate()
    {
        let mut stale_runtime =
            MemberRuntimeStore::load(tmp.path(), "towerhouse-product-team", member_name)
                .expect("load stale runtime");
        stale_runtime.health = HealthState::SessionDead;
        stale_runtime.session_id = None;
        stale_runtime.daemon_pid = None;
        stale_runtime.last_seen_at = Some(stale_seen_at);
        stale_runtime.attached_at = Some(stale_seen_at);
        MemberRuntimeStore::save(
            tmp.path(),
            "towerhouse-product-team",
            member_name,
            &stale_runtime,
        )
        .expect("save stale runtime");

        let mut live_runtime = MemberRuntimeStore::load(tmp.path(), "taurhaus-team", member_name)
            .expect("load live runtime");
        live_runtime.health = HealthState::Healthy;
        live_runtime.session_id = Some(format!("live-session-{member_name}"));
        live_runtime.daemon_pid = Some(9_000 + index as u32);
        live_runtime.last_seen_at = Some(live_seen_at);
        live_runtime.attached_at = Some(live_seen_at);
        if live_runtime.pane_id.is_none() {
            live_runtime.pane_id = Some(format!("%{}", index + 10));
        }
        MemberRuntimeStore::save(tmp.path(), "taurhaus-team", member_name, &live_runtime)
            .expect("save live runtime");
    }

    // Regression: commit 439d04b preferred persisted active-team mappings but still fell back to
    // first-match folder order when the mapping file was missing, so a stale older team could win.
    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("taurhaus-team"));
    assert!(snapshot.team_status.is_some());
    assert_eq!(
        ActiveProjectTeamStore::load_active_team(tmp.path(), "proj-web")
            .expect("load repaired active team")
            .as_deref(),
        Some("taurhaus-team")
    );
}

#[test]
fn project_mesh_snapshot_resolves_role_metadata_when_initialize_request_only_has_role_ids() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let request = InitializeTeamRequest {
        team_name: "review-team".to_string(),
        team_description: Some("Review-focused team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "claude-opus-4-6".to_string(),
            role_id: Some("claude-orchestrator".to_string()),
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: None,
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
        agents: vec![
            AgentSetupConfig {
                name: "reviewer-1".to_string(),
                cli_tool: "claude".to_string(),
                model: "claude-opus-4-6".to_string(),
                role_id: Some("claude-reviewer".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: None,
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
            AgentSetupConfig {
                name: "reviewer-2".to_string(),
                cli_tool: "claude".to_string(),
                model: "claude-opus-4-6".to_string(),
                role_id: Some("claude-reviewer".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: None,
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
        ],
    };

    coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");
    let team_status = snapshot.team_status.expect("team status");

    let reviewer = team_status
        .members
        .iter()
        .find(|member| member.name == "reviewer-1")
        .expect("reviewer-1 should be present");

    assert_eq!(reviewer.role_id.as_deref(), Some("claude-reviewer"));
    assert_eq!(reviewer.role_name.as_deref(), Some("Claude Reviewer"));
    assert_eq!(
        reviewer.focus_area.as_deref(),
        Some("Risk-focused review and validation")
    );
    assert!(reviewer
        .context_summary
        .as_deref()
        .unwrap_or_default()
        .contains("risk hotspots"));
    assert!(reviewer
        .behavior_summary
        .as_deref()
        .unwrap_or_default()
        .contains("Reports findings by severity"));
    assert!(reviewer
        .description
        .as_deref()
        .unwrap_or_default()
        .contains("Perform code reviews focused on correctness"));
}

// Regression: a79d392 hydrated role models without checking the requested CLI,
// so request normalization could pair Claude with a Codex-only model slug.
#[test]
fn role_hydration_cli_mismatch_uses_the_requested_tools_catalog_default() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let mut request = sample_preflight_request();
    request.agents[0].cli_tool = "claude".to_string();
    request.agents[0].model.clear();
    request.agents[0].role_id = Some("v3-developer-codex".to_string());
    request.agents[0].role_name = None;

    let hydrated = hydrate_initialize_request_role_metadata(&state, request)
        .expect("role hydration should succeed");

    assert_eq!(hydrated.agents[0].model, "opus");
}

// Regression: b345de1 (PR 5c) gave presets a lead pin for model and effort, but
// preset-driven request hydration composed with `CompositionOverrides::default()`,
// so a minimal initialize payload launched the lead on the lead role's defaults and
// silently dropped the pin the preset stores.
#[test]
fn initialize_request_hydration_applies_the_preset_lead_pin() {
    let tmp = TempDir::new().expect("tempdir");
    let presets_dir = tmp.path().join("templates").join("presets");
    std::fs::create_dir_all(&presets_dir).expect("presets dir");
    std::fs::write(
        presets_dir.join("lead-pinned.yaml"),
        concat!(
            "schema:\n",
            "  kind: team_preset\n",
            "  version: 1\n",
            "preset_id: lead-pinned\n",
            "name: Lead Pinned\n",
            "description: Lead pinned to a model and effort\n",
            "version: \"1.0.0\"\n",
            "lead_role_id: v3-lead-claude\n",
            "lead_overrides:\n",
            "  model: claude-sonnet-4-5\n",
            "  reasoning_effort: xhigh\n",
            "agent_slots:\n",
            "  - role_id: quick-dev-codex\n",
            "    count: 1\n",
            "    project_binding: lead_project\n",
            "defaults:\n",
            "  team_name_pattern: \"{project}-team\"\n",
            "  tmux_layout: tiled\n",
        ),
    )
    .expect("write preset");

    let state = test_state(tmp.path().join("teams"));
    let mut request = sample_preflight_request();
    request.preset_id = Some("lead-pinned".to_string());
    request.lead.cli_tool.clear();
    request.lead.model.clear();
    request.lead.reasoning_effort = None;
    request.lead.role_id = None;
    request.lead.role_name = None;
    request.agents.truncate(1);
    request.agents[0].cli_tool.clear();
    request.agents[0].model.clear();
    request.agents[0].role_id = None;
    request.agents[0].role_name = None;

    let hydrated = hydrate_initialize_request_role_metadata(&state, request)
        .expect("preset hydration should succeed");

    assert_eq!(hydrated.lead.model, "claude-sonnet-4-5");
    assert_eq!(hydrated.lead.reasoning_effort.as_deref(), Some("xhigh"));
}

#[test]
fn initialize_request_hydrates_from_preset_when_frontend_sends_minimal_payload() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));

    let request = InitializeTeamRequest {
        team_name: "dev-team".to_string(),
        team_description: Some("Dev-focused team".to_string()),
        preset_id: Some("dev-team".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: String::new(),
            model: String::new(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: None,
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
        agents: vec![
            AgentSetupConfig {
                name: "dev-1".to_string(),
                cli_tool: String::new(),
                model: String::new(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-core".to_string(),
                description: None,
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
            AgentSetupConfig {
                name: "dev-2".to_string(),
                cli_tool: String::new(),
                model: String::new(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-core".to_string(),
                description: None,
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
        ],
    };

    let report = coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    assert_eq!(report.team_name, "dev-team");

    let stored = TeamConfigStore::load(state.teams_dir(), "dev-team").expect("load team config");
    let lead = stored
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .expect("lead");
    let developer = stored
        .members
        .iter()
        .find(|member| member.name == "dev-1")
        .expect("developer");

    assert_eq!(lead.role_id.as_deref(), Some("v3-lead-claude"));
    assert_eq!(lead.role_name.as_deref(), Some("V3 Team Lead (Claude)"));
    assert_eq!(lead.cli_tool, CliTool::Claude);
    assert_eq!(developer.role_id.as_deref(), Some("v4-developer-codex"));
    assert_eq!(developer.role_name.as_deref(), Some("V4 Developer (Codex)"));
    assert_eq!(developer.cli_tool, CliTool::Codex);
    assert_eq!(developer.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(developer.reasoning_effort.as_deref(), Some("medium"));
    assert!(developer
        .handoff_expectations
        .as_ref()
        .is_some_and(|items| !items.is_empty()));
    assert!(developer
        .instructions
        .as_deref()
        .unwrap_or("")
        .contains("one user-visible behavior"));
}

#[test]
fn project_mesh_snapshot_matches_windows_project_path_to_linux_team_config() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut request = sample_preflight_request();
    request.lead.project_id = "/home/user/projects/lead".to_string();
    request.agents[0].project_id = "/mnt/c/Users/me/code/taurhaus".to_string();
    request.agents[1].project_id = "/home/user/projects/reviewer".to_string();

    coordination_initialize_team_internal(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        r"C:\Users\me\code\taurhaus".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(snapshot.team_status.is_some());
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn project_mesh_snapshot_reports_mesh_unavailable_when_binary_is_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["tmux"]);

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "proj-core".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert!(!snapshot.mesh_available);
    assert!(snapshot.tmux_available);
    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::None);
    assert_eq!(snapshot.team_name, None);
}

#[test]
fn project_mesh_snapshot_skips_missing_config_dirs_without_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    std::fs::create_dir_all(tmp.path().join("default").join("inboxes")).expect("create stale dir");
    std::fs::write(
        tmp.path()
            .join("default")
            .join("inboxes")
            .join("team-lead.json"),
        "{}",
    )
    .expect("write stale inbox");

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "proj-core".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(
        snapshot.warnings.is_empty(),
        "missing config folders should be silently skipped"
    );
}

#[test]
fn initialize_team_request_round_trip() {
    let value = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
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
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
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
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "agy".to_string(),
                model: "pro".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-api".to_string(),
                description: None,
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
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize init request");
    let decoded: InitializeTeamRequest =
        serde_json::from_str(&json).expect("deserialize init request");
    assert_eq!(decoded, value);
}

#[test]
fn initialize_report_round_trip_includes_required_fields() {
    let value = InitializeReport {
        team_name: "architecture-final".to_string(),
        succeeded_steps: vec![
            "validate_configuration".to_string(),
            "create_team".to_string(),
        ],
        failed_step: Some("launch_sessions".to_string()),
        retryable: true,
        message: "launch failed for one member".to_string(),
        steps: vec![
            StepProgress {
                step: "validate_configuration".to_string(),
                status: StepStatus::Succeeded,
                message: Some("ok".to_string()),
            },
            StepProgress {
                step: "launch_sessions".to_string(),
                status: StepStatus::Failed,
                message: Some("codex binary missing".to_string()),
            },
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize init report");
    let decoded: InitializeReport = serde_json::from_str(&json).expect("deserialize init report");
    assert_eq!(decoded, value);
    assert_eq!(decoded.failed_step, Some("launch_sessions".to_string()));
    assert!(decoded.retryable);
}

#[test]
fn add_agent_request_and_report_round_trip() {
    let request = AddAgentRequest {
        team_name: "architecture-final".to_string(),
        agent: AgentSetupConfig {
            name: "backend-dev".to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
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
    let req_json = serde_json::to_string(&request).expect("serialize add-agent request");
    let req_decoded: AddAgentRequest =
        serde_json::from_str(&req_json).expect("deserialize add-agent request");
    assert_eq!(req_decoded, request);

    let report = AddAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        succeeded_steps: vec![
            "create_pane".to_string(),
            "launch_session".to_string(),
            "join_mesh".to_string(),
        ],
        failed_step: None,
        retryable: false,
        message: "agent added".to_string(),
        steps: vec![StepProgress {
            step: "join_mesh".to_string(),
            status: StepStatus::Succeeded,
            message: Some("joined".to_string()),
        }],
    };
    let report_json = serde_json::to_string(&report).expect("serialize add-agent report");
    let report_decoded: AddAgentReport =
        serde_json::from_str(&report_json).expect("deserialize add-agent report");
    assert_eq!(report_decoded, report);
}

#[test]
fn resume_member_request_and_report_round_trip() {
    let request = sample_resume_request("architecture-final", "backend-dev");
    let req_json = serde_json::to_string(&request).expect("serialize resume-member request");
    let req_decoded: ResumeMemberRequest =
        serde_json::from_str(&req_json).expect("deserialize resume-member request");
    assert_eq!(req_decoded, request);

    let report = ResumeAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        resumed: true,
        succeeded_steps: vec![
            "validate".to_string(),
            "resolve_pane".to_string(),
            "launch_session".to_string(),
            "update_runtime".to_string(),
        ],
        failed_step: None,
        retryable: false,
        message: "member resumed".to_string(),
        steps: vec![StepProgress {
            step: "launch_session".to_string(),
            status: StepStatus::Succeeded,
            message: Some("session launched".to_string()),
        }],
        warnings: vec![],
        pane_id: Some("%12".to_string()),
        reused_pane: true,
    };
    let report_json = serde_json::to_string(&report).expect("serialize resume-member report");
    let report_decoded: ResumeAgentReport =
        serde_json::from_str(&report_json).expect("deserialize resume-member report");
    assert_eq!(report_decoded, report);
}

#[test]
fn resume_team_request_and_report_round_trip() {
    let request = ResumeTeamRequest {
        team_name: "architecture-final".to_string(),
    };
    let req_json = serde_json::to_string(&request).expect("serialize resume-team request");
    let req_decoded: ResumeTeamRequest =
        serde_json::from_str(&req_json).expect("deserialize resume-team request");
    assert_eq!(req_decoded, request);

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
fn remove_agent_report_round_trip() {
    let value = RemoveAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        removed: true,
        message: "member removed with 1 warning".to_string(),
        steps: vec![
            StepProgress {
                step: "leave_mesh".to_string(),
                status: StepStatus::Succeeded,
                message: Some("mesh presence removed".to_string()),
            },
            StepProgress {
                step: "kill_pane".to_string(),
                status: StepStatus::Failed,
                message: Some("skipped pane kill for %2 due to ownership mismatch".to_string()),
            },
        ],
        warnings: vec!["skipped pane teardown for %2: ownership mismatch".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize remove-agent report");
    let decoded: RemoveAgentReport =
        serde_json::from_str(&json).expect("deserialize remove-agent report");
    assert_eq!(decoded, value);
}

#[test]
fn live_team_status_round_trip() {
    let value = LiveTeamStatus {
        team_name: "architecture-final".to_string(),
        lead_name: "team-lead".to_string(),
        runtime_snapshot_freshness:
            crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::Fresh,
        members: vec![
            LiveAgentStatus {
                name: "team-lead".to_string(),
                role: AgentRole::Lead,
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                reasoning_effort: None,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-core".to_string(),
                is_cross_project: false,
                project_label: String::new(),
                description: Some("orchestrates work".to_string()),
                session_status: SessionStatus::Active,
                pane_id: Some("%1".to_string()),
                session_id: Some("sess-lead".to_string()),
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
            },
            LiveAgentStatus {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                reasoning_effort: Some("high".to_string()),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-web".to_string(),
                is_cross_project: true,
                project_label: "proj-web".to_string(),
                description: None,
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
                session_id: None,
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
            },
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize live team status");
    let decoded: LiveTeamStatus =
        serde_json::from_str(&json).expect("deserialize live team status");
    assert_eq!(decoded, value);
}

#[test]
fn project_mesh_snapshot_round_trip() {
    let value = ProjectMeshSnapshotResponse {
        mesh_available: true,
        tmux_available: true,
        team_runtime_state: TeamRuntimeState::Active,
        team_name: Some("architecture-final".to_string()),
        team_status: Some(FastTeamSnapshot {
            lead_name: "team-lead".to_string(),
            members: vec![FastAgentSnapshot {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                model: Some("gpt-5.4".to_string()),
                reasoning_effort: Some("high".to_string()),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-web".to_string(),
                is_cross_project: true,
                project_label: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
                session_id: Some("sess-frontend".to_string()),
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
            }],
        }),
        warnings: vec!["skipped team folder 'broken-team'".to_string()],
    };

    let json = serde_json::to_string(&value).expect("serialize project mesh snapshot");
    let decoded: ProjectMeshSnapshotResponse =
        serde_json::from_str(&json).expect("deserialize project mesh snapshot");
    assert_eq!(decoded, value);
}

#[test]
fn live_status_test_helper_invokes_live_status_impl() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let status =
        coordination_get_live_team_status_for_tests(&state, "architecture-final".to_string())
            .expect("live status should succeed");
    assert_eq!(status.team_name, "architecture-final");
    assert_eq!(status.lead_name, "team-lead");
    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::AttachmentsOnly
    );
    assert!(!status.members.is_empty());
    let frontend_dev = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should be present");
    assert!(frontend_dev.is_cross_project);
    assert_eq!(frontend_dev.project_label, "proj-web");
}

#[test]
fn live_status_uses_lightweight_presence_reconcile_without_heavy_daemon_calls() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let call_count_before_live_status = runtime.calls().len();

    let status =
        coordination_get_live_team_status_for_tests(&state, "architecture-final".to_string())
            .expect("live status should succeed");

    assert_eq!(status.team_name, "architecture-final");
    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::AttachmentsOnly
    );
    let delta = &runtime.calls()[call_count_before_live_status..];
    assert!(
        !delta.is_empty(),
        "live status should do pane-presence checks"
    );
    assert!(delta.iter().all(|call| {
        matches!(
            call,
            RuntimeCall::CheckPaneExists { .. }
                | RuntimeCall::CheckPaneDead { .. }
                | RuntimeCall::CheckPaneShell { .. }
                | RuntimeCall::InspectPane { .. }
        )
    }));
}

#[test]
fn live_status_provider_snapshot_yields_to_current_pane_loss() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let frontend_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("frontend runtime");
    let lead_runtime = MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead")
        .expect("lead runtime");
    let reviewer_runtime = MemberRuntimeStore::load(tmp.path(), "architecture-final", "reviewer")
        .expect("reviewer runtime");

    let frontend_pane_id = frontend_runtime.pane_id.clone().expect("frontend pane id");
    runtime.set_pane_exists(&frontend_pane_id, false);

    let snapshot_payload = serde_json::json!({
        "version": 3,
        "display_sessions": [],
        "runtime_sessions": [
            {
                "pid": 101,
                "project_path": "/projects/core",
                "tty": "pts/1",
                "args": "claude",
                "cli_tool": "claude",
                "tmux_session": "taurhaus",
                "tmux_window": "@1",
                "tmux_pane": lead_runtime.pane_id.clone().expect("lead pane id"),
                "tmux_window_name": "architecture-final:team-lead",
                "state": "active",
                "session_id": lead_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "team-lead"
            },
            {
                "pid": 202,
                "project_path": "/projects/web",
                "tty": "pts/2",
                "args": "codex",
                "cli_tool": "codex",
                "tmux_session": "taurhaus",
                "tmux_window": "@2",
                "tmux_pane": frontend_pane_id,
                "tmux_window_name": "architecture-final:frontend-dev",
                "state": "active",
                "session_id": frontend_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "frontend-dev"
            },
            {
                "pid": 303,
                "project_path": "/projects/api",
                "tty": "pts/3",
                "args": "agy",
                "cli_tool": "agy",
                "tmux_session": "taurhaus",
                "tmux_window": "@3",
                "tmux_pane": reviewer_runtime.pane_id.clone().expect("reviewer pane id"),
                "tmux_window_name": "architecture-final:reviewer",
                "state": "active",
                "session_id": reviewer_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "reviewer"
            }
        ],
        "focus": null,
        "foreground_project_path": "/projects/core"
    });
    let decoded_snapshot =
        crate::commands::runtime_snapshot::decode_daemon_runtime_session_snapshot(Some(
            snapshot_payload.clone(),
        ))
        .expect("decode runtime snapshot payload");
    assert_eq!(decoded_snapshot.runtime_sessions.len(), 3);

    let daemon = start_stub_daemon(serde_json::json!({
        "result": snapshot_payload,
        "error": null
    }));
    let provider = ProviderState {
        local: taurhaus_lib::provider::local::LocalProvider,
        daemon: Some(
            taurhaus_lib::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    // Regression: provider-backed live status trusted a stale daemon runtime
    // snapshot and kept pane-loss members active, so the runtime bar stayed on
    // "Team running normally" and exposed Add Agent instead of degraded actions.
    let status = coordination_get_live_team_status_impl(
        &state,
        Some(&provider),
        "architecture-final".to_string(),
    )
    .expect("live status should succeed");

    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::Fresh
    );
    let frontend_dev = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should exist");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("reload frontend runtime");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
}

#[test]
fn step_progress_event_round_trip() {
    let value = StepProgressEvent {
        team_name: "architecture-final".to_string(),
        operation: "initialize_team".to_string(),
        progress: StepProgress {
            step: "send_onboarding".to_string(),
            status: StepStatus::Running,
            message: Some("sending to frontend-dev".to_string()),
        },
        canonical_stages: vec![MemberActivationStage::DeliverOnboarding],
    };

    let json = serde_json::to_string(&value).expect("serialize step progress event");
    let decoded: StepProgressEvent =
        serde_json::from_str(&json).expect("deserialize step progress event");
    assert_eq!(decoded, value);
}

#[test]
fn initialize_progress_events_include_canonical_stage_metadata() {
    let report = InitializeReport {
        team_name: "architecture-final".to_string(),
        succeeded_steps: vec!["create_panes".to_string()],
        failed_step: None,
        retryable: false,
        message: "ok".to_string(),
        steps: vec![StepProgress {
            step: "create_panes".to_string(),
            status: StepStatus::Succeeded,
            message: Some("opened panes".to_string()),
        }],
    };

    let events = super::progress::initialize_progress_events(&report);

    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].canonical_stages,
        vec![
            MemberActivationStage::AcquirePane,
            MemberActivationStage::LaunchSession,
        ]
    );
    assert_eq!(events[1].canonical_stages, events[0].canonical_stages);
}

#[test]
fn initialize_stage_mapping_preserves_existing_batch_steps() {
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::PrepareMember),
        Some("validate_configuration")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::AcquirePane),
        Some("create_panes")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::LaunchSession),
        Some("create_panes")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(
            MemberActivationStage::CaptureSessionIdentity
        ),
        Some("launch_sessions")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::CommitRuntime),
        None
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::DeliverOnboarding),
        Some("send_onboarding")
    );
}

#[test]
fn resume_member_progress_event_helper_uses_canonical_stage_step_name() {
    let event = super::progress::resume_member_progress_event_for_stage(
        "architecture-final",
        MemberActivationStage::CommitRuntime,
        StepStatus::Running,
        Some("writing runtime".to_string()),
    );

    assert_eq!(event.team_name, "architecture-final");
    assert_eq!(event.operation, "resume_member");
    assert_eq!(event.progress.step, "commit_runtime");
    assert_eq!(event.progress.status, StepStatus::Running);
    assert_eq!(
        event.canonical_stages,
        vec![MemberActivationStage::CommitRuntime]
    );
}

#[test]
fn resume_team_progress_event_round_trip() {
    let value = ResumeTeamProgressEvent {
        operation: "resume_team".to_string(),
        team_name: "architecture-final".to_string(),
        member_name: "frontend-dev".to_string(),
        member_index: 2,
        member_count: 3,
        stage: MemberActivationStage::CommitRuntime,
        status: StepStatus::Running,
        message: Some("writing runtime state".to_string()),
    };

    let json = serde_json::to_string(&value).expect("serialize resume-team progress event");
    let decoded: ResumeTeamProgressEvent =
        serde_json::from_str(&json).expect("deserialize resume-team progress event");
    assert_eq!(decoded, value);
}

// Regression: add-agent pipeline failure with db present must not mask the
// real error. Before the fix, cleanup_add_agent_failure removed the member,
// then sync_member_snapshot_after_change tried to look up the now-deleted
// member and threw "member not found in team", hiding the actual pipeline
// failure. Commit 077d57d.
#[test]
fn add_agent_pipeline_failure_does_not_mask_error_with_member_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding delivery failure".to_string(),
    ));
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    coordination_create_team_impl(&state, "arch".to_string()).expect("create team");
    let (db, _db_file) = test_db_state();

    let report = coordination_add_agent_internal(
        &state,
        Some(&db),
        sample_add_agent_request("arch", "dev2"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return structured failure report, not Err");

    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(
        report
            .message
            .contains("simulated onboarding delivery failure"),
        "expected original pipeline error, got: {}",
        report.message
    );
}

// Regression: add-agent onboarding for Claude agents must route through
// deliver_message (per-member backend selection) rather than the default
// backend directly. Before the fix, Claude agents got routed through
// MeshBridgedBackend instead of ClaudeNativeBackend, causing delivery
// failures. Commit 0a0ec11.
#[test]
fn add_agent_onboarding_routes_through_deliver_message_audit_trail() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let mut claude_agent = sample_add_agent_request("arch", "claude-dev");
    claude_agent.agent.cli_tool = "claude".to_string();
    claude_agent.agent.focus_area = Some("backend".to_string());
    claude_agent.agent.context_summary = Some("Rust backend developer".to_string());

    let report = coordination_add_agent_internal(
        &state,
        None,
        claude_agent,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return report");

    assert!(
        report.failed_step.is_none(),
        "add-agent failed at step {:?}: {}",
        report.failed_step,
        report.message
    );
    assert!(report
        .succeeded_steps
        .contains(&"send_onboarding".to_string()));

    // deliver_message emits audit events; verify the onboarding delivery
    // went through the audited path (not self.backend.deliver directly)
    let audit = state
        .with_orchestrator(|orchestrator| {
            Ok::<_, CoordinationError>(orchestrator.drain_audit_log())
        })
        .expect("drain audit");
    let event_types: Vec<&str> = audit.iter().map(|event| event.event_type()).collect();
    assert!(
        event_types.contains(&"delivery_attempted"),
        "expected delivery_attempted audit event for Claude onboarding, got: {:?}",
        event_types
    );
}

#[test]
fn live_team_status_carries_the_member_runtime_session_id() {
    // Regression: 9e15e4e keyed the mesh canvas run tree on a node's Claude
    // session, but LiveAgentStatus never serialized the session the runtime
    // record already held, so no runtime node could ask for its workflow runs.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.session_id = Some("sess-frontend".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.session_id.as_deref(), Some("sess-frontend"));
}

#[test]
fn live_team_status_carries_the_task_effort_the_lead_asked_for() {
    // The node shows the launch effort; the level the lead attached to the
    // current assignment is a different number and has to travel beside it.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    seed_assignment_effort(tmp.path(), "frontend-dev", "high", "irreversible migration");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.task_effort.as_deref(), Some("high"));
    assert_eq!(
        member.task_effort_why.as_deref(),
        Some("irreversible migration")
    );

    let untouched = status
        .members
        .iter()
        .find(|member| member.name != "frontend-dev")
        .expect("the team has another member");
    assert_eq!(untouched.task_effort, None);
    assert_eq!(untouched.task_effort_why, None);
}

#[test]
fn project_mesh_snapshot_carries_the_task_effort_the_lead_asked_for() {
    // The cold-start roster feeds the same canvas, so it carries the same pair.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    seed_assignment_effort(tmp.path(), "frontend-dev", "medium", "routine lane work");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let member = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.task_effort.as_deref(), Some("medium"));
    assert_eq!(member.task_effort_why.as_deref(), Some("routine lane work"));
}

fn seed_assignment_effort(teams_dir: &std::path::Path, member_name: &str, level: &str, why: &str) {
    // mesh writes the effort onto the task record it assigns; taurhaus reads
    // it back off the task the member is on, so that is what a fixture seeds.
    let config =
        crate::coordination::stores::TeamConfigStore::load(teams_dir, "architecture-final")
            .expect("team config");
    let project_path = config
        .members
        .iter()
        .find(|member| member.name == member_name)
        .expect("member is on the roster")
        .project_path
        .display()
        .to_string();

    let db = tempfile::NamedTempFile::new().expect("temp db");
    let conn = taurhaus_lib::db::init_db(db.path()).expect("db");
    taurhaus_lib::db::task_queries::upsert_task(
        &conn,
        &taurhaus_lib::db::task_queries::PersistedTask {
            project_path,
            source: "claude".to_string(),
            source_key: "session-1".to_string(),
            source_task_id: "42".to_string(),
            subject: "Run the migration".to_string(),
            description: None,
            active_form: None,
            status: "in_progress".to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: Some(member_name.to_string()),
            session_id: None,
            first_seen_at: "2026-08-29T09:00:00Z".to_string(),
            state_changed_at: Some("2026-08-29T09:00:00Z".to_string()),
            updated_at: "2026-08-29T09:00:00Z".to_string(),
            archived_at: None,
            last_status: Some("in_progress".to_string()),
            archived_reason: None,
            effort: Some(level.to_string()),
            effort_why: Some(why.to_string()),
        },
    )
    .expect("upsert assigned task");

    crate::coordination::operational_context::sync_team_snapshots(
        teams_dir,
        &conn,
        "architecture-final",
    )
    .expect("sync snapshots");
}

#[test]
fn project_mesh_snapshot_carries_the_member_runtime_session_id() {
    // Regression: the fast snapshot fed the same canvas and dropped the same
    // field, so the cold-start roster could not load runs either (9e15e4e).
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.session_id = Some("sess-frontend".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let member = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.session_id.as_deref(), Some("sess-frontend"));
}

#[test]
fn live_team_status_carries_the_member_workflow_activity() {
    // Regression: d442cf6 gave a runtime node its Claude session but not the
    // workflow hint, so a member whose run tree was visibly live still read
    // Active or Idle on the canvas — the node carried nothing but coordination
    // health, and `activitySignal` had no workflow evidence to promote.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    // A scratch transcript with one summary-less run whose agent just wrote.
    let transcripts = tmp.path().join("transcripts");
    let transcript = transcripts.join("sess-frontend.jsonl");
    let run_dir = transcripts.join("sess-frontend/subagents/workflows/wf_live");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(&transcript, "").expect("transcript");
    std::fs::write(run_dir.join("agent-a1.jsonl"), "{}\n").expect("agent transcript");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.cli_tool = Some(CliTool::Claude);
    record.session_id = Some("sess-frontend".to_string());
    record.jsonl_path = Some(transcript);
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(
        member
            .workflow_activity
            .as_ref()
            .map(|activity| activity.live_runs),
        Some(1)
    );
}

fn daemon_runtime_session(
    team_name: &str,
    member_name: &str,
    jsonl_path: &str,
    workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
) -> crate::session_scanner::RuntimeSession {
    crate::session_scanner::RuntimeSession {
        pid: 4242,
        project_path: "/tmp/taurhaus".to_string(),
        tty: "/dev/pts/3".to_string(),
        args: "claude".to_string(),
        cli_tool: CliTool::Claude,
        tmux_session: Some("taurhaus".to_string()),
        tmux_window: None,
        tmux_pane: Some("%17".to_string()),
        tmux_window_name: None,
        state: crate::session_scanner::SessionState::Active,
        session_id: Some("sess-frontend".to_string()),
        jsonl_path: Some(jsonl_path.to_string()),
        recent_io: false,
        last_output_age_secs: None,
        activity_confidence: Default::default(),
        activity_attribution: Default::default(),
        project_unattributed_active: false,
        group_kind: crate::session_scanner::SessionGroupKind::MeshTeam,
        group_id: Some(team_name.to_string()),
        group_label: None,
        member_name: Some(member_name.to_string()),
        workflow_activity,
    }
}

// Regression: acefb7a answered "is this member running a workflow?" by
// rescanning the transcript in the desktop process. The daemon already computed
// that hint and ships it on the runtime session, and on Windows the transcript
// it names is a WSL path the desktop cannot open — the rescan found nothing and
// the member never showed Working beside its live run tree. The daemon's value
// wins; a local scan is only the fallback, and only for a path this host can
// actually read.
#[test]
fn member_workflow_activity_prefers_the_daemon_hint_over_a_local_rescan() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    // A transcript this host really can read, whose session dir holds one live run.
    let transcripts = tmp.path().join("transcripts");
    let local_transcript = transcripts.join("sess-frontend.jsonl");
    let run_dir = transcripts.join("sess-frontend/subagents/workflows/wf_live");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(&local_transcript, "").expect("transcript");
    std::fs::write(run_dir.join("agent-a1.jsonl"), "{}\n").expect("agent transcript");
    let local_transcript = local_transcript.display().to_string();
    // The path a WSL daemon reports to a Windows desktop.
    let remote_transcript = "/home/daemon-host/.claude/projects/-tmp-taurhaus/sess-frontend.jsonl";

    let member_view = |jsonl_path: &str, live_runs: Option<u32>| {
        let sessions = vec![daemon_runtime_session(
            "architecture-final",
            "frontend-dev",
            jsonl_path,
            live_runs.map(|live_runs| crate::workflow_runs::WorkflowActivity {
                live_runs,
                last_write_at: 1_772_000_000_000,
            }),
        )];
        crate::coordination::roster::get_team_roster_with_runtime_sessions(
            tmp.path(),
            "architecture-final",
            &sessions,
        )
        .expect("roster")
        .into_iter()
        .find(|member| member.member_name == "frontend-dev")
        .expect("frontend-dev is on the roster")
    };

    // A path this host cannot read: the daemon's count is all there is, and it stands.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(remote_transcript, Some(3)))
            .map(|activity| activity.live_runs),
        Some(3)
    );
    // A readable path whose scan would disagree: the daemon's count still wins,
    // which is only possible if no local scan was consulted.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(&local_transcript, Some(3)))
            .map(|activity| activity.live_runs),
        Some(3)
    );
    // No daemon value and a readable path: the local scan still answers.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(&local_transcript, None))
            .map(|activity| activity.live_runs),
        Some(1)
    );
    // No daemon value and a path this host cannot read: nothing to say.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(remote_transcript, None)),
        None
    );
}
