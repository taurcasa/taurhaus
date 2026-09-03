//! Linux E2E test for onboarding launch flow with real system runtime calls.

#![cfg(all(feature = "mesh-bridged-backend", target_os = "linux"))]
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;

#[path = "support/coordination_shims.rs"]
mod coordination_shims;
pub use coordination_shims::provider;
pub use coordination_shims::{
    daemon, errors, models, session_scanner, templates, tmux_layout, workflow_runs,
};

#[path = "../src/coordination/mod.rs"]
mod coordination;

use coordination::backend::MeshBridgedBackend;
use coordination::orchestrator::CoordinationOrchestrator;
use coordination::requests::{AgentSetupConfig, InitializeTeamRequest, LeadMode};
use coordination::runtime::SystemCoordinationRuntime;
use coordination::stores::{MemberRuntimeStore, MeshInboxStore, OPERATOR_SENDER_NAME};

const LOG_ENV: &str = "TAURHAUS_ONBOARDING_E2E_LOG";
const COUNTER_ENV: &str = "TAURHAUS_ONBOARDING_E2E_COUNTER";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

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

fn compile_executable_c(path: &Path, source: &str) {
    let source_path = path.with_extension("c");
    fs::write(&source_path, source).expect("write c source");
    let output = Command::new("cc")
        .arg(&source_path)
        .arg("-O2")
        .arg("-o")
        .arg(path)
        .output()
        .expect("compile c helper");
    assert!(
        output.status.success(),
        "cc failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
            account_id: None,
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
                project_id: frontend_project.to_string(),
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
                account_id: None,
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
                project_id: reviewer_project.to_string(),
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
                account_id: None,
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

#[test]
fn onboarding_flow_launches_lead_and_agents_via_direct_tmux_commands() {
    // Keep this public re-export referenced so test-only module imports remain warning-free.
    let _ = std::any::TypeId::of::<coordination::backend::FakeBackend>();

    let tmp = TempDir::new().expect("tempdir");
    let fake_home = tmp.path().join("home");
    let fake_claude_dir = fake_home.join(".claude");
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
    fs::create_dir_all(&fake_claude_dir).expect("create fake claude dir");
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
if [[ "$cmd" == "list-windows" ]]; then
  if [[ -f "{session_marker}" ]]; then
    n="$(cat "{counter}")"
    pane_count="$((n + 1))"
    printf '0\tbash\t%s\n' "$pane_count"
  fi
  exit 0
fi
if [[ "$cmd" == "list-panes" ]]; then
  if [[ -f "{session_marker}" ]]; then
    n="$(cat "{counter}")"
    i=0
    while [[ "$i" -le "$n" ]]; do
      printf '%%%s\t%s\n' "$i" "$i"
      i="$((i + 1))"
    done
  fi
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

    let mesh_source = format!(
        r#"#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static void append_log(int argc, char **argv) {{
    FILE *log = fopen("{log}", "a");
    if (!log) {{
        return;
    }}
    fputs("mesh:", log);
    for (int i = 1; i < argc; i++) {{
        if (i > 1) {{
            fputc(' ', log);
        }}
        fputs(argv[i], log);
    }}
    fputc('\n', log);
    fclose(log);
}}

static void mkdir_p(const char *path) {{
    char buffer[PATH_MAX];
    size_t len = strlen(path);
    if (len >= sizeof(buffer)) {{
        abort();
    }}
    memcpy(buffer, path, len + 1);
    for (char *cursor = buffer + 1; *cursor; cursor++) {{
        if (*cursor == '/') {{
            *cursor = '\0';
            if (mkdir(buffer, 0777) != 0 && errno != EEXIST) {{
                abort();
            }}
            *cursor = '/';
        }}
    }}
    if (mkdir(buffer, 0777) != 0 && errno != EEXIST) {{
        abort();
    }}
}}

static const char *flag_value(int argc, char **argv, const char *flag) {{
    for (int i = 1; i + 1 < argc; i++) {{
        if (strcmp(argv[i], flag) == 0) {{
            return argv[i + 1];
        }}
    }}
    return "";
}}

int main(int argc, char **argv) {{
    append_log(argc, argv);
    if (argc > 1 && strcmp(argv[1], "daemon") == 0) {{
        const char *claude_dir = flag_value(argc, argv, "--claude-dir");
        const char *team = flag_value(argc, argv, "--team");
        const char *name = flag_value(argc, argv, "--name");
        char daemon_dir[PATH_MAX];
        char pid_path[PATH_MAX];
        snprintf(daemon_dir, sizeof(daemon_dir), "%s/teams/%s/daemons", claude_dir, team);
        snprintf(pid_path, sizeof(pid_path), "%s/%s.pid", daemon_dir, name);
        mkdir_p(daemon_dir);

        pid_t child = fork();
        if (child == 0) {{
            sleep(30);
            _exit(0);
        }}
        if (child < 0) {{
            return 1;
        }}

        FILE *pid_file = fopen(pid_path, "w");
        if (!pid_file) {{
            return 1;
        }}
        fprintf(pid_file, "%d\n", child);
        fclose(pid_file);
        return 0;
    }}
    return 0;
}}
"#,
        log = log_path.display()
    );
    compile_executable_c(&fake_mesh_bin.join("mesh"), &mesh_source);

    let original_path = std::env::var("PATH").unwrap_or_default();
    let path_with_fakes = if original_path.is_empty() {
        fake_bin.display().to_string()
    } else {
        format!("{}:{original_path}", fake_bin.display())
    };

    let _home_guard = EnvGuard::set("HOME", fake_home.display().to_string());
    let _claude_dir_guard = EnvGuard::set(
        CLAUDE_DIR_OVERRIDE_ENV,
        fake_claude_dir.display().to_string(),
    );
    let _path_guard = EnvGuard::set("PATH", path_with_fakes);
    let _log_guard = EnvGuard::set(LOG_ENV, log_path.display().to_string());
    let _counter_guard = EnvGuard::set(COUNTER_ENV, counter_path.display().to_string());

    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        teams_dir.clone(),
        Arc::new(MeshBridgedBackend::new_with_teams_dir(teams_dir.clone())),
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
    let frontend_runtime =
        MemberRuntimeStore::load(&teams_dir, "linux-onboarding-e2e", "frontend-dev")
            .expect("frontend runtime should exist");
    let reviewer_runtime = MemberRuntimeStore::load(&teams_dir, "linux-onboarding-e2e", "reviewer")
        .expect("reviewer runtime should exist");
    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let inbox = MeshInboxStore::load(&teams_dir, "linux-onboarding-e2e", member_name)
            .expect("onboarding inbox should exist beneath the orchestrator teams root");
        assert_eq!(inbox.len(), 1, "one onboarding record for {member_name}");
        assert_ne!(
            inbox[0].from, member_name,
            "operator onboarding must never self-send"
        );
        assert!(
            inbox[0].from == OPERATOR_SENDER_NAME || inbox[0].from == "team-lead",
            "onboarding sender should be the operator identity or configured lead"
        );
    }

    let log = fs::read_to_string(&log_path).expect("read call log");
    // tmux receives the rendered launch as a nested single-quoted shell word.
    // Decode that one escaping layer before asserting the LaunchSpec output.
    let rendered_log = log.replace("'\\''", "'");
    assert!(log.contains("tmux:has-session -t taurhaus"));
    assert!(log.contains("tmux:new-session -d -s taurhaus"));
    assert!(log.contains(
        "tmux:new-window -n proj-core -t taurhaus: -P -F #{pane_id} exec \"$SHELL\" -ic 'cd "
    ));
    assert!(log.contains(
        "tmux:new-window -n proj-web -t taurhaus: -P -F #{pane_id} exec \"$SHELL\" -ic 'cd "
    ));
    assert!(log.contains(
        "tmux:new-window -n proj-api -t taurhaus: -P -F #{pane_id} exec \"$SHELL\" -ic 'cd "
    ));
    assert!(log.contains(&project_core.display().to_string()));
    assert!(log.contains(&project_web.display().to_string()));
    assert!(log.contains(&project_api.display().to_string()));

    assert!(rendered_log.contains(
        "CLAUDECODE=1 CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 claude --dangerously-skip-permissions"
    ));
    // Regression: 791f6be deliberately quoted LaunchSpec values and removed
    // the invented gpt-5.3 alias, leaving this end-to-end guard stale.
    assert!(rendered_log.contains("--team-name 'linux-onboarding-e2e'"));
    assert!(rendered_log.contains("--agent-name 'team-lead'"));
    assert!(rendered_log.contains("--agent-id 'team-lead@linux-onboarding-e2e'"));
    assert!(rendered_log.contains("--agent-type 'orchestrator'"));
    assert!(rendered_log.contains("-n 'team-lead'"));
    assert!(rendered_log.contains("--model 'opus'"));
    assert!(rendered_log.contains("codex --yolo -m 'gpt-5.3'"));
    assert!(!rendered_log.contains("gpt-5.3-codex"));
    assert!(log.contains("agy"));
    assert!(
        !log.contains("tmux:send-keys"),
        "fresh launches should be attached to pane creation, not injected later"
    );

    assert!(log.contains("mesh:join --team linux-onboarding-e2e --name frontend-dev"));
    assert!(log.contains("mesh:join --team linux-onboarding-e2e --name reviewer"));
    assert!(log.contains("mesh:daemon --pane %2 --team linux-onboarding-e2e --name frontend-dev"));
    assert!(log.contains("mesh:daemon --pane %3 --team linux-onboarding-e2e --name reviewer"));

    for pid in [frontend_runtime.daemon_pid, reviewer_runtime.daemon_pid]
        .into_iter()
        .flatten()
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}
