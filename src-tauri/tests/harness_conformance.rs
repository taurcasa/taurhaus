#![cfg(feature = "mesh-bridged-backend")]

use std::fs;
use std::sync::Arc;

use base64::Engine as _;
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
    CommandError, CommandOutput, HttpClient, HttpError, HttpErrorKind, HttpResponse, ProviderEnv,
    UsageStatus,
};
use taurhaus_lib::session_scanner::cli_tool::{
    all, spec, CliTool, CompactionDelivery, RuntimeEffort, SessionRoot, StopStrategy,
};
use taurhaus_lib::session_scanner::idle::{AgyHooksActivitySource, IdleResult, SessionSource};
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
        tool: CliTool::Agy,
        model: "gemini-3.7-flash-high",
        effort: Some("high"),
        bypass_hook_trust: false,
        expected: include_str!("fixtures/launch/agy.golden.txt"),
    },
    LaunchGolden {
        tool: CliTool::Grok,
        model: "grok-4.6",
        effort: Some("xhigh"),
        bypass_hook_trust: false,
        expected: include_str!("fixtures/launch/grok.golden.txt"),
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
        .filter(|entry| entry.capabilities.account_selector.is_some())
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
    // Regression: 9a6b9596 repointed the backend defaults at the canonical
    // roles without updating either frontend mirror of this sealed contract.
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
    assert!(claude.capabilities.workflow_runs);
    assert_eq!(claude.stop_strategy, StopStrategy::SlashExit);

    let codex = spec(CliTool::Codex);
    assert!(codex.capabilities.compaction_hook);
    assert!(codex.capabilities.authoritative_idle);
    assert!(codex.capabilities.account_selection);
    assert!(codex.capabilities.usage);
    assert!(codex.capabilities.notify_sink);
    assert_eq!(codex.stop_strategy, StopStrategy::Interrupt);

    let grok = spec(CliTool::Grok);
    assert_eq!(
        grok.capabilities.auto_approve_flag,
        Some("--always-approve")
    );
    assert!(grok.capabilities.compaction_hook_compat_import);
    // grok's registry row appears at the first prompt, so identity is captured
    // and backfilled rather than skipped — two grok members can share a project.
    assert!(grok.capabilities.runtime_session_capture);
    // grok documents passive-hook stdout as ignored, so its card is queued in
    // the mesh inbox rather than answered on stdout.
    assert_eq!(
        grok.capabilities.compaction_delivery,
        CompactionDelivery::MeshInbox
    );
    assert_eq!(
        claude.capabilities.compaction_delivery,
        CompactionDelivery::HookStdout
    );
    assert_eq!(
        codex.capabilities.compaction_delivery,
        CompactionDelivery::HookStdout
    );
    assert!(!grok.capabilities.usage);
    assert!(grok.capabilities.usage_note.is_some());
    assert_eq!(grok.stop_strategy, StopStrategy::SlashExit);
    assert_eq!(grok.exit_command, "/quit");

    let agy = spec(CliTool::Agy);
    assert!(agy.capabilities.session_source);
    assert!(!agy.capabilities.runtime_session_capture);
    assert!(!agy.capabilities.compaction_hook);
    assert!(!agy.capabilities.transcript_parser);
    assert!(!agy.capabilities.catalog || agy.capabilities.model_flag.is_some());
    assert_eq!(agy.stop_strategy, StopStrategy::SlashExit);

    assert!(all()
        .iter()
        .filter(|entry| entry.capabilities.workflow_runs)
        .all(|entry| entry.tool == CliTool::Claude));

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
    // rollout for Codex and other harnesses.
    assert_eq!(
        all()
            .iter()
            .map(|entry| (entry.tool, entry.capabilities.account_selector))
            .collect::<Vec<_>>(),
        vec![
            (CliTool::Claude, Some("CLAUDE_CONFIG_DIR")),
            (CliTool::Codex, Some("CODEX_HOME")),
            (CliTool::Agy, None),
            (CliTool::Grok, Some("GROK_HOME")),
        ]
    );

    assert_eq!(
        all()
            .iter()
            .filter(|entry| entry.capabilities.usage)
            .map(|entry| entry.tool)
            .collect::<Vec<_>>(),
        vec![CliTool::Claude, CliTool::Codex, CliTool::Agy]
    );
}

#[test]
fn account_providers_are_registered_behind_the_capability_slice() {
    // Regression: commits d6839a3 and a574720 put Claude account detection in
    // command call sites, so adding another provider required cloning the
    // whole pipeline instead of registering one account slice.
    assert!(spec(CliTool::Claude).account_provider().is_some());
    assert!(spec(CliTool::Codex).account_provider().is_some());
    assert!(spec(CliTool::Agy).account_provider().is_some());
    assert!(spec(CliTool::Grok).account_provider().is_some());
}

#[test]
fn grok_declares_the_slices_it_does_not_have() {
    // Regression: commit bfecae9 fixed the harness set at three CLIs, so a
    // fourth could only arrive by branching outside the capability slices. Its
    // registry entry must be honest about which slices it has not landed yet.
    let grok = spec(CliTool::Grok);
    assert!(grok.transcript_parser().is_none());
    assert!(grok.compaction_signal_source().is_some());
    assert!(
        grok.usage_provider().is_none(),
        "grok publishes no subscription quota endpoint"
    );
    assert_eq!(
        grok.capabilities.usage_note,
        Some("Grok shows credits in its own /usage")
    );
}

#[test]
fn every_harness_declares_how_a_running_session_changes_effort() {
    // The lead's per-assignment effort reaches a running member one of two
    // ways, and the registry is the only place that says which: mesh types the
    // slash command into the pane before it delivers the notice, or taurhaus
    // resumes the member with the effort flag. A harness with neither leaves
    // the launch effort standing.
    for entry in all() {
        let runtime_effort = entry.capabilities.runtime_effort;
        let expected = match entry.tool {
            CliTool::Claude | CliTool::Agy | CliTool::Grok => RuntimeEffort::SlashCommand,
            CliTool::Codex => RuntimeEffort::ResumeWithFlag,
            CliTool::Unknown => RuntimeEffort::None,
        };
        assert_eq!(
            runtime_effort, expected,
            "{} declares the wrong runtime effort path",
            entry.name
        );
        if runtime_effort != RuntimeEffort::None {
            assert!(
                entry.capabilities.effort_flag.is_some(),
                "{} changes effort at runtime but declares no launch effort flag",
                entry.name
            );
        }
    }
    assert_eq!(
        spec(CliTool::Unknown).capabilities.runtime_effort,
        RuntimeEffort::None
    );
}

fn write_usage_credentials(tool: CliTool, config_dir: &std::path::Path) {
    match tool {
        CliTool::Claude => fs::write(
            config_dir.join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"conformance-token","expiresAt":4102444800000}}"#,
        )
        .expect("Claude usage fixture credentials"),
        CliTool::Codex => {
            let payload =
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"exp":4102444800}"#);
            fs::write(
                config_dir.join("auth.json"),
                format!(
                    r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"fixture.{payload}.token","account_id":"conformance-account"}}}}"#
                ),
            )
            .expect("Codex usage fixture credentials");
        }
        CliTool::Agy => {
            fs::create_dir_all(config_dir.join("antigravity-cli"))
                .expect("Antigravity app-data fixture");
            fs::write(
                config_dir.join("google_accounts.json"),
                r#"{"active":"fixture@example.com"}"#,
            )
            .expect("Antigravity account fixture");
            fs::write(
                config_dir.join("antigravity-cli/antigravity-oauth-token"),
                "{}",
            )
            .expect("Antigravity credential fixture");
        }
        CliTool::Grok => unreachable!("Grok declares no usage provider"),
        CliTool::Unknown => panic!("unknown tools do not have usage credentials"),
    }
}

struct ConformanceHttp {
    status: Option<u16>,
}

struct ConformanceAgyEnv {
    http: ConformanceHttp,
    status: Option<u16>,
}

impl ProviderEnv for ConformanceAgyEnv {
    fn http(&self) -> &dyn HttpClient {
        &self.http
    }

    fn run_command(
        &self,
        _argv: &[&str],
        _cwd: &std::path::Path,
        _timeout: std::time::Duration,
        _env: &[(&str, &str)],
    ) -> Result<CommandOutput, CommandError> {
        match self.status {
            None => Err(CommandError { timed_out: false }),
            Some(401) => Ok(CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "Please sign in".to_string(),
            }),
            Some(_) => unreachable!("conformance only exercises failure statuses"),
        }
    }
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
        write_usage_credentials(entry.tool, config_dir.path());
        let stale_http = ConformanceHttp { status: None };
        let stale_agy = ConformanceAgyEnv {
            http: ConformanceHttp { status: None },
            status: None,
        };
        let stale_env: &dyn ProviderEnv = if entry.tool == CliTool::Agy {
            &stale_agy
        } else {
            &stale_http
        };
        assert_eq!(
            usage.fetch(config_dir.path(), stale_env).status,
            UsageStatus::Stale,
            "{} network failure status",
            entry.name
        );
        let unauthorized_http = ConformanceHttp { status: Some(401) };
        let unauthorized_agy = ConformanceAgyEnv {
            http: ConformanceHttp { status: Some(401) },
            status: Some(401),
        };
        let unauthorized_env: &dyn ProviderEnv = if entry.tool == CliTool::Agy {
            &unauthorized_agy
        } else {
            &unauthorized_http
        };
        assert_eq!(
            usage.fetch(config_dir.path(), unauthorized_env).status,
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
    assert_eq!(
        account_tools,
        vec![CliTool::Claude, CliTool::Codex, CliTool::Grok]
    );

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
    assert_eq!(
        usage_tools,
        vec![CliTool::Claude, CliTool::Codex, CliTool::Agy]
    );
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

    let mut capabilities = spec(CliTool::Agy).capabilities;
    capabilities.catalog = false;
    capabilities.model_flag = None;
    capabilities.effort_flag = None;
    capabilities.auto_approve_flag = None;
    let rendered = LaunchSpec {
        tool: CliTool::Agy,
        mode: LaunchMode::Fresh,
        base: "agy",
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

    assert_eq!(rendered.command, "agy");
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
    // Regression: f90b362 mapped the third harness's declared session source
    // to the floor while tests instantiated its resolver directly.
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
    let temp = tempfile::tempdir().expect("agy activity conformance root");
    let sink = temp.path().join("agy-hooks.jsonl");
    let root = temp.path().join(".gemini");
    let transcript = temp.path().join("conversation-1.db");
    std::fs::write(&transcript, b"fixture").expect("agy transcript fixture");
    taurhaus_lib::daemon::agy_hooks::append_event_at(
        &sink,
        taurhaus_lib::daemon::agy_hooks::AgyHookEvent::Busy,
        r#"{"conversationId":"conversation-1"}"#,
        chrono::Utc::now(),
    )
    .expect("agy hook fixture");
    let heuristic = IdleResult {
        state: SessionState::Active,
        session_id: Some("conversation-1".to_string()),
        jsonl_path: Some(transcript.to_string_lossy().into_owned()),
        last_output_age_secs: None,
        authoritative: false,
    };

    // Regression: commit 4e9e2c5 used no session id here, so the test returned
    // before exercising the opt-in hooks gate it claimed to protect.
    assert!(AgyHooksActivitySource::authoritative_state_at(&heuristic, &sink, &root).is_none());
}

#[test]
fn grok_identity_and_activity_answer_from_its_own_files() {
    // Regression: commit 16de5ec registered grok on the session-source floor,
    // so its live registry and turn lifecycle had no declared slice at all.
    let home = tempfile::tempdir().expect("grok conformance home");
    let project = std::env::current_dir().expect("conformance cwd");
    let project_path = project.to_string_lossy().into_owned();
    let session_id = "01a04585-2d53-7123-8000-9a0f4d0b21ce";
    let pid = std::process::id();
    fs::write(
        home.path().join("active_sessions.json"),
        serde_json::json!([{
            "session_id": session_id,
            "pid": pid,
            "cwd": project_path,
            "opened_at": "2026-08-27T23:22:06.993848110Z",
        }])
        .to_string(),
    )
    .expect("grok session registry fixture");
    let session_dir = home
        .path()
        .join("sessions")
        .join("%2Fconformance")
        .join(session_id);
    fs::create_dir_all(&session_dir).expect("grok session dir fixture");
    fs::write(
        session_dir.join("summary.json"),
        serde_json::json!({ "info": { "id": session_id, "cwd": project_path } }).to_string(),
    )
    .expect("grok summary fixture");
    fs::write(
        session_dir.join("events.jsonl"),
        "{\"ts\":\"1\",\"type\":\"turn_started\",\"turn_number\":0}\n",
    )
    .expect("grok events fixture");

    let resolved = taurhaus_lib::session_scanner::idle::GrokResolver::resolve_at(
        home.path(),
        &project_path,
        pid,
    );
    assert_eq!(resolved.session_id.as_deref(), Some(session_id));
    assert_eq!(
        resolved.jsonl_path.as_deref(),
        session_dir.join("events.jsonl").to_str()
    );
    assert!(!resolved.authoritative);

    let busy =
        taurhaus_lib::session_scanner::idle::GrokEventsActivitySource::authoritative_state_at(
            &resolved,
        )
        .expect("grok activity source answers from events.jsonl");
    assert_eq!(busy.state, SessionState::Active);
    assert_eq!(busy.source, "grok_events");

    fs::write(
        session_dir.join("events.jsonl"),
        "{\"ts\":\"1\",\"type\":\"turn_started\",\"turn_number\":0}\n{\"ts\":\"2\",\"type\":\"turn_ended\",\"outcome\":\"completed\"}\n",
    )
    .expect("grok settled events fixture");
    assert_eq!(
        taurhaus_lib::session_scanner::idle::GrokEventsActivitySource::authoritative_state_at(
            &resolved
        )
        .expect("settled turn")
        .state,
        SessionState::Idle
    );

    // The registry row is grok's own clean-stop proof.
    assert_eq!(spec(CliTool::Grok).stop_strategy, StopStrategy::SlashExit);
    assert!(spec(CliTool::Grok).stop_registry_release);
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
        (
            CliTool::Grok,
            temp.path()
                .join(".grok/sessions/%2Fproject/session/updates.jsonl"),
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

    assert!(spec(CliTool::Agy).compaction_signal_source().is_none());

    // grok speaks camelCase keys with snake_case event values, and names its
    // workspace root rather than a cwd.
    let grok: taurhaus_lib::coordination::compact_hook::CompactHookInput = serde_json::from_str(
        &serde_json::json!({
            "hookEventName": "post_compact",
            "sessionId": "01a04585-2d53-7123",
            "trigger": "auto",
            "workspaceRoot": "/home/user/projects/taurhaus",
            "transcriptPath": "/home/user/.grok/sessions/%2Fp/01a04585/updates.jsonl",
        })
        .to_string(),
    )
    .expect("grok camelCase hook payload");
    assert_eq!(grok.inferred_tool(), Some(CliTool::Grok));
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
    assert!(spec(CliTool::Agy).transcript_parser().is_none());
}
