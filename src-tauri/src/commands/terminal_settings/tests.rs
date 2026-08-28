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
