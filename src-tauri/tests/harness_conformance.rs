#![cfg(feature = "mesh-bridged-backend")]

use std::fs;
use std::sync::Arc;

use pretty_assertions::assert_eq;
use taurhaus_lib::coordination::backend::MeshBridgedBackend;
use taurhaus_lib::coordination::domain::MemberRole;
use taurhaus_lib::coordination::orchestrator::CoordinationOrchestrator;
use taurhaus_lib::coordination::requests::{
    AgentSetupConfig, DeliveryRequest, InitializeTeamRequest, LeadMode, OperatorNoticeDelivery,
};
use taurhaus_lib::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
use taurhaus_lib::coordination::stores::MeshInboxStore;
use taurhaus_lib::daemon::protocol::LaunchMode;
use taurhaus_lib::models::{CliCommandSettings, ModelCatalog};
use taurhaus_lib::session_scanner::accounts::{
    HttpClient, HttpError, HttpErrorKind, HttpResponse, UsageStatus,
};
use taurhaus_lib::session_scanner::cli_tool::{all, spec, CliTool, SessionRoot, StopStrategy};
use taurhaus_lib::session_scanner::idle::{IdleResult, SessionSource};
use taurhaus_lib::session_scanner::launch::{
    base_command, LaunchCapability, LaunchNote, LaunchSpec, ModelSpec, TeamContext,
};
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
            account_dir: None,
            selector: None,
        }
        .render();

        assert_eq!(format!("{}\n", rendered.command), golden.expected);
        assert!(rendered.notes.is_empty());
    }
}

#[test]
fn account_dir_launch_cases_use_each_registry_selector() {
    // Regression: d6839a3 rendered the account directory only inside the
    // Claude launch arm; provider rollout must need data, not another branch.
    let commands = CliCommandSettings::default();
    let rendered = all()
        .iter()
        .map(|entry| {
            let account_dir = std::path::PathBuf::from(format!("/accounts/{}", entry.name));
            let command = LaunchSpec {
                tool: entry.tool,
                mode: LaunchMode::Fresh,
                base: base_command(&commands, entry.tool, LaunchMode::Fresh),
                model: ModelSpec::default(),
                team: None,
                codex_bypass_hook_trust: false,
                codex_notify_executable: None,
                account_dir: Some(&account_dir),
                selector: entry.capabilities.account_selector,
            }
            .render()
            .command;
            let selector = entry
                .capabilities
                .account_selector
                .expect("account-dir conformance case has a selector");
            assert_eq!(
                command.matches(&format!("{selector}=")).count(),
                1,
                "{} selector must be rendered exactly once",
                entry.name
            );
            format!("{}={command}", entry.name)
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        format!("{rendered}\n"),
        include_str!("fixtures/launch/account-dirs.golden.txt")
    );
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

    let frontend_fixture: Vec<taurhaus_lib::session_scanner::cli_tool::CliToolDescriptor> =
        serde_json::from_str(include_str!("../../src/lib/fixtures/tool-registry.json"))
            .expect("frontend registry fixture");
    assert_eq!(
        taurhaus_lib::session_scanner::cli_tool::descriptors(),
        frontend_fixture,
        "Rust descriptors and the pre-settings frontend fallback must stay identical"
    );
}

#[test]
fn registry_declares_native_and_floor_capabilities() {
    // Regression: commits d6839a3 and a574720 added Claude/Codex-only native
    // features in their callers; capability ownership belongs in the registry.
    let claude = spec(CliTool::Claude);
    assert_eq!(
        claude.capabilities.account_selector,
        Some("CLAUDE_CONFIG_DIR")
    );
    assert!(claude.capabilities.usage);
    assert!(claude.capabilities.native_inbox_poller);
    assert_eq!(claude.stop_strategy, StopStrategy::SlashExit);

    let codex = spec(CliTool::Codex);
    assert!(codex.capabilities.compaction_hook);
    assert!(codex.capabilities.authoritative_idle);
    assert!(codex.capabilities.notify_sink);
    assert_eq!(codex.stop_strategy, StopStrategy::Interrupt);

    let gemini = spec(CliTool::Gemini);
    assert!(gemini.capabilities.session_source);
    assert!(!gemini.capabilities.runtime_session_capture);
    assert!(!gemini.capabilities.compaction_hook);
    assert!(!gemini.capabilities.transcript_parser);
    assert!(!gemini.capabilities.catalog || gemini.capabilities.model_flag.is_some());
    assert_eq!(gemini.stop_strategy, StopStrategy::SlashExit);

    for entry in all() {
        assert!(
            !entry.capabilities.runtime_session_capture || entry.capabilities.session_source,
            "{} cannot capture a runtime session without a session source",
            entry.name
        );
    }
}

#[test]
fn account_selectors_are_declared_independently_of_provider_rollout() {
    // Regression: commit 07fc8f3 overloaded `config_dir_env` as both launch
    // data and an account-provider predicate, preventing floor-only provider
    // rollout for Codex and Gemini.
    assert_eq!(
        all()
            .iter()
            .map(|entry| (entry.tool, entry.capabilities.account_selector))
            .collect::<Vec<_>>(),
        vec![
            (CliTool::Claude, Some("CLAUDE_CONFIG_DIR")),
            (CliTool::Codex, Some("CODEX_HOME")),
            (CliTool::Gemini, Some("GEMINI_CLI_HOME")),
        ]
    );

    assert_eq!(
        all()
            .iter()
            .filter(|entry| entry.capabilities.usage)
            .map(|entry| entry.tool)
            .collect::<Vec<_>>(),
        vec![CliTool::Claude]
    );
}

#[test]
fn claude_account_provider_is_registered_behind_the_capability_slice() {
    // Regression: commits d6839a3 and a574720 put Claude account detection in
    // command call sites, so adding another provider required cloning the
    // whole pipeline instead of registering one account slice.
    assert!(spec(CliTool::Claude).account_provider().is_some());
    assert!(spec(CliTool::Codex).account_provider().is_none());
    assert!(spec(CliTool::Gemini).account_provider().is_none());
}

struct ConformanceHttp {
    status: Option<u16>,
}

impl HttpClient for ConformanceHttp {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _timeout: std::time::Duration,
    ) -> Result<HttpResponse, HttpError> {
        match self.status {
            Some(status) => Ok(HttpResponse {
                status,
                body: String::new(),
            }),
            None => Err(HttpError {
                kind: HttpErrorKind::Network,
            }),
        }
    }
}

#[test]
fn account_and_usage_provider_slices_obey_the_registry_contract() {
    // Regression: commits d6839a3 and a574720 wired account detection and
    // usage directly to Claude, leaving new registry entries without a tested
    // provider floor or injectable HTTP boundary.
    for entry in all() {
        let provider = entry.account_provider();
        if entry.capabilities.account_selector.is_some() && provider.is_none() {
            assert!(
                !entry.capabilities.account_selection && !entry.capabilities.usage,
                "{} must stay on the logged provider floor until its slice lands",
                entry.name
            );
        }

        if let Some(provider) = provider {
            let home = tempfile::tempdir().expect("empty account-provider home");
            let default_dir = provider.default_dir(home.path());
            assert_eq!(
                provider.candidate_dirs(home.path(), &[]),
                vec![default_dir.clone()],
                "{} empty-home candidates",
                entry.name
            );
            fs::create_dir_all(&default_dir).expect("empty provider default dir");
            assert_eq!(
                provider.identify(&default_dir),
                None,
                "{} must not invent an identity for an empty directory",
                entry.name
            );
        }

        let usage = entry.usage_provider();
        assert_eq!(
            entry.capabilities.usage,
            usage.is_some(),
            "{} usage capability/provider mismatch",
            entry.name
        );
        let Some(usage) = usage else {
            continue;
        };
        let config_dir = tempfile::tempdir().expect("usage fixture account dir");
        fs::write(
            config_dir.path().join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"conformance-token","expiresAt":4102444800000}}"#,
        )
        .expect("usage fixture credentials");
        assert_eq!(
            usage
                .fetch(config_dir.path(), &ConformanceHttp { status: None })
                .status,
            UsageStatus::Stale,
            "{} network failure status",
            entry.name
        );
        assert_eq!(
            usage
                .fetch(config_dir.path(), &ConformanceHttp { status: Some(401) },)
                .status,
            UsageStatus::Unauthorized,
            "{} rejected credential status",
            entry.name
        );
    }
}

#[test]
fn claude_only_capabilities_are_declared_independently() {
    // Regression: d6839a3 and a574720 introduced Claude account selection and
    // usage bridging; 07fc8f3 then overloaded config_dir_env as the predicate
    // for account discovery, session roots, and team config namespaces.
    let app_managed_roots = all()
        .iter()
        .filter(|entry| entry.capabilities.session_root == SessionRoot::AppManagedClaudeDir)
        .map(|entry| entry.tool)
        .collect::<Vec<_>>();
    assert_eq!(app_managed_roots, vec![CliTool::Claude]);

    let account_tools = all()
        .iter()
        .filter(|entry| entry.capabilities.account_selection)
        .map(|entry| entry.tool)
        .collect::<Vec<_>>();
    assert_eq!(account_tools, vec![CliTool::Claude]);

    let team_namespace_tools = all()
        .iter()
        .filter(|entry| entry.capabilities.team_config_namespace)
        .map(|entry| entry.tool)
        .collect::<Vec<_>>();
    assert_eq!(team_namespace_tools, vec![CliTool::Claude]);

    let usage_tools = all()
        .iter()
        .filter(|entry| entry.capabilities.usage)
        .map(|entry| entry.tool)
        .collect::<Vec<_>>();
    assert_eq!(usage_tools, vec![CliTool::Claude]);
}

#[test]
fn catalog_defaults_conform_for_every_registry_entry() {
    // Regression: 07fc8f3 declared catalog capability data without checking
    // that every registry entry has a valid default model and effort pair.
    for entry in all() {
        let catalog_entries = ModelCatalog::entries_for(entry.tool);
        if entry.capabilities.catalog {
            assert!(!catalog_entries.is_empty(), "{} catalog", entry.name);
            let default = ModelCatalog::default_for(entry.tool).expect("declared catalog default");
            assert!(
                ModelCatalog::entry_for(entry.tool, &default.id).is_some(),
                "{} default model",
                entry.name
            );
            if let Some(effort) = default.default_effort.as_deref() {
                assert!(
                    ModelCatalog::supports_effort(entry.tool, Some(&default.id), effort),
                    "{} default effort",
                    entry.name
                );
            }
        } else {
            assert!(
                catalog_entries.is_empty(),
                "{} declares catalog: none",
                entry.name
            );
        }
    }
}

#[test]
fn absent_catalog_and_launch_flags_use_the_declared_floor() {
    // Regression: e86980b indexed an empty catalog, and e17f3eb expected every
    // launch arm to declare flags although `catalog: none` is a valid entry.
    assert!(ModelCatalog::default_from_entries(&[], false).is_none());

    let mut capabilities = spec(CliTool::Gemini).capabilities;
    capabilities.catalog = false;
    capabilities.model_flag = None;
    capabilities.effort_flag = None;
    let rendered = LaunchSpec {
        tool: CliTool::Gemini,
        mode: LaunchMode::Fresh,
        base: "gemini --yolo",
        model: ModelSpec {
            model: Some("future-model".to_string()),
            reasoning_effort: Some("high".to_string()),
        },
        team: None,
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
        account_dir: None,
        selector: None,
    }
    .render_with_capabilities(capabilities);

    assert_eq!(rendered.command, "gemini --yolo");
    assert_eq!(
        rendered.notes,
        vec![
            LaunchNote::CapabilityMissing {
                capability: LaunchCapability::Model,
                found: "future-model".to_string(),
            },
            LaunchNote::CapabilityMissing {
                capability: LaunchCapability::Effort,
                found: "high".to_string(),
            },
        ]
    );
}

fn setup_config(tool: CliTool) -> AgentSetupConfig {
    let default = ModelCatalog::default_for(tool).expect("conformance catalog default");
    AgentSetupConfig {
        name: "team-lead".to_string(),
        cli_tool: tool.to_string(),
        model: default.id.clone(),
        reasoning_effort: default.default_effort.clone(),
        project_id: "/tmp/taurhaus-conformance-project".to_string(),
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
        handoff_expectations: None,
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
    }
}

#[test]
fn every_registry_entry_launches_and_receives_an_operator_notice_through_the_floor() {
    // Regression: 07fc8f3 added registry entries without exercising the real
    // tmux/mesh launch and inbox-delivery floor for every registered harness.
    for entry in all() {
        let temp = tempfile::tempdir().expect("coordination conformance root");
        let team_name = format!("{}-conformance", entry.name);
        let credential_dir = temp
            .path()
            .join(&team_name)
            .join("state")
            .join("control_auth");
        fs::create_dir_all(&credential_dir).expect("credential dir");
        fs::write(
            credential_dir.join("team-lead.json"),
            r#"{"name":"team-lead","token":"conformance-token"}"#,
        )
        .expect("lead credential");

        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(temp.path());
        let backend = Arc::new(MeshBridgedBackend::new_with_teams_dir(
            temp.path().to_path_buf(),
        ));
        let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
            temp.path().to_path_buf(),
            backend,
            runtime.clone(),
        );
        let report = orchestrator
            .initialize_team(&InitializeTeamRequest {
                team_name: team_name.clone(),
                team_description: Some("harness conformance".to_string()),
                lead_mode: LeadMode::LaunchNew,
                lead: setup_config(entry.tool),
                agents: vec![],
            })
            .expect("team launch");
        assert!(
            report.failed_step.is_none(),
            "{} launch failed: {report:?}",
            entry.name
        );

        orchestrator
            .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: "team-lead".to_string(),
                team_name: team_name.clone(),
                message: format!("{} conformance notice", entry.name),
                sender_name: Some("operator".to_string()),
                operational_context: None,
            }))
            .expect("operator notice");

        let calls = runtime.calls();
        assert!(
            calls
                .iter()
                .any(|call| matches!(call, RuntimeCall::CreatePane { .. })),
            "{} pane launch",
            entry.name
        );
        assert!(
            calls
                .iter()
                .any(|call| matches!(call, RuntimeCall::SendKeys { .. })),
            "{} command launch",
            entry.name
        );
        assert_eq!(
            calls
                .iter()
                .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })),
            !entry.capabilities.native_inbox_poller,
            "{} member daemon floor",
            entry.name
        );
        let inbox =
            MeshInboxStore::load(temp.path(), &team_name, "team-lead").expect("operator inbox");
        assert!(
            inbox
                .iter()
                .any(|message| message.text == format!("{} conformance notice", entry.name)),
            "{} inbox append",
            entry.name
        );
    }
}

#[test]
fn undeclared_session_source_uses_the_non_authoritative_floor() {
    // Regression: f90b362 must preserve a non-authoritative floor for future
    // harnesses that declare no transcript-backed session source.
    let result = taurhaus_lib::session_scanner::idle::NoSessionSource.resolve(
        "/tmp/taurhaus-conformance-project",
        42,
        Some("%42"),
    );

    assert_eq!(result.session_id, None);
    assert_eq!(result.jsonl_path, None);
    assert!(!result.authoritative);
}

#[test]
fn session_source_wiring_matches_every_registry_declaration() {
    // Regression: f90b362 mapped Gemini's declared session source to the floor,
    // while tests instantiated GeminiResolver directly and missed the registry.
    for entry in all() {
        assert_eq!(
            entry.session_source().is_floor(),
            !entry.capabilities.session_source,
            "{} session source declaration",
            entry.name
        );
    }
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
        let parsed: taurhaus_lib::coordination::compact_hook::CompactHookInput =
            serde_json::from_str(&payload).expect("shared hook payload");
        assert_eq!(parsed.inferred_tool(), Some(tool));

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
