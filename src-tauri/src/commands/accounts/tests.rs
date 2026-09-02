use super::*;
use crate::db::queries;
use crate::session_scanner::accounts::{install_detection_override, AccountIdentity, AccountScan};
use std::path::Path;
use std::sync::Mutex;
use tempfile::{NamedTempFile, TempDir};

fn db_with_project(project_id: &str) -> (DbState, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("temp db");
    let conn = crate::db::init_db(tmp.path()).expect("init db");
    let now = chrono::Utc::now().to_rfc3339();
    queries::insert_project(
        &conn,
        &crate::models::Project {
            id: project_id.to_string(),
            name: "test-project".to_string(),
            path: "/home/user/projects/test-project".to_string(),
            description: None,
            last_activity_at: None,
            hero_preference: None,
            created_at: now.clone(),
            updated_at: now,
            cached_branch: None,
            cached_is_dirty: None,
            account_memory: Default::default(),
        },
    )
    .expect("insert project");
    (DbState(Mutex::new(conn)), tmp)
}

fn stored_account(db: &DbState, project_id: &str) -> Option<String> {
    let conn = db.0.lock().expect("db lock");
    queries::get_project(&conn, project_id)
        .expect("get project")
        .expect("project exists")
        .account_memory
        .get("claude")
        .map(|memory| memory.account_id.clone())
}

#[test]
fn setting_and_clearing_a_project_account_round_trips() {
    let (db, _tmp) = db_with_project("p1");

    set_project_account_impl(&db, "p1", CliTool::Claude, Some("account-2")).expect("set account");
    assert_eq!(stored_account(&db, "p1").as_deref(), Some("account-2"));

    set_project_account_impl(&db, "p1", CliTool::Claude, None).expect("clear account");
    assert_eq!(stored_account(&db, "p1"), None);
}

#[test]
fn setting_the_account_of_an_unknown_project_is_an_error() {
    let (db, _tmp) = db_with_project("p1");

    let error = set_project_account_impl(&db, "missing", CliTool::Claude, Some("account-2"))
        .expect_err("unknown project");

    assert!(error.contains("Project not found"), "{error}");
}

#[test]
fn account_relationships_reverse_index_pins_last_use_and_default_root_teams() {
    let (db, _tmp) = db_with_project("p1");
    {
        let conn = db.0.lock().expect("db lock");
        let now = chrono::Utc::now().to_rfc3339();
        queries::insert_project(
            &conn,
            &crate::models::Project {
                id: "p2".to_string(),
                name: "second-project".to_string(),
                path: "/home/user/projects/second".to_string(),
                description: None,
                last_activity_at: None,
                hero_preference: None,
                created_at: now.clone(),
                updated_at: now,
                cached_branch: None,
                cached_is_dirty: None,
                account_memory: Default::default(),
            },
        )
        .expect("insert second project");
        queries::set_project_account(&conn, "p1", "claude", Some("account-2"))
            .expect("pin project");
        queries::remember_last_used_account(&conn, "p2", "claude", "account-2")
            .expect("remember account");
    }

    let teams = TempDir::new().expect("teams root");
    let config_dir = teams.path().join("wave-a");
    std::fs::create_dir_all(&config_dir).expect("team dir");
    std::fs::write(
        config_dir.join("config.json"),
        serde_json::to_vec(&serde_json::json!({
            "name": "wave-a",
            "createdAt": 1_772_399_806_546_i64,
            "members": [{
                "name": "lead",
                "model": "claude-sonnet-4-5",
                "project_path": "/home/user/projects/test-project"
            }]
        }))
        .expect("team json"),
    )
    .expect("write team");

    let index = account_relationships_impl(&db, teams.path(), CliTool::Claude, Some("account-1"))
        .expect("relationships");

    let remembered = index.by_account.get("account-2").expect("account-2");
    assert_eq!(remembered.pinned_projects.len(), 1);
    assert_eq!(remembered.last_used_projects.len(), 1);
    // Regression: 971d964 bypassed TeamConfigStore and therefore ignored the
    // store's legacy tool inference for otherwise valid team configs.
    let default = index.by_account.get("account-1").expect("default account");
    assert_eq!(default.teams.len(), 1);
    assert_eq!(default.teams[0].name, "wave-a");
}

#[test]
fn registry_home_owns_default_root_teams_when_process_home_differs() {
    let account = |id: &str, dir: &str, is_default, is_process_default| Account {
        tool: CliTool::Claude,
        id: id.to_string(),
        dir: PathBuf::from(dir),
        identity: AccountIdentity {
            id: id.to_string(),
            label: id.to_string(),
            display_name: None,
            organization: None,
            plan: None,
            logged_in: true,
            usage_capable: true,
            credential_expires_at: None,
        },
        is_default,
        is_process_default,
        usage: None,
    };
    let accounts = vec![
        account("process", "/home/user/.claude", false, true),
        account("registry", "/home/user/.claude-work", true, false),
    ];

    // Regression: 971d964 attributed teams to the process default even though
    // managed launches pin the registry tool home.
    assert_eq!(
        registry_home_account_id(&accounts, Path::new("/home/user/.claude-work")).as_deref(),
        Some("registry")
    );
}

#[test]
fn account_directory_host_path_uses_the_shared_path_authority() {
    // Regression: f60cb250 duplicated Linux-to-Windows conversion in the
    // frontend instead of routing account paths through provider::path.
    assert_eq!(
        account_directory_host_path_impl("/mnt/d/work/accounts", Some("Ubuntu"))
            .expect("mounted drive"),
        r"D:\work\accounts"
    );
    assert_eq!(
        account_directory_host_path_impl("/home/user/.claude-work", Some("Ubuntu"))
            .expect("WSL home"),
        r"\\wsl.localhost\Ubuntu\home\user\.claude-work"
    );
    assert_eq!(
        account_directory_host_path_impl("/home/user/.claude-work", Some("native"))
            .expect("native path"),
        "/home/user/.claude-work"
    );
}

#[test]
fn setting_global_default_updates_only_the_requested_tool() {
    let (db, _tmp) = db_with_project("p1");

    set_global_default_account_impl(&db, CliTool::Claude, Some("account-2")).expect("set default");
    set_global_default_account_impl(&db, CliTool::Codex, Some("codex-work"))
        .expect("set codex default");
    set_global_default_account_impl(&db, CliTool::Claude, None).expect("clear default");

    let conn = db.0.lock().expect("db lock");
    let settings = crate::db::settings_queries::get_all_settings(&conn).expect("settings");
    assert_eq!(settings.terminal.default_account_ids.get("claude"), None);
    assert_eq!(
        settings
            .terminal
            .default_account_ids
            .get("codex")
            .map(String::as_str),
        Some("codex-work")
    );
}

#[test]
fn account_directory_plan_is_a_safe_sibling_of_the_registry_home() {
    let default_dir = Path::new("/home/user/.claude");

    let planned = account_directory_plan(default_dir, "Work Two").expect("plan");
    assert_eq!(planned, Path::new("/home/user/.claude-work-two"));
    assert!(
        !planned.to_string_lossy().contains('\\'),
        "the Linux launch path must not contain a host separator: {}",
        planned.display()
    );
    // Regression: 971d964 joined a Linux launch-namespace parent with the
    // host separator, producing `/home/user\\.claude-work` on Windows.
    let windows_parent = Path::new(r"\home\user/.claude");
    assert_eq!(
        account_directory_plan(windows_parent, "Work").expect("Windows-host plan"),
        Path::new("/home/user/.claude-work")
    );
    // Regression: 971d964 rejected the hyphenated account labels used by the
    // approved add-account journey even though output directories use hyphens.
    assert_eq!(
        account_directory_plan(default_dir, "work-2").expect("hyphenated plan"),
        Path::new("/home/user/.claude-work-2")
    );
    assert!(account_directory_plan(default_dir, "../../tokens").is_err());
    assert!(account_directory_plan(default_dir, "---").is_err());
}

#[test]
fn login_command_comes_from_the_registry_and_quotes_the_selector_dir() {
    assert_eq!(
        account_login_command(CliTool::Codex, Path::new("/home/user/.codex-work account"))
            .expect("login command"),
        "CODEX_HOME='/home/user/.codex-work account' codex login"
    );
    assert!(account_login_command(CliTool::Agy, Path::new("/home/user/.gemini")).is_err());
}

#[test]
fn account_login_directory_stays_at_the_registry_home_or_a_named_sibling() {
    let default_dir = Path::new("/home/user/.claude");

    assert!(validate_account_login_dir_against(default_dir, default_dir).is_ok());
    assert!(
        validate_account_login_dir_against(default_dir, Path::new("/home/user/.claude-work"))
            .is_ok()
    );
    assert!(validate_account_login_dir_against(
        default_dir,
        Path::new("/home/user/.claude-work/../../etc")
    )
    .is_err());
    assert!(
        validate_account_login_dir_against(default_dir, Path::new("/home/other/.claude")).is_err()
    );
}

/// Without a daemon there is nothing to ask on Windows. The call still
/// succeeds — but the empty list is silence, not an answer, and it says so.
#[test]
fn a_missing_daemon_yields_a_degraded_report_instead_of_an_error() {
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let report = daemon_accounts_report(&provider, CliTool::Claude);

    assert!(report.accounts.is_empty());
    assert!(report.degraded, "a daemon that cannot be asked is degraded");
    assert!(report.error.is_some());
    assert_eq!(report.source, SOURCE_DAEMON);
}

// Regression: 518aace ran every daemon failure through `unwrap_or_default()`.
// A disconnect, a timeout and an undecodable payload all arrived at the
// frontend as a successful empty list, so the chooser vanished and the chip
// with it — while the accounts were still there and still signed in.
#[test]
fn a_daemon_that_never_answered_is_degraded_not_empty() {
    let report = daemon_accounts_report_from(daemon_answer::<protocol::AccountsResult>(
        Err(crate::errors::AppError::DaemonTransport(
            "connection reset by peer".to_string(),
        )),
        "Claude accounts",
    ));

    assert!(report.accounts.is_empty());
    assert!(report.degraded);
    assert!(
        report.error.as_deref().is_some_and(|e| e.contains("reset")),
        "{:?}",
        report.error
    );
}

#[test]
fn an_undecodable_account_list_is_degraded_not_empty() {
    let response =
        protocol::DaemonResponse::ok("list-claude-accounts", serde_json::json!({"accounts": 7}));

    let report = daemon_accounts_report_from(daemon_answer::<protocol::AccountsResult>(
        Ok(response),
        "Claude accounts",
    ));

    assert!(report.degraded);
    assert!(report.error.is_some());
}

/// The one empty list that *is* an answer: a daemon built before the method
/// existed. Launches then render exactly as they did before this feature.
#[test]
fn an_older_daemon_reports_no_accounts_without_degrading() {
    let response = protocol::DaemonResponse::err(
        "list-claude-accounts",
        "UNKNOWN_METHOD",
        "Unknown method: list_accounts",
    );

    let report = daemon_accounts_report_from(daemon_answer::<protocol::AccountsResult>(
        Ok(response),
        "Claude accounts",
    ));

    assert!(report.accounts.is_empty());
    assert!(!report.degraded);
    assert_eq!(report.error, None);
}

/// A resume derives its subscription from the transcript that owns the
/// project's history. A lookup that never ran must not read as "no history".
#[test]
fn a_transcript_lookup_the_daemon_could_not_answer_says_so() {
    let lookup = transcript_lookup_from(daemon_answer::<protocol::ProjectTranscriptResult>(
        Err(crate::errors::AppError::DaemonTransport(
            "timed out waiting for daemon".to_string(),
        )),
        "Claude transcript",
    ));

    assert_eq!(lookup.transcript, None);
    assert!(lookup.unavailable.is_some());
}

#[test]
fn an_older_daemon_reports_no_transcript_without_degrading() {
    let response = protocol::DaemonResponse::err(
        "claude-project-transcript",
        "UNKNOWN_METHOD",
        "Unknown method: claude_project_transcript",
    );

    let lookup = transcript_lookup_from(daemon_answer::<protocol::ProjectTranscriptResult>(
        Ok(response),
        "Claude transcript",
    ));

    assert_eq!(lookup.transcript, None);
    assert_eq!(lookup.unavailable, None);
}

// Regression: 0.8.4 / PR #75 resolved every configured launch command while
// listing accounts. On 2026-08-30 those interactive-shell probes held Tauri's
// dispatcher long enough for every project section to hit its 5 s timeout.
#[test]
fn listing_accounts_never_resolves_launch_commands() {
    // This delayed stand-in is the 1.5 s daemon/shell answer from the incident.
    // A correct read never calls it, so the test itself remains fast.
    let fake_daemon = install_test_resolution_probe(Duration::from_millis(1_500));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let started = std::time::Instant::now();
    let report = list_accounts_impl(&provider, CliTool::Claude);

    assert!(
        started.elapsed() < Duration::from_millis(100),
        "a read-only account report waited on launch-base I/O"
    );
    assert!(
        report.resolved_bases.is_empty(),
        "the read path must carry only already-cached answers, never probe"
    );
    assert_eq!(
        fake_daemon.calls(),
        0,
        "list_accounts issued a probe request"
    );
}

/// Settings has to name the account a launch will really run on, and only the
/// dedicated resolver asks what the configured launch commands mean.
#[test]
fn resolving_launch_bases_carries_what_the_pane_shell_makes_of_each_command() {
    let (db, _tmp) = db_with_project("p1");
    {
        let conn = db.0.lock().expect("db lock");
        let mut settings = crate::db::settings_queries::get_all_settings(&conn).expect("settings");
        settings.terminal.cli_commands.claude.fresh =
            "claude2 --dangerously-skip-permissions".to_string();
        crate::db::settings_queries::save_settings(&conn, &settings).expect("save settings");
    }
    let _aliases = crate::session_scanner::launch_base::install_alias_override(&[(
        "claude2",
        "CLAUDE_CONFIG_DIR=/homes/two claude",
    )]);
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let resolved_bases = resolve_launch_bases_impl(&db, &provider, CliTool::Claude, false);

    let resolved: Vec<&str> = resolved_bases
        .iter()
        .map(|base| base.base.command.as_str())
        .collect();
    assert_eq!(resolved.len(), 3, "one per configured mode: {resolved:?}");
    assert!(
        resolved.contains(&"CLAUDE_CONFIG_DIR=/homes/two claude --dangerously-skip-permissions"),
        "{resolved:?}"
    );
    let expansion = resolved_bases
        .iter()
        .find_map(|base| base.base.expansions.first())
        .expect("the alias that carried the selector");
    assert_eq!(expansion.name, "claude2");
    let selected = resolved_bases
        .iter()
        .find(|base| {
            base.base
                .command
                .starts_with("CLAUDE_CONFIG_DIR=/homes/two")
        })
        .expect("resolved base with selector");
    assert_eq!(selected.selector_value.as_deref(), Some("/homes/two"));
}

#[test]
fn managed_team_resolution_is_carried_on_the_coordination_payload() {
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.fresh = "claude2 --dangerously-skip-permissions".to_string();

    apply_team_launch_base_resolutions_with(&mut commands, [CliTool::Claude], |base, tool| {
        let command = if base.starts_with("claude2") {
            base.replacen(
                "claude2",
                "CLAUDE_CONFIG_DIR=/home/user/.claude-account2 claude",
                1,
            )
        } else {
            base.to_string()
        };
        (
            crate::session_scanner::launch_base::ResolvedBase {
                command,
                expansions: Vec::new(),
                opaque_head: None,
            },
            tool == CliTool::Claude,
        )
    });

    assert_eq!(
        commands
            .resolved_bases
            .get(&(CliTool::Claude, protocol::LaunchMode::Fresh))
            .expect("fresh resolution")
            .command,
        "CLAUDE_CONFIG_DIR=/home/user/.claude-account2 claude --dangerously-skip-permissions"
    );
    assert!(commands
        .resolved_bases
        .contains_key(&(CliTool::Claude, protocol::LaunchMode::Resume)));
    assert!(!commands
        .resolved_bases
        .keys()
        .any(|(tool, _)| *tool == CliTool::Codex));
}

#[test]
fn unavailable_managed_team_resolution_leaves_the_payload_literal() {
    let mut commands = crate::models::CliCommandSettings::default();

    apply_team_launch_base_resolutions_with(&mut commands, [CliTool::Claude], |base, _| {
        (
            crate::session_scanner::launch_base::ResolvedBase {
                command: base.to_string(),
                expansions: Vec::new(),
                opaque_head: None,
            },
            false,
        )
    });

    assert!(commands.resolved_bases.is_empty());
}

#[test]
fn managed_team_account_dirs_keep_windows_wsl_home_forms_until_rendering() {
    let mut commands = crate::models::CliCommandSettings::default();

    apply_team_account_selector_dirs_with(&mut commands, [CliTool::Claude], |_| {
        std::path::PathBuf::from(r"\\wsl.localhost\Ubuntu\home\user\.claude")
    });

    assert_eq!(
        commands.account_selector_dirs.get("CLAUDE_CONFIG_DIR"),
        Some(&std::path::PathBuf::from(
            r"\\wsl.localhost\Ubuntu\home\user\.claude"
        ))
    );
    assert_eq!(
        crate::session_scanner::accounts::to_launch_namespace(
            commands
                .account_selector_dirs
                .get("CLAUDE_CONFIG_DIR")
                .expect("carried team account dir")
        ),
        std::path::PathBuf::from("/home/user/.claude")
    );
}

#[test]
fn managed_claude_team_launches_name_the_root_that_owns_the_team_inbox() {
    let _guard = crate::test_support::acquire_env_test_guard();
    let claude_root = TempDir::new().expect("temp Claude root");
    std::env::set_var("TAURHAUS_CLAUDE_DIR", claude_root.path());
    let mut commands = crate::models::CliCommandSettings::default();

    apply_team_account_selector_dirs(&mut commands, [CliTool::Claude]);

    let selected = commands
        .account_selector_dirs
        .get("CLAUDE_CONFIG_DIR")
        .expect("managed Claude selector");
    assert_eq!(
        selected,
        &crate::provider::platform_paths::PlatformPaths::claude_dir()
    );
    assert_eq!(
        crate::provider::platform_paths::PlatformPaths::teams_dir().parent(),
        Some(selected.as_path()),
        "the selected root is the parent of the managed team inbox"
    );
    std::env::remove_var("TAURHAUS_CLAUDE_DIR");
}

/// A daemon that can read the WSL shell answers what the base command means.
#[test]
fn a_resolved_base_from_the_daemon_is_used_as_the_launch_base() {
    let response = protocol::DaemonResponse::ok(
        "resolve-launch-base-claude",
        serde_json::json!({
            "command": "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions",
            "expansions": [{
                "name": "claude2",
                "body": "CLAUDE_CONFIG_DIR=~/.claude-account2 claude"
            }],
            "opaqueHead": null
        }),
    );

    let resolved = resolved_base_from(
        daemon_answer(Ok(response), "the resolved launch base"),
        "claude2 --dangerously-skip-permissions",
    );

    assert_eq!(
        resolved.command,
        "CLAUDE_CONFIG_DIR=~/.claude-account2 claude --dangerously-skip-permissions"
    );
    assert_eq!(resolved.expansions.len(), 1);
}

// Regression: bc4457a sent `resolve_launch_base` through `send_status_request`,
// whose 5 s ping timeout is shorter than the resolution the daemon runs to
// answer it — up to three interactive-shell probes plus finding the pane shell.
// A request that expires first is a failure, a failure puts the literal base
// back, and the alias's own selector is once again what selects the account.
#[test]
fn the_resolve_request_outlives_the_resolution_it_asks_for() {
    assert!(
        RESOLVE_LAUNCH_BASE_TIMEOUT > launch_base::RESOLUTION_BUDGET,
        "a {RESOLVE_LAUNCH_BASE_TIMEOUT:?} request cannot carry a {:?} resolution",
        launch_base::RESOLUTION_BUDGET,
    );
}

/// An older daemon has no shell to ask, so the base stays exactly as
/// configured — which is what every launch did before this feature.
#[test]
fn an_older_daemon_leaves_the_base_command_literal() {
    let unsupported = protocol::DaemonResponse::err(
        "resolve-launch-base-claude",
        "UNKNOWN_METHOD",
        "Unknown method: resolve_launch_base",
    );

    for answer in [
        daemon_answer(Ok(unsupported), "the resolved launch base"),
        daemon_answer(
            Err(crate::errors::AppError::DaemonTransport(
                "timed out waiting for daemon".to_string(),
            )),
            "the resolved launch base",
        ),
    ] {
        let resolved = resolved_base_from(answer, "claude2 --dangerously-skip-permissions");

        assert_eq!(resolved.command, "claude2 --dangerously-skip-permissions");
        assert!(resolved.expansions.is_empty());
        assert_eq!(resolved.opaque_head, None);
    }
}

/// A transcript where Claude Code writes one: `<config dir>/projects/<slug>/`.
pub(crate) fn write_transcript(config_dir: &Path, project_path: &str, name: &str) -> PathBuf {
    let dir = config_dir
        .join("projects")
        .join(crate::session_scanner::idle::path_to_slug(project_path));
    std::fs::create_dir_all(&dir).expect("transcript dir");
    let path = dir.join(name);
    std::fs::write(&path, "{}\n").expect("transcript");
    path
}

// Regression: 760f776 looked for a project's transcripts only under the config
// dirs that detection had parsed into an account. Claude Code rewrites
// `.claude.json` in place, so a dir read mid-write names nothing — and the
// scan cached that absence for a minute. The history was still on disk, but
// `--resume` stopped seeing it and fell through to the project's own choice,
// opening a different subscription's sessions.
#[test]
fn a_resume_finds_its_transcript_in_a_config_dir_that_names_no_account() {
    let home = TempDir::new().expect("home");
    let config_dir = home.path().join(".claude-account2");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    // Caught mid-rewrite: the file is there and names no account at all.
    std::fs::write(config_dir.join(".claude.json"), "").expect("config file");
    let project_path = "/home/user/projects/mid-write";
    let transcript = write_transcript(&config_dir, project_path, "abc.jsonl");
    let _scan = install_detection_override(
        CliTool::Claude,
        AccountScan {
            config_dirs: vec![config_dir],
            accounts: Vec::new(),
        },
    );
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let lookup = project_transcript(&provider, CliTool::Claude, project_path);
    assert_eq!(lookup.transcript.as_deref(), Some(transcript.as_path()));
    assert_eq!(lookup.unavailable, None);
}

/// Regression: a Settings save sends `force` so the cache that answers is
/// really invalidated. A fail-soft literal reply (daemon absent or
/// unreachable) must not swallow it — the force is carried to the next base
/// until one resolution consumed it.
#[test]
fn a_forced_refresh_is_carried_until_a_resolution_consumes_it() {
    let mut forces = Vec::new();
    let mut answers = [false, true, true].into_iter();
    let resolved = resolve_bases_threading_force(&["one", "two", "three"], true, |base, force| {
        forces.push(force);
        (
            crate::session_scanner::launch_base::ResolvedBase {
                command: base.to_string(),
                expansions: Vec::new(),
                opaque_head: None,
            },
            answers.next().expect("one answer per base"),
        )
    });
    assert_eq!(
        forces,
        vec![true, true, false],
        "the force travels until consumed, then stops"
    );
    assert_eq!(resolved.len(), 3);
}

#[test]
fn an_unforced_resolution_never_invents_a_force() {
    let mut forces = Vec::new();
    resolve_bases_threading_force(&["one", "two"], false, |base, force| {
        forces.push(force);
        (
            crate::session_scanner::launch_base::ResolvedBase {
                command: base.to_string(),
                expansions: Vec::new(),
                opaque_head: None,
            },
            true,
        )
    });
    assert_eq!(forces, vec![false, false]);
}

/// Pins the production source of a managed team's selector dir: the registry
/// session-home authority (`PlatformPaths::tool_home`). Swapping it for the
/// account authority is a named follow-up; until then this test makes any
/// drift visible.
#[test]
fn team_selector_dirs_come_from_the_registry_tool_home() {
    let _guard = crate::test_support::acquire_env_test_guard();
    let claude_home = tempfile::tempdir().expect("claude home");
    std::env::set_var("TAURHAUS_CLAUDE_DIR", claude_home.path());
    let mut commands = crate::models::CliCommandSettings::default();

    apply_team_account_selector_dirs(&mut commands, [CliTool::Claude]);
    std::env::remove_var("TAURHAUS_CLAUDE_DIR");

    assert_eq!(
        commands
            .account_selector_dirs
            .get("CLAUDE_CONFIG_DIR")
            .expect("claude selector dir seeded"),
        &crate::session_scanner::accounts::to_launch_namespace(claude_home.path()),
    );
}
