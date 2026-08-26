//! Integration tests for initialize/add-agent/disband workflows.

#![cfg(feature = "mesh-bridged-backend")]
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

#[path = "support/coordination_shims.rs"]
mod coordination_shims;
pub use coordination_shims::provider;
pub use coordination_shims::{daemon, errors, models, session_scanner, templates, tmux_layout};

pub mod startup {
    pub mod compaction {
        pub fn reconcile_compaction_runtime<R: tauri::Runtime>(
            _app: &tauri::AppHandle<R>,
            _mode: crate::models::CodexCompactionMode,
            _reason: &str,
        ) -> Result<(), crate::coordination::errors::CoordinationError> {
            Ok(())
        }
    }
}

pub mod terminal {
    use std::sync::{Mutex, OnceLock};

    pub use taurhaus_lib::terminal::TerminalIntent;

    fn recorded_terminal_intents() -> &'static Mutex<Vec<TerminalIntent>> {
        static RECORDED_TERMINAL_INTENTS: OnceLock<Mutex<Vec<TerminalIntent>>> = OnceLock::new();
        RECORDED_TERMINAL_INTENTS.get_or_init(|| Mutex::new(Vec::new()))
    }

    pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
        recorded_terminal_intents()
            .lock()
            .expect("terminal intent recorder lock")
            .push(intent.clone());
        taurhaus_lib::terminal::handle_terminal(intent)
    }

    pub fn take_recorded_terminal_intents() -> Vec<TerminalIntent> {
        std::mem::take(
            &mut *recorded_terminal_intents()
                .lock()
                .expect("terminal intent recorder lock"),
        )
    }
}

#[path = "../src/commands/coordination.rs"]
pub mod commands_coordination;
#[path = "../src/commands/coordination_types.rs"]
pub mod commands_coordination_types;

mod commands {
    pub use crate::commands_coordination as coordination;
    pub use crate::commands_coordination_types as coordination_types;

    pub mod runtime_snapshot {
        use taurhaus_lib::daemon_api::protocol;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum RuntimeSnapshotFreshness {
            Fresh,
            Cached,
            Unavailable,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct RuntimeSnapshotOutcome {
            pub snapshot: Option<taurhaus_lib::daemon_api::protocol::RuntimeSessionSnapshotResult>,
            pub freshness: RuntimeSnapshotFreshness,
        }

        pub fn daemon_runtime_session_snapshot(
            provider: &taurhaus_lib::ProviderState,
        ) -> Result<RuntimeSnapshotOutcome, String> {
            let Some(ref daemon) = provider.daemon else {
                return Ok(RuntimeSnapshotOutcome {
                    snapshot: None,
                    freshness: RuntimeSnapshotFreshness::Unavailable,
                });
            };

            if !daemon.is_connected() && !daemon.try_reconnect() {
                return Ok(RuntimeSnapshotOutcome {
                    snapshot: None,
                    freshness: RuntimeSnapshotFreshness::Unavailable,
                });
            }

            match request_daemon_runtime_session_snapshot(daemon) {
                Ok(Some(snapshot)) => Ok(RuntimeSnapshotOutcome {
                    snapshot: Some(snapshot),
                    freshness: RuntimeSnapshotFreshness::Fresh,
                }),
                Ok(None) => Ok(RuntimeSnapshotOutcome {
                    snapshot: None,
                    freshness: RuntimeSnapshotFreshness::Unavailable,
                }),
                Err(error) => {
                    if taurhaus_lib::daemon_api::is_busy_transport_error(&error) {
                        return Ok(RuntimeSnapshotOutcome {
                            snapshot: None,
                            freshness: RuntimeSnapshotFreshness::Unavailable,
                        });
                    }

                    if daemon.try_reconnect() {
                        match request_daemon_runtime_session_snapshot(daemon) {
                            Ok(Some(snapshot)) => Ok(RuntimeSnapshotOutcome {
                                snapshot: Some(snapshot),
                                freshness: RuntimeSnapshotFreshness::Fresh,
                            }),
                            Ok(None) | Err(_) => Ok(RuntimeSnapshotOutcome {
                                snapshot: None,
                                freshness: RuntimeSnapshotFreshness::Unavailable,
                            }),
                        }
                    } else {
                        Ok(RuntimeSnapshotOutcome {
                            snapshot: None,
                            freshness: RuntimeSnapshotFreshness::Unavailable,
                        })
                    }
                }
            }
        }

        pub fn decode_daemon_runtime_session_snapshot(
            payload: Option<serde_json::Value>,
        ) -> Result<taurhaus_lib::daemon_api::protocol::RuntimeSessionSnapshotResult, String>
        {
            match payload {
                Some(value) => serde_json::from_value(value)
                    .map_err(|error| format!("Runtime session snapshot decode error: {error}")),
                None => Ok(
                    taurhaus_lib::daemon_api::protocol::RuntimeSessionSnapshotResult {
                        version: 0,
                        display_sessions: Vec::new(),
                        runtime_sessions: Vec::new(),
                        focus: None,
                        foreground_project_path: None,
                        degraded: false,
                    },
                ),
            }
        }

        fn request_daemon_runtime_session_snapshot(
            daemon: &taurhaus_lib::provider::daemon_client::DaemonProvider,
        ) -> Result<Option<protocol::RuntimeSessionSnapshotResult>, String> {
            let request = protocol::DaemonRequest::new(
                "runtime-session-snapshot",
                protocol::method::GET_RUNTIME_SESSION_SNAPSHOT,
                serde_json::Value::Null,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    decode_daemon_runtime_session_snapshot(response.result).map(Some)
                }
                Ok(_) => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        }
    }

    pub mod projects {
        use std::sync::{Arc, Mutex};

        pub struct DbState(pub Arc<Mutex<rusqlite::Connection>>);
    }

    pub mod terminal_settings {
        use crate::coordination::errors::CoordinationError;
        use crate::models::{CliCommandSettings, CodexCompactionMode, TerminalSettings};

        use super::projects::DbState;

        pub fn load_cli_commands(_db: &DbState) -> CliCommandSettings {
            CliCommandSettings::default()
        }

        pub fn load_terminal_settings(_db: &DbState) -> TerminalSettings {
            TerminalSettings::default()
        }

        pub fn reconcile_codex_compaction(
            _mode: CodexCompactionMode,
            _has_managed_codex: bool,
        ) -> Result<bool, CoordinationError> {
            Ok(false)
        }
    }

    pub mod lifecycle {
        pub struct IpcCommandSpan;

        impl IpcCommandSpan {
            pub fn start(_command: &'static str) -> Self {
                Self
            }

            pub fn finish_result<T, E: std::fmt::Display>(&self, _result: &Result<T, E>) {}
        }
    }
}

#[path = "../src/coordination/mod.rs"]
mod coordination;

use coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};

#[test]
fn daemon_compact_hook_stdout_is_json_when_a_team_config_is_unparseable() {
    // Regression: 6fe0aa3 installed a stdout tracing subscriber in daemon hook mode,
    // so one malformed team config prefixed the JSON hook response with a WARN line.
    use std::io::Write;
    use std::process::{Command, Stdio};

    let tmp = tempfile::tempdir().expect("tempdir");
    let claude_dir = tmp.path().join("claude");
    let broken_team_dir = claude_dir.join("teams").join("broken-team");
    std::fs::create_dir_all(&broken_team_dir).expect("create broken team dir");
    std::fs::write(broken_team_dir.join("config.json"), "{not-json")
        .expect("write malformed team config");

    let mut child = Command::new(env!("CARGO_BIN_EXE_taurhaus-daemon"))
        .arg("--compact-hook")
        .env("TAURHAUS_CLAUDE_DIR", &claude_dir)
        .env("TAURHAUS_DATA_DIR", tmp.path().join("data"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon compact hook mode");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(
            br#"{"hook_event_name":"SessionStart","session_id":"x","source":"compact","cwd":"/tmp","transcript_path":"/tmp/.codex/sessions/rollout-a.jsonl"}"#,
        )
        .expect("write hook payload");

    let output = child.wait_with_output().expect("wait for compact hook");
    assert!(
        output.status.success(),
        "hook mode should exit successfully"
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "daemon compact hook stdout must be exactly one JSON response: {error}; stdout={:?}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
}
use coordination::requests::{AddAgentRequest, AgentSetupConfig, InitializeTeamRequest, LeadMode};
use coordination::runtime::{CoordinationRuntime, RecordingCoordinationRuntime};
use coordination::state::CoordinationState;
use coordination::stores::{MemberRuntimeStore, TeamConfigStore};

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
) -> (CoordinationState, Arc<RecordingCoordinationRuntime>) {
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let runtime_for_factory = runtime.clone();
    let state = CoordinationState::with_components_and_runtime(
        teams_dir,
        BackendSelector::m0(),
        Arc::new(|_kind, _teams_dir| {
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
    );
    (state, runtime)
}

fn make_request(team_name: &str) -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: team_name.to_string(),
        team_description: Some("integration".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: "proj-core".to_string(),
            description: Some("lead".to_string()),
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
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-web".to_string(),
                description: Some("ui".to_string()),
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
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: "proj-api".to_string(),
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
            },
        ],
    }
}

fn make_ipc_request(team_name: &str) -> commands::coordination::InitializeTeamRequest {
    let request = make_request(team_name);
    commands::coordination::InitializeTeamRequest {
        team_name: request.team_name,
        team_description: request.team_description,
        preset_id: None,
        lead_mode: request.lead_mode,
        lead: commands::coordination::AgentSetupConfig {
            name: request.lead.name,
            cli_tool: request.lead.cli_tool,
            model: request.lead.model,
            project_id: request.lead.project_id,
            description: request.lead.description,
            role_id: request.lead.role_id,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            instructions: request.lead.instructions,
            behavioral_contract: request.lead.behavioral_contract,
            quality_gates: None,
            reasoning_effort: request.lead.reasoning_effort,
            handoff_expectations: request.lead.handoff_expectations,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: request.lead.capabilities,
        },
        agents: request
            .agents
            .into_iter()
            .map(|agent| commands::coordination::AgentSetupConfig {
                name: agent.name,
                cli_tool: agent.cli_tool,
                model: agent.model,
                project_id: agent.project_id,
                description: agent.description,
                role_id: agent.role_id,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: agent.instructions,
                behavioral_contract: agent.behavioral_contract,
                quality_gates: None,
                reasoning_effort: agent.reasoning_effort,
                handoff_expectations: agent.handoff_expectations,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: agent.capabilities,
            })
            .collect(),
    }
}

fn make_add_request(team_name: &str, member_name: &str) -> AddAgentRequest {
    AddAgentRequest {
        team_name: team_name.to_string(),
        agent: AgentSetupConfig {
            name: member_name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.3".to_string(),
            project_id: "proj-ops".to_string(),
            description: Some("hot-add".to_string()),
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
    }
}

#[test]
fn initialize_team_end_to_end() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let team_name = "integration-init";
    let request = make_request(team_name);

    let report = state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&request))
        .expect("initialize should succeed");
    assert_eq!(report.team_name, team_name);
    assert!(report.failed_step.is_none());

    let status = state
        .with_orchestrator(|orchestrator| orchestrator.get_team_status(team_name))
        .expect("status should exist");
    assert_eq!(status.config.name, team_name);
    assert_eq!(status.config.members.len(), 3);
    assert_eq!(status.members_runtime.len(), 3);

    let config = TeamConfigStore::load(tmp.path(), team_name).expect("config persisted");
    assert_eq!(config.members.len(), 3);
    let runtime_members = MemberRuntimeStore::list(tmp.path(), team_name).expect("runtime list");
    assert_eq!(runtime_members.len(), 3);
}

#[test]
fn hot_add_agent_to_running_team() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let team_name = "integration-hot-add";

    let init = make_request(team_name);
    state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&init))
        .expect("initialize should succeed");

    let add = make_add_request(team_name, "qa-dev");
    let add_report = state
        .with_orchestrator(|orchestrator| orchestrator.add_agent_to_team(&add))
        .expect("hot-add should succeed");
    assert_eq!(add_report.member_name, "qa-dev");
    assert!(add_report.failed_step.is_none());

    let status = state
        .with_orchestrator(|orchestrator| orchestrator.get_team_status(team_name))
        .expect("status should exist");
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == "qa-dev"));
}

#[test]
fn disband_end_to_end_and_reopen() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let team_name = "integration-disband";

    let init = make_request(team_name);
    state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&init))
        .expect("initialize should succeed");
    assert!(tmp.path().join(team_name).exists());

    let first = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(team_name, None))
        .expect("disband should succeed");
    assert!(first.disbanded);
    assert!(!first.already_disbanded);
    assert!(!tmp.path().join(team_name).exists());

    let discovery = state
        .with_orchestrator(|orchestrator| orchestrator.discover_teams())
        .expect("discover should succeed");
    assert!(discovery.teams.is_empty());

    let reopen = make_request(team_name);
    let reopen_report = state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&reopen))
        .expect("reopen initialize should succeed");
    assert!(reopen_report.failed_step.is_none());
}

#[test]
fn disband_is_idempotent() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let team_name = "integration-idempotent";

    let init = make_request(team_name);
    state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&init))
        .expect("initialize should succeed");

    let first = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(team_name, None))
        .expect("first disband should succeed");
    let second = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(team_name, None))
        .expect("second disband should succeed");
    assert!(first.disbanded);
    assert!(!first.already_disbanded);
    assert!(!second.disbanded);
    assert!(second.already_disbanded);
}

#[test]
fn initialize_with_duplicate_team_name_fails_partially() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let team_name = "integration-duplicate";

    let first = make_request(team_name);
    let first_report = state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&first))
        .expect("first initialize should succeed");
    assert!(first_report.failed_step.is_none());

    let second = make_request(team_name);
    let second_report = state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&second))
        .expect("second initialize should return structured failure");
    assert_eq!(second_report.failed_step.as_deref(), Some("create_team"));
    assert!(second_report.retryable);
    assert_eq!(
        second_report.succeeded_steps,
        vec!["validate_configuration".to_string()]
    );
}

#[test]
fn preflight_check_with_real_lookup_returns_stable_shape() {
    let report =
        commands::coordination::coordination_preflight_check(make_ipc_request("preflight"))
            .expect("preflight should succeed");
    assert_eq!(report.can_initialize, report.blocking_errors.is_empty());
    for err in &report.blocking_errors {
        assert!(!err.trim().is_empty());
    }
    for warning in &report.agent_warnings {
        assert!(!warning.agent_name.trim().is_empty());
        assert!(!warning.cli_tool.trim().is_empty());
        assert!(!warning.message.trim().is_empty());
    }
}

#[test]
fn live_status_reconciles_member_to_offline_when_pane_disappears() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, runtime) = test_state_with_runtime(tmp.path().to_path_buf());
    let team_name = "integration-live-status-reconcile";

    let init = make_request(team_name);
    state
        .with_orchestrator(|orchestrator| orchestrator.initialize_team(&init))
        .expect("initialize should succeed");

    let frontend_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, "frontend-dev").expect("frontend runtime");
    let pane_id = frontend_runtime
        .pane_id
        .clone()
        .expect("frontend-dev should have pane id after initialize");
    runtime.set_pane_exists(&pane_id, false);

    let status = commands::coordination::coordination_get_live_team_status_for_tests(
        &state,
        team_name.to_string(),
    )
    .expect("live status should succeed");

    let frontend_row = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev row should exist");
    assert_eq!(
        frontend_row.session_status,
        commands::coordination::SessionStatus::Offline
    );

    let reconciled =
        MemberRuntimeStore::load(tmp.path(), team_name, "frontend-dev").expect("frontend runtime");
    assert_eq!(
        reconciled.health,
        coordination::domain::HealthState::SessionDead
    );
}
