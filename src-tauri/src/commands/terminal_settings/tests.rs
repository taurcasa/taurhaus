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

    reconcile_codex_compaction_at(&codex_home, CodexCompactionMode::Hooks, true, &exe)
        .expect("install hook");
    assert!(codex_home.join("hooks.json").exists());

    reconcile_codex_compaction_at(&codex_home, CodexCompactionMode::Transcript, true, &exe)
        .expect("remove hook");
    let hooks = std::fs::read_to_string(codex_home.join("hooks.json")).expect("hooks json");
    assert!(!hooks.contains("taurhaus-session-start-compact"));
}

#[test]
fn daemon_compaction_does_not_guess_mode_from_an_app_database_path() {
    // Regression: 6fe0aa3 made the WSL daemon guess the desktop app's SQLite path
    // and fail open to hooks, disabling transcript fallback on the shipping layout.
    let daemon_source = include_str!("../../daemon/compaction.rs");
    assert!(!daemon_source.contains("persisted_codex_compaction_mode"));
}
