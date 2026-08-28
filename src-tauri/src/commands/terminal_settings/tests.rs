use super::*;

use crate::models::CodexCompactionMode;
use crate::session_scanner::cli_tool::all;

// Regression: 0b87699 had no setting transition that could remove the Codex
// hook and restore the transcript fallback without touching a real home.
#[test]
fn transcript_setting_removes_the_isolated_codex_hook() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        Some(true),
        &exe,
    )
    .expect("install hook");
    assert!(codex_home.join("hooks.json").exists());

    reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Transcript,
        true,
        Some(true),
        &exe,
    )
    .expect("remove hook");
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(!hooks.contains("taurhaus-session-start-compact"));
}

// Regression: 6fe0aa3 installed the Codex compact hook without checking the
// installed CLI, even though the hook contract starts at Codex 0.147.
#[test]
fn unsupported_codex_version_removes_hook_instead_of_installing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");

    reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        Some(true),
        &exe,
    )
    .expect("install supported hook");
    let changed = reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        Some(false),
        &exe,
    )
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

    reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        Some(true),
        &exe,
    )
    .expect("install supported hook");
    let changed = reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        None,
        &exe,
    )
    .expect("leave hook untouched when the version is unknown");

    assert!(!changed);
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(hooks.contains("taurhaus-session-start-compact"));
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

#[test]
fn daemon_compaction_does_not_guess_mode_from_an_app_database_path() {
    // Regression: 6fe0aa3 made the WSL daemon guess the desktop app's SQLite path
    // and fail open to hooks, disabling transcript fallback on the shipping layout.
    let daemon_source = include_str!("../../daemon/compaction.rs");
    assert!(!daemon_source.contains("persisted_codex_compaction_mode"));
}

fn write_grok_team(teams_dir: &std::path::Path, team_name: &str, cli_tool: &str) {
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

    write_grok_team(&teams_dir, "grok-team", "grok");
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
    write_grok_team(&teams_dir, "grok-team", "grok");
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
    write_grok_team(&teams_dir, "grok-team", "grok");
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
