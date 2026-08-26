use super::*;

use crate::models::CodexCompactionMode;

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
        true,
        &exe,
    )
    .expect("install hook");
    assert!(codex_home.join("hooks.json").exists());

    reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Transcript,
        true,
        true,
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
        true,
        &exe,
    )
    .expect("install supported hook");
    let changed = reconcile_codex_compaction_at_with_support(
        &codex_home,
        CodexCompactionMode::Hooks,
        true,
        false,
        &exe,
    )
    .expect("remove unsupported hook");

    assert!(changed);
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(!hooks.contains("taurhaus-session-start-compact"));
}

// Regression: 791f6be had no version-gated managed notify input, so adding the
// flag directly to LaunchSpec would also have rewritten unmanaged/user bases.
#[test]
fn codex_notify_input_requires_managed_launch_and_supported_version() {
    let daemon = std::path::Path::new("/home/test/.local/bin/taurhaus-daemon");
    let mut commands = crate::models::CliCommandSettings::default();

    apply_managed_codex_launch_inputs_with_support(&mut commands, true, false, false, daemon);
    assert!(commands.codex_notify_executable.is_none());

    apply_managed_codex_launch_inputs_with_support(&mut commands, false, false, true, daemon);
    assert!(commands.codex_notify_executable.is_none());

    apply_managed_codex_launch_inputs_with_support(&mut commands, true, false, true, daemon);
    assert_eq!(commands.codex_notify_executable.as_deref(), Some(daemon));
}

#[test]
fn daemon_compaction_does_not_guess_mode_from_an_app_database_path() {
    // Regression: 6fe0aa3 made the WSL daemon guess the desktop app's SQLite path
    // and fail open to hooks, disabling transcript fallback on the shipping layout.
    let daemon_source = include_str!("../../daemon/compaction.rs");
    assert!(!daemon_source.contains("persisted_codex_compaction_mode"));
}
