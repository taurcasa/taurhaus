#![cfg(feature = "mesh-bridged-backend")]

use std::io::Write;
use std::process::{Command, Stdio};

use pretty_assertions::assert_eq;
use taurhaus_lib::coordination::delivery::{DeliveryRenderer, RoleContext};
use taurhaus_lib::coordination::domain::MemberRole;
use taurhaus_lib::daemon::protocol::LaunchMode;
use taurhaus_lib::models::CliCommandSettings;
use taurhaus_lib::session_scanner::cli_tool::CliTool;
use taurhaus_lib::session_scanner::launch::{base_command, LaunchSpec, ModelSpec, TeamContext};
use taurhaus_lib::templates::types::RoleTemplate;

fn run_renderer(flag: &str, request: &serde_json::Value) -> String {
    let temp = tempfile::tempdir().expect("renderer data dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_taurhaus"))
        .args([flag, "-"])
        .env("TAURHAUS_DATA_DIR", temp.path())
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
    let temp = tempfile::tempdir().expect("renderer data dir");
    let output = Command::new(env!("CARGO_BIN_EXE_taurhaus"))
        .args([flag, &serde_json::to_string(request).unwrap()])
        .env("TAURHAUS_DATA_DIR", temp.path())
        .output()
        .expect("run real taurhaus binary");
    assert!(
        output.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 renderer output")
}

fn run_renderer_error(flag: &str, request: &serde_json::Value) -> String {
    let temp = tempfile::tempdir().expect("renderer data dir");
    let output = Command::new(env!("CARGO_BIN_EXE_taurhaus"))
        .args([flag, &serde_json::to_string(request).unwrap()])
        .env("TAURHAUS_DATA_DIR", temp.path())
        .output()
        .expect("run real taurhaus binary");
    assert!(!output.status.success(), "renderer unexpectedly succeeded");
    String::from_utf8(output.stderr).expect("utf8 renderer error")
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
        codex_notify_executable: None,
    }
    .render()
    .command;

    for response in [
        run_renderer("--launch-command", &request),
        run_renderer_argument("--launch-command", &request),
    ] {
        let response: serde_json::Value =
            serde_json::from_str(response.trim()).expect("launch response is JSON");
        assert_eq!(response["command"], expected);
        assert_eq!(response["notes"][0]["event"], "launch.model.deprecated");
    }
}

#[test]
fn launch_command_cli_surfaces_ignored_effort_notes() {
    // Regression: commit bdcf8ea discarded LaunchSpec notes, so taureval could
    // record a requested effort even when the rendered command omitted it.
    let request = serde_json::json!({
        "tool": "codex",
        "mode": "fresh",
        "base": "codex --yolo",
        "model": "gpt-5.4",
        "reasoningEffort": "max"
    });
    let response: serde_json::Value =
        serde_json::from_str(run_renderer("--launch-command", &request).trim())
            .expect("launch response is JSON");

    assert_eq!(response["command"], "codex --yolo -m 'gpt-5.4'");
    assert_eq!(response["notes"][0]["event"], "launch.model.deprecated");
    assert_eq!(response["notes"][1]["event"], "launch.effort.invalid");
    assert_eq!(response["notes"][1]["found"], "max");
    assert_eq!(response["notes"][1]["reason"], "invalid");
}

#[test]
fn launch_command_cli_uses_default_base_and_snake_case_claude_team() {
    let request = serde_json::json!({
        "tool": "claude",
        "mode": "fresh",
        "model": "opus",
        "reasoning_effort": "high",
        "team": {
            "team_name": "taureval-golden",
            "agent_name": "agent-under-test",
            "role": "agent"
        }
    });
    let defaults = CliCommandSettings::default();
    let expected = LaunchSpec {
        tool: CliTool::Claude,
        mode: LaunchMode::Fresh,
        base: base_command(&defaults, CliTool::Claude, LaunchMode::Fresh),
        model: ModelSpec {
            model: Some("opus".to_string()),
            reasoning_effort: Some("high".to_string()),
        },
        team: Some(TeamContext {
            team_name: "taureval-golden",
            agent_name: "agent-under-test",
            role: MemberRole::Agent,
        }),
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
    }
    .render()
    .command;
    let response: serde_json::Value =
        serde_json::from_str(run_renderer("--launch-command", &request).trim())
            .expect("launch response is JSON");

    assert_eq!(response["command"], expected);
    assert_eq!(response["notes"], serde_json::json!([]));
}

#[test]
fn launch_command_cli_surfaces_cross_tool_model_replacement() {
    let request = serde_json::json!({
        "tool": "codex",
        "mode": "fresh",
        "model": "opus"
    });
    let response: serde_json::Value =
        serde_json::from_str(run_renderer("--launch-command", &request).trim())
            .expect("launch response is JSON");

    assert_eq!(response["notes"][0]["event"], "launch.model.invalid");
    assert_eq!(response["notes"][0]["found"], "opus");
    assert_eq!(response["notes"][0]["replacement"], "gpt-5.6-sol");
}

#[test]
fn renderer_cli_rejects_unknown_fields_and_multiline_commands() {
    assert!(run_renderer_error(
        "--launch-command",
        &serde_json::json!({
            "tool": "codex",
            "mode": "fresh",
            "model": "gpt-5.6-sol",
            "effort": "high"
        }),
    )
    .contains("unknown field `effort`"));

    assert!(run_renderer_error(
        "--launch-command",
        &serde_json::json!({
            "tool": "codex",
            "mode": "fresh",
            "base": "codex --yolo\necho unexpected",
            "model": "gpt-5.6-sol"
        }),
    )
    .contains("Command override must be a single line"));
}

#[test]
fn render_onboarding_cli_matches_delivery_renderer_bytes() {
    // Regression: commit 7b852ed copied only part of DeliveryRenderer into
    // taureval, dropping workflow fields whenever the taurhaus role evolved.
    let role_yaml = include_str!("../resources/templates/roles/quick-dev-codex.yaml");
    let role: RoleTemplate = serde_norway::from_str(role_yaml).expect("bundled role parses");
    let role_wire: serde_norway::Value =
        serde_norway::from_str(role_yaml).expect("bundled role parses as wire value");

    for (tool, expected, golden) in [
        (
            "codex",
            DeliveryRenderer::render_onboarding(
                "taureval-golden",
                "agent-under-test",
                "evaluator",
                RoleContext::from(&role),
            ),
            include_str!("quick-dev-codex-onboarding.golden.txt"),
        ),
        (
            "claude",
            DeliveryRenderer::render_claude_role_context(
                "taureval-golden",
                "agent-under-test",
                "evaluator",
                RoleContext::from(&role),
            ),
            include_str!("quick-dev-claude-onboarding.golden.txt"),
        ),
    ] {
        let request = serde_json::json!({
            "tool": tool,
            "team_name": "taureval-golden",
            "member_name": "agent-under-test",
            "lead_name": "evaluator",
            "role": role_wire.clone()
        });
        let actual = run_renderer("--render-onboarding", &request);

        assert_eq!(actual, format!("{expected}\n"));
        assert_eq!(actual, golden);
    }
}
