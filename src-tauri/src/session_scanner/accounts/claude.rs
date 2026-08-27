//! Claude subscription accounts — one per Claude Code config directory.
//!
//! Claude Code selects the subscription purely through `CLAUDE_CONFIG_DIR`:
//! each config root holds its own `.credentials.json`, its own `.claude.json`
//! with an `oauthAccount` block, and — the part the user feels — its own
//! `projects/` transcripts and `sessions/` registry. Two subscriptions are two
//! directories, and nothing but the environment variable connects a launch to
//! one of them.
//!
//! Detection is deliberately dumb: read the config file and keep the dirs that
//! name an account. Whether that account is signed in is the one platform
//! question here — Linux and Windows/WSL write `.credentials.json`, macOS keeps
//! the tokens in the login keychain and writes nothing. Nothing here writes to
//! a config dir.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(not(test))]
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::daemon::claude_usage::ClaudeAccountUsage;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::accounts::AccountOrigin;
#[cfg(test)]
use crate::session_scanner::cli_tool::CliTool;
#[cfg(test)]
use crate::session_scanner::types::RuntimeSession;

use super::{
    Account, AccountIdentity, AccountProvider, HttpClient, Severity, UsageProvider, UsageSnapshot,
    UsageStatus, UsageWindow,
};

/// Per-account configuration file, at the root of every config dir.
const CONFIG_FILENAME: &str = ".claude.json";

/// Written on login, removed on logout.
const CREDENTIALS_FILENAME: &str = ".credentials.json";

/// The config dir Claude Code uses when `CLAUDE_CONFIG_DIR` is unset.
const DEFAULT_CONFIG_DIRNAME: &str = ".claude";

/// Sibling config dirs are conventionally `~/.claude-<something>`.
const CONFIG_DIRNAME_PREFIX: &str = ".claude-";

/// Transcripts live under `<config dir>/projects/<slug>/`.
const PROJECTS_SUBDIR: &str = "projects";

/// Extension of a Claude transcript.
const TRANSCRIPT_EXTENSION: &str = "jsonl";

/// Detection re-reads at most once a minute. It is cheap, but it runs on the
/// launch path and on every settings/chooser open.
#[cfg(not(test))]
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Where Claude Code keeps the OAuth tokens of a config dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialStore {
    /// `<config dir>/.credentials.json` — Linux and Windows/WSL.
    File,
    /// The macOS login keychain, under `Claude Code-credentials`. There is no
    /// file to look for, and reading the keychain would pop an authorization
    /// prompt, so a config dir that names an account counts as signed in.
    Keychain,
}

/// The credential store of the host this build runs on.
const fn host_credential_store() -> CredentialStore {
    if cfg!(target_os = "macos") {
        CredentialStore::Keychain
    } else {
        CredentialStore::File
    }
}

/// Claude Code account detection behind the registry capability slice.
pub struct ClaudeAccountProvider;

impl AccountProvider for ClaudeAccountProvider {
    fn default_dir(&self, home: &Path) -> PathBuf {
        home.join(DEFAULT_CONFIG_DIRNAME)
    }

    fn candidate_dirs(&self, home: &Path, live_selector_values: &[PathBuf]) -> Vec<PathBuf> {
        config_dir_candidates(home, live_selector_values, &self.default_dir(home))
    }

    fn identify(&self, dir: &Path) -> Option<AccountIdentity> {
        let account = read_account(dir, false, false, host_credential_store())?;
        Some(AccountIdentity {
            id: account.id,
            label: account.email,
            display_name: account.display_name,
            organization: account.organization,
            plan: account.seat_tier,
            logged_in: account.logged_in,
            credential_expires_at: credential_expires_at(dir),
        })
    }

    fn session_dir(&self, transcript: &Path) -> Option<PathBuf> {
        transcript.ancestors().find_map(|ancestor| {
            (ancestor.file_name().and_then(|name| name.to_str()) == Some(PROJECTS_SUBDIR))
                .then(|| ancestor.parent().map(Path::to_path_buf))
                .flatten()
        })
    }
}

#[derive(Debug, Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OAuthCredentials>,
}

#[derive(Debug, Deserialize)]
struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

fn credential_expires_at(config_dir: &Path) -> Option<i64> {
    let raw = std::fs::read_to_string(config_dir.join(CREDENTIALS_FILENAME)).ok()?;
    let expires_at = serde_json::from_str::<CredentialsFile>(&raw)
        .ok()?
        .oauth?
        .expires_at?;
    Some(if expires_at >= 1_000_000_000_000 {
        expires_at / 1_000
    } else {
        expires_at
    })
}

pub struct ClaudeUsageProvider;

impl UsageProvider for ClaudeUsageProvider {
    fn fetch(&self, dir: &Path, http: &dyn HttpClient) -> UsageSnapshot {
        let observed_at = chrono::Utc::now();
        let credentials = std::fs::read_to_string(dir.join(CREDENTIALS_FILENAME))
            .ok()
            .and_then(|raw| serde_json::from_str::<CredentialsFile>(&raw).ok())
            .and_then(|file| file.oauth);
        let Some(credentials) = credentials else {
            return usage_snapshot(observed_at, UsageStatus::Unauthorized, Vec::new());
        };
        let expires_at = credentials.expires_at.map(epoch_seconds);
        if expires_at.is_some_and(|expires| expires <= observed_at.timestamp()) {
            return usage_snapshot(observed_at, UsageStatus::Unauthorized, Vec::new());
        }
        let Some(token) = credentials.access_token else {
            return usage_snapshot(observed_at, UsageStatus::Unauthorized, Vec::new());
        };
        let user_agent = format!("taurhaus/{}", env!("CARGO_PKG_VERSION"));
        let authorization = format!("Bearer {token}");
        let headers = [
            ("Authorization", authorization.as_str()),
            ("anthropic-beta", "oauth-2025-04-20"),
            ("Content-Type", "application/json"),
            ("User-Agent", user_agent.as_str()),
        ];
        let response = match http.get(
            "https://api.anthropic.com/api/oauth/usage",
            &headers,
            std::time::Duration::from_secs(5),
        ) {
            Ok(response) => response,
            Err(_) => return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new()),
        };
        if matches!(response.status, 401 | 403) {
            return usage_snapshot(observed_at, UsageStatus::Unauthorized, Vec::new());
        }
        if response.status != 200 {
            return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new());
        }
        let Ok(payload) = serde_json::from_str::<UsagePayload>(&response.body) else {
            return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new());
        };
        let windows = normalize_usage(payload);
        usage_snapshot(observed_at, UsageStatus::Ok, windows)
    }
}

fn epoch_seconds(value: i64) -> i64 {
    if value >= 1_000_000_000_000 {
        value / 1_000
    } else {
        value
    }
}

fn usage_snapshot(
    observed_at: chrono::DateTime<chrono::Utc>,
    status: UsageStatus,
    windows: Vec<UsageWindow>,
) -> UsageSnapshot {
    UsageSnapshot {
        observed_at,
        status,
        windows,
        note: None,
    }
}

#[derive(Deserialize)]
struct UsagePayload {
    #[serde(default)]
    limits: Vec<UsageLimit>,
    five_hour: Option<UsageMirror>,
    seven_day: Option<UsageMirror>,
    seven_day_sonnet: Option<UsageMirror>,
}

#[derive(Deserialize)]
struct UsageLimit {
    kind: String,
    percent: f64,
    severity: Option<String>,
    resets_at: Option<String>,
    #[serde(default)]
    scope: Option<UsageScope>,
    #[serde(default)]
    is_active: bool,
}

#[derive(Deserialize)]
struct UsageScope {
    model: Option<UsageModel>,
}

#[derive(Deserialize)]
struct UsageModel {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct UsageMirror {
    utilization: f64,
    resets_at: Option<String>,
}

fn normalize_usage(payload: UsagePayload) -> Vec<UsageWindow> {
    let mut windows: Vec<UsageWindow> = if payload.limits.is_empty() {
        [
            ("session", "Current session", payload.five_hour.as_ref()),
            (
                "weekly_all",
                "Current week (all models)",
                payload.seven_day.as_ref(),
            ),
        ]
        .into_iter()
        .filter_map(|(key, title, mirror)| {
            mirror.map(|mirror| UsageWindow {
                key: key.to_string(),
                title: title.to_string(),
                used_percentage: mirror.utilization,
                resets_at: parse_reset(mirror.resets_at.as_deref()),
                severity: Severity::Normal,
                is_active: false,
            })
        })
        .collect()
    } else {
        payload
            .limits
            .into_iter()
            .filter_map(|limit| {
                let title = match limit.kind.as_str() {
                    "session" => "Current session".to_string(),
                    "weekly_all" => "Current week (all models)".to_string(),
                    "weekly_scoped" => format!(
                        "Current week ({})",
                        limit
                            .scope
                            .as_ref()
                            .and_then(|scope| scope.model.as_ref())
                            .and_then(|model| model.display_name.as_deref())
                            .unwrap_or("scoped")
                    ),
                    _ => return None,
                };
                Some(UsageWindow {
                    key: limit.kind,
                    title,
                    used_percentage: limit.percent,
                    resets_at: parse_reset(limit.resets_at.as_deref()),
                    severity: match limit.severity.as_deref() {
                        Some("warning") => Severity::Warning,
                        Some("critical") => Severity::Critical,
                        _ => Severity::Normal,
                    },
                    is_active: limit.is_active,
                })
            })
            .collect()
    };
    if let Some(sonnet) = payload.seven_day_sonnet {
        windows.push(UsageWindow {
            key: "weekly_sonnet".to_string(),
            title: "Current week (Sonnet only)".to_string(),
            used_percentage: sonnet.utilization,
            resets_at: parse_reset(sonnet.resets_at.as_deref()),
            severity: Severity::Normal,
            is_active: false,
        });
    }
    windows
}

fn parse_reset(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp())
}

/// One Claude subscription, identified by the config dir it lives in.
///
/// `Eq` is deliberately absent: `usage` carries the percentages Claude Code
/// reports, and those are `number` in its schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// This is `<home>/.claude`, the dir Claude Code reads when
    /// `CLAUDE_CONFIG_DIR` is unset — and the only account a launch may leave
    /// the variable off for. It is *not* `is_default`: `TAURHAUS_CLAUDE_DIR`
    /// moves taurhaus's root, and Claude Code knows nothing about that.
    #[serde(default)]
    pub is_process_default: bool,
    /// What this subscription's status line last reported about its 5-hour and
    /// 7-day limits. `None` while nothing has reported: usage only flows while
    /// a session of that account is running, so a fresh install, an account
    /// that has not been used, and an account at 0 % are three different
    /// things and only the last of them is a number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ClaudeAccountUsage>,
}

impl From<ClaudeAccount> for Account {
    fn from(account: ClaudeAccount) -> Self {
        let usage = account.usage.map(|usage| {
            let mut windows = Vec::new();
            if let Some(window) = usage.five_hour {
                windows.push(UsageWindow {
                    key: "session".to_string(),
                    title: "Current session".to_string(),
                    used_percentage: window.used_percentage,
                    resets_at: window.resets_at,
                    severity: Severity::Normal,
                    is_active: true,
                });
            }
            if let Some(window) = usage.seven_day {
                windows.push(UsageWindow {
                    key: "weekly_all".to_string(),
                    title: "Current week (all models)".to_string(),
                    used_percentage: window.used_percentage,
                    resets_at: window.resets_at,
                    severity: Severity::Normal,
                    is_active: true,
                });
            }
            UsageSnapshot {
                observed_at: usage.observed_at,
                status: UsageStatus::Ok,
                windows,
                note: None,
            }
        });
        Self {
            tool: crate::session_scanner::cli_tool::CliTool::Claude,
            id: account.id.clone(),
            dir: account.config_dir,
            identity: AccountIdentity {
                id: account.id,
                label: account.email,
                display_name: account.display_name,
                organization: account.organization,
                plan: account.seat_tier,
                logged_in: account.logged_in,
                credential_expires_at: None,
            },
            is_default: account.is_default,
            is_process_default: account.is_process_default,
            usage,
        }
    }
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
#[derive(Debug, Clone, PartialEq)]
pub struct AccountResolution {
    /// Config dir to render into the command — `None` only when it is the dir
    /// Claude Code reads with `CLAUDE_CONFIG_DIR` unset, so a single-account
    /// user's command is unchanged.
    pub config_dir: Option<PathBuf>,
    pub account: Option<ClaudeAccount>,
    pub source: AccountOrigin,
    /// The account id that was asked for but could not be used.
    pub fallback_from: Option<String>,
    /// Nothing selected this launch's account and more than one subscription
    /// could have run it. The launch still goes ahead on the fallback; the
    /// chooser exists to turn this into an answer before it does.
    pub needs_choice: bool,
}

/// One scan of this host's Claude config dirs.
///
/// The dirs and the accounts are deliberately separate answers. `.claude.json`
/// is rewritten in place by Claude Code, so a dir caught mid-write names no
/// account — while its `projects/` transcripts sit there untouched. Anything
/// looking for a project's history reads `config_dirs`; only the chooser, the
/// chip and the Settings block need `accounts`.
#[derive(Debug, Clone, Default)]
pub struct ClaudeScan {
    pub config_dirs: Vec<PathBuf>,
    pub accounts: Vec<ClaudeAccount>,
}

/// Accounts under `home`, plus any `extra_dirs` found elsewhere.
///
/// `extra_dirs` carries the config dirs of live Claude processes: a session
/// started with `CLAUDE_CONFIG_DIR=/somewhere/else` is a real account this
/// scan would otherwise never see.
pub fn detect_claude_accounts_in(
    home: &Path,
    extra_dirs: &[PathBuf],
    default_dir: &Path,
) -> Vec<ClaudeAccount> {
    detect_claude_accounts_rooted(
        home,
        extra_dirs,
        default_dir,
        &home.join(DEFAULT_CONFIG_DIRNAME),
    )
}

/// `detect_claude_accounts_in` with the dir Claude Code reads on its own named
/// outright.
///
/// It is not derivable from the scan root. `TAURHAUS_CLAUDE_DIR` moves the
/// scan, Claude Code has never heard of that variable, and an override that
/// happens to be named `.claude` would otherwise pass for the process default
/// and lose the `CLAUDE_CONFIG_DIR` assignment that makes it real.
pub fn detect_claude_accounts_rooted(
    home: &Path,
    extra_dirs: &[PathBuf],
    default_dir: &Path,
    process_default_dir: &Path,
) -> Vec<ClaudeAccount> {
    scan_with_store(
        home,
        extra_dirs,
        default_dir,
        process_default_dir,
        host_credential_store(),
    )
    .accounts
}

/// Every config dir one scan should look at, deduped by canonical path.
fn config_dir_candidates(home: &Path, extra_dirs: &[PathBuf], default_dir: &Path) -> Vec<PathBuf> {
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

    let mut seen = Vec::new();
    let mut unique = Vec::new();
    for candidate in candidates {
        let key = canonical_key(&candidate);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        unique.push(candidate);
    }
    unique
}

/// A scan against a named credential store, so both platform behaviours are
/// testable from any host.
fn scan_with_store(
    home: &Path,
    extra_dirs: &[PathBuf],
    default_dir: &Path,
    process_default_dir: &Path,
    store: CredentialStore,
) -> ClaudeScan {
    let default_key = canonical_key(default_dir);
    let process_default_key = canonical_key(process_default_dir);
    let mut config_dirs = Vec::new();
    let mut accounts = Vec::new();
    for candidate in config_dir_candidates(home, extra_dirs, default_dir) {
        let key = canonical_key(&candidate);
        if candidate.is_dir() {
            config_dirs.push(candidate.clone());
        }
        if let Some(account) = read_account(
            &candidate,
            key == default_key,
            key == process_default_key,
            store,
        ) {
            accounts.push(account);
        }
    }

    accounts.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.email.cmp(&right.email))
    });
    ClaudeScan {
        config_dirs,
        accounts,
    }
}

/// `scan_with_store`'s accounts for a scan root that *is* the process's home.
#[cfg(test)]
fn detect_with_store(
    home: &Path,
    extra_dirs: &[PathBuf],
    default_dir: &Path,
    store: CredentialStore,
) -> Vec<ClaudeAccount> {
    scan_with_store(
        home,
        extra_dirs,
        default_dir,
        &home.join(DEFAULT_CONFIG_DIRNAME),
        store,
    )
    .accounts
}

/// Read one config dir. `None` when it names no account at all.
fn read_account(
    config_dir: &Path,
    is_default: bool,
    is_process_default: bool,
    store: CredentialStore,
) -> Option<ClaudeAccount> {
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
        logged_in: signed_in(config_dir, store),
        is_default,
        is_process_default,
        // Detection reads config dirs; usage lives in taurhaus's own sink and
        // is attached by the callers that surface accounts to the user.
        usage: None,
    })
}

/// Whether the account in `config_dir` holds credentials it can launch with.
fn signed_in(config_dir: &Path, store: CredentialStore) -> bool {
    match store {
        CredentialStore::File => config_dir.join(CREDENTIALS_FILENAME).exists(),
        // Nothing on disk answers this on macOS, and the absence of a file the
        // platform never writes is not evidence of a logout.
        CredentialStore::Keychain => true,
    }
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

/// A scan injected by a test, so no test ever reads the developer's real
/// `~/.claude*` — detection under test is always a fixture.
#[cfg(test)]
static DETECTION_OVERRIDE: Mutex<Option<ClaudeScan>> = Mutex::new(None);

/// The override is process-wide, so the tests that install one run one at a
/// time — two fixtures in flight at once would answer each other's launches.
#[cfg(test)]
static DETECTION_OVERRIDE_LOCK: Mutex<()> = Mutex::new(());

/// Holds the fixture in place, and the lock with it, until the test ends.
#[cfg(test)]
pub(crate) struct DetectionOverrideGuard {
    #[allow(dead_code)]
    legacy_lock: std::sync::MutexGuard<'static, ()>,
    #[allow(dead_code)]
    generic: super::DetectionOverrideGuard,
}

#[cfg(test)]
impl Drop for DetectionOverrideGuard {
    fn drop(&mut self) {
        *DETECTION_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}

/// Make the cached scan report `accounts` — and their config dirs — for the
/// life of the returned guard.
#[cfg(test)]
pub(crate) fn install_detection_override(accounts: Vec<ClaudeAccount>) -> DetectionOverrideGuard {
    let config_dirs = accounts
        .iter()
        .map(|account| account.config_dir.clone())
        .collect();
    install_scan_override(ClaudeScan {
        config_dirs,
        accounts,
    })
}

/// `install_detection_override` for a scan whose config dirs and accounts do
/// not line up — a dir whose `.claude.json` names nothing, say.
#[cfg(test)]
pub(crate) fn install_scan_override(scan: ClaudeScan) -> DetectionOverrideGuard {
    let lock = DETECTION_OVERRIDE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let generic = super::install_detection_override(
        CliTool::Claude,
        super::AccountScan {
            config_dirs: scan.config_dirs.clone(),
            accounts: scan.accounts.iter().cloned().map(Account::from).collect(),
        },
    );
    *DETECTION_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(scan);
    DetectionOverrideGuard {
        legacy_lock: lock,
        generic,
    }
}

/// This app run's scan of the Claude config dirs, re-read at most once a
/// minute.
pub fn scan_claude_config_cached() -> ClaudeScan {
    #[cfg(test)]
    {
        return DETECTION_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_default();
    }

    #[cfg(not(test))]
    scan_claude_config_uncached()
}

/// Detected accounts for this app run.
pub fn detect_claude_accounts_cached() -> Vec<ClaudeAccount> {
    scan_claude_config_cached().accounts
}

/// Config dirs a project's transcripts may live in.
///
/// Deliberately wider than the detected accounts: a config dir whose
/// `.claude.json` is empty, half-written or unreadable names no account, and
/// the scan caches that for a minute. Its transcripts are still on disk, and
/// `--resume` needs the dir that holds them, not the metadata beside it.
pub fn transcript_config_dirs() -> Vec<PathBuf> {
    scan_claude_config_cached().config_dirs
}

#[cfg(not(test))]
fn scan_claude_config_uncached() -> ClaudeScan {
    static CACHE: Mutex<Option<(Instant, ClaudeScan)>> = Mutex::new(None);

    let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if let Some((observed_at, scan)) = cache.as_ref() {
        if observed_at.elapsed() < CACHE_TTL {
            return scan.clone();
        }
    }

    let home_dir = dirs::home_dir();
    let scan_home = detection_home_for(PlatformPaths::claude_dir_override(), home_dir.clone());
    let scan = scan_with_store(
        &scan_home,
        &config_dirs_of_live_sessions(),
        &PlatformPaths::claude_dir(),
        &process_default_config_dir(home_dir.as_deref()),
        host_credential_store(),
    );
    tracing::debug!(
        accounts = scan.accounts.len(),
        config_dirs = scan.config_dirs.len(),
        "scanned Claude config dirs"
    );
    *cache = Some((Instant::now(), scan.clone()));
    scan
}

/// Where the `<home>/.claude*` scan runs.
///
/// `TAURHAUS_CLAUDE_DIR` moves taurhaus's Claude root, and an E2E run points it
/// at a scratch directory precisely so the run touches nothing of the user's.
/// Scanning the real home from under that override would hand the isolated run
/// the developer's own subscriptions — and then launch them.
fn detection_home_for(override_dir: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    if let Some(root) = override_dir {
        return root.parent().map(Path::to_path_buf).unwrap_or(root);
    }
    home.unwrap_or_else(|| PathBuf::from("/"))
}

/// The dir Claude Code reads when `CLAUDE_CONFIG_DIR` is unset.
///
/// Always the *process's* own home, never the scan root: the two part company
/// under `TAURHAUS_CLAUDE_DIR`, and deriving this from the scan root makes an
/// override that happens to be named `.claude` pass for the default.
fn process_default_config_dir(home: Option<&Path>) -> PathBuf {
    home.map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(DEFAULT_CONFIG_DIRNAME)
}

/// The configured Claude root, but only when Claude Code would not find it on
/// its own.
///
/// `TAURHAUS_CLAUDE_DIR` moves taurhaus's whole Claude root — the teams dir
/// included. Claude Code reads only `CLAUDE_CONFIG_DIR` and otherwise takes the
/// process's own `~/.claude`, so anything that has to launch *into* the
/// configured root has to say so out loud.
pub fn configured_root_to_name() -> Option<PathBuf> {
    let configured = PlatformPaths::claude_dir_override()?;
    let process_default = process_default_config_dir(dirs::home_dir().as_deref());
    (canonical_key(&configured) != canonical_key(&process_default)).then_some(configured)
}

/// A config dir as the shell that runs the launch will read it.
///
/// Launches run in the daemon's filesystem namespace, which on Windows is
/// WSL's. Dirs that arrive from the daemon are already in Linux form and pass
/// through unchanged.
pub fn to_launch_namespace(dir: &Path) -> PathBuf {
    let raw = dir.to_string_lossy().to_string();
    PathBuf::from(crate::provider::path::to_linux(&raw).unwrap_or(raw))
}

/// The newest Claude transcript any of `config_dirs` holds for `project_path`.
///
/// The sightings below only know what this process watched. The transcripts are
/// the durable record: Claude Code writes them to
/// `<config dir>/projects/<slug>/<id>.jsonl` under the config dir the session
/// ran in, so the newest one names the subscription that owns the project's
/// history — after a restart, and on Windows where the app never sees the
/// sessions the daemon scans.
pub fn newest_project_transcript(config_dirs: &[PathBuf], project_path: &str) -> Option<PathBuf> {
    let slug = crate::session_scanner::idle::path_to_slug(project_path);
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for config_dir in config_dirs {
        let Ok(entries) = std::fs::read_dir(config_dir.join(PROJECTS_SUBDIR).join(&slug)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != TRANSCRIPT_EXTENSION)
            {
                continue;
            }
            let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
                continue;
            };
            if newest.as_ref().is_none_or(|(seen, _)| modified > *seen) {
                newest = Some((modified, path));
            }
        }
    }
    newest.map(|(_, path)| path)
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
        .filter(|session| {
            crate::session_scanner::cli_tool::spec(session.cli_tool)
                .capabilities
                .account_selection
        })
        .filter_map(|session| session.jsonl_path)
        .filter_map(|transcript| {
            crate::session_scanner::idle::config_dir_for_transcript(Path::new(&transcript))
        })
        .collect()
}

/// Pick the account one launch runs on.
///
/// Precedence: an explicit request, then the session being resumed, then the
/// project's choice, then `fallback` — the global default, the default config
/// dir while its account can run, or a lone signed-in account. A selection that
/// is gone or signed out takes that same fallback and says which id it was; an
/// empty account list means detection could not run at all, which is not a
/// fallback and renders nothing.
pub fn resolve_launch_account(
    accounts: &[ClaudeAccount],
    request: AccountRequest<'_>,
) -> AccountResolution {
    if accounts.is_empty() {
        // A transcript names its own config dir outright — that answer never
        // needed detection to have worked, and dropping it would resume in
        // some other subscription's history.
        if let Some(config_dir) = request
            .session_transcript
            .and_then(crate::session_scanner::idle::config_dir_for_transcript)
        {
            return AccountResolution {
                config_dir: Some(config_dir),
                account: None,
                source: AccountOrigin::Session,
                fallback_from: None,
                needs_choice: false,
            };
        }
        return AccountResolution {
            config_dir: None,
            account: None,
            source: AccountOrigin::DefaultConfigDir,
            fallback_from: None,
            needs_choice: false,
        };
    }

    // An explicit pick outranks everything, including the session: the user
    // just answered this question for this launch.
    if let Some(wanted) = trimmed(request.requested_account_id) {
        return match usable(accounts, wanted) {
            Some(account) => selected(account, AccountOrigin::Request),
            None => fallback(
                accounts,
                request.default_account_id,
                Some(wanted.to_string()),
            ),
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
            // Leaving the variable off is only correct for the dir Claude Code
            // reads on its own; an unrecognised root is named outright.
            let implicit = account
                .as_ref()
                .is_some_and(|account| account.is_process_default);
            return AccountResolution {
                config_dir: (!implicit).then_some(config_dir),
                account,
                source: AccountOrigin::Session,
                fallback_from: None,
                needs_choice: false,
            };
        }
    }

    if let Some(wanted) = trimmed(request.project_account_id) {
        return match usable(accounts, wanted) {
            Some(account) => selected(account, AccountOrigin::Project),
            None => fallback(
                accounts,
                request.default_account_id,
                Some(wanted.to_string()),
            ),
        };
    }

    fallback(accounts, request.default_account_id, None)
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// The named account, if it is detected and can actually run a session.
fn usable<'a>(accounts: &'a [ClaudeAccount], wanted: &str) -> Option<&'a ClaudeAccount> {
    accounts
        .iter()
        .find(|account| account.id == wanted && account.logged_in)
}

/// The account a launch runs on when nothing usable was selected for it.
///
/// The configured global default answers first — it exists for exactly this
/// case. Then the default config dir, but only while it can run: a detected
/// account that is signed out would greet the user with a login prompt, so any
/// signed-in account is the better answer, and `needs_choice` says the user
/// still owes this launch a real one.
fn fallback(
    accounts: &[ClaudeAccount],
    global_default_id: Option<&str>,
    mut fallback_from: Option<String>,
) -> AccountResolution {
    if let Some(wanted) = trimmed(global_default_id) {
        if let Some(account) = usable(accounts, wanted) {
            return AccountResolution {
                fallback_from,
                ..selected(account, AccountOrigin::GlobalDefault)
            };
        }
        fallback_from = fallback_from.or_else(|| Some(wanted.to_string()));
    }

    // Nothing has selected this launch's account, so more than one subscription
    // that could run it is a question only the user can settle.
    let mut signed_in = accounts.iter().filter(|account| account.logged_in);
    let first_signed_in = signed_in.next();
    let needs_choice = first_signed_in.is_some() && signed_in.next().is_some();

    let default_account = accounts.iter().find(|account| account.is_default);

    // Undetected is not the same as signed out: a config dir detection could
    // not read still launches the way it always has.
    if default_account.is_none_or(|account| account.logged_in) {
        return AccountResolution {
            // The configured root is not necessarily the one Claude Code picks
            // by itself (`TAURHAUS_CLAUDE_DIR`), and only that one may go
            // unnamed.
            config_dir: default_account
                .filter(|account| !account.is_process_default)
                .map(|account| account.config_dir.clone()),
            account: default_account.cloned(),
            source: AccountOrigin::DefaultConfigDir,
            fallback_from,
            needs_choice,
        };
    }

    match first_signed_in {
        Some(account) => AccountResolution {
            fallback_from,
            needs_choice,
            ..selected(account, AccountOrigin::SignedIn)
        },
        // Every detected account is signed out. Nothing here can improve on
        // the default config dir, and the login prompt is the honest outcome.
        None => AccountResolution {
            config_dir: default_account
                .filter(|account| !account.is_process_default)
                .map(|account| account.config_dir.clone()),
            account: default_account.cloned(),
            source: AccountOrigin::DefaultConfigDir,
            fallback_from,
            needs_choice,
        },
    }
}

/// Resolution for an account that was found and can run. The config dir is
/// omitted only for the dir Claude Code reads with `CLAUDE_CONFIG_DIR` unset,
/// so nothing changes for a host with a single subscription.
fn selected(account: &ClaudeAccount, source: AccountOrigin) -> AccountResolution {
    AccountResolution {
        config_dir: (!account.is_process_default).then(|| account.config_dir.clone()),
        account: Some(account.clone()),
        source,
        fallback_from: None,
        needs_choice: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const PRIMARY_ID: &str = "11111111-1111-1111-1111-111111111111";
    const SECOND_ID: &str = "22222222-2222-2222-2222-222222222222";
    const THIRD_ID: &str = "33333333-3333-3333-3333-333333333333";

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

    fn write_project_transcript(config_dir: &Path, project: &str, name: &str) -> PathBuf {
        let dir = config_dir
            .join(PROJECTS_SUBDIR)
            .join(crate::session_scanner::idle::path_to_slug(project));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, "{}\n").unwrap();
        path
    }

    fn set_modified(path: &Path, at: std::time::SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(at)
            .unwrap();
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
    fn account_provider_identifies_fixture_identity_and_credential_expiry() {
        // Regression: commits d6839a3 and a574720 left identity and credential
        // state inside the Claude-only command pipeline instead of the provider.
        let home = TempDir::new().unwrap();
        let dir = write_account(
            home.path(),
            ".claude",
            PRIMARY_ID,
            "fixture@example.com",
            true,
        );
        fs::write(
            dir.join(CREDENTIALS_FILENAME),
            r#"{"claudeAiOauth":{"expiresAt":1788283433000}}"#,
        )
        .unwrap();

        let identity = ClaudeAccountProvider
            .identify(&dir)
            .expect("fixture account");

        assert_eq!(identity.id, PRIMARY_ID);
        assert_eq!(identity.label, "fixture@example.com");
        assert_eq!(identity.plan.as_deref(), Some("claude_max"));
        assert!(identity.logged_in);
        assert_eq!(identity.credential_expires_at, Some(1_788_283_433));
    }

    #[test]
    fn account_provider_empty_home_has_only_the_default_candidate() {
        let home = TempDir::new().unwrap();
        assert_eq!(
            ClaudeAccountProvider.candidate_dirs(home.path(), &[]),
            vec![home.path().join(DEFAULT_CONFIG_DIRNAME)]
        );
    }

    #[test]
    fn account_provider_derives_the_config_dir_from_a_transcript() {
        let transcript = Path::new("/tmp/account/projects/project/session.jsonl");
        assert_eq!(
            ClaudeAccountProvider.session_dir(transcript).as_deref(),
            Some(Path::new("/tmp/account"))
        );
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

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                requested_account_id: Some(SECOND_ID),
                project_account_id: Some(PRIMARY_ID),
                default_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Request);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
        assert_eq!(resolved.fallback_from, None);
    }

    #[test]
    fn the_project_choice_wins_over_the_global_default() {
        let (_home, accounts) = accounts_fixture();

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                default_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Project);
        assert_eq!(
            resolved.account.as_ref().map(|a| a.email.as_str()),
            Some("m.stier@giesi.com")
        );
    }

    #[test]
    fn the_global_default_applies_when_the_project_has_no_choice() {
        let (home, accounts) = accounts_fixture();

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                default_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::GlobalDefault);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
    }

    #[test]
    fn the_default_config_dir_needs_no_prefix() {
        let (_home, accounts) = accounts_fixture();

        let resolved = resolve_launch_account(
            &accounts,
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
        let (_home, accounts) = accounts_fixture();

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                project_account_id: Some("deleted-account"),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::DefaultConfigDir);
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
        let _home = TempDir::new().unwrap();

        let resolved = resolve_launch_account(
            &[],
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::DefaultConfigDir);
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
        let transcript = home
            .path()
            .join(".claude-account2")
            .join("projects")
            .join("-home-user-projects-taurhaus")
            .join("f3286b16-ffc7-4d16-915d-046705823a3d.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                session_transcript: Some(&transcript),
                project_account_id: Some(PRIMARY_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Session);
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
        let transcript = home
            .path()
            .join(".claude-account2")
            .join("projects")
            .join("-home-user-projects-taurhaus")
            .join("f3286b16-ffc7-4d16-915d-046705823a3d.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                requested_account_id: Some(PRIMARY_ID),
                session_transcript: Some(&transcript),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Request);
        assert_eq!(resolved.config_dir, None);
    }

    // Regression: c982822 read `logged_in` as the presence of
    // `<config dir>/.credentials.json`. macOS keeps Claude Code's OAuth tokens
    // in the login keychain and writes no such file, so every account on a Mac
    // came back signed out: the chooser disabled them and every project or
    // global selection fell back to the default config dir.
    #[test]
    fn a_keychain_host_reads_an_account_without_a_credentials_file_as_signed_in() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);
        write_account(
            home.path(),
            ".claude-account2",
            SECOND_ID,
            "b@example.com",
            false,
        );
        let default_dir = home.path().join(".claude");

        let on_file_hosts =
            detect_with_store(home.path(), &[], &default_dir, CredentialStore::File);
        assert!(
            on_file_hosts.iter().all(|account| !account.logged_in),
            "{on_file_hosts:?}"
        );

        let on_macos = detect_with_store(home.path(), &[], &default_dir, CredentialStore::Keychain);
        assert!(
            on_macos.iter().all(|account| account.logged_in),
            "{on_macos:?}"
        );

        let resolved = resolve_launch_account(
            &on_macos,
            AccountRequest {
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Project);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn this_host_resolves_a_keychain_backed_account() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);
        write_account(
            home.path(),
            ".claude-account2",
            SECOND_ID,
            "b@example.com",
            false,
        );
        let default_dir = home.path().join(".claude");

        let accounts = detect_claude_accounts_in(home.path(), &[], &default_dir);

        assert!(accounts.iter().all(|account| account.logged_in));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn this_host_reads_a_missing_credentials_file_as_signed_out() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);
        let default_dir = home.path().join(".claude");

        let accounts = detect_claude_accounts_in(home.path(), &[], &default_dir);

        assert!(accounts.iter().all(|account| !account.logged_in));
    }

    // Regression: c982822 resolved "nothing selected" straight to the physical
    // default config dir without asking whether that account can run, so a
    // signed-out `~/.claude` next to one signed-in sibling launched the dir
    // that only offers a login prompt.
    #[test]
    fn a_signed_out_default_gives_way_to_the_only_usable_account() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);
        write_account(
            home.path(),
            ".claude-account2",
            SECOND_ID,
            "b@example.com",
            true,
        );
        let default_dir = home.path().join(".claude");
        let accounts = detect_with_store(home.path(), &[], &default_dir, CredentialStore::File);

        let resolved = resolve_launch_account(&accounts, request());

        assert_eq!(resolved.source, AccountOrigin::SignedIn);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
        assert_eq!(resolved.fallback_from, None);
    }

    // Regression: c982822 returned the default config dir the moment a stored
    // choice was unusable, skipping the global default the user configured for
    // exactly that case.
    #[test]
    fn a_stale_project_choice_falls_back_to_the_configured_global_default() {
        let (home, accounts) = accounts_fixture();

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                project_account_id: Some("deleted-account"),
                default_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::GlobalDefault);
        assert_eq!(resolved.fallback_from.as_deref(), Some("deleted-account"));
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(home.path().join(".claude-account2").as_path())
        );
    }

    // Regression: 518aace established that a signed-out default config dir must
    // give way, then returned it anyway whenever more than one sibling was
    // signed in — so a project pinned to a deleted account launched straight
    // into the login prompt of an account nobody selected.
    #[test]
    fn a_signed_out_default_never_launches_while_usable_siblings_remain() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", false);
        write_account(home.path(), ".claude-b", SECOND_ID, "b@example.com", true);
        write_account(home.path(), ".claude-c", THIRD_ID, "c@example.com", true);
        let default_dir = home.path().join(".claude");
        let accounts = detect_with_store(home.path(), &[], &default_dir, CredentialStore::File);

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                project_account_id: Some("deleted-account"),
                ..request()
            },
        );

        assert!(
            resolved
                .account
                .as_ref()
                .is_some_and(|account| account.logged_in),
            "{resolved:?}"
        );
        assert_ne!(resolved.config_dir, None, "{resolved:?}");
        // Two siblings could have run this: only the user can say which.
        assert!(resolved.needs_choice, "{resolved:?}");
    }

    // Regression: c982822 scanned `dirs::home_dir()` whatever taurhaus's Claude
    // root was set to, so an isolated run (E2E sets `TAURHAUS_CLAUDE_DIR`)
    // detected — and would launch — the developer's own subscriptions.
    #[test]
    fn an_overridden_claude_root_never_scans_the_real_home() {
        let home = PathBuf::from("/home/dev");
        let isolated = PathBuf::from("/tmp/e2e-run/claude");

        assert_eq!(
            detection_home_for(Some(isolated), Some(home.clone())),
            PathBuf::from("/tmp/e2e-run")
        );
        assert_eq!(detection_home_for(None, Some(home.clone())), home);
    }

    #[test]
    fn the_newest_transcript_names_the_config_dir_that_owns_a_project() {
        let home = TempDir::new().unwrap();
        let project = "/home/user/projects/durable";
        let older = write_project_transcript(&home.path().join(".claude"), project, "old.jsonl");
        let newer =
            write_project_transcript(&home.path().join(".claude-account2"), project, "new.jsonl");
        set_modified(
            &older,
            std::time::SystemTime::now() - std::time::Duration::from_secs(3600),
        );

        let found = newest_project_transcript(
            &[
                home.path().join(".claude"),
                home.path().join(".claude-account2"),
                home.path().join(".claude-missing"),
            ],
            project,
        );

        assert_eq!(found.as_deref(), Some(newer.as_path()));
    }

    #[test]
    fn a_project_with_no_transcript_anywhere_resolves_to_nothing() {
        let home = TempDir::new().unwrap();
        write_project_transcript(&home.path().join(".claude"), "/home/user/other", "a.jsonl");

        assert_eq!(
            newest_project_transcript(&[home.path().join(".claude")], "/home/user/projects/none"),
            None
        );
    }

    // Regression: c982822 omitted `CLAUDE_CONFIG_DIR` for every launch on
    // `PlatformPaths::claude_dir()`, which honours `TAURHAUS_CLAUDE_DIR`.
    // Claude Code reads only `CLAUDE_CONFIG_DIR`: with the variable unset it
    // uses the process's own `~/.claude`, so a configured root was silently
    // swapped for a different subscription.
    #[test]
    fn a_configured_root_that_is_not_the_process_default_is_named_in_the_launch() {
        let (home, _) = accounts_fixture();
        let configured = home.path().join(".claude-account2");
        let accounts = detect_claude_accounts_in(home.path(), &[], &configured);

        let resolved = resolve_launch_account(&accounts, request());

        assert_eq!(resolved.config_dir.as_deref(), Some(configured.as_path()));
    }

    /// The Claude arm of the renderer, as a project launch reaches it.
    fn rendered_claude_command(config_dir: Option<&Path>) -> String {
        crate::session_scanner::launch::LaunchSpec {
            tool: crate::session_scanner::cli_tool::CliTool::Claude,
            mode: crate::daemon::protocol::LaunchMode::Fresh,
            base: "claude --dangerously-skip-permissions",
            model: crate::session_scanner::launch::ModelSpec::default(),
            codex_bypass_hook_trust: false,
            codex_notify_executable: None,
            account_dir: config_dir,
            selector: Some("CLAUDE_CONFIG_DIR"),
            team: None,
        }
        .render()
        .command
    }

    // Regression: 760f776 read "the dir Claude Code uses on its own" off the
    // scan root, which `TAURHAUS_CLAUDE_DIR` moves. An override that itself
    // ends in `.claude` — `/tmp/run/.claude`, the natural shape for an
    // isolated run — therefore passed for the process default, the launch
    // rendered no `CLAUDE_CONFIG_DIR`, and Claude opened the real `~/.claude`
    // subscription instead of the configured root.
    #[test]
    fn an_overridden_root_named_claude_is_still_named_in_the_launch() {
        let run = TempDir::new().unwrap();
        let real_home = TempDir::new().unwrap();
        write_account(
            run.path(),
            ".claude",
            PRIMARY_ID,
            "isolated@example.com",
            true,
        );
        write_account(
            real_home.path(),
            ".claude",
            SECOND_ID,
            "real@example.com",
            true,
        );
        let configured = run.path().join(".claude");

        let accounts = detect_claude_accounts_rooted(
            run.path(),
            &[],
            &configured,
            &real_home.path().join(DEFAULT_CONFIG_DIRNAME),
        );

        assert_eq!(accounts.len(), 1, "{accounts:?}");
        assert!(!accounts[0].is_process_default, "{accounts:?}");

        let resolved = resolve_launch_account(&accounts, request());
        assert_eq!(resolved.config_dir.as_deref(), Some(configured.as_path()));

        let command = rendered_claude_command(resolved.config_dir.as_deref());
        assert!(
            command.starts_with(&format!("CLAUDE_CONFIG_DIR='{}' ", configured.display())),
            "{command}"
        );
    }

    #[test]
    fn the_process_default_config_dir_ignores_the_configured_root() {
        assert_eq!(
            process_default_config_dir(Some(Path::new("/home/dev"))),
            PathBuf::from("/home/dev/.claude")
        );
    }

    // Regression: 760f776 looked for a project's transcripts only under config
    // dirs whose `.claude.json` parsed into an account. Claude Code rewrites
    // that file in place, so a dir read mid-write names nothing — and the scan
    // cached that absence for a minute. The transcripts never moved, but
    // `--resume` stopped seeing them.
    #[test]
    fn a_config_dir_whose_account_file_is_unreadable_still_holds_its_transcripts() {
        let home = TempDir::new().unwrap();
        write_account(home.path(), ".claude", PRIMARY_ID, "a@example.com", true);
        let truncated = home.path().join(".claude-account2");
        fs::create_dir_all(&truncated).unwrap();
        // Caught mid-rewrite: the file exists and parses into nothing.
        fs::write(truncated.join(".claude.json"), "").unwrap();
        let transcript = write_project_transcript(&truncated, "/home/user/projects/mid", "a.jsonl");

        let scan = scan_with_store(
            home.path(),
            &[],
            &home.path().join(".claude"),
            &home.path().join(".claude"),
            CredentialStore::File,
        );

        assert_eq!(scan.accounts.len(), 1, "{:?}", scan.accounts);
        assert!(
            scan.config_dirs.contains(&truncated),
            "{:?}",
            scan.config_dirs
        );
        assert_eq!(
            newest_project_transcript(&scan.config_dirs, "/home/user/projects/mid").as_deref(),
            Some(transcript.as_path())
        );
    }

    // Regression: 760f776 dropped a resume's transcript the moment detection
    // came back empty — every `.claude.json` unreadable at once, an isolated
    // run, a daemon that answers nothing. The transcript names its config dir
    // by itself, and losing it resumes in another subscription's history.
    #[test]
    fn a_transcript_still_places_a_resume_when_no_account_could_be_read() {
        let transcript =
            Path::new("/home/user/.claude-account2/projects/-home-user-projects-x/a.jsonl");

        let resolved = resolve_launch_account(
            &[],
            AccountRequest {
                session_transcript: Some(transcript),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Session);
        assert_eq!(
            resolved.config_dir.as_deref(),
            Some(Path::new("/home/user/.claude-account2"))
        );
    }

    fn claude_session(project: &str, transcript: &str, age_secs: Option<u64>) -> RuntimeSession {
        RuntimeSession {
            pid: 4242,
            project_path: project.to_string(),
            tty: "/dev/pts/3".to_string(),
            args: "claude".to_string(),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: crate::session_scanner::types::SessionState::Idle,
            session_id: None,
            jsonl_path: Some(transcript.to_string()),
            recent_io: false,
            last_output_age_secs: age_secs,
            activity_confidence: Default::default(),
            activity_attribution: Default::default(),
            project_unattributed_active: false,
            group_kind: Default::default(),
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    // Regression: c982822 ranked a project's candidate transcripts with
    // `std::fs::metadata` in the app process. On Windows those are the WSL
    // daemon's Linux paths, which cannot be stat'ed there, so every key came
    // back `None` and an arbitrary session decided the subscription.
    #[test]
    fn the_freshest_session_decides_without_reading_the_transcripts() {
        let project = "/home/user/projects/ranked";
        let stale = "/home/user/.claude/projects/-home-user-projects-ranked/aaa.jsonl";
        let fresh = "/home/user/.claude-account2/projects/-home-user-projects-ranked/bbb.jsonl";

        super::super::record_session_transcripts(&[
            claude_session(project, stale, Some(400)),
            claude_session(project, fresh, Some(3)),
            claude_session(project, "/home/user/.claude/projects/x/ccc.jsonl", None),
        ]);

        assert_eq!(
            super::super::remembered_transcript(CliTool::Claude, project).as_deref(),
            Some(Path::new(fresh))
        );
    }

    // Regression: c982822 read the resume transcript out of the live runtime
    // snapshot only. Resume is normally reached for once Claude has exited, and
    // the session is gone from that snapshot by then, so the account the
    // history belongs to was lost exactly when it was needed.
    #[test]
    fn a_sighting_outlives_the_session_that_produced_it() {
        let project = "/home/user/projects/outlives";
        let transcript =
            "/home/user/.claude-account2/projects/-home-user-projects-outlives/a.jsonl";
        super::super::record_session_transcripts(&[claude_session(project, transcript, Some(2))]);

        // The next snapshot no longer lists the session: Claude exited.
        super::super::record_session_transcripts(&[]);

        assert_eq!(
            super::super::remembered_transcript(CliTool::Claude, project).as_deref(),
            Some(Path::new(transcript))
        );
    }

    // Regression: 518aace replaced a project's sighting on every snapshot that
    // mentioned it. A fresh session on one subscription that exits while an
    // older idle pane on another survives is exactly that: the next snapshot
    // carries only the older pane, and it overwrote the account the history
    // actually belongs to.
    #[test]
    fn an_older_surviving_session_never_overwrites_a_fresher_sighting() {
        let project = "/home/user/projects/sequential";
        let fresh = "/home/user/.claude-account2/projects/-home-user-projects-sequential/b.jsonl";
        let stale = "/home/user/.claude/projects/-home-user-projects-sequential/a.jsonl";

        super::super::record_session_transcripts(&[
            claude_session(project, fresh, Some(2)),
            claude_session(project, stale, Some(400)),
        ]);
        // The fresh session exits; the older pane is still open and is all the
        // next snapshot reports.
        super::super::record_session_transcripts(&[claude_session(project, stale, Some(430))]);

        assert_eq!(
            super::super::remembered_transcript(CliTool::Claude, project).as_deref(),
            Some(Path::new(fresh))
        );
    }

    #[test]
    fn a_transcript_outside_a_known_layout_leaves_the_project_choice_alone() {
        let (home, accounts) = accounts_fixture();
        let transcript = home.path().join("stray.jsonl");

        let resolved = resolve_launch_account(
            &accounts,
            AccountRequest {
                session_transcript: Some(&transcript),
                project_account_id: Some(SECOND_ID),
                ..request()
            },
        );

        assert_eq!(resolved.source, AccountOrigin::Project);
    }

    struct FakeHttp {
        status: u16,
        body: String,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl super::super::HttpClient for FakeHttp {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _timeout: std::time::Duration,
        ) -> Result<super::super::HttpResponse, super::super::HttpError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(super::super::HttpResponse {
                status: self.status,
                body: self.body.clone(),
            })
        }
    }

    #[test]
    fn oauth_usage_fixture_normalizes_windows_in_provider_order() {
        // Regression: a574720 modeled usage as two status-line fields; the
        // OAuth response adds an ordered scoped weekly window that must survive.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(CREDENTIALS_FILENAME),
            r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":4102444800000}}"#,
        )
        .unwrap();
        let http = FakeHttp {
            status: 200,
            body: include_str!("../../daemon/fixtures/claude-oauth-usage-2.1.247.json").to_string(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };

        let snapshot = ClaudeUsageProvider.fetch(dir.path(), &http);
        assert_eq!(snapshot.status, UsageStatus::Ok);
        assert_eq!(
            snapshot
                .windows
                .iter()
                .map(|window| (
                    window.key.as_str(),
                    window.title.as_str(),
                    window.used_percentage
                ))
                .collect::<Vec<_>>(),
            vec![
                ("session", "Current session", 3.0),
                ("weekly_all", "Current week (all models)", 28.0),
                ("weekly_scoped", "Current week (Fable)", 29.0),
            ]
        );
        assert!(snapshot
            .windows
            .iter()
            .all(|window| window.resets_at.is_some()));
    }

    #[test]
    fn expired_credentials_return_unauthorized_without_http() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(CREDENTIALS_FILENAME),
            r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":1}}"#,
        )
        .unwrap();
        let http = FakeHttp {
            status: 200,
            body: "{}".to_string(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };

        let snapshot = ClaudeUsageProvider.fetch(dir.path(), &http);
        assert_eq!(snapshot.status, UsageStatus::Unauthorized);
        assert_eq!(http.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn oauth_usage_without_limits_uses_legacy_mirrors() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(CREDENTIALS_FILENAME),
            r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":4102444800000}}"#,
        )
        .unwrap();
        let http = FakeHttp {
            status: 200,
            body: r#"{"five_hour":{"utilization":7,"resets_at":null},"seven_day":{"utilization":14,"resets_at":null},"seven_day_sonnet":null}"#.to_string(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let snapshot = ClaudeUsageProvider.fetch(dir.path(), &http);
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].used_percentage, 7.0);
        assert_eq!(snapshot.windows[1].used_percentage, 14.0);
    }

    #[test]
    fn oauth_usage_401_is_unauthorized() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(CREDENTIALS_FILENAME),
            r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":4102444800000}}"#,
        )
        .unwrap();
        let http = FakeHttp {
            status: 401,
            body: "{}".to_string(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        assert_eq!(
            ClaudeUsageProvider.fetch(dir.path(), &http).status,
            UsageStatus::Unauthorized
        );
    }
}
