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
fn a_claude_only_launch_keeps_the_hook_needed_by_another_codex_team() {
    // Regression: d673af1 reconciled the host-global Codex hook from only the
    // team or agent being launched, so a Claude-only operation uninstalled the
    // hook while another managed Codex team was still live.
    let tmp = tempfile::tempdir().expect("tempdir");
    let teams_dir = tmp.path().join("teams");
    let codex_home = tmp.path().join("codex-home");
    let exe = tmp.path().join("taurhaus-daemon");
    std::fs::create_dir_all(&teams_dir).expect("teams dir");
    std::fs::write(&exe, b"daemon").expect("daemon fixture");
    write_team(&teams_dir, "codex-team", "codex");
    write_team(&teams_dir, "claude-team", "claude");

    reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
        .expect("install hook for the managed Codex team");
    reconcile_codex_hook_for_managed_launch_at(&teams_dir, &codex_home, false, Some(true), &exe)
        .expect("reconcile before the Claude-only launch");

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

/// The Codex compact hook lives in one host-global `<CODEX_HOME>/hooks.json`,
/// so every managed launch has to reconcile it against the whole roster and not
/// just the team it is about to touch.
///
// Regression: commit 7ada241 introduced the roster-wide reconciler
// (`reconcile_codex_hook_for_managed_launch`) because d673af1 reconciled that
// one global file from the operated team alone. A non-Codex team operation
// therefore uninstalled the hook while another team's Codex member was still
// live, and that member silently lost compaction reinjection for the rest of
// its session. Coordination has exactly four managed-launch entry points and
// all four are affected, so this module pins both halves: each site's own
// `has_codex` derivation run through the roster-wide reconciler leaves a live
// member's hook alone, and each app- or daemon-owned site is wired to it.
mod managed_launch_sites {
    use super::*;

    use crate::coordination::compact_hook::{
        codex_compact_hook_is_installed_at, team_has_managed_codex_member,
    };
    use crate::session_scanner::cli_tool::{spec, CliTool};

    /// The five `#[tauri::command]` entry points in `commands/coordination.rs`
    /// that launch or resume a managed pane.
    const LAUNCH_SITES: [&str; 5] = [
        "coordination_initialize_team",
        "coordination_add_agent",
        "coordination_resume_member",
        "coordination_resume_team",
        "coordination_switch_team_account",
    ];

    /// A host running two teams: one with a live Codex member that owns the
    /// installed global hook, one with only a Claude member. Nothing here
    /// touches a real `~/.codex`, `~/.claude` or `~/.grok`.
    struct Host {
        _tmp: tempfile::TempDir,
        teams_dir: std::path::PathBuf,
        codex_home: std::path::PathBuf,
        exe: std::path::PathBuf,
    }

    fn host_with_a_live_codex_member() -> Host {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let codex_home = tmp.path().join("codex-home");
        let exe = tmp.path().join("taurhaus-daemon");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        std::fs::write(&exe, b"daemon").expect("daemon fixture");
        write_team(&teams_dir, "codex-team", "codex");
        write_team(&teams_dir, "claude-team", "claude");

        reconcile_codex_hook_at_with_support(&codex_home, true, Some(true), &exe)
            .expect("install the hook the live Codex member needs");
        assert!(codex_compact_hook_is_installed_at(&codex_home));

        Host {
            _tmp: tmp,
            teams_dir,
            codex_home,
            exe,
        }
    }

    /// `coordination_initialize_team` and `coordination_add_agent` derive
    /// `has_codex` from the *requested* member, before that member is on any
    /// roster.
    fn requested_member_has_codex(cli_tool: &str) -> bool {
        CliTool::from_alias(cli_tool).is_ok_and(|tool| spec(tool).capabilities.hook_trust)
    }

    /// What one launch site passes as its `has_codex` argument when the
    /// operation targets the Claude-only team. `resume_member`/`resume_team`
    /// read the operated team's own roster instead of a requested member.
    fn has_codex_for(site: &str, teams_dir: &std::path::Path) -> bool {
        match site {
            "coordination_initialize_team" | "coordination_add_agent" => {
                requested_member_has_codex("claude")
            }
            _ => team_has_managed_codex_member(teams_dir, "claude-team")
                .expect("read the operated team's roster"),
        }
    }

    #[test]
    fn no_launch_site_uninstalls_a_live_codex_members_hook() {
        let mut offenders = Vec::new();
        for site in LAUNCH_SITES {
            // A fresh host per site, so one site's outcome cannot mask the next.
            let host = host_with_a_live_codex_member();
            let has_codex = has_codex_for(site, &host.teams_dir);
            assert!(
                !has_codex,
                "{site} operates on the Claude-only team, so its own signal is false"
            );

            reconcile_codex_hook_for_managed_launch_at(
                &host.teams_dir,
                &host.codex_home,
                has_codex,
                Some(true),
                &host.exe,
            )
            .unwrap_or_else(|error| panic!("{site}: reconcile before the launch: {error}"));
            if !codex_compact_hook_is_installed_at(&host.codex_home) {
                offenders.push(site);
            }
        }

        assert!(
            offenders.is_empty(),
            "these launch sites uninstalled the hook the live codex-team member still needs: {offenders:?}"
        );
    }

    #[test]
    fn the_last_codex_team_still_takes_the_hook_away() {
        // The roster-wide reconciler must not degrade into "never uninstall":
        // once no team runs Codex, a launch has to clean the global file up.
        let mut offenders = Vec::new();
        for site in LAUNCH_SITES {
            let host = host_with_a_live_codex_member();
            let has_codex = has_codex_for(site, &host.teams_dir);
            std::fs::remove_dir_all(host.teams_dir.join("codex-team"))
                .expect("disband the Codex team");

            reconcile_codex_hook_for_managed_launch_at(
                &host.teams_dir,
                &host.codex_home,
                has_codex,
                Some(true),
                &host.exe,
            )
            .unwrap_or_else(|error| panic!("{site}: reconcile before the launch: {error}"));
            if codex_compact_hook_is_installed_at(&host.codex_home) {
                offenders.push(site);
            }
        }

        assert!(
            offenders.is_empty(),
            "these launch sites left the global hook behind after the last Codex member went away: {offenders:?}"
        );
    }

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
    fn every_launch_site_is_wired_to_the_roster_wide_reconciler() {
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
                .contains("terminal_settings::managed_codex_hook_trust_for_launch("),
            "daemon-owned launches must call the roster-wide reconciler locally"
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
