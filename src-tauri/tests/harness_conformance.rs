use pretty_assertions::assert_eq;
use taurhaus_lib::coordination::domain::MemberRole;
use taurhaus_lib::daemon::protocol::LaunchMode;
use taurhaus_lib::models::CliCommandSettings;
use taurhaus_lib::session_scanner::cli_tool::CliTool;
use taurhaus_lib::session_scanner::launch::{base_command, LaunchSpec, ModelSpec, TeamContext};
use taurhaus_lib::session_scanner::process::detect_cli_tool;

struct LaunchGolden {
    tool: CliTool,
    model: &'static str,
    effort: Option<&'static str>,
    bypass_hook_trust: bool,
    expected: &'static str,
}

const LAUNCH_GOLDENS: &[LaunchGolden] = &[
    LaunchGolden {
        tool: CliTool::Claude,
        model: "opus",
        effort: Some("high"),
        bypass_hook_trust: false,
        expected: include_str!("fixtures/launch/claude.golden.txt"),
    },
    LaunchGolden {
        tool: CliTool::Codex,
        model: "gpt-5.6-sol",
        effort: Some("high"),
        bypass_hook_trust: true,
        expected: include_str!("fixtures/launch/codex.golden.txt"),
    },
    LaunchGolden {
        tool: CliTool::Gemini,
        model: "gemini-3.1-pro",
        effort: None,
        bypass_hook_trust: false,
        expected: include_str!("fixtures/launch/gemini.golden.txt"),
    },
];

#[test]
fn default_fresh_commands_are_detected_for_every_harness() {
    // Regression: commit 9a66d1c introduced the multi-harness process branches;
    // adding another harness must not leave its configured launch invisible.
    let commands = CliCommandSettings::default();
    for golden in LAUNCH_GOLDENS {
        let base = base_command(&commands, golden.tool, LaunchMode::Fresh);
        assert_eq!(detect_cli_tool(base), Some(golden.tool), "{base}");
    }
}

#[test]
fn launch_rendering_stays_byte_identical_to_the_pre_refactor_goldens() {
    // Regression: commit 9a66d1c distributed tool-specific launch behaviour;
    // collapsing those branches must preserve the command bytes for every tool.
    let commands = CliCommandSettings::default();
    for golden in LAUNCH_GOLDENS {
        let rendered = LaunchSpec {
            tool: golden.tool,
            mode: LaunchMode::Fresh,
            base: base_command(&commands, golden.tool, LaunchMode::Fresh),
            model: ModelSpec {
                model: Some(golden.model.to_string()),
                reasoning_effort: golden.effort.map(str::to_string),
            },
            team: Some(TeamContext {
                team_name: "golden-team",
                agent_name: "golden-agent",
                role: MemberRole::Agent,
            }),
            codex_bypass_hook_trust: golden.bypass_hook_trust,
            codex_notify_executable: None,
            claude_config_dir: None,
        }
        .render();

        assert_eq!(format!("{}\n", rendered.command), golden.expected);
        assert!(rendered.notes.is_empty());
    }
}
