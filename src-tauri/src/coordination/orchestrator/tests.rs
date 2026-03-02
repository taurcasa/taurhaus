use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::commands::coordination_types::{
    AddAgentRequest, AgentSetupConfig, InitializeTeamRequest, LeadMode, StepStatus,
};
use crate::coordination::backend::fake::FakeBackend;
use crate::coordination::domain::MemberRole;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};

fn sample_member(name: &str, tool: CliTool) -> Member {
    Member {
        name: name.to_string(),
        role: MemberRole::Agent,
        instructions: Some("focus on implementation".to_string()),
        project_path: PathBuf::from("/tmp/taurhaus"),
        cli_tool: tool,
    }
}

fn new_orchestrator(tmp: &TempDir) -> CoordinationOrchestrator {
    CoordinationOrchestrator::new(tmp.path().to_path_buf(), Arc::new(FakeBackend::default()))
}

fn new_orchestrator_with_backend(
    tmp: &TempDir,
    backend: Arc<dyn CoordinationBackend>,
) -> CoordinationOrchestrator {
    CoordinationOrchestrator::new(tmp.path().to_path_buf(), backend)
}

fn initialize_request(team_name: &str) -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: team_name.to_string(),
        team_description: Some("init pipeline test".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: "/tmp/lead".to_string(),
            description: Some("lead".to_string()),
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "/tmp/frontend".to_string(),
                description: Some("frontend".to_string()),
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: "/tmp/reviewer".to_string(),
                description: Some("review".to_string()),
            },
        ],
    }
}

fn add_agent_request(team_name: &str, agent_name: &str, cli_tool: &str) -> AddAgentRequest {
    AddAgentRequest {
        team_name: team_name.to_string(),
        agent: AgentSetupConfig {
            name: agent_name.to_string(),
            cli_tool: cli_tool.to_string(),
            model: "model".to_string(),
            project_id: format!("/tmp/{agent_name}"),
            description: Some("hot-added".to_string()),
        },
    }
}

fn create_running_team(orchestrator: &mut CoordinationOrchestrator, team_name: &str) {
    orchestrator
        .create_team(team_name, Some("running".to_string()))
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                instructions: Some("lead".to_string()),
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member("existing-dev", CliTool::Codex))
        .expect("add existing member");
}

fn assert_conflict(err: CoordinationError) {
    match err {
        CoordinationError::Conflict(_) => {}
        other => panic!("expected conflict, got {other:?}"),
    }
}

fn assert_not_found(err: CoordinationError) {
    match err {
        CoordinationError::NotFound(_) => {}
        other => panic!("expected not_found, got {other:?}"),
    }
}

#[test]
fn create_team_then_list() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", Some("desc".to_string()))
        .expect("create should succeed");

    let teams = orchestrator.list_teams().expect("list should succeed");
    assert_eq!(teams, vec!["architecture-final".to_string()]);
}

#[test]
fn discover_teams_resolves_lead_project_anchor() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    create_running_team(&mut orchestrator, team_name);

    let discovery = orchestrator
        .discover_teams()
        .expect("discover should succeed");
    assert_eq!(discovery.warnings.len(), 0);
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, team_name);
    assert_eq!(
        discovery.teams[0].lead_project_path.as_deref(),
        Some(std::path::Path::new("/tmp/lead"))
    );
}

#[test]
fn discover_teams_skips_corrupt_folder_with_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let valid_team = "alpha";
    create_running_team(&mut orchestrator, valid_team);

    let broken_dir = tmp.path().join("broken-team");
    std::fs::create_dir_all(&broken_dir).expect("create broken dir");
    std::fs::write(broken_dir.join("config.json"), "{ broken json").expect("write broken");

    let discovery = orchestrator
        .discover_teams()
        .expect("discover should succeed");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, valid_team);
    assert_eq!(discovery.warnings.len(), 1);
    assert!(discovery.warnings[0].contains("broken-team"));

    let teams = orchestrator.list_teams().expect("list should succeed");
    assert_eq!(teams, vec![valid_team.to_string()]);
}

#[test]
fn create_team_duplicate_returns_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", None)
        .expect("first create should succeed");
    let err = orchestrator
        .create_team("architecture-final", None)
        .expect_err("duplicate create should fail");
    assert_conflict(err);
}

#[test]
fn disband_team_removes_directory() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let result = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");
    assert!(result.disbanded);
    assert!(!result.already_disbanded);

    assert!(
        !tmp.path().join(team_name).exists(),
        "team directory should be removed"
    );
}

#[test]
fn disband_nonexistent_team_returns_already_disbanded() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let result = orchestrator
        .disband_team("missing-team", None)
        .expect("idempotent disband should succeed");
    assert!(!result.disbanded);
    assert!(result.already_disbanded);
}

#[test]
fn disband_is_idempotent_and_does_not_invoke_backend_controls() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let first = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("first disband");
    let second = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("second disband");

    assert!(first.disbanded);
    assert!(!first.already_disbanded);
    assert!(!second.disbanded);
    assert!(second.already_disbanded);
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 0),
        "disband should not touch backend session controls"
    );
}

#[test]
fn add_member_then_get_status() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert_eq!(status.config.members.len(), 1);
    assert_eq!(status.config.members[0].name, member_name);
    assert_eq!(status.members_runtime.len(), 1);
    assert_eq!(status.members_runtime[0].0, member_name);
    assert_eq!(status.members_runtime[0].1.health, HealthState::SessionDead);
}

#[test]
fn add_duplicate_member_returns_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member = sample_member("codex-reviewer", CliTool::Codex);

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, member.clone())
        .expect("first add should succeed");
    let err = orchestrator
        .add_member(team_name, member)
        .expect_err("duplicate add should fail");
    assert_conflict(err);
}

#[test]
fn remove_member_cleans_runtime() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert!(status.config.members.is_empty());
    assert!(status.members_runtime.is_empty());
}

#[test]
fn remove_nonexistent_member_returns_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let err = orchestrator
        .remove_member(team_name, "missing-member", None)
        .expect_err("expected not_found");
    assert_not_found(err);
}

#[test]
fn audit_log_captures_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    orchestrator
        .remove_member(team_name, member_name, None)
        .expect("remove should succeed");

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec!["team_created", "member_added", "member_removed"]
    );
}

#[test]
fn deliver_operator_notice_succeeds() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let result = orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
        }))
        .expect("delivery should succeed");
    assert!(result.delivered);

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_succeeded"
        ]
    );
}

#[test]
fn deliver_to_nonexistent_member_fails() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");

    let err = orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: "missing-member".to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
        }))
        .expect_err("delivery should fail");
    assert_not_found(err);
}

#[test]
fn deliver_updates_runtime_last_seen() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let before = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
        .expect("runtime should exist before delivery");
    assert!(before.last_seen_at.is_none());

    orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
        }))
        .expect("delivery should succeed");

    let after = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
        .expect("runtime should exist after delivery");
    assert!(after.last_seen_at.is_some());
}

#[test]
fn deliver_backend_failure_emits_failed_event() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated delivery failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let err = orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
        }))
        .expect_err("delivery should fail");
    match err {
        CoordinationError::Backend(msg) => assert!(msg.contains("simulated")),
        other => panic!("expected backend error, got {other:?}"),
    }

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_failed"
        ]
    );
}

#[test]
fn full_lifecycle() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, Some("lifecycle".to_string()))
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member("member-a", CliTool::Codex))
        .expect("add a should succeed");
    orchestrator
        .add_member(team_name, sample_member("member-b", CliTool::Claude))
        .expect("add b should succeed");
    orchestrator
        .remove_member(team_name, "member-a", Some("done".to_string()))
        .expect("remove should succeed");
    orchestrator
        .disband_team(team_name, Some("shutdown".to_string()))
        .expect("disband should succeed");

    let events = orchestrator.drain_audit_log();
    let event_types: Vec<&str> = events.iter().map(|event| event.event_type()).collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "member_added",
            "member_removed",
            "team_disbanded"
        ]
    );
}

#[test]
fn flush_audit_to_log_clears_buffer() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", Some("desc".to_string()))
        .expect("create should succeed");
    assert!(
        !orchestrator.drain_audit_log().is_empty(),
        "sanity: event should exist"
    );

    orchestrator
        .create_team("second-team", Some("desc".to_string()))
        .expect("create should succeed");
    orchestrator.flush_audit_to_log();
    assert!(
        orchestrator.drain_audit_log().is_empty(),
        "flush should clear buffered events"
    );
}

#[test]
fn lease_claimed_emits_event() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator.record_lease_claimed("architecture-final", "codex-reviewer", 4242, "inst-1");

    let events = orchestrator.drain_audit_log();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AuditEvent::LeaseClaimed(payload) => {
            assert_eq!(payload.team_name, "architecture-final");
            assert_eq!(payload.member_name, "codex-reviewer");
            assert_eq!(payload.owner_pid, 4242);
            assert_eq!(payload.instance_uuid, "inst-1");
        }
        other => panic!("expected lease_claimed event, got {other:?}"),
    }
}

#[test]
fn lease_reclaimed_emits_event() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator.record_lease_reclaimed("architecture-final", "codex-reviewer", 1111, 2222);

    let events = orchestrator.drain_audit_log();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AuditEvent::LeaseReclaimed(payload) => {
            assert_eq!(payload.team_name, "architecture-final");
            assert_eq!(payload.member_name, "codex-reviewer");
            assert_eq!(payload.previous_pid, 1111);
            assert_eq!(payload.new_pid, 2222);
        }
        other => panic!("expected lease_reclaimed event, got {other:?}"),
    }
}

#[test]
fn all_mutations_emit_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, Some("audit coverage".to_string()))
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "check status".to_string(),
        }))
        .expect("delivery should succeed");
    orchestrator.record_lease_claimed(team_name, member_name, 4242, "inst-1");
    orchestrator.record_lease_reclaimed(team_name, member_name, 4242, 5252);
    orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");
    orchestrator
        .disband_team(team_name, Some("shutdown".to_string()))
        .expect("disband should succeed");

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_succeeded",
            "lease_claimed",
            "lease_reclaimed",
            "member_removed",
            "team_disbanded"
        ]
    );
}

#[test]
fn invalid_team_name_is_rejected_for_create() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let err = orchestrator
        .create_team("bad/name", None)
        .expect_err("path separators must be rejected");
    match err {
        CoordinationError::Validation(message) => assert!(message.contains("must not contain")),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn invalid_member_name_is_rejected_for_add_member() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");

    let err = orchestrator
        .add_member(team_name, sample_member("bad/member", CliTool::Codex))
        .expect_err("invalid member name should fail");
    match err {
        CoordinationError::Validation(message) => assert!(message.contains("path separators")),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn deliver_to_nonexistent_team_fails_without_delivery_audit_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let err = orchestrator
        .deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: "codex-reviewer".to_string(),
            team_name: "missing-team".to_string(),
            message: "status?".to_string(),
        }))
        .expect_err("delivery should fail");
    assert_not_found(err);

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert!(
        event_types.is_empty(),
        "no delivery audit event should be emitted before team lookup succeeds"
    );
}

#[test]
fn initialize_team_full_success_path() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-init");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.team_name, "architecture-final-init");
    assert!(report.failed_step.is_none());
    assert!(!report.retryable);
    assert_eq!(
        report.succeeded_steps,
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
            "launch_sessions",
            "join_mesh",
            "start_daemons",
            "send_onboarding",
        ]
    );
}

#[test]
fn initialize_team_duplicate_team_returns_partial_failure_report() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-init");

    orchestrator
        .create_team("architecture-final-init", None)
        .expect("seed team");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("create_team"));
    assert!(report.retryable);
    assert_eq!(report.succeeded_steps, vec!["validate_configuration"]);
    assert_eq!(report.steps[0].step, "validate_configuration");
    assert_eq!(report.steps[1].step, "create_team");
    assert_eq!(report.steps[1].status, StepStatus::Failed);
}

#[test]
fn initialize_team_agent_addition_failure_is_partial() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let mut request = initialize_request("architecture-final-init");
    request.agents[1].name = "bad/member".to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("create_panes"));
    assert!(report.retryable);
    assert_eq!(
        report.succeeded_steps,
        vec!["validate_configuration", "create_team", "add_lead"]
    );
    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| step.step.as_str())
            .collect::<Vec<_>>(),
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
        ]
    );
}

#[test]
fn initialize_team_steps_are_ordered() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-order");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
    assert_eq!(
        step_names,
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
            "launch_sessions",
            "join_mesh",
            "start_daemons",
            "send_onboarding",
        ]
    );
}

#[test]
fn add_agent_to_team_full_success() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());
    assert!(!report.retryable);
    assert_eq!(report.member_name, "new-agent");
    assert_eq!(
        report.succeeded_steps,
        vec![
            "validate",
            "create_pane",
            "launch_session",
            "join_mesh",
            "start_daemon",
            "send_onboarding",
            "update_roster",
        ]
    );

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == "new-agent"));
}

#[test]
fn add_agent_duplicate_name_rejected() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "existing-dev", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("validate"));
    assert!(report.retryable);
    assert!(report.succeeded_steps.is_empty());
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].status, StepStatus::Failed);
}

#[test]
fn add_agent_team_not_found_fails_before_pipeline_progress() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = add_agent_request("missing-team", "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("validate"));
    assert!(report.succeeded_steps.is_empty());
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].step, "validate");
}

#[test]
fn add_agent_onboarding_stub_ignores_backend_delivery_error() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake;
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let before = orchestrator
        .get_team_status(team_name)
        .expect("status before")
        .config
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());
    assert!(!report.retryable);

    let after = orchestrator
        .get_team_status(team_name)
        .expect("status after")
        .config
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(after.len(), before.len() + 1);
    assert!(after.contains(&"new-agent".to_string()));
}

#[test]
fn add_agent_step_ordering_is_stable() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final-hot-add-order";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
    assert_eq!(
        step_names,
        vec![
            "validate",
            "create_pane",
            "launch_session",
            "join_mesh",
            "start_daemon",
            "send_onboarding",
            "update_roster",
        ]
    );
}
