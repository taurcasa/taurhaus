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

/// Without a daemon there is nothing to ask on Windows, and the app must read
/// that as "no accounts detected" rather than failing the call.
#[test]
fn a_missing_daemon_yields_no_accounts_instead_of_an_error() {
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    assert!(daemon_claude_accounts(&provider).is_none());
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

    assert_eq!(
        claude_project_transcript(&provider, project_path).as_deref(),
        Some(transcript.as_path())
    );
}
