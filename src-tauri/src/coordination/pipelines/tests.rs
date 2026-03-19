use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use crate::coordination::backend::fake::FakeBackend;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    AddAgentRequest, AgentSetupConfig, DeliveryRequest, InitializeTeamRequest, LeadMode,
    ResumeMemberRequest, StepStatus,
};
use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::templates::types::BehavioralContract;

fn member(name: &str, role: MemberRole, cli_tool: CliTool, project: &str) -> Member {
    Member {
        name: name.to_string(),
        role,
        role_id: None,
        role_name: None,
        focus_area: None,
        context_summary: None,
        behavior_summary: None,
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
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
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
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
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
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
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
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
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
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
                runtime_compact_summary: None,
                instructions: Some("Implement safely".to_string()),
                behavioral_contract: Some(BehavioralContract {
                    communication: vec!["post updates".to_string()],
                    execution: vec!["ship patches".to_string()],
                    escalation: vec!["raise blockers".to_string()],
                }),
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
                runtime_compact_summary: None,
                instructions: Some("Investigate tradeoffs and summarize findings.".to_string()),
                behavioral_contract: Some(BehavioralContract {
                    communication: vec!["post concise updates".to_string()],
                    execution: vec!["run experiments".to_string()],
                    escalation: vec!["escalate blockers immediately".to_string()],
                }),
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
