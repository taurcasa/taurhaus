//! Integration tests for initialize/add-agent/disband workflows.

#![cfg(feature = "mesh-bridged-backend")]
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

mod errors {
    pub use taurhaus_lib::errors::*;
}

mod models {
    pub use taurhaus_lib::models::*;
}

mod session_scanner {
    pub mod cli_tool {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum CliTool {
            Claude,
            Codex,
            Gemini,
        }

        impl std::fmt::Display for CliTool {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    CliTool::Claude => write!(f, "claude"),
                    CliTool::Codex => write!(f, "codex"),
                    CliTool::Gemini => write!(f, "gemini"),
                }
            }
        }
    }

    pub mod control {
        use crate::daemon::protocol::LaunchMode;
        use crate::models::CliCommandSettings;

        use super::cli_tool::CliTool;

        pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
            let first_token = cmd.split_whitespace().next().unwrap_or("");
            let base_name = first_token.rsplit('/').next().unwrap_or(first_token);
            const ALLOWED_TOOLS: &[&str] = &["claude", "codex", "gemini"];
            if !ALLOWED_TOOLS.contains(&base_name) {
                return Err(format!(
                    "Command override must start with claude/codex/gemini, got: {base_name}"
                ));
            }
            Ok(())
        }

        pub fn resolve_configured_tool_command(
            cmds: &CliCommandSettings,
            tool: CliTool,
            mode: LaunchMode,
        ) -> String {
            let tool_cmds = match tool {
                CliTool::Claude => &cmds.claude,
                CliTool::Codex => &cmds.codex,
                CliTool::Gemini => &cmds.gemini,
            };
            match mode {
                LaunchMode::Continue => tool_cmds.continue_cmd.clone(),
                LaunchMode::Fresh => tool_cmds.fresh.clone(),
                LaunchMode::Resume => tool_cmds.resume.clone(),
            }
        }

        pub fn build_team_launch_command(
            cmds: &CliCommandSettings,
            tool: CliTool,
            model: &str,
        ) -> String {
            match tool {
                CliTool::Claude => cmds.claude.fresh.clone(),
                CliTool::Gemini => cmds.gemini.fresh.clone(),
                CliTool::Codex => {
                    let base = cmds.codex.fresh.clone();
                    let model = model.trim();
                    if model.is_empty() || base.contains("-m ") || base.contains("--model") {
                        return base;
                    }
                    let model = if model.eq_ignore_ascii_case("gpt-5.3") {
                        "gpt-5.3-codex".to_string()
                    } else {
                        model.to_string()
                    };
                    format!("{base} -m '{model}'")
                }
            }
        }
    }
}

mod daemon {
    pub mod protocol {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum LaunchMode {
            Continue,
            Fresh,
            Resume,
        }
    }
}

#[path = "../src/commands/coordination.rs"]
pub mod commands_coordination;
#[path = "../src/commands/coordination_types.rs"]
pub mod commands_coordination_types;

mod commands {
    pub use crate::commands_coordination as coordination;
    pub use crate::commands_coordination_types as coordination_types;

    pub mod projects {
        use std::sync::{Arc, Mutex};

        pub struct DbState(pub Arc<Mutex<rusqlite::Connection>>);
    }

    pub mod terminal_settings {
        use crate::models::CliCommandSettings;

        use super::projects::DbState;

        pub fn load_cli_commands(_db: &DbState) -> CliCommandSettings {
            CliCommandSettings::default()
        }
    }
}

#[path = "../src/coordination/mod.rs"]
mod coordination;

use commands::coordination::{AddAgentRequest, AgentSetupConfig, InitializeTeamRequest, LeadMode};
use coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
use coordination::runtime::{CoordinationRuntime, RecordingCoordinationRuntime};
use coordination::state::CoordinationState;
use coordination::stores::{MemberRuntimeStore, TeamConfigStore};

fn test_state(teams_dir: PathBuf) -> CoordinationState {
    CoordinationState::with_components_and_runtime(
        teams_dir,
        BackendSelector::m0(),
        Arc::new(|_kind| Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)),
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
        Arc::new(|_kind| Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)),
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
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.3".to_string(),
                project_id: "proj-web".to_string(),
                description: Some("ui".to_string()),
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: "proj-api".to_string(),
                description: None,
            },
        ],
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
    let report = commands::coordination::coordination_preflight_check(make_request("preflight"))
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
