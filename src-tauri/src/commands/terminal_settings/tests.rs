use super::*;

use crate::session_scanner::cli_tool::all;

// Regression: 6fe0aa3 selected the current process on non-Windows hosts, so
// app and daemon hook reconciliation rewrote the script back and forth.
#[test]
fn compact_hook_writers_share_the_daemon_executable_on_every_platform() {
    assert_eq!(
        compact_hook_executable().expect("compact hook executable"),
        crate::provider::platform_paths::PlatformPaths::daemon_binary_path()
    );
}

// Regression: 6fe0aa3 installed the Codex compact hook without checking the
// installed CLI, even though the hook contract starts at Codex 0.147.
#[test]
fn unsupported_codex_version_removes_hook_instead_of_installing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
        .expect("install supported hook");
    let changed = reconcile_codex_hook_at_with_support(&codex_home, true, Some(false), &exe)
        .expect("remove unsupported hook");

    assert!(changed);
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(!hooks.contains("taurhaus-session-start-compact"));
}

// Regression: 61e9a24 collapsed an unresolved Codex probe into `false`, so an
// app startup could delete a valid PR 9 hook without proving Codex was old.
#[test]
fn unknown_codex_version_leaves_existing_hook_untouched() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
        .expect("install supported hook");
    let changed = reconcile_codex_hook_at_with_support(&codex_home, true, None, &exe)
        .expect("leave hook untouched when the version is unknown");

    assert!(!changed);
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(hooks.contains("taurhaus-session-start-compact"));
}

#[test]
fn unknown_codex_version_without_a_visible_member_leaves_the_hook_untouched() {
    // Regression: d673af1 made the no-managed-member arm remove the hook even
    // when the Codex version probe was unavailable, treating missing evidence
    // as proof that an installed host-global hook was obsolete.
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
        .expect("install supported hook");
    let changed = reconcile_codex_hook_at_with_support(&codex_home, false, None, &exe)
        .expect("leave hook untouched when the version is unknown");

    assert!(!changed);
    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&codex_home));
}

#[test]
fn the_last_codex_member_removes_the_managed_hook() {
    // Regression: 1615cea collapsed Codex hook reconciliation but left the
    // no-managed-member arm as a no-op, so disbanding the last Codex team left
    // taurhaus running from the user's hooks.json on every future session.
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
        .expect("install for the first managed Codex member");
    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&codex_home));

    let changed = reconcile_codex_hook_at_with_support(&codex_home, false, Some(true), &exe)
        .expect("remove after the last managed Codex member");
    assert!(changed);
    assert!(!crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&codex_home));
}

// Regression: 791f6be had no version-gated managed notify input, so adding the
// flag directly to LaunchSpec would also have rewritten unmanaged/user bases.
#[test]
fn codex_notify_input_requires_managed_launch_and_supported_version() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let daemon = tmp.path().join("taurhaus-daemon");
    std::fs::write(&daemon, b"daemon").expect("daemon fixture");
    let mut commands = crate::models::CliCommandSettings::default();

    apply_managed_codex_launch_inputs_with_support(
        &mut commands,
        true,
        false,
        false,
        false,
        &daemon,
    );
    assert!(commands.codex_notify_executable.is_none());

    apply_managed_codex_launch_inputs_with_support(
        &mut commands,
        false,
        false,
        true,
        false,
        &daemon,
    );
    assert!(commands.codex_notify_executable.is_none());

    apply_managed_codex_launch_inputs_with_support(
        &mut commands,
        true,
        false,
        true,
        false,
        &daemon,
    );
    assert_eq!(
        commands.codex_notify_executable.as_deref(),
        Some(daemon.as_path())
    );
}

#[test]
fn managed_codex_selector_is_inserted_and_removed_by_registry_capability() {
    // Regression: commit 2c49132 inferred the managed home from hook trust and
    // notify support, so an unrelated capability change could select the
    // wrong tool's account directory.
    let root = tempfile::tempdir().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let selector = all()
        .iter()
        .find(|entry| entry.capabilities.managed_home)
        .and_then(|entry| entry.capabilities.account_selector)
        .expect("Codex selector capability");
    let mut commands = crate::models::CliCommandSettings::default();

    apply_managed_account_selector(&mut commands, true, codex_home.clone());
    assert_eq!(
        commands.account_selector_dirs.get(selector),
        Some(&codex_home)
    );

    apply_managed_account_selector(&mut commands, false, codex_home);
    assert!(!commands.account_selector_dirs.contains_key(selector));
}

// Regression: 61e9a24 rendered a notifier path without checking that the
// separately installed daemon existed, making every turn fail silently.
#[test]
fn codex_notify_input_skips_missing_daemon_executable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("missing-taurhaus-daemon");
    let mut commands = crate::models::CliCommandSettings::default();

    apply_managed_codex_launch_inputs_with_support(
        &mut commands,
        true,
        false,
        true,
        false,
        &missing,
    );

    assert!(commands.codex_notify_executable.is_none());
}

// Regression: 61e9a24 only inspected the launch base, so its per-launch `-c`
// silently replaced a notifier already selected in CODEX_HOME/config.toml.
#[test]
fn codex_notify_input_preserves_user_config_toml_notify() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let daemon = tmp.path().join("taurhaus-daemon");
    std::fs::write(&daemon, b"daemon").expect("daemon fixture");
    let config = tmp.path().join("config.toml");
    std::fs::write(&config, "notify = [\"my-notifier\"]\n").expect("Codex config fixture");
    let mut commands = crate::models::CliCommandSettings::default();

    apply_managed_codex_launch_inputs_with_support(
        &mut commands,
        true,
        false,
        true,
        codex_config_has_notify(&config).expect("parse Codex config"),
        &daemon,
    );

    assert!(commands.codex_notify_executable.is_none());
}

fn write_team(teams_dir: &std::path::Path, team_name: &str, cli_tool: &str) {
    let dir = teams_dir.join(team_name);
    std::fs::create_dir_all(&dir).expect("team dir");
    std::fs::write(
        dir.join("config.json"),
        format!(
            r#"{{"name":"{team_name}","createdAt":1772399806546,"members":[{{"name":"builder","agentType":"general-purpose","cli_tool":"{cli_tool}","cwd":"/tmp/project"}}]}}"#
        ),
    )
    .expect("write team config");
}

fn write_team_with_account(
    teams_dir: &std::path::Path,
    team_name: &str,
    cli_tool: &str,
    account_id: &str,
) {
    let dir = teams_dir.join(team_name);
    std::fs::create_dir_all(&dir).expect("team dir");
    std::fs::write(
        dir.join("config.json"),
        format!(
            r#"{{"name":"{team_name}","createdAt":1772399806546,"members":[{{"name":"builder","agentType":"general-purpose","cli_tool":"{cli_tool}","cwd":"/tmp/project","accountId":"{account_id}"}}]}}"#
        ),
    )
    .expect("write team config");
}

#[test]
fn the_first_grok_team_installs_the_hook_and_the_last_removal_takes_it_away() {
    // Regression: commit c1005ec reconciled the global grok hook only at startup
    // and on a Settings save, so a team created afterwards ran without the hook
    // until the next restart, and removing the last grok member left it behind.
    let tmp = tempfile::tempdir().expect("tempdir");
    let teams_dir = tmp.path().join("teams");
    let grok_home = tmp.path().join("grok-home");
    let exe = tmp.path().join("taurhaus");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"fixture").expect("executable fixture");

    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe).expect("no grok member");
    assert!(!crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home));

    write_team(&teams_dir, "grok-team", "grok");
    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe).expect("first team");
    assert!(crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home));

    std::fs::remove_dir_all(teams_dir.join("grok-team")).expect("disband the team");
    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe).expect("last removal");
    assert!(!crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home));
}

#[test]
fn a_grok_team_whose_config_cannot_be_parsed_never_uninstalls_the_hook() {
    // Regression: commit c1005ec logged and swallowed each failed per-team
    // config load during managed-member discovery and answered `false`, so a
    // single malformed `config.json` — the team's own — read as "no grok
    // members left" and uninstalled the hook a live grok session needed.
    let tmp = tempfile::tempdir().expect("tempdir");
    let teams_dir = tmp.path().join("teams");
    let grok_home = tmp.path().join("grok-home");
    let exe = tmp.path().join("taurhaus");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"fixture").expect("executable fixture");
    write_team(&teams_dir, "grok-team", "grok");
    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe).expect("install");

    // The team is still listed; only its config is unreadable.
    std::fs::write(
        teams_dir.join("grok-team").join("config.json"),
        b"{ this is not json",
    )
    .expect("corrupt the team config");

    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe)
        .expect_err("an unparseable team config is a discovery failure, not an empty roster");
    assert!(
        crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home),
        "a team whose config cannot be read is not proof its grok member is gone"
    );
}

#[test]
fn a_roster_that_cannot_be_read_never_uninstalls_the_grok_hook() {
    // Regression: commit c1005ec turned a failed managed-member discovery into
    // "no grok members" (`.ok().unwrap_or(false)`), so an unreadable team
    // directory silently uninstalled a hook a live grok member still needed.
    let tmp = tempfile::tempdir().expect("tempdir");
    let teams_dir = tmp.path().join("teams");
    let grok_home = tmp.path().join("grok-home");
    let exe = tmp.path().join("taurhaus");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"fixture").expect("executable fixture");
    write_team(&teams_dir, "grok-team", "grok");
    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe).expect("install");

    // A file where the team directory belongs makes discovery fail outright.
    std::fs::remove_dir_all(&teams_dir).expect("clear teams dir");
    std::fs::write(&teams_dir, b"not a directory").expect("block the teams dir");

    reconcile_grok_hooks_for_roster_at(&teams_dir, &grok_home, true, &exe)
        .expect_err("discovery failure is reported, not answered with false");
    assert!(
        crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home),
        "an unreadable roster is not proof the last grok member is gone"
    );
}

#[test]
fn agy_hooks_reconciliation_follows_the_cli_version_gate() {
    // Regression: 4e9e2c5 installed the Antigravity hook sink for any CLI
    // version, but Stop hooks are unreachable before agy 1.1.10.
    use crate::coordination::agy_hooks_installer::agy_hooks_installed_at;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".gemini");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    assert!(!reconcile_agy_hooks_at(&root, true, Some(false), &exe).expect("unsupported agy"));
    assert!(!agy_hooks_installed_at(&root));

    assert!(!reconcile_agy_hooks_at(&root, true, None, &exe).expect("unknown agy version"));
    assert!(!agy_hooks_installed_at(&root));

    assert!(reconcile_agy_hooks_at(&root, true, Some(true), &exe).expect("supported agy"));
    assert!(agy_hooks_installed_at(&root));

    assert!(reconcile_agy_hooks_at(&root, false, Some(true), &exe).expect("setting turned off"));
    assert!(!agy_hooks_installed_at(&root));
}

#[test]
fn an_unsupported_agy_removes_hooks_a_newer_cli_installed() {
    // Regression: 4e9e2c5 had no gate at all, so a downgrade left a hook that
    // the running CLI can register but never fire, pinning sessions busy.
    use crate::coordination::agy_hooks_installer::agy_hooks_installed_at;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".gemini");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    reconcile_agy_hooks_at(&root, true, Some(true), &exe).expect("install on a supported agy");

    assert!(reconcile_agy_hooks_at(&root, true, Some(false), &exe).expect("downgraded agy"));
    assert!(!agy_hooks_installed_at(&root));
}

#[test]
fn an_unknown_agy_version_leaves_an_installed_hook_alone() {
    // Regression: a transient version-probe failure must not uninstall the
    // hook a live agy session is relying on for its idle edge.
    use crate::coordination::agy_hooks_installer::agy_hooks_installed_at;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join(".gemini");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    reconcile_agy_hooks_at(&root, true, Some(true), &exe).expect("install on a supported agy");

    assert!(!reconcile_agy_hooks_at(&root, true, None, &exe).expect("unknown agy version"));
    assert!(agy_hooks_installed_at(&root));
}

/// Managed launches reconcile hooks in every account home the daemon host knows
/// for a launched tool — installing where a member still launches, removing
/// where none does. Startup and the roster reconcilers only ever visit each
/// tool's *default* home, so they cannot stand in for that; the removal contract
/// itself is pinned by `a_launch_removes_the_codex_hook_no_roster_member_still_needs`
/// and its Grok twin. This module only pins that each site is wired to the
/// account-home reconciler in the first place.
mod managed_launch_sites {
    /// The five `#[tauri::command]` entry points in `commands/coordination.rs`
    /// that launch or resume a managed pane.
    const LAUNCH_SITES: [&str; 5] = [
        "coordination_initialize_team",
        "coordination_add_agent",
        "coordination_resume_member",
        "coordination_resume_team",
        "coordination_switch_team_account",
    ];

    fn coordination_source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/coordination.rs"),
        )
        .expect("commands/coordination.rs is readable")
    }

    fn daemon_initialize_source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/initialize_runs.rs"),
        )
        .expect("daemon/initialize_runs.rs is readable")
    }

    fn daemon_coordination_runs_source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/daemon/coordination_runs.rs"),
        )
        .expect("daemon/coordination_runs.rs is readable")
    }

    fn daemon_team_runs_source() -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/team_runs.rs"),
        )
        .expect("daemon/team_runs.rs is readable")
    }

    /// The body of one top-level `fn <name>`, up to its closing brace in
    /// column 0.
    fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
        let needle = format!("fn {name}(");
        let start = source
            .find(&needle)
            .unwrap_or_else(|| panic!("{name} is defined in commands/coordination.rs"));
        let rest = &source[start..];
        let end = rest
            .find("\n}\n")
            .unwrap_or_else(|| panic!("{name} has a top-level closing brace"));
        &rest[..end]
    }

    #[test]
    fn every_launch_site_is_wired_to_the_account_home_reconciler() {
        let source = coordination_source();

        for site in LAUNCH_SITES {
            let body = function_body(&source, site);
            assert!(
                !body.contains("terminal_settings::reconcile_codex_hook("),
                "{site} must leave host-local hook reconciliation to the daemon"
            );
        }

        let daemon_source = daemon_initialize_source();
        let daemon_prepare = function_body(&daemon_source, "prepare_daemon_launch_inputs");
        assert!(
            daemon_prepare.contains("prepare_daemon_launch_inputs_for_tools("),
            "daemon-owned initialization must use the shared launch input preparation"
        );
        let shared_daemon_source = daemon_coordination_runs_source();
        let shared_daemon_prepare = function_body(
            &shared_daemon_source,
            "prepare_daemon_launch_inputs_for_tools",
        );
        assert!(
            shared_daemon_prepare
                .contains("terminal_settings::reconcile_managed_account_hooks_for_launch("),
            "daemon-owned launches must reconcile every selected account home locally"
        );
        let daemon_team_source = daemon_team_runs_source();
        let daemon_team_prepare =
            function_body(&daemon_team_source, "prepare_resume_team_launch_inputs");
        assert!(
            daemon_team_prepare.contains("prepare_daemon_launch_inputs_for_tools("),
            "daemon-owned team resume must use the shared launch input preparation"
        );
        let initialize_command = function_body(&source, "coordination_initialize_team");
        assert!(
            initialize_command.contains("initialize_team_through_daemon("),
            "the app-owned initialize entry point must route to the daemon host"
        );
        let daemon_client = function_body(&source, "initialize_team_through_daemon_with");
        assert!(daemon_client.contains("COORDINATION_INITIALIZE_TEAM"));
    }
}

// Regression: 0bc79ceb launched a switched Codex member from its selected
// CODEX_HOME but left the compact hook in the previous account home.
#[test]
fn account_switch_moves_the_codex_hook_to_the_selected_home() {
    use crate::coordination::compact_hook::codex_compact_hook_is_installed_at;
    use crate::session_scanner::cli_tool::CompactionDelivery;

    let temp = tempfile::tempdir().expect("tempdir");
    let personal = temp.path().join("codex-personal");
    let work = temp.path().join("codex-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    reconcile_codex_hook_at_with_support(&personal, true, Some(true), &exe)
        .expect("seed personal hook");

    reconcile_account_switch_hooks_at(
        AccountSwitchHookContext {
            teams_dir: &temp.path().join("teams"),
            team_name: "switching-team",
            cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
            delivery: CompactionDelivery::HookStdout,
            accounts: &[],
            codex_hooks_supported: Some(true),
            grok_enabled: true,
            taurhaus_exe: &exe,
        },
        &work,
        std::slice::from_ref(&personal),
    )
    .expect("move hook");

    assert!(codex_compact_hook_is_installed_at(&work));
    assert!(!codex_compact_hook_is_installed_at(&personal));
}

// Regression: 0bc79ceb had the same split-home failure for Grok: GROK_HOME
// moved with the member while the native hook stayed in the prior home.
#[test]
fn account_switch_moves_the_enabled_grok_hook_to_the_selected_home() {
    use crate::coordination::compact_hook::grok_compact_hook_is_installed_at;
    use crate::session_scanner::cli_tool::CompactionDelivery;

    let temp = tempfile::tempdir().expect("tempdir");
    let personal = temp.path().join("grok-personal");
    let work = temp.path().join("grok-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    reconcile_grok_hooks_at(&personal, true, true, &exe).expect("seed personal hook");

    reconcile_account_switch_hooks_at(
        AccountSwitchHookContext {
            teams_dir: &temp.path().join("teams"),
            team_name: "switching-team",
            cli_tool: crate::session_scanner::cli_tool::CliTool::Grok,
            delivery: CompactionDelivery::MeshInbox,
            accounts: &[],
            codex_hooks_supported: Some(true),
            grok_enabled: true,
            taurhaus_exe: &exe,
        },
        &work,
        std::slice::from_ref(&personal),
    )
    .expect("move hook");

    assert!(grok_compact_hook_is_installed_at(&work));
    assert!(!grok_compact_hook_is_installed_at(&personal));
}

// Regression: 0bc79ceb pinned a managed Codex member to its selected
// CODEX_HOME while launch-time reconciliation still installed only in the
// default home, so compaction reinjection silently disappeared.
#[test]
fn managed_launch_installs_the_codex_hook_in_the_selected_home() {
    use crate::models::ManagedLaunchAccount;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let work = temp.path().join("codex-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        crate::session_scanner::cli_tool::CliTool::Codex,
        vec![ManagedLaunchAccount {
            id: "codex-work".to_string(),
            label: "Work".to_string(),
            dir: work.clone(),
            logged_in: true,
            is_default: false,
        }],
    );

    let trusted = reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(
            crate::session_scanner::cli_tool::CliTool::Codex,
            Some("codex-work".to_string()),
        )],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile selected home");

    assert!(trusted);
    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&work));
}

// Regression: 0bc79ceb also pinned managed Grok members to a selected
// GROK_HOME without installing the inbox-delivery hook in that home.
#[test]
fn managed_launch_installs_the_enabled_grok_hook_in_the_selected_home() {
    use crate::models::ManagedLaunchAccount;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let work = temp.path().join("grok-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        crate::session_scanner::cli_tool::CliTool::Grok,
        vec![ManagedLaunchAccount {
            id: "grok-work".to_string(),
            label: "Work".to_string(),
            dir: work.clone(),
            logged_in: true,
            is_default: false,
        }],
    );

    reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(
            crate::session_scanner::cli_tool::CliTool::Grok,
            Some("grok-work".to_string()),
        )],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile selected home");

    assert!(crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&work));
}

// Regression: 96f69205 inferred Grok enablement from leftover hook files,
// allowing a switch intent to resurrect a setting the operator disabled.
#[test]
fn disabled_grok_hook_setting_survives_the_daemon_settings_payload() {
    let commands = crate::models::CliCommandSettings {
        grok_hooks_enabled: Some(false),
        ..Default::default()
    };

    let wire = serde_json::to_string(&commands).expect("serialize daemon launch settings");
    let decoded: crate::models::CliCommandSettings =
        serde_json::from_str(&wire).expect("deserialize daemon launch settings");

    assert_eq!(decoded.grok_hooks_enabled, Some(false));
}

// Regression: 96f69205 removed the hook from the switching team's previous
// home without checking another team's members that still launch there.
#[test]
fn account_switch_keeps_the_previous_hook_when_another_team_uses_that_home() {
    use crate::models::ManagedLaunchAccount;
    use crate::session_scanner::cli_tool::{CliTool, CompactionDelivery};

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let personal = temp.path().join("codex-personal");
    let work = temp.path().join("codex-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    write_team(&teams_dir, "team-a", "codex");
    write_team(&teams_dir, "team-b", "codex");
    reconcile_codex_hook_at_with_support(&personal, true, Some(true), &exe)
        .expect("seed shared hook");
    let accounts = vec![
        ManagedLaunchAccount {
            id: "personal".to_string(),
            label: "Personal".to_string(),
            dir: personal.clone(),
            logged_in: true,
            is_default: true,
        },
        ManagedLaunchAccount {
            id: "work".to_string(),
            label: "Work".to_string(),
            dir: work.clone(),
            logged_in: true,
            is_default: false,
        },
    ];

    reconcile_account_switch_hooks_at(
        AccountSwitchHookContext {
            teams_dir: &teams_dir,
            team_name: "team-a",
            cli_tool: CliTool::Codex,
            delivery: CompactionDelivery::HookStdout,
            accounts: &accounts,
            codex_hooks_supported: Some(true),
            grok_enabled: true,
            taurhaus_exe: &exe,
        },
        &work,
        std::slice::from_ref(&personal),
    )
    .expect("move one team");

    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&work));
    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&personal));
}

// Regression: 59ff36ee replaced the two-way launch reconciliation with an
// install-only pass over the resolved account homes. The launch could no longer
// uninstall anything, and every remaining remover is default-home-only, so a
// hook taurhaus wrote into a non-default CODEX_HOME outlived the team that
// needed it and kept invoking the daemon from the user's own sessions there.
#[test]
fn a_launch_removes_the_codex_hook_no_roster_member_still_needs() {
    use crate::models::ManagedLaunchAccount;
    use crate::session_scanner::cli_tool::CliTool;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let personal = temp.path().join("codex-personal");
    let work = temp.path().join("codex-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    // The surviving team runs on Personal; the Work home is what a disbanded
    // team left behind.
    write_team_with_account(&teams_dir, "team-b", "codex", "personal");
    reconcile_codex_hook_at_with_support(&personal, true, Some(true), &exe)
        .expect("seed the live home");
    reconcile_codex_hook_at_with_support(&work, true, Some(true), &exe)
        .expect("seed the disbanded team's home");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![
            ManagedLaunchAccount {
                id: "personal".to_string(),
                label: "Personal".to_string(),
                dir: personal.clone(),
                logged_in: true,
                is_default: true,
            },
            ManagedLaunchAccount {
                id: "work".to_string(),
                label: "Work".to_string(),
                dir: work.clone(),
                logged_in: true,
                is_default: false,
            },
        ],
    );

    reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(CliTool::Codex, Some("personal".to_string()))],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile every known account home");

    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&personal));
    assert!(!crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&work));
}

// Regression: 59ff36ee left the Grok arm of the launch reconciler install-only
// too, so an account home no roster member launches from kept the taurhaus
// inbox-delivery hook forever.
#[test]
fn a_launch_removes_the_grok_hook_no_roster_member_still_needs() {
    use crate::models::ManagedLaunchAccount;
    use crate::session_scanner::cli_tool::CliTool;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let personal = temp.path().join("grok-personal");
    let work = temp.path().join("grok-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    write_team_with_account(&teams_dir, "team-b", "grok", "personal");
    reconcile_grok_hooks_at(&personal, true, true, &exe).expect("seed the live home");
    reconcile_grok_hooks_at(&work, true, true, &exe).expect("seed the disbanded team's home");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Grok,
        vec![
            ManagedLaunchAccount {
                id: "personal".to_string(),
                label: "Personal".to_string(),
                dir: personal.clone(),
                logged_in: true,
                is_default: true,
            },
            ManagedLaunchAccount {
                id: "work".to_string(),
                label: "Work".to_string(),
                dir: work.clone(),
                logged_in: true,
                is_default: false,
            },
        ],
    );

    reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(CliTool::Grok, Some("personal".to_string()))],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile every known account home");

    assert!(crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&personal));
    assert!(!crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&work));
}

// Regression: 59ff36ee's install-only pass had no removal to guard, so nothing
// pinned the conservative arm the switch reconciler already honours
// (`managed_home_needed_after_switch` answers "still needed" for a member whose
// home cannot be resolved). A member pinned to an account detection no longer
// lists is missing evidence, not proof that an installed hook is obsolete.
#[test]
fn an_unresolvable_roster_member_keeps_every_account_hook_installed() {
    use crate::models::ManagedLaunchAccount;
    use crate::session_scanner::cli_tool::CliTool;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let work = temp.path().join("codex-work");
    let archive = temp.path().join("codex-archive");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    write_team_with_account(&teams_dir, "team-b", "codex", "vanished");
    reconcile_codex_hook_at_with_support(&archive, true, Some(true), &exe)
        .expect("seed the home the unresolvable member may still be using");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![
            ManagedLaunchAccount {
                id: "work".to_string(),
                label: "Work".to_string(),
                dir: work.clone(),
                logged_in: true,
                is_default: false,
            },
            ManagedLaunchAccount {
                id: "archive".to_string(),
                label: "Archive".to_string(),
                dir: archive.clone(),
                logged_in: true,
                is_default: false,
            },
        ],
    );

    reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(CliTool::Codex, Some("work".to_string()))],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile with an unresolvable roster member");

    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&work));
    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&archive));
}

/// A member runtime record that says the session is alive and names the
/// account home its launch actually used.
fn live_runtime_record(
    member_name: &str,
    account_id: &str,
) -> crate::coordination::stores::MemberRuntimeRecord {
    crate::coordination::stores::MemberRuntimeRecord {
        schema_version: 3,
        member_name: member_name.to_string(),
        cli_tool: None,
        project_path: None,
        pane_id: Some("%1".to_string()),
        pane_pid: None,
        pane_start_time: None,
        session_id: Some(format!("session-{member_name}")),
        jsonl_path: None,
        daemon_pid: None,
        health: crate::coordination::domain::HealthState::Healthy,
        delivery_lease: None,
        attached_at: None,
        last_seen_at: None,
        applied_effort: None,
        effort_resume_failure: None,
        launch_account: taurhaus_lib::session_scanner::launch_base::LaunchAccountResult {
            account_applied: Some(true),
            account_id: Some(account_id.to_string()),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

// Regression: 30bc9b90 re-derived every roster member's hook home from its
// configured account id plus current detection, so a member already running in
// Work was reassigned to the detected default the moment Work stopped
// reporting `logged_in`, and the next same-tool launch removed the hook the
// live Work session still needs.
#[test]
fn a_live_runtime_keeps_the_hook_in_the_home_it_launched_from() {
    use crate::coordination::stores::MemberRuntimeStore;
    use crate::models::ManagedLaunchAccount;
    use crate::session_scanner::cli_tool::CliTool;

    let temp = tempfile::tempdir().expect("tempdir");
    let teams_dir = temp.path().join("teams");
    let personal = temp.path().join("codex-personal");
    let work = temp.path().join("codex-work");
    let exe = temp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    write_team_with_account(&teams_dir, "team-a", "codex", "work");
    write_team_with_account(&teams_dir, "team-b", "codex", "personal");
    MemberRuntimeStore::save(
        &teams_dir,
        "team-a",
        "builder",
        &live_runtime_record("builder", "work"),
    )
    .expect("seed the live Work session");
    reconcile_codex_hook_at_with_support(&personal, true, Some(true), &exe)
        .expect("seed the default home");
    reconcile_codex_hook_at_with_support(&work, true, Some(true), &exe)
        .expect("seed the live session's home");
    let mut commands = crate::models::CliCommandSettings::default();
    commands.managed_accounts.insert(
        CliTool::Codex,
        vec![
            ManagedLaunchAccount {
                id: "personal".to_string(),
                label: "Personal".to_string(),
                dir: personal.clone(),
                logged_in: true,
                is_default: true,
            },
            ManagedLaunchAccount {
                id: "work".to_string(),
                label: "Work".to_string(),
                dir: work.clone(),
                logged_in: false,
                is_default: false,
            },
        ],
    );

    reconcile_managed_account_hooks_for_launch_at(
        &teams_dir,
        &[(CliTool::Codex, Some("personal".to_string()))],
        &commands,
        Some(true),
        true,
        &exe,
    )
    .expect("reconcile while a live session runs in a signed-out account");

    assert!(crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&personal));
    assert!(
        crate::coordination::compact_hook::codex_compact_hook_is_installed_at(&work),
        "a live runtime that launched in Work still needs its hook"
    );
}
