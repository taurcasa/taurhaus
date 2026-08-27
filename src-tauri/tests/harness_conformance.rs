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

#[test]
fn compaction_sources_are_idempotent_removable_and_parse_their_payloads() {
    // Regression: commit 6fe0aa3 introduced a second hook settings format;
    // installers and payload parsing must stay behind one capability slice.
    let temp = tempfile::tempdir().expect("compaction conformance root");
    let executable = temp.path().join("taurhaus");
    std::fs::write(&executable, b"test executable").expect("fake executable");

    for (tool, transcript) in [
        (
            CliTool::Claude,
            temp.path().join("claude/projects/project/session.jsonl"),
        ),
        (
            CliTool::Codex,
            temp.path()
                .join("codex/sessions/2026/08/27/rollout-session.jsonl"),
        ),
    ] {
        let source = spec(tool)
            .compaction_signal_source()
            .expect("declared compaction source");
        let config_dir = temp.path().join(tool.to_string());
        assert!(source
            .install(&config_dir, &executable)
            .expect("first install changes files"));
        assert!(!source
            .install(&config_dir, &executable)
            .expect("second install is idempotent"));

        let payload = serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": "session",
            "source": "compact",
            "transcript_path": transcript,
        })
        .to_string();
        assert_eq!(
            source
                .parse_payload(&payload)
                .expect("source parses payload")
                .inferred_tool(),
            Some(tool)
        );

        assert!(source
            .remove(&config_dir)
            .expect("first removal changes files"));
        assert!(!source
            .remove(&config_dir)
            .expect("second removal is idempotent"));
    }

    assert!(spec(CliTool::Gemini).compaction_signal_source().is_none());
}

#[test]
fn transcript_parsers_match_declared_capabilities() {
    // Regression: commit 9a66d1c wired transcript formats at their consumers;
    // parser availability must be declared once for every registered harness.
    for entry in all() {
        assert_eq!(
            entry.transcript_parser().is_some(),
            entry.capabilities.transcript_parser,
            "{} transcript parser declaration",
            entry.name
        );
    }

    let codex = spec(CliTool::Codex)
        .transcript_parser()
        .expect("Codex transcript parser");
    let boundary = codex
        .parse_compaction_boundary(
            r#"{"timestamp":"2026-08-27T08:00:00Z","type":"compacted"}"#,
            123,
        )
        .expect("Codex compaction boundary");
    assert_eq!(boundary.jsonl_offset, 123);

    assert!(spec(CliTool::Claude)
        .transcript_parser()
        .expect("Claude transcript parser")
        .parse_compaction_boundary("{}", 0)
        .is_none());
    assert!(spec(CliTool::Gemini).transcript_parser().is_none());
}
