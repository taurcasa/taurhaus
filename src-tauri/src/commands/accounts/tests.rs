use super::*;
use crate::db::queries;
use crate::session_scanner::accounts::{install_detection_override, AccountScan};
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

// Regression: c11770e answered the Windows refresh with
// `send_status_request(...).is_ok()`, which says only that the daemon replied.
// The daemon's own payload says whether a fetch began, and the five-second
// debounce in `usage_poller::refresh` answers `{"started": false}` — reported
// here as a refresh that had started. A launch waiting for the reading it asked
// for then waited for one nothing was going to publish, and gave up only at its
// 30-second deadline: every remembered-account launch on Windows stalled for
// half a minute whenever anything had just refreshed usage.
#[test]
fn a_debounced_daemon_refresh_started_nothing() {
    let response = protocol::DaemonResponse::ok(
        "refresh-usage-claude",
        serde_json::json!({"started": false}),
    );

    assert!(!refresh_started(Ok(response)));
}

#[test]
fn a_daemon_refresh_that_began_is_reported_as_started() {
    let response =
        protocol::DaemonResponse::ok("refresh-usage-claude", serde_json::json!({"started": true}));

    assert!(refresh_started(Ok(response)));
}

/// Nothing answered, so nothing started: the launch judges what it already has
/// rather than waiting for a fetch that was never asked for.
#[test]
fn a_daemon_that_could_not_be_asked_started_nothing() {
    assert!(!refresh_started(Err(
        crate::errors::AppError::DaemonTransport("connection reset by peer".to_string(),)
    )));
}

#[test]
fn an_older_daemon_starts_no_refresh() {
    let response = protocol::DaemonResponse::err(
        "refresh-usage-claude",
        "UNKNOWN_METHOD",
        "Unknown method: refresh_usage",
    );

    assert!(!refresh_started(Ok(response)));
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
