use std::collections::HashSet;
use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
use crate::coordination::domain::HealthState;
use crate::coordination::runtime::RecordingCoordinationRuntime;
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::MemberRuntimeStore;

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
        Arc::new(|_kind| Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    )
}

fn sample_preflight_request() -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                role_id: None,
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: "proj-api".to_string(),
                description: None,
                role_id: None,
                instructions: None,
                behavioral_contract: None,
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
            model: "gpt-5.3".to_string(),
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        },
    }
}

fn sample_resume_request(team_name: &str, member_name: &str) -> ResumeMemberRequest {
    ResumeMemberRequest {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        context_mode: ResumeContextMode::Continue,
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
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex", "gemini"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(report.can_initialize);
    assert!(report.blocking_errors.is_empty());
    assert!(report.agent_warnings.is_empty());
}

#[test]
fn preflight_mesh_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["tmux", "claude", "codex", "gemini"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report.blocking_errors.contains(
        &"Mesh CLI not found. Install it to enable multi-agent collaboration.".to_string()
    ));
}

#[test]
fn preflight_tmux_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "claude", "codex", "gemini"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report
        .blocking_errors
        .contains(&"tmux is required for multi-agent sessions.".to_string()));
}

#[test]
fn preflight_agent_tool_missing_returns_warning() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "gemini"]);
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
        .any(|w| w.agent_name == "reviewer" && w.message.contains("Gemini CLI not found")));
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
    let request = sample_preflight_request();

    let report = coordination_initialize_team_internal(
        &state,
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
fn add_agent_ipc_returns_add_agent_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let report = coordination_add_agent_internal(
        &state,
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
    let state = test_state(tmp.path().to_path_buf());
    coordination_initialize_team_internal(
        &state,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let result = coordination_reonboard_impl(
        &state,
        ReonboardRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
        },
    )
    .expect("reonboard should succeed");

    assert!(result.delivered);
}

#[test]
fn reonboard_fails_for_nonexistent_team_or_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let missing_team = coordination_reonboard_impl(
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
        AddAgentRequest {
            team_name: " ".to_string(),
            agent: AgentSetupConfig {
                name: "".to_string(),
                cli_tool: "".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "".to_string(),
                description: None,
                role_id: None,
                instructions: None,
                behavioral_contract: None,
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
        &state,
        ReonboardRequest {
            team_name: "".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("empty team_name should fail");
    assert!(reonboard_team_err.contains("team_name"));

    let reonboard_member_err = coordination_reonboard_impl(
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
        ResumeMemberRequest {
            team_name: "".to_string(),
            member_name: "agent".to_string(),
            context_mode: ResumeContextMode::Continue,
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty team_name should fail");
    assert!(err.contains("team_name"));

    let err = coordination_resume_member_internal(
        &state,
        ResumeMemberRequest {
            team_name: "arch".to_string(),
            member_name: " ".to_string(),
            context_mode: ResumeContextMode::Fresh,
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
        ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
            context_mode: ResumeContextMode::Continue,
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
    assert_eq!(snapshot.team_name, None);
    assert_eq!(snapshot.team_status, None);
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn project_mesh_snapshot_returns_fast_team_snapshot_for_matching_project() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    coordination_initialize_team_internal(
        &state,
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
    assert_eq!(frontend_dev.session_status, SessionStatus::Active);
    assert_eq!(frontend_dev.pane_id.as_deref(), Some("%9"));
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
    assert_eq!(snapshot.team_name, None);
}

#[test]
fn initialize_team_request_round_trip() {
    let value = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                role_id: None,
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: "proj-api".to_string(),
                description: None,
                role_id: None,
                instructions: None,
                behavioral_contract: None,
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
            model: "gpt-5.3".to_string(),
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
            role_id: None,
            instructions: None,
            behavioral_contract: None,
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
        members: vec![
            LiveAgentStatus {
                name: "team-lead".to_string(),
                role: AgentRole::Lead,
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                project_id: "proj-core".to_string(),
                description: Some("orchestrates work".to_string()),
                session_status: SessionStatus::Active,
                pane_id: Some("%1".to_string()),
            },
            LiveAgentStatus {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-web".to_string(),
                description: None,
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
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
        team_name: Some("architecture-final".to_string()),
        team_status: Some(FastTeamSnapshot {
            lead_name: "team-lead".to_string(),
            members: vec![FastAgentSnapshot {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
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
    assert!(!status.members.is_empty());
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
    };

    let json = serde_json::to_string(&value).expect("serialize step progress event");
    let decoded: StepProgressEvent =
        serde_json::from_str(&json).expect("deserialize step progress event");
    assert_eq!(decoded, value);
}
