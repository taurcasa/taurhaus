//! Claude subscription accounts — one per Claude Code config directory.
//!
//! Claude Code selects the subscription purely through `CLAUDE_CONFIG_DIR`:
//! each config root holds its own `.credentials.json`, its own `.claude.json`
//! with an `oauthAccount` block, and — the part the user feels — its own
//! `projects/` transcripts and `sessions/` registry. Two subscriptions are two
//! directories, and nothing but the environment variable connects a launch to
//! one of them.
//!
//! Detection is deliberately dumb: read the config file, keep the dirs that
//! name an account, and let a missing `.credentials.json` mean "logged out"
//! rather than "gone". Nothing here writes to a config dir.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(not(test))]
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::provider::platform_paths::PlatformPaths;

/// Per-account configuration file, at the root of every config dir.
const CONFIG_FILENAME: &str = ".claude.json";

/// Written on login, removed on logout.
const CREDENTIALS_FILENAME: &str = ".credentials.json";

/// The config dir Claude Code uses when `CLAUDE_CONFIG_DIR` is unset.
const DEFAULT_CONFIG_DIRNAME: &str = ".claude";

/// Sibling config dirs are conventionally `~/.claude-<something>`.
const CONFIG_DIRNAME_PREFIX: &str = ".claude-";

/// Detection re-reads at most once a minute. It is cheap, but it runs on the
/// launch path and on every settings/chooser open.
#[cfg(not(test))]
const CACHE_TTL: Duration = Duration::from_secs(60);

/// One Claude subscription, identified by the config dir it lives in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccount {
    /// `oauthAccount.accountUuid`, or the config dir when a release stops
    /// writing one — the id only has to be stable and addressable.
    pub id: String,
    pub config_dir: PathBuf,
    pub email: String,
    pub display_name: Option<String>,
    pub organization: Option<String>,
    /// `seatTier` where present. Both Max accounts observed on the audit host
    /// leave it null, so `organizationType` (`claude_max`) is the fallback.
    pub seat_tier: Option<String>,
    pub logged_in: bool,
    /// This is `PlatformPaths::claude_dir()` — the dir a launch uses when no
    /// account is selected, and the only root Claude agent teams read.
    pub is_default: bool,
}

/// `.claude.json`, reduced to the account block. Every other key is ignored;
/// the file also carries the user's whole project history.
#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(rename = "oauthAccount")]
    oauth_account: Option<OauthAccount>,
}

#[derive(Debug, Deserialize)]
struct OauthAccount {
    #[serde(rename = "accountUuid")]
    account_uuid: Option<String>,
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "organizationName")]
    organization_name: Option<String>,
    #[serde(rename = "seatTier")]
    seat_tier: Option<String>,
    #[serde(rename = "organizationType")]
    organization_type: Option<String>,
}

/// Where a launch's account came from. Ordered by precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountSource {
    /// The user picked it for this launch (the chooser).
    Request,
    /// Derived from the transcript of the session being resumed.
    Session,
    /// The project's stored choice.
    Project,
    /// The global default account.
    GlobalDefault,
    /// Nothing selected an account: the default config dir it is.
    DefaultConfigDir,
}

/// Everything that can select an account for one launch.
#[derive(Debug, Clone, Copy, Default)]
pub struct AccountRequest<'a> {
    pub requested_account_id: Option<&'a str>,
    /// Transcript of the session being resumed, if the request names one.
    pub session_transcript: Option<&'a Path>,
    pub project_account_id: Option<&'a str>,
    pub default_account_id: Option<&'a str>,
}

/// The account a launch runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountResolution {
    /// Config dir to render into the command — `None` when it is the default
    /// one, so a single-account user's command is unchanged.
    pub config_dir: Option<PathBuf>,
    pub account: Option<ClaudeAccount>,
    pub source: AccountSource,
    /// The account id that was asked for but could not be used.
    pub fallback_from: Option<String>,
}

/// Accounts under `home`, plus any `extra_dirs` found elsewhere.
///
/// `extra_dirs` carries the config dirs of live Claude processes: a session
/// started with `CLAUDE_CONFIG_DIR=/somewhere/else` is a real account this
/// scan would otherwise never see.
pub fn detect_claude_accounts(home: &Path, extra_dirs: &[PathBuf]) -> Vec<ClaudeAccount> {
    detect_claude_accounts_in(home, extra_dirs, &PlatformPaths::claude_dir())
}

/// `detect_claude_accounts` with the default config dir supplied explicitly.
pub fn detect_claude_accounts_in(
    home: &Path,
    extra_dirs: &[PathBuf],
    default_dir: &Path,
) -> Vec<ClaudeAccount> {
    let mut candidates = vec![home.join(DEFAULT_CONFIG_DIRNAME), default_dir.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(home) {
        let mut siblings: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(CONFIG_DIRNAME_PREFIX))
            })
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        siblings.sort();
        candidates.extend(siblings);
    }
    candidates.extend(extra_dirs.iter().cloned());

    let default_key = canonical_key(default_dir);
    let mut seen = Vec::new();
    let mut accounts = Vec::new();
    for candidate in candidates {
        let key = canonical_key(&candidate);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());
        if let Some(account) = read_account(&candidate, key == default_key) {
            accounts.push(account);
        }
    }

    accounts.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.email.cmp(&right.email))
    });
    accounts
}

/// Read one config dir. `None` when it holds no signed-in account.
fn read_account(config_dir: &Path, is_default: bool) -> Option<ClaudeAccount> {
    let raw = std::fs::read_to_string(config_dir.join(CONFIG_FILENAME)).ok()?;
    let oauth = match serde_json::from_str::<ConfigFile>(&raw) {
        Ok(parsed) => parsed.oauth_account?,
        Err(error) => {
            tracing::debug!(
                path = %config_dir.display(),
                error = %error,
                "unparsable Claude config file; not an account"
            );
            return None;
        }
    };

    let email = oauth
        .email_address
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;

    Some(ClaudeAccount {
        id: oauth
            .account_uuid
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| config_dir.display().to_string()),
        config_dir: config_dir.to_path_buf(),
        email,
        display_name: non_empty(oauth.display_name),
        organization: non_empty(oauth.organization_name),
        seat_tier: non_empty(oauth.seat_tier).or_else(|| non_empty(oauth.organization_type)),
        logged_in: config_dir.join(CREDENTIALS_FILENAME).exists(),
        is_default,
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Identity for dedupe: the canonical path where the dir exists, the path
/// itself otherwise (a candidate that does not exist is still distinct).
fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Accounts injected by a test, so no test ever reads the developer's real
/// `~/.claude*` — detection under test is always a fixture.
#[cfg(test)]
static DETECTION_OVERRIDE: Mutex<Option<Vec<ClaudeAccount>>> = Mutex::new(None);

/// Install (or clear with `None`) the accounts `detect_claude_accounts_cached`
/// reports. Returns a guard that restores the previous value.
#[cfg(test)]
pub(crate) fn set_detection_override(accounts: Option<Vec<ClaudeAccount>>) {
    *DETECTION_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = accounts;
}

/// Detected accounts for this app run, re-read at most once a minute.
pub fn detect_claude_accounts_cached() -> Vec<ClaudeAccount> {
    #[cfg(test)]
    {
        return DETECTION_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_default();
    }

    #[cfg(not(test))]
    detect_claude_accounts_cached_uncached()
}

#[cfg(not(test))]
fn detect_claude_accounts_cached_uncached() -> Vec<ClaudeAccount> {
    static CACHE: Mutex<Option<(Instant, Vec<ClaudeAccount>)>> = Mutex::new(None);

    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if let Some((observed_at, accounts)) = cache.as_ref() {
        if observed_at.elapsed() < CACHE_TTL {
            return accounts.clone();
        }
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let accounts = detect_claude_accounts(&home, &config_dirs_of_live_sessions());
    tracing::debug!(count = accounts.len(), "detected Claude accounts");
    *cache = Some((Instant::now(), accounts.clone()));
    accounts
}

/// Config dirs of the Claude sessions the last scan saw.
#[cfg(not(test))]
///
/// A live session's transcript already names its config root (PR 3 resolves it
/// per process), so the last published scan answers this for free — detection
/// must never start a process inventory of its own, least of all on the launch
/// path. Where no scan has run in this process the list is simply empty and
/// the `<home>/.claude*` candidates stand alone.
fn config_dirs_of_live_sessions() -> Vec<PathBuf> {
    crate::session_scanner::latest_compaction_runtime_sessions()
        .into_iter()
        .filter(|session| session.cli_tool == crate::session_scanner::cli_tool::CliTool::Claude)
        .filter_map(|session| session.jsonl_path)
        .filter_map(|transcript| {
            crate::session_scanner::idle::config_dir_for_transcript(Path::new(&transcript))
        })
        .collect()
}

/// Pick the account one launch runs on.
///
/// Precedence: an explicit request, then the session being resumed, then the
/// project's choice, then the global default. A selected account that is gone
/// or logged out falls back to the default config dir and says so; an empty
/// account list means detection could not run at all, which is not a fallback.
pub fn resolve_launch_account(
    accounts: &[ClaudeAccount],
    default_dir: &Path,
    request: AccountRequest<'_>,
) -> AccountResolution {
    let default_resolution = |fallback_from: Option<String>| AccountResolution {
        config_dir: None,
        account: accounts.iter().find(|account| account.is_default).cloned(),
        source: AccountSource::DefaultConfigDir,
        fallback_from,
    };

    if accounts.is_empty() {
        return default_resolution(None);
    }

    // An explicit pick outranks everything, including the session: the user
    // just answered this question for this launch.
    if let Some(wanted) = request
        .requested_account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return match accounts
            .iter()
            .find(|account| account.id == wanted && account.logged_in)
        {
            Some(account) => selected(account, default_dir, AccountSource::Request),
            None => default_resolution(Some(wanted.to_string())),
        };
    }

    if let Some(transcript) = request.session_transcript {
        if let Some(config_dir) =
            crate::session_scanner::idle::config_dir_for_transcript(transcript)
        {
            let key = canonical_key(&config_dir);
            let account = accounts
                .iter()
                .find(|account| canonical_key(&account.config_dir) == key)
                .cloned();
            return AccountResolution {
                config_dir: (key != canonical_key(default_dir)).then_some(config_dir),
                account,
                source: AccountSource::Session,
                fallback_from: None,
            };
        }
    }

    let selections = [
        (request.project_account_id, AccountSource::Project),
        (request.default_account_id, AccountSource::GlobalDefault),
    ];

    for (wanted, source) in selections {
        let Some(wanted) = wanted.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };
        let Some(account) = accounts
            .iter()
            .find(|account| account.id == wanted && account.logged_in)
        else {
            return default_resolution(Some(wanted.to_string()));
        };
        return selected(account, default_dir, source);
    }

    default_resolution(None)
}

/// Resolution for an account that was found and can run. The config dir is
/// omitted when it is the default one, so nothing changes for a host with a
/// single subscription.
fn selected(
    account: &ClaudeAccount,
    default_dir: &Path,
    source: AccountSource,
) -> AccountResolution {
    AccountResolution {
        config_dir: (canonical_key(&account.config_dir) != canonical_key(default_dir))
            .then(|| account.config_dir.clone()),
        account: Some(account.clone()),
        source,
        fallback_from: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const PRIMARY_ID: &str = "11111111-1111-1111-1111-111111111111";
    const SECOND_ID: &str = "22222222-2222-2222-2222-222222222222";

    /// Shape of a real `.claude.json` on this host, secrets and the project
    /// history removed. `seatTier` is null on both observed Max accounts.
    fn config_json(id: &str, email: &str, display: &str) -> String {
        format!(
            r#"{{"numStartups":42,"oauthAccount":{{"accountUuid":"{id}","emailAddress":"{email}","organizationUuid":"99999999-9999-9999-9999-999999999999","seatTier":null,"displayName":"{display}","organizationName":"{email}'s Organization","organizationType":"claude_max"}},"projects":{{}}}}"#
        )
    }

    fn write_account(
        home: &Path,
        dirname: &str,
        id: &str,
        email: &str,
        logged_in: bool,
    ) -> PathBuf {
        let dir = home.join(dirname);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".claude.json"), config_json(id, email, "Who")).unwrap();
        if logged_in {
            fs::write(dir.join(".credentials.json"), "{}").unwrap();
        }
        dir
    }

    fn accounts_fixture() -> (TempDir, Vec<ClaudeAccount>) {
        let home = TempDir::new().unwrap();
        write_account(
            home.path(),
            ".claude",
            PRIMARY_ID,
            "stierms@gmail.com",
            true,
        );
        write_account(
            home.path(),
            ".claude-account2",
            SECOND_ID,
            "m.stier@giesi.com",
            true,
        );
        let default_dir = home.path().join(".claude");
        let accounts = detect_claude_accounts_in(home.path(), &[], &default_dir);
        (home, accounts)
    }

    #[test]
    fn detects_every_config_dir_with_the_default_first() {
        let (_home, accounts) = accounts_fixture();

        assert_eq!(accounts.len(), 2, "{accounts:?}");
        assert_eq!(accounts[0].email, "stierms@gmail.com");
        assert!(accounts[0].is_default);
        assert_eq!(accounts[0].id, PRIMARY_ID);
        assert_eq!(accounts[0].display_name.as_deref(), Some("Who"));
        assert_eq!(
            accounts[0].organization.as_deref(),
            Some("stierms@gmail.com's Organization")
        );
        assert_eq!(accounts[0].seat_tier.as_deref(), Some("claude_max"));
        assert!(accounts[0].logged_in);
        assert_eq!(accounts[1].email, "m.stier@giesi.com");
        assert!(!accounts[1].is_default);
    }

    #[test]
    fn a_config_dir_without_an_oauth_account_is_not_an_account() {
        let home = TempDir::new().unwrap();
        let dir = home.path().join(".claude-empty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".claude.json"), r#"{"numStartups":1}"#).unwrap();
        let unparsable = home.path().join(".claude-broken");
        fs::create_dir_all(&unparsable).unwrap();
        fs::write(unparsable.join(".claude.json"), "{not json").unwrap();
        fs::create_dir_all(home.path().join(".claude-bare")).unwrap();

        let accounts = detect_claude_accounts_in(home.path(), &[], &home.path().join(".claude"));

        assert!(accounts.is_empty(), "{accounts:?}");
    }

    #[test]
    fn a_config_dir_without_credentials_is_detected_but_logged_out() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);

        let accounts = detect_claude_accounts_in(home.path(), &[], &home.path().join(".claude"));

        assert_eq!(accounts.len(), 1);
        assert!(!accounts[0].logged_in);
    }

    #[test]
    fn repeated_candidates_are_deduped_by_canonical_path() {
        let (home, _) = accounts_fixture();
        let default_dir = home.path().join(".claude");
        let extras = vec![
            default_dir.clone(),
            home.path().join(".claude-account2"),
            home.path().join(".claude").join(".").join(""),
        ];

        let accounts = detect_claude_accounts_in(home.path(), &extras, &default_dir);

        assert_eq!(accounts.len(), 2, "{accounts:?}");
    }

    #[test]
    fn a_config_dir_outside_home_is_discovered_through_the_extra_dirs() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", true);
        let elsewhere = TempDir::new().unwrap();
        write_account(elsewhere.path(), "work", SECOND_ID, "b@example.com", true);

        let accounts = detect_claude_accounts_in(
            home.path(),
            &[elsewhere.path().join("work")],
            &home.path().join(".claude"),
        );

        assert_eq!(accounts.len(), 2, "{accounts:?}");
        assert!(accounts.iter().any(|a| a.email == "b@example.com"));
    }

    fn request<'a>() -> AccountRequest<'a> {
        AccountRequest::default()
    }

    #[test]
    fn an_explicit_request_wins_over_every_stored_choice() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                requested_account_id: Some(SECOND_ID),
                project_account_id: Some(PRIMARY_ID),
                default_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::Request);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
        assert_eq!(resolved.fallback_from, None);
    }

    #[test]
    fn the_project_choice_wins_over_the_global_default() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                default_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::Project);
        assert_eq!(
            resolved.account.as_ref().map(|a| a.email.as_str()),
            Some("m.stier@giesi.com")
        );
    }

    #[test]
    fn the_global_default_applies_when_the_project_has_no_choice() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                default_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::GlobalDefault);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
    }

    #[test]
    fn the_default_config_dir_needs_no_prefix() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                project_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.config_dir, None);
        assert_eq!(
            resolved.account.as_ref().map(|a| a.email.as_str()),
            Some("stierms@gmail.com")
        );
    }

    #[test]
    fn a_vanished_account_falls_back_to_the_default_and_reports_it() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                project_account_id: Some("deleted-account"),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::DefaultConfigDir);
        assert_eq!(resolved.config_dir, None);
        assert_eq!(resolved.fallback_from.as_deref(), Some("deleted-account"));
    }

    #[test]
    fn a_logged_out_account_falls_back_to_the_default_and_reports_it() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", true);
        write_account(
            home.path(),
            ".claude-out",
            SECOND_ID,
            "b@example.com",
            false,
        );
        let default_dir = home.path().join(".claude");
        let accounts = detect_claude_accounts_in(home.path(), &[], &default_dir);

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.fallback_from.as_deref(), Some(SECOND_ID));
        assert_eq!(resolved.config_dir, None);
    }

    #[test]
    fn an_undetectable_account_list_never_reports_a_fallback() {
        let home = TempDir::new().unwrap();
        let default_dir = home.path().join(".claude");

        let resolved = resolve_launch_account(
            &[],
            &default_dir,
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::DefaultConfigDir);
        assert_eq!(resolved.fallback_from, None);
        assert_eq!(resolved.config_dir, None);
    }

    // Regression: c9669ef made a Claude process keep its transcripts under its
    // own `CLAUDE_CONFIG_DIR`, so `--resume` run in the wrong config dir sees a
    // different history entirely. The session the user picked has to decide the
    // account, over anything the project stored.
    #[test]
    fn the_session_transcript_decides_the_account_over_the_project_choice() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");
        let transcript = home
            .path()
            .join(".claude-account2")
            .join("projects")
            .join("-home-user-projects-taurhaus")
            .join("f3286b16-ffc7-4d16-915d-046705823a3d.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                session_transcript: Some(&transcript),
                project_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::Session);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
        assert_eq!(
            resolved.account.as_ref().map(|a| a.email.as_str()),
            Some("m.stier@giesi.com")
        );
    }

    #[test]
    fn an_explicit_request_still_wins_over_the_session_transcript() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");
        let transcript = home
            .path()
            .join(".claude-account2")
            .join("projects")
            .join("-home-user-projects-taurhaus")
            .join("f3286b16-ffc7-4d16-915d-046705823a3d.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                requested_account_id: Some(PRIMARY_ID),
                session_transcript: Some(&transcript),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::Request);
        assert_eq!(resolved.config_dir, None);
    }

    #[test]
    fn a_transcript_outside_a_known_layout_leaves_the_project_choice_alone() {
        let (home, accounts) = accounts_fixture();
        let default_dir = home.path().join(".claude");
        let transcript = home.path().join("stray.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            &default_dir,
            AccountRequest {
                session_transcript: Some(&transcript),
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountSource::Project);
    }
}
