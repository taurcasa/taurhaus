//! Linux E2E test for onboarding launch flow with real system runtime calls.

#![cfg(all(feature = "mesh-bridged-backend", target_os = "linux"))]
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use tempfile::TempDir;

#[path = "support/coordination_shims.rs"]
mod coordination_shims;
pub use coordination_shims::{daemon, errors, models, session_scanner, templates};

#[path = "../src/coordination/mod.rs"]
mod coordination;

use coordination::backend::MeshBridgedBackend;
use coordination::orchestrator::CoordinationOrchestrator;
use coordination::requests::{AgentSetupConfig, InitializeTeamRequest, LeadMode};
use coordination::runtime::SystemCoordinationRuntime;
use coordination::stores::MemberRuntimeStore;

const LOG_ENV: &str = "TAURHAUS_ONBOARDING_E2E_LOG";
const COUNTER_ENV: &str = "TAURHAUS_ONBOARDING_E2E_COUNTER";

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value.into());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_deref() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn write_executable_script(path: &Path, body: &str) {
    fs::write(path, body).expect("write script");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod +x");
}

fn make_request(
    team_name: &str,
    lead_project: &str,
    frontend_project: &str,
    reviewer_project: &str,
) -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: team_name.to_string(),
        team_description: Some("linux onboarding e2e".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            project_id: lead_project.to_string(),
            description: Some("lead".to_string()),
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
                project_id: frontend_project.to_string(),
                description: Some("ui".to_string()),
                role_id: None,
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "gemini".to_string(),
                model: "pro".to_string(),
                project_id: reviewer_project.to_string(),
                description: None,
                role_id: None,
                instructions: None,
                behavioral_contract: None,
                capabilities: None,
            },
        ],
    }
}

#[test]
fn onboarding_flow_launches_lead_and_injects_commands_with_enter() {
    // Keep this public re-export referenced so test-only module imports remain warning-free.
    let _ = std::any::TypeId::of::<coordination::backend::FakeBackend>();

    let tmp = TempDir::new().expect("tempdir");
    let fake_home = tmp.path().join("home");
    let fake_bin = tmp.path().join("bin");
    let fake_mesh_bin = fake_home.join(".local/bin");
    let teams_dir = tmp.path().join("teams");
    let project_core = tmp.path().join("proj-core");
    let project_web = tmp.path().join("proj-web");
    let project_api = tmp.path().join("proj-api");
    let log_path = tmp.path().join("calls.log");
    let counter_path = tmp.path().join("pane_counter.txt");
    let session_marker = tmp.path().join("tmux_session_created");

    fs::create_dir_all(&fake_bin).expect("create fake bin dir");
    fs::create_dir_all(&fake_mesh_bin).expect("create fake mesh bin dir");
    fs::create_dir_all(&teams_dir).expect("create teams dir");
    fs::create_dir_all(&project_core).expect("create lead project dir");
    fs::create_dir_all(&project_web).expect("create frontend project dir");
    fs::create_dir_all(&project_api).expect("create reviewer project dir");
    fs::write(&counter_path, "0\n").expect("seed counter");

    let tmux_script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
echo "tmux:$*" >> "{log}"
cmd="${{1:-}}"
if [[ "$cmd" == "has-session" ]]; then
  if [[ -f "{session_marker}" ]]; then
    exit 0
  fi
  exit 1
fi
if [[ "$cmd" == "new-session" ]]; then
  touch "{session_marker}"
  exit 0
fi
if [[ "$cmd" == "new-window" || "$cmd" == "split-window" ]]; then
  n="$(cat "{counter}")"
  n="$((n + 1))"
  echo "$n" > "{counter}"
  echo "%$n"
  exit 0
fi
exit 0
"#,
        log = log_path.display(),
        counter = counter_path.display(),
        session_marker = session_marker.display()
    );
    write_executable_script(&fake_bin.join("tmux"), &tmux_script);

    let mesh_script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
echo "mesh:$*" >> "{log}"
exit 0
"#,
        log = log_path.display()
    );
    write_executable_script(&fake_mesh_bin.join("mesh"), &mesh_script);

    let original_path = std::env::var("PATH").unwrap_or_default();
    let path_with_fakes = if original_path.is_empty() {
        fake_bin.display().to_string()
    } else {
        format!("{}:{original_path}", fake_bin.display())
    };

    let _home_guard = EnvGuard::set("HOME", fake_home.display().to_string());
    let _path_guard = EnvGuard::set("PATH", path_with_fakes);
    let _log_guard = EnvGuard::set(LOG_ENV, log_path.display().to_string());
    let _counter_guard = EnvGuard::set(COUNTER_ENV, counter_path.display().to_string());

    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        teams_dir.clone(),
        Arc::new(MeshBridgedBackend::default()),
        Arc::new(SystemCoordinationRuntime),
    );
    let request = make_request(
        "linux-onboarding-e2e",
        project_core.to_string_lossy().as_ref(),
        project_web.to_string_lossy().as_ref(),
        project_api.to_string_lossy().as_ref(),
    );

    let report = orchestrator
        .initialize_team(&request)
        .expect("initialize should succeed");
    assert!(
        report.failed_step.is_none(),
        "initialize unexpectedly failed: {report:?}"
    );

    let lead_runtime = MemberRuntimeStore::load(&teams_dir, "linux-onboarding-e2e", "team-lead")
        .expect("lead runtime should exist");
    assert_eq!(lead_runtime.pane_id.as_deref(), Some("%1"));
    assert!(lead_runtime.daemon_pid.is_none());

    let log = fs::read_to_string(&log_path).expect("read call log");
    assert!(log.contains("tmux:has-session -t taurhaus"));
    assert!(log.contains("tmux:new-session -d -s taurhaus"));
    assert!(log.contains(&format!(
        "tmux:new-window -n proj-core -t taurhaus: -P -F #{{pane_id}} -c {}",
        project_core.display()
    )));
    assert!(log.contains(&format!(
        "tmux:new-window -n proj-web -t taurhaus: -P -F #{{pane_id}} -c {}",
        project_web.display()
    )));
    assert!(log.contains(&format!(
        "tmux:new-window -n proj-api -t taurhaus: -P -F #{{pane_id}} -c {}",
        project_api.display()
    )));

    assert!(log.contains("tmux:send-keys -t %1 -l CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 claude --dangerously-skip-permissions"));
    assert!(log.contains("--team-name linux-onboarding-e2e"));
    assert!(log.contains("--agent-name team-lead"));
    assert!(log.contains("--agent-id team-lead@linux-onboarding-e2e"));
    assert!(log.contains("--agent-type orchestrator"));
    assert!(log.contains("tmux:send-keys -t %2 -l codex --yolo -m 'gpt-5.3-codex'"));
    assert!(log.contains("tmux:send-keys -t %3 -l gemini --yolo"));
    assert!(log.contains("tmux:send-keys -t %1 Enter"));
    assert!(log.contains("tmux:send-keys -t %2 Enter"));
    assert!(log.contains("tmux:send-keys -t %3 Enter"));

    assert!(log.contains("mesh:join --team linux-onboarding-e2e --name frontend-dev"));
    assert!(log.contains("mesh:join --team linux-onboarding-e2e --name reviewer"));
    assert!(log.contains("mesh:daemon --pane %2 --team linux-onboarding-e2e --name frontend-dev"));
    assert!(log.contains("mesh:daemon --pane %3 --team linux-onboarding-e2e --name reviewer"));
}
