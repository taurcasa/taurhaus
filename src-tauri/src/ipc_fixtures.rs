use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use pretty_assertions::assert_eq;
use serde::Serialize;

use crate::commands::accounts::AccountsResult;
use crate::commands::coordination_types::{
    AgentRole, LiveAgentStatus, LiveRuntimeSnapshotFreshness, LiveTeamStatus, SessionStatus,
};
use crate::coordination::requests::{DeliveryMethod, DeliveryResult, WakeDisposition};
use crate::models::{
    ActivityThresholds, AppPlatform, CliCommandSettings, CliVersions, CodeThemeSettings,
    DaemonSettings, HarnessSettings, ModelCatalog, ModelCatalogEntry, Settings,
    TerminalPlatformContract, TerminalSettings, ToolCommands,
};
use crate::session_scanner::accounts::{
    Account, AccountIdentity, Severity, UsageSnapshot, UsageStatus, UsageWindow,
};
use crate::session_scanner::cli_tool::{
    CliCapabilityDescriptor, CliTool, CliToolDescriptor, EffortFlagDescriptor, SessionRoot,
};
use crate::workflow_runs::WorkflowActivity;

const UPDATE_FIXTURES: &str = "UPDATE_IPC_FIXTURES";

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/lib/ipc/__fixtures__")
        .join(name)
}

fn canonical_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(entries) => {
            serde_json::Value::Array(entries.into_iter().map(canonical_json).collect())
        }
        serde_json::Value::Object(entries) => {
            let mut entries = entries.into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            serde_json::Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonical_json(value)))
                    .collect(),
            )
        }
        scalar => scalar,
    }
}

fn assert_fixture(name: &str, value: &impl Serialize) {
    let path = fixture_path(name);
    let value = canonical_json(serde_json::to_value(value).expect("IPC fixture value serializes"));
    let actual = format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("IPC fixture value formats")
    );

    if std::env::var_os(UPDATE_FIXTURES).is_some() {
        std::fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create IPC fixture directory");
        std::fs::write(&path, &actual).expect("write IPC fixture");
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "read {}: {error}; regenerate with `cd src-tauri && {UPDATE_FIXTURES}=1 cargo test --lib -- ipc_fixtures`",
            path.display()
        )
    });
    assert_eq!(
        expected,
        actual,
        "{} drifted from its Rust contract",
        path.display()
    );
}

fn commands(prefix: &str) -> ToolCommands {
    ToolCommands {
        continue_cmd: format!("{prefix} --continue"),
        fresh: prefix.to_string(),
        resume: format!("{prefix} --resume fixture-session"),
    }
}

fn model_entry(id: &str) -> ModelCatalogEntry {
    ModelCatalogEntry {
        id: id.to_string(),
        label: format!("Fixture {id}"),
        efforts: vec!["high".to_string()],
        default_effort: Some("high".to_string()),
        deprecated: true,
        replacement: Some(format!("{id}-replacement")),
    }
}

fn fully_populated_settings() -> Settings {
    let cli_commands = CliCommandSettings {
        claude: commands("claude"),
        codex: commands("codex"),
        agy: commands("agy"),
        grok: commands("grok"),
        codex_bypass_hook_trust: true,
        codex_notify_executable: Some(PathBuf::from("/fixtures/taurhaus-daemon")),
        account_selector_dirs: HashMap::from([(
            "codex".to_string(),
            PathBuf::from("/fixtures/codex-home"),
        )]),
        resolved_bases: HashMap::new(),
    };
    let model_catalog = ModelCatalog {
        claude: vec![model_entry("fixture-claude")],
        codex: vec![model_entry("fixture-codex")],
        agy: vec![model_entry("fixture-agy")],
        grok: vec![model_entry("fixture-grok")],
    };
    let cli_versions = CliVersions {
        codex: Some("0.150.1".to_string()),
        claude: Some("2.1.246".to_string()),
        agy: Some("1.1.22".to_string()),
        codex_compaction_hooks_supported: true,
        codex_notify_supported: true,
        codex_queue_wake_supported: true,
        agy_hooks_supported: true,
    };
    let tools = vec![CliToolDescriptor {
        id: CliTool::Codex,
        label: "Codex fixture".to_string(),
        display_name: "Codex fixture CLI".to_string(),
        accent: "sky".to_string(),
        medallion_accent: "emerald".to_string(),
        default_agent_role_id: "fixture-developer".to_string(),
        aliases: vec!["codex".to_string(), "fixture-codex".to_string()],
        capabilities: CliCapabilityDescriptor {
            model_flag: Some("-m".to_string()),
            effort_flag: Some(EffortFlagDescriptor::Config {
                flag: "-c".to_string(),
                key: "model_reasoning_effort".to_string(),
            }),
            auto_approve_flag: Some("--yolo".to_string()),
            display_name_flag: Some("--display-name".to_string()),
            team_flags: true,
            native_inbox_poller: true,
            session_source: true,
            runtime_session_capture: true,
            authoritative_idle: true,
            compaction_hook: true,
            compaction_hook_compat_import: true,
            transcript_parser: true,
            catalog: true,
            session_root: SessionRoot::ToolHome,
            account_selector: Some("CODEX_HOME".to_string()),
            account_selection: true,
            team_config_namespace: true,
            usage: true,
            usage_note: Some("Fixture usage note".to_string()),
            notify_sink: true,
            hook_trust: true,
            managed_home: true,
        },
    }];

    Settings {
        scan_directories: vec!["/fixtures/projects".to_string()],
        thresholds: ActivityThresholds {
            active_days: 3,
            recent_days: 14,
            stale_days: 45,
        },
        ignore_patterns: vec!["fixture-ignore".to_string()],
        daemon: DaemonSettings {
            port: 17333,
            path: "/fixtures/taurhaus-daemon".to_string(),
            auto_start: true,
        },
        code_theme: CodeThemeSettings {
            light: "fixture-light".to_string(),
            dark: "fixture-dark".to_string(),
        },
        terminal: TerminalSettings {
            emulator: "manual".to_string(),
            custom_command: "fixture-terminal {tmux_session}".to_string(),
            tmux_layout: "new_window".to_string(),
            cli_commands: cli_commands.clone(),
            harness: HarnessSettings {
                agy_hooks: true,
                grok_hooks: true,
            },
            default_account_ids: HashMap::from([
                ("claude".to_string(), "fixture-claude-account".to_string()),
                ("codex".to_string(), "fixture-codex-account".to_string()),
            ]),
        },
        dark_mode: true,
        project_dialog_last_path: "/fixtures/last-project".to_string(),
        terminal_contract: TerminalPlatformContract {
            platform: AppPlatform::Linux,
            default_emulator: "manual".to_string(),
            supported_emulators: vec!["manual".to_string(), "custom".to_string()],
            cli_command_defaults: cli_commands,
            model_catalog,
            cli_versions,
            tools,
        },
    }
}

fn fully_populated_live_team_status() -> LiveTeamStatus {
    LiveTeamStatus {
        team_name: "fixture-team".to_string(),
        lead_name: "team-lead".to_string(),
        runtime_snapshot_freshness: LiveRuntimeSnapshotFreshness::Fresh,
        members: vec![LiveAgentStatus {
            name: "fixture-developer".to_string(),
            role: AgentRole::Member,
            cli_tool: "codex".to_string(),
            model: "gpt-5.6-sol".to_string(),
            reasoning_effort: Some("high".to_string()),
            role_id: Some("codex-developer".to_string()),
            role_name: Some("Codex Developer".to_string()),
            focus_area: Some("Lossless IPC".to_string()),
            context_summary: Some("Own the frontend contract".to_string()),
            behavior_summary: Some("Preserve future fields".to_string()),
            project_id: "/fixtures/project".to_string(),
            is_cross_project: true,
            project_label: "fixture-project".to_string(),
            description: Some("Fixture member".to_string()),
            session_status: SessionStatus::Active,
            pane_id: Some("%42".to_string()),
            session_id: Some("fixture-session".to_string()),
            workflow_activity: Some(WorkflowActivity {
                live_runs: 2,
                last_write_at: 1_800_000_000_000,
            }),
            task_effort: Some("high".to_string()),
            task_effort_why: Some("The fixture covers every field".to_string()),
            account_applied: Some(false),
            account_note: Some("opaque_base_command".to_string()),
            account_note_detail: Some("fixture-wrapper".to_string()),
        }],
    }
}

fn fully_populated_accounts_result() -> AccountsResult {
    AccountsResult {
        accounts: vec![Account {
            tool: CliTool::Codex,
            id: "fixture-codex-account".to_string(),
            dir: PathBuf::from("/fixtures/codex-home"),
            identity: AccountIdentity {
                id: "fixture-provider-id".to_string(),
                label: "Fixture account".to_string(),
                display_name: Some("Fixture Developer".to_string()),
                organization: Some("Fixture Organization".to_string()),
                plan: Some("fixture-plan".to_string()),
                logged_in: true,
                usage_capable: true,
                credential_expires_at: Some(1_900_000_000),
            },
            is_default: true,
            is_process_default: true,
            usage: Some(UsageSnapshot {
                observed_at: Utc.with_ymd_and_hms(2026, 8, 30, 12, 0, 0).unwrap(),
                status: UsageStatus::Ok,
                windows: vec![UsageWindow {
                    key: "weekly".to_string(),
                    title: "Fixture week".to_string(),
                    used_percentage: 37.5,
                    resets_at: Some(1_900_000_100),
                    severity: Severity::Warning,
                    is_active: true,
                    compact: true,
                }],
                note: Some("Fixture usage is synthetic".to_string()),
            }),
        }],
        source: "native".to_string(),
        degraded: true,
        error: Some("Fixture degraded reason".to_string()),
        // Wire compatibility only: the backend never fills this — the
        // dedicated `resolve_launch_bases` command carries what the pane
        // shell makes of the configured commands.
        resolved_bases: Vec::new(),
    }
}

fn fully_populated_delivery_result() -> DeliveryResult {
    DeliveryResult {
        delivered: true,
        method: DeliveryMethod::InboxFile,
        durable: true,
        wake: WakeDisposition::Adopted { pid: 4242 },
        post_write_warnings: vec![
            "fixture operational context warning".to_string(),
            "fixture runtime state warning".to_string(),
        ],
    }
}

// Regenerate with:
// `cd src-tauri && UPDATE_IPC_FIXTURES=1 cargo test --lib -- ipc_fixtures`
#[test]
fn settings_fixture_matches_the_exported_contract() {
    assert_fixture("settings.json", &fully_populated_settings());
}

// Regenerate with:
// `cd src-tauri && UPDATE_IPC_FIXTURES=1 cargo test --lib -- ipc_fixtures`
#[test]
fn live_team_status_fixture_matches_the_exported_contract() {
    assert_fixture("live-team-status.json", &fully_populated_live_team_status());
}

// Regenerate with:
// `cd src-tauri && UPDATE_IPC_FIXTURES=1 cargo test --lib -- ipc_fixtures`
#[test]
fn accounts_result_fixture_matches_the_exported_contract() {
    assert_fixture("accounts-result.json", &fully_populated_accounts_result());
}

// Regenerate with:
// `cd src-tauri && UPDATE_IPC_FIXTURES=1 cargo test --lib -- ipc_fixtures`
#[test]
fn delivery_result_fixture_matches_the_exported_contract() {
    // This pins the Rust wire shape only; DeliveryResult has no JS consumer.
    assert_fixture("delivery-result.json", &fully_populated_delivery_result());
}
