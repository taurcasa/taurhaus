#![cfg(feature = "mesh-bridged-backend")]

use std::io::Write;
use std::process::{Command, Stdio};

use pretty_assertions::assert_eq;
use taurhaus_lib::coordination::delivery::{DeliveryRenderer, RoleContext};
use taurhaus_lib::coordination::domain::MemberRole;
use taurhaus_lib::daemon::protocol::LaunchMode;
use taurhaus_lib::session_scanner::cli_tool::CliTool;
use taurhaus_lib::session_scanner::launch::{LaunchSpec, ModelSpec, TeamContext};
use taurhaus_lib::templates::types::RoleTemplate;

fn run_renderer(flag: &str, request: &serde_json::Value) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_taurhaus"))
        .args([flag, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real taurhaus binary");
    child
        .stdin
        .as_mut()
        .expect("renderer stdin")
        .write_all(serde_json::to_string(request).unwrap().as_bytes())
        .expect("write renderer request");

    let output = child.wait_with_output().expect("renderer output");
    assert!(
        output.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 renderer output")
}

fn run_renderer_argument(flag: &str, request: &serde_json::Value) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_taurhaus"))
        .args([flag, &serde_json::to_string(request).unwrap()])
        .output()
        .expect("run real taurhaus binary");
    assert!(
        output.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 renderer output")
}

#[test]
fn launch_command_cli_matches_launch_spec_bytes() {
    // Regression: commit 7b852ed duplicated launch rendering in taureval, so its
    // hard-coded model and missing effort could drift from LaunchSpec.
    let request = serde_json::json!({
        "tool": "codex",
        "mode": "fresh",
        "base": "codex --yolo",
        "model": "gpt-5.4",
        "reasoningEffort": "high",
        "team": {
            "teamName": "taureval-golden",
            "agentName": "agent-under-test",
            "role": "agent"
        }
    });
    let expected = LaunchSpec {
        tool: CliTool::Codex,
        mode: LaunchMode::Fresh,
        base: "codex --yolo",
        model: ModelSpec {
            model: Some("gpt-5.4".to_string()),
            reasoning_effort: Some("high".to_string()),
        },
        team: Some(TeamContext {
            team_name: "taureval-golden",
            agent_name: "agent-under-test",
            role: MemberRole::Agent,
        }),
        codex_bypass_hook_trust: false,
    }
    .render()
    .command;

    assert_eq!(
        run_renderer("--launch-command", &request),
        format!("{expected}\n")
    );
    assert_eq!(
        run_renderer_argument("--launch-command", &request),
        format!("{expected}\n")
    );
}

#[test]
fn render_onboarding_cli_matches_delivery_renderer_bytes() {
    // Regression: commit 7b852ed copied only part of DeliveryRenderer into
    // taureval, dropping workflow fields whenever the taurhaus role evolved.
    let role_yaml = include_str!("../resources/templates/roles/quick-dev-codex.yaml");
    let role: RoleTemplate = serde_norway::from_str(role_yaml).expect("bundled role parses");
    let request = serde_json::json!({
        "tool": "codex",
        "teamName": "taureval-golden",
        "memberName": "agent-under-test",
        "leadName": "evaluator",
        "role": role
    });
    let expected = DeliveryRenderer::render_onboarding(
        "taureval-golden",
        "agent-under-test",
        "evaluator",
        RoleContext::from(&role),
    );

    assert_eq!(
        run_renderer("--render-onboarding", &request),
        format!("{expected}\n")
    );
}
