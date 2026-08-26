use super::*;
use crate::db::queries;
use crate::session_scanner::claude_accounts::{install_scan_override, ClaudeScan};
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
            claude_account_id: None,
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
        .claude_account_id
}

#[test]
fn setting_and_clearing_a_project_account_round_trips() {
    let (db, _tmp) = db_with_project("p1");

    set_project_claude_account_impl(&db, "p1", Some("account-2")).expect("set account");
    assert_eq!(stored_account(&db, "p1").as_deref(), Some("account-2"));

    set_project_claude_account_impl(&db, "p1", None).expect("clear account");
    assert_eq!(stored_account(&db, "p1"), None);
}

#[test]
fn setting_the_account_of_an_unknown_project_is_an_error() {
    let (db, _tmp) = db_with_project("p1");

    let error = set_project_claude_account_impl(&db, "missing", Some("account-2"))
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

    let report = daemon_accounts_report(&provider);

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
    let report = daemon_accounts_report_from(daemon_answer::<protocol::ClaudeAccountsResult>(
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

    let report = daemon_accounts_report_from(daemon_answer::<protocol::ClaudeAccountsResult>(
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
        "Unknown method: list_claude_accounts",
    );

    let report = daemon_accounts_report_from(daemon_answer::<protocol::ClaudeAccountsResult>(
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
    let lookup = transcript_lookup_from(daemon_answer::<protocol::ClaudeProjectTranscriptResult>(
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

    let lookup = transcript_lookup_from(daemon_answer::<protocol::ClaudeProjectTranscriptResult>(
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
    let _scan = install_scan_override(ClaudeScan {
        config_dirs: vec![config_dir],
        accounts: Vec::new(),
    });
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let lookup = claude_project_transcript(&provider, project_path);
    assert_eq!(lookup.transcript.as_deref(), Some(transcript.as_path()));
    assert_eq!(lookup.unavailable, None);
}
