use pretty_assertions::assert_eq;
use taurhaus_lib::coordination::domain::MemberRole;
use taurhaus_lib::daemon::protocol::LaunchMode;
use taurhaus_lib::models::CliCommandSettings;
use taurhaus_lib::session_scanner::cli_tool::{all, spec, CliTool, StopStrategy};
use taurhaus_lib::session_scanner::idle::IdleResult;
use taurhaus_lib::session_scanner::launch::{base_command, LaunchSpec, ModelSpec, TeamContext};
use taurhaus_lib::session_scanner::process::detect_cli_tool;
use taurhaus_lib::session_scanner::SessionState;

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

#[test]
fn registry_is_complete_and_drives_the_terminal_contract() {
    // Regression: commit 9a66d1c spread harness identity and defaults across
    // call sites, so adding a tool could silently omit its UI contract entry.
    let registered = all();
    assert_eq!(registered.len(), LAUNCH_GOLDENS.len());
    assert_eq!(
        registered
            .iter()
            .map(|entry| entry.tool)
            .collect::<Vec<_>>(),
        LAUNCH_GOLDENS
            .iter()
            .map(|golden| golden.tool)
            .collect::<Vec<_>>()
    );

    let contract = taurhaus_lib::models::TerminalPlatformContract::for_platform(
        taurhaus_lib::models::AppPlatform::Linux,
    );
    assert_eq!(
        contract
            .tools
            .iter()
            .map(|descriptor| descriptor.id)
            .collect::<Vec<_>>(),
        registered
            .iter()
            .map(|entry| entry.tool)
            .collect::<Vec<_>>()
    );

    for entry in registered {
        assert_eq!(entry.name.parse::<CliTool>(), Ok(entry.tool));
        for alias in entry.aliases {
            assert_eq!(CliTool::from_alias(alias), Ok(entry.tool));
        }
        assert_eq!(
            detect_cli_tool(&entry.default_commands.fresh),
            Some(entry.tool)
        );
    }
}

#[test]
fn registry_declares_native_and_floor_capabilities() {
    // Regression: commits d6839a3 and a574720 added Claude/Codex-only native
    // features in their callers; capability ownership belongs in the registry.
    let claude = spec(CliTool::Claude);
    assert_eq!(
        claude.capabilities.config_dir_env,
        Some("CLAUDE_CONFIG_DIR")
    );
    assert!(claude.capabilities.usage_bridge);
    assert!(claude.capabilities.native_inbox_poller);
    assert_eq!(claude.stop_strategy, StopStrategy::SlashExit);

    let codex = spec(CliTool::Codex);
    assert!(codex.capabilities.compaction_hook);
    assert!(codex.capabilities.authoritative_idle);
    assert!(codex.capabilities.notify_sink);
    assert_eq!(codex.stop_strategy, StopStrategy::SlashExit);

    let gemini = spec(CliTool::Gemini);
    assert!(!gemini.capabilities.compaction_hook);
    assert!(!gemini.capabilities.transcript_parser);
    assert!(!gemini.capabilities.catalog || gemini.capabilities.model_flag.is_some());
    assert_eq!(gemini.stop_strategy, StopStrategy::SlashExit);
}

#[test]
fn undeclared_session_source_uses_the_non_authoritative_floor() {
    // Regression: commit cb32d7a made Gemini identity depend on a bespoke
    // project transcript lookup; an undeclared source must stay on the floor.
    let result = spec(CliTool::Gemini).session_source().resolve(
        "/tmp/taurhaus-conformance-project",
        42,
        Some("%42"),
    );

    assert_eq!(result.session_id, None);
    assert_eq!(result.jsonl_path, None);
    assert!(!result.authoritative);
}

#[test]
fn undeclared_activity_source_never_claims_authority() {
    // Regression: commit c0aa59a added Codex notify handling at the resolver;
    // native state must only be consumed through a declared activity source.
    let heuristic = IdleResult {
        state: SessionState::Active,
        session_id: None,
        jsonl_path: None,
        last_output_age_secs: None,
        authoritative: false,
    };

    assert!(spec(CliTool::Gemini)
        .activity_source()
        .authoritative_state("/tmp/taurhaus-conformance-project", 42, &heuristic)
        .is_none());
}
