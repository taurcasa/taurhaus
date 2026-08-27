//! Tool-agnostic account and subscription-usage contracts.
//!
//! Per-tool implementations live in sibling modules. Consumers use the
//! registry-provided traits and these normalised wire types.

pub mod claude;
pub mod legacy_statusline;

use std::collections::HashMap;
#[cfg(not(test))]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(not(test))]
use std::sync::OnceLock;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::cli_tool::SessionRoot;
use super::cli_tool::{spec, CliTool};
use super::RuntimeSession;

/// Where a launch's account came from. Ordered by precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOrigin {
    /// The user picked it for this launch.
    Request,
    /// Derived from the transcript of the session being resumed.
    Session,
    /// The project's stored pin.
    Project,
    /// The last account observed for this project and tool.
    LastUsed,
    /// The global default account.
    GlobalDefault,
    /// Selected by an account selector already present in the base command.
    BaseCommand,
    /// A usable detected account used because the default dir is signed out.
    SignedIn,
    /// Nothing selected an account: the tool's default directory.
    DefaultConfigDir,
}

impl AccountOrigin {
    /// Stable wire name used by the frontend and structured logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Session => "session",
            Self::Project => "project",
            Self::LastUsed => "last_used",
            Self::GlobalDefault => "global_default",
            Self::BaseCommand => "base_command",
            Self::SignedIn => "signed_in",
            Self::DefaultConfigDir => "default_config_dir",
        }
    }
}

/// Normalised display identity returned by an account provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdentity {
    /// Provider-stable account identifier. It is copied to [`Account::id`]
    /// and intentionally omitted from the nested wire object.
    #[serde(skip)]
    pub id: String,
    pub label: String,
    pub display_name: Option<String>,
    pub organization: Option<String>,
    pub plan: Option<String>,
    pub logged_in: bool,
    pub credential_expires_at: Option<i64>,
}

/// One detected account for one CLI tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub tool: CliTool,
    pub id: String,
    pub dir: PathBuf,
    pub identity: AccountIdentity,
    pub is_default: bool,
    pub is_process_default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageSnapshot>,
}

/// One scan, including dirs whose identity file was temporarily unreadable.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountScan {
    pub config_dirs: Vec<PathBuf>,
    pub accounts: Vec<Account>,
}

/// Every input that may choose the account for one launch.
#[derive(Clone, Copy, Default)]
pub struct AccountRequest<'a> {
    pub requested_account_id: Option<&'a str>,
    pub session_transcript: Option<&'a Path>,
    pub pinned_account_id: Option<&'a str>,
    pub last_used_account_id: Option<&'a str>,
    pub default_account_id: Option<&'a str>,
    pub base_command: Option<&'a str>,
    pub selector: Option<&'a str>,
}

/// Account and selector directory chosen for one launch.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountResolution {
    pub account_dir: Option<PathBuf>,
    pub account: Option<Account>,
    pub origin: AccountOrigin,
    pub fallback_from: Option<String>,
    pub needs_choice: bool,
}

/// Resolve account precedence without branching on tool identity.
pub fn resolve_launch_account(
    accounts: &[Account],
    provider: &dyn AccountProvider,
    request: AccountRequest<'_>,
) -> AccountResolution {
    let mut fallback_from = None;

    if let Some(wanted) = non_empty(request.requested_account_id) {
        if let Some(account) = usable(accounts, wanted) {
            return selected(account, AccountOrigin::Request, false, None);
        }
        fallback_from = Some(wanted.to_string());
    }

    if let Some(transcript) = request.session_transcript {
        if let Some(dir) = provider.session_dir(transcript) {
            let account = account_for_dir(accounts, &dir);
            match account {
                Some(account) if account.identity.logged_in => {
                    return selected(account, AccountOrigin::Session, false, fallback_from);
                }
                Some(account) => {
                    fallback_from.get_or_insert_with(|| account.id.clone());
                }
                None => {
                    return AccountResolution {
                        account_dir: Some(dir),
                        account: None,
                        origin: AccountOrigin::Session,
                        fallback_from,
                        needs_choice: false,
                    };
                }
            }
        }
    }

    for (wanted, origin) in [
        (request.pinned_account_id, AccountOrigin::Project),
        (request.last_used_account_id, AccountOrigin::LastUsed),
        (request.default_account_id, AccountOrigin::GlobalDefault),
    ] {
        let Some(wanted) = non_empty(wanted) else {
            continue;
        };
        if let Some(account) = usable(accounts, wanted) {
            return selected(account, origin, false, fallback_from);
        }
        fallback_from.get_or_insert_with(|| wanted.to_string());
    }

    if let (Some(base), Some(selector)) = (request.base_command, request.selector) {
        if let Some(assignment) = command_env_assignment(base, selector) {
            match assignment {
                Some(dir) => match account_for_dir(accounts, &dir) {
                    Some(account) if account.identity.logged_in => {
                        return selected(account, AccountOrigin::BaseCommand, true, fallback_from);
                    }
                    Some(account) => {
                        fallback_from.get_or_insert_with(|| account.id.clone());
                    }
                    None => {
                        return AccountResolution {
                            account_dir: None,
                            account: None,
                            origin: AccountOrigin::BaseCommand,
                            fallback_from,
                            needs_choice: false,
                        };
                    }
                },
                None => {
                    return AccountResolution {
                        account_dir: None,
                        account: None,
                        origin: AccountOrigin::BaseCommand,
                        fallback_from,
                        needs_choice: false,
                    };
                }
            }
        }
    }

    let needs_choice = accounts
        .iter()
        .filter(|account| account.identity.logged_in)
        .take(2)
        .count()
        >= 2;
    let default_account = accounts.iter().find(|account| account.is_default);
    if default_account.is_none_or(|account| account.identity.logged_in) {
        return AccountResolution {
            account_dir: default_account
                .filter(|account| !account.is_process_default)
                .map(|account| account.dir.clone()),
            account: default_account.cloned(),
            origin: AccountOrigin::DefaultConfigDir,
            fallback_from,
            needs_choice,
        };
    }
    if let Some(account) = accounts.iter().find(|account| account.identity.logged_in) {
        return selected(account, AccountOrigin::SignedIn, false, fallback_from);
    }
    AccountResolution {
        account_dir: default_account
            .filter(|account| !account.is_process_default)
            .map(|account| account.dir.clone()),
        account: default_account.cloned(),
        origin: AccountOrigin::DefaultConfigDir,
        fallback_from,
        needs_choice,
    }
}

fn selected(
    account: &Account,
    origin: AccountOrigin,
    base_command_owns_selector: bool,
    fallback_from: Option<String>,
) -> AccountResolution {
    AccountResolution {
        account_dir: (!base_command_owns_selector && !account.is_process_default)
            .then(|| account.dir.clone()),
        account: Some(account.clone()),
        origin,
        fallback_from,
        needs_choice: false,
    }
}

fn usable<'a>(accounts: &'a [Account], wanted: &str) -> Option<&'a Account> {
    accounts
        .iter()
        .find(|account| account.id == wanted && account.identity.logged_in)
}

fn account_for_dir<'a>(accounts: &'a [Account], dir: &Path) -> Option<&'a Account> {
    let wanted = path_key(dir);
    accounts
        .iter()
        .find(|account| path_key(&account.dir) == wanted)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn path_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A configured tool root that differs from the selector-free process default.
pub fn configured_default_dir(tool: CliTool) -> Option<PathBuf> {
    let tool_spec = spec(tool);
    let provider = tool_spec.account_provider()?;
    let home = dirs::home_dir()?;
    let process_default = provider.default_dir(&home);
    let configured = match tool_spec.capabilities.session_root {
        SessionRoot::AppManagedClaudeDir => {
            crate::provider::platform_paths::PlatformPaths::claude_dir_override()
        }
        SessionRoot::ToolHome => None,
    }?;
    (path_key(&configured) != path_key(&process_default)).then_some(configured)
}

/// Convert an account dir into the namespace used by the launch shell.
pub fn to_launch_namespace(dir: &Path) -> PathBuf {
    let raw = dir.to_string_lossy().into_owned();
    PathBuf::from(crate::provider::path::to_linux(&raw).unwrap_or(raw))
}

/// Whether a shell base contains an assignment for the account selector.
pub fn command_contains_env(command: &str, selector: &str) -> bool {
    command_env_assignment(command, selector).is_some()
}

/// `None` = absent; `Some(None)` = present but not parseable as a directory.
fn command_env_assignment(command: &str, selector: &str) -> Option<Option<PathBuf>> {
    let prefix = format!("{selector}=");
    shell_words(command).into_iter().find_map(|word| {
        word.strip_prefix(&prefix).map(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| PathBuf::from(value))
        })
    })
}

fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut characters = command.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            character if character.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                for quoted in characters.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    word.push(quoted);
                }
            }
            '"' => {
                started = true;
                while let Some(quoted) = characters.next() {
                    match quoted {
                        '"' => break,
                        '\\' if matches!(characters.peek(), Some('"' | '\\' | '$' | '`')) => {
                            word.extend(characters.next());
                        }
                        quoted => word.push(quoted),
                    }
                }
            }
            '\\' => {
                started = true;
                word.extend(characters.next());
            }
            character => {
                started = true;
                word.push(character);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

#[cfg(not(test))]
const DETECTION_TTL: Duration = Duration::from_secs(60);

#[cfg(not(test))]
static DETECTION_CACHE: Mutex<Option<HashMap<CliTool, (Instant, AccountScan)>>> = Mutex::new(None);

#[cfg(not(test))]
static MISSING_PROVIDER_FLOORS: OnceLock<Mutex<HashSet<CliTool>>> = OnceLock::new();

#[cfg(test)]
static DETECTION_OVERRIDE: Mutex<Option<HashMap<CliTool, AccountScan>>> = Mutex::new(None);

#[cfg(test)]
static DETECTION_OVERRIDE_LOCK: Mutex<()> = Mutex::new(());

/// Keeps a test-owned account scan installed without reading a real CLI home.
#[cfg(test)]
pub(crate) struct DetectionOverrideGuard {
    tool: CliTool,
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for DetectionOverrideGuard {
    fn drop(&mut self) {
        let mut overrides = DETECTION_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(overrides) = overrides.as_mut() {
            overrides.remove(&self.tool);
        }
    }
}

/// Install a fixture scan. Tests use this seam instead of any real home.
#[cfg(test)]
pub(crate) fn install_detection_override(
    tool: CliTool,
    scan: AccountScan,
) -> DetectionOverrideGuard {
    let lock = DETECTION_OVERRIDE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    DETECTION_OVERRIDE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(tool, scan);
    DetectionOverrideGuard { tool, _lock: lock }
}

/// Detect accounts for one tool, cached for one minute per registry entry.
pub fn detect(tool: CliTool) -> Vec<Account> {
    scan(tool).accounts
}

/// Candidate dirs from the latest detection pass, including unidentified dirs.
pub fn transcript_dirs(tool: CliTool) -> Vec<PathBuf> {
    scan(tool).config_dirs
}

fn scan(tool: CliTool) -> AccountScan {
    #[cfg(test)]
    {
        return DETECTION_OVERRIDE
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .and_then(|overrides| overrides.get(&tool))
            .cloned()
            .unwrap_or_default();
    }

    #[cfg(not(test))]
    {
        let mut cache = DETECTION_CACHE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let cache = cache.get_or_insert_with(HashMap::new);
        if let Some((observed_at, scan)) = cache.get(&tool) {
            if observed_at.elapsed() < DETECTION_TTL {
                return scan.clone();
            }
        }

        let result = scan_uncached(tool);
        cache.insert(tool, (Instant::now(), result.clone()));
        result
    }
}

#[cfg(not(test))]
fn scan_uncached(tool: CliTool) -> AccountScan {
    let tool_spec = spec(tool);
    let Some(provider) = tool_spec.account_provider() else {
        if let Some(selector) = tool_spec.capabilities.account_selector {
            let first = MISSING_PROVIDER_FLOORS
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(tool);
            if first {
                tracing::info!(
                    event = "account.provider.floor",
                    tool = %tool,
                    selector,
                    "account selector is declared but its provider has not landed"
                );
            }
        }
        return AccountScan::default();
    };

    let process_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let process_default = provider.default_dir(&process_home);
    let configured_default =
        if tool_spec.capabilities.session_root == SessionRoot::AppManagedClaudeDir {
            crate::provider::platform_paths::PlatformPaths::claude_dir()
        } else {
            process_default.clone()
        };
    let scan_home = if canonical_key(&configured_default) == canonical_key(&process_default) {
        process_home
    } else {
        configured_default
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| configured_default.clone())
    };

    let mut live_selector_values = live_selector_values(tool, provider);
    live_selector_values.push(configured_default.clone());
    let configured_key = canonical_key(&configured_default);
    let process_key = canonical_key(&process_default);
    let mut config_dirs = Vec::new();
    let mut accounts = Vec::new();

    for dir in provider.candidate_dirs(&scan_home, &live_selector_values) {
        let key = canonical_key(&dir);
        if dir.is_dir() {
            config_dirs.push(dir.clone());
        }
        let Some(identity) = provider.identify(&dir) else {
            continue;
        };
        accounts.push(Account {
            tool,
            id: identity.id.clone(),
            dir,
            identity,
            is_default: key == configured_key,
            is_process_default: key == process_key,
            usage: None,
        });
    }

    accounts.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.identity.label.cmp(&right.identity.label))
    });
    tracing::debug!(tool = %tool, accounts = accounts.len(), config_dirs = config_dirs.len(), "scanned account config dirs");
    AccountScan {
        config_dirs,
        accounts,
    }
}

#[cfg(not(test))]
fn live_selector_values(tool: CliTool, provider: &dyn AccountProvider) -> Vec<PathBuf> {
    crate::session_scanner::latest_compaction_runtime_sessions()
        .into_iter()
        .filter(|session| session.cli_tool == tool)
        .filter_map(|session| session.jsonl_path)
        .filter_map(|transcript| provider.session_dir(Path::new(&transcript)))
        .collect()
}

#[cfg(not(test))]
pub(crate) fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The freshest transcript observed for one project and tool.
struct TranscriptSighting {
    tool: CliTool,
    project_key: String,
    transcript: PathBuf,
    last_output_at: Option<u64>,
}

/// Runtime sightings outlive the process that produced them so resume can
/// recover the account after the session exits.
static TRANSCRIPT_SIGHTINGS: Mutex<Vec<TranscriptSighting>> = Mutex::new(Vec::new());

/// Remember the freshest provider-backed transcript per project and tool.
pub(crate) fn record_session_transcripts(sessions: &[RuntimeSession]) {
    let now = unix_now();
    let mut freshest: Vec<(CliTool, String, &str, Option<u64>)> = Vec::new();
    for session in sessions
        .iter()
        .filter(|session| spec(session.cli_tool).account_provider().is_some())
    {
        let Some(transcript) = session.jsonl_path.as_deref() else {
            continue;
        };
        let key = crate::provider::path::normalize_project_path(&session.project_path);
        let last_output_at = session
            .last_output_age_secs
            .map(|age| now.saturating_sub(age));
        match freshest
            .iter_mut()
            .find(|(tool, seen, _, _)| *tool == session.cli_tool && *seen == key)
        {
            Some(entry) if newer(last_output_at, entry.3) => {
                entry.2 = transcript;
                entry.3 = last_output_at;
            }
            Some(_) => {}
            None => freshest.push((session.cli_tool, key, transcript, last_output_at)),
        }
    }

    let mut sightings = TRANSCRIPT_SIGHTINGS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for (tool, project_key, transcript, last_output_at) in freshest {
        let transcript = PathBuf::from(transcript);
        match sightings
            .iter_mut()
            .find(|sighting| sighting.tool == tool && sighting.project_key == project_key)
        {
            Some(sighting) if newer(last_output_at, sighting.last_output_at) => {
                sighting.transcript = transcript;
                sighting.last_output_at = last_output_at;
            }
            Some(_) => {}
            None => sightings.push(TranscriptSighting {
                tool,
                project_key,
                transcript,
                last_output_at,
            }),
        }
    }
}

/// The transcript most recently seen for one project and tool.
pub(crate) fn remembered_transcript(tool: CliTool, project_path: &str) -> Option<PathBuf> {
    let project_key = crate::provider::path::normalize_project_path(project_path);
    TRANSCRIPT_SIGHTINGS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .find(|sighting| sighting.tool == tool && sighting.project_key == project_key)
        .map(|sighting| sighting.transcript.clone())
}

fn newer(left: Option<u64>, right: Option<u64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left >= right,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => true,
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct LastUsedWrite {
    selector_key: String,
    checked_at: Instant,
}

#[cfg(not(test))]
static LAST_USED_WRITES: Mutex<Option<HashMap<(String, CliTool), LastUsedWrite>>> =
    Mutex::new(None);

fn live_account_check_is_due(
    checks: &HashMap<(String, CliTool), LastUsedWrite>,
    key: &(String, CliTool),
    selector_key: Option<&str>,
    now: Instant,
) -> bool {
    checks.get(key).is_none_or(|write| {
        selector_key.is_some_and(|selector| selector != write.selector_key)
            || now.saturating_duration_since(write.checked_at) >= Duration::from_secs(60)
    })
}

/// Persist a launch observation through the app-owned database connection
/// without disturbing a user pin.
pub(crate) fn remember_last_used_in(
    connection: &rusqlite::Connection,
    project_id: &str,
    tool: CliTool,
    account_id: &str,
) -> Result<bool, String> {
    crate::db::queries::remember_last_used_account(
        connection,
        project_id,
        &tool.to_string(),
        account_id,
    )
    .map_err(|error| error.to_string())
}

/// A scanner observation that the app may fold into project account memory.
///
/// The scanner owns process inspection; the app owns SQLite. This small wire
/// value is the boundary between them and never contains credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAccountObservation {
    pub project_path: String,
    pub tool: CliTool,
    pub account_id: String,
}

struct AccountPersistenceCheck {
    account_id: String,
    checked_at: Instant,
}

static LAST_PERSISTED_OBSERVATIONS: Mutex<
    Option<HashMap<(String, CliTool), AccountPersistenceCheck>>,
> = Mutex::new(None);

struct ExaminedLiveAccount {
    throttle_key: (String, CliTool),
    selector_key: String,
    observation: Option<LiveAccountObservation>,
}

fn finish_live_account_checks(
    checks: &mut HashMap<(String, CliTool), LastUsedWrite>,
    examined: Vec<ExaminedLiveAccount>,
    now: Instant,
) -> Vec<LiveAccountObservation> {
    examined
        .into_iter()
        .filter_map(|examined| {
            checks.insert(
                examined.throttle_key,
                LastUsedWrite {
                    selector_key: examined.selector_key,
                    checked_at: now,
                },
            );
            examined.observation
        })
        .collect()
}

/// Resolve live selector values on the scanner side and emit memory
/// observations for the app. No database is opened here.
#[cfg(not(test))]
pub(crate) fn observe_live_session_accounts(
    sessions: &[RuntimeSession],
) -> Vec<LiveAccountObservation> {
    if cfg!(target_os = "windows") {
        return Vec::new();
    }

    let now = Instant::now();
    let mut seen = HashSet::new();
    let candidates = sessions
        .iter()
        .filter_map(|session| {
            let tool = session.cli_tool;
            let tool_spec = spec(tool);
            let (Some(selector), Some(provider)) = (
                tool_spec.capabilities.account_selector,
                tool_spec.account_provider(),
            ) else {
                return None;
            };
            let throttle_key = (
                crate::provider::path::normalize_project_path(&session.project_path),
                tool,
            );
            if !seen.insert(throttle_key.clone()) {
                return None;
            }
            let selected_hint = session
                .jsonl_path
                .as_deref()
                .and_then(|transcript| provider.session_dir(Path::new(transcript)));
            let selector_key = selected_hint.as_ref().map(|dir| {
                crate::provider::path::normalize_project_path(&dir.display().to_string())
            });
            let due = LAST_USED_WRITES
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_ref()
                .is_none_or(|checks| {
                    live_account_check_is_due(checks, &throttle_key, selector_key.as_deref(), now)
                });
            due.then_some((
                session,
                tool,
                selector,
                provider,
                throttle_key,
                selected_hint,
            ))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut account_dirs: HashMap<CliTool, Vec<(PathBuf, String)>> = HashMap::new();
    for (_, tool, _, _, _, _) in &candidates {
        account_dirs.entry(*tool).or_insert_with(|| {
            detect(*tool)
                .into_iter()
                .map(|account| (canonical_key(&account.dir), account.id))
                .collect()
        });
    }

    let examined = candidates
        .into_iter()
        .map(
            |(session, tool, selector, provider, throttle_key, selected_hint)| {
                let selected_dir = selected_hint
                    .or_else(|| super::process::process_selector_value(session.pid, selector))
                    .or_else(|| dirs::home_dir().map(|home| provider.default_dir(&home)));
                let selector_key = selected_dir
                    .as_ref()
                    .map(|dir| {
                        crate::provider::path::normalize_project_path(&dir.display().to_string())
                    })
                    .unwrap_or_default();
                let observation = selected_dir.and_then(|selected_dir| {
                    let selected_key = canonical_key(&selected_dir);
                    let account_id = account_dirs
                        .get(&tool)?
                        .iter()
                        .find_map(|(dir, id)| (*dir == selected_key).then(|| id.clone()))?;
                    Some(LiveAccountObservation {
                        project_path: session.project_path.clone(),
                        tool,
                        account_id,
                    })
                });
                ExaminedLiveAccount {
                    throttle_key,
                    selector_key,
                    observation,
                }
            },
        )
        .collect::<Vec<_>>();

    let mut writes = LAST_USED_WRITES
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let writes = writes.get_or_insert_with(HashMap::new);
    finish_live_account_checks(writes, examined, now)
}

fn project_ids_by_path(connection: &rusqlite::Connection) -> HashMap<String, String> {
    let Ok(mut statement) = connection.prepare("SELECT id, path FROM projects") else {
        return HashMap::new();
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return HashMap::new();
    };
    rows.filter_map(Result::ok)
        .map(|(id, path)| (crate::provider::path::normalize_project_path(&path), id))
        .collect()
}

/// Persist scanner observations through the app's managed SQLite connection.
pub(crate) fn persist_live_account_observations_in(
    connection: &rusqlite::Connection,
    observations: &[LiveAccountObservation],
) -> Result<usize, String> {
    let mut checks = LAST_PERSISTED_OBSERVATIONS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let checks = checks.get_or_insert_with(HashMap::new);
    persist_live_account_observations_with_checks(connection, observations, checks, Instant::now())
}

fn persist_live_account_observations_with_checks(
    connection: &rusqlite::Connection,
    observations: &[LiveAccountObservation],
    checks: &mut HashMap<(String, CliTool), AccountPersistenceCheck>,
    now: Instant,
) -> Result<usize, String> {
    let due = observations
        .iter()
        .filter_map(|observation| {
            let project_key =
                crate::provider::path::normalize_project_path(&observation.project_path);
            let key = (project_key, observation.tool);
            let is_due = checks.get(&key).is_none_or(|check| {
                check.account_id != observation.account_id
                    || now.saturating_duration_since(check.checked_at) >= Duration::from_secs(60)
            });
            if !is_due {
                return None;
            }
            checks.insert(
                key.clone(),
                AccountPersistenceCheck {
                    account_id: observation.account_id.clone(),
                    checked_at: now,
                },
            );
            Some((key.0, observation))
        })
        .collect::<Vec<_>>();
    if due.is_empty() {
        return Ok(0);
    }
    let project_ids = project_ids_by_path(connection);
    let mut persisted = 0;
    for (project_key, observation) in due {
        let Some(project_id) = project_ids.get(&project_key) else {
            continue;
        };
        if remember_last_used_in(
            connection,
            project_id,
            observation.tool,
            &observation.account_id,
        )? {
            persisted += 1;
        }
    }
    Ok(persisted)
}

/// Resolve the display label for the account that owns an archived transcript.
pub fn account_label_for_session(
    tool: CliTool,
    project_path: &str,
    session_id: &str,
) -> Option<String> {
    let tool_spec = spec(tool);
    let provider = tool_spec.account_provider()?;
    let scan = scan(tool);
    let slug = crate::session_scanner::idle::path_to_slug(project_path);
    let transcript_name = format!("{session_id}.{}", tool_spec.session_extension);
    let owner = scan.config_dirs.iter().find_map(|config_dir| {
        let transcript = config_dir
            .join(tool_spec.projects_subdir)
            .join(&slug)
            .join(&transcript_name);
        transcript
            .is_file()
            .then(|| provider.session_dir(&transcript))
            .flatten()
    })?;
    let account = account_for_dir(&scan.accounts, &owner)?;
    Some(
        account
            .identity
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&account.identity.label)
            .to_string(),
    )
}

#[cfg(test)]
pub(crate) fn observe_live_session_accounts(
    _sessions: &[RuntimeSession],
) -> Vec<LiveAccountObservation> {
    Vec::new()
}

/// Newest transcript for a project across all candidate account dirs.
pub fn newest_project_transcript(
    tool: CliTool,
    config_dirs: &[PathBuf],
    project_path: &str,
) -> Option<PathBuf> {
    let tool_spec = spec(tool);
    let slug = crate::session_scanner::idle::path_to_slug(project_path);
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for config_dir in config_dirs {
        let Ok(entries) = std::fs::read_dir(config_dir.join(tool_spec.projects_subdir).join(&slug))
        else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != tool_spec.session_extension)
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

/// Result status for a usage observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Ok,
    Stale,
    Unauthorized,
    Unsupported,
}

/// Provider-supplied meter severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Normal,
    Warning,
    Critical,
}

/// One ordered usage window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub key: String,
    pub title: String,
    pub used_percentage: f64,
    pub resets_at: Option<i64>,
    pub severity: Severity,
    pub is_active: bool,
}

/// A provider's latest normalised usage observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub observed_at: DateTime<Utc>,
    pub status: UsageStatus,
    pub windows: Vec<UsageWindow>,
    pub note: Option<String>,
}

/// Minimal response exposed to usage providers.
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Failure kind safe to log without request headers or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpErrorKind {
    Network,
    Timeout,
}

/// HTTP failure safe to pass across the provider boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpError {
    pub kind: HttpErrorKind,
}

/// Injectable HTTP seam. Tests provide fakes and never call live endpoints.
pub trait HttpClient: Sync {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, HttpError>;
}

/// Production blocking client. Calls run only on the usage poller thread.
pub struct ReqwestHttpClient;

static REQWEST_HTTP_CLIENT: LazyLock<Option<reqwest::blocking::Client>> =
    LazyLock::new(|| reqwest::blocking::Client::builder().build().ok());

impl HttpClient for ReqwestHttpClient {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, HttpError> {
        let client = REQWEST_HTTP_CLIENT.as_ref().ok_or(HttpError {
            kind: HttpErrorKind::Network,
        })?;
        let mut request = client.get(url).timeout(timeout);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|error| HttpError {
            kind: if error.is_timeout() {
                HttpErrorKind::Timeout
            } else {
                HttpErrorKind::Network
            },
        })?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|_| HttpError {
            kind: HttpErrorKind::Network,
        })?;
        Ok(HttpResponse { status, body })
    }
}

/// Per-tool account detection and resume derivation.
pub trait AccountProvider: Sync {
    fn default_dir(&self, home: &Path) -> PathBuf;
    fn candidate_dirs(&self, home: &Path, live_selector_values: &[PathBuf]) -> Vec<PathBuf>;
    fn identify(&self, dir: &Path) -> Option<AccountIdentity>;
    fn session_dir(&self, transcript: &Path) -> Option<PathBuf>;
}

/// Per-tool subscription-usage fetch and normalisation.
pub trait UsageProvider: Sync {
    fn credential_path(&self, dir: &Path) -> Option<PathBuf>;
    fn fetch(&self, dir: &Path, http: &dyn HttpClient) -> UsageSnapshot;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeProvider;

    impl AccountProvider for FakeProvider {
        fn default_dir(&self, home: &Path) -> PathBuf {
            home.join("default")
        }

        fn candidate_dirs(&self, _home: &Path, _live: &[PathBuf]) -> Vec<PathBuf> {
            Vec::new()
        }

        fn identify(&self, _dir: &Path) -> Option<AccountIdentity> {
            None
        }

        fn session_dir(&self, transcript: &Path) -> Option<PathBuf> {
            transcript.parent().map(Path::to_path_buf)
        }
    }

    fn account(id: &str, dir: &str, is_default: bool) -> Account {
        Account {
            tool: CliTool::Claude,
            id: id.to_string(),
            dir: PathBuf::from(dir),
            identity: AccountIdentity {
                id: id.to_string(),
                label: format!("{id}@example.com"),
                display_name: None,
                organization: None,
                plan: None,
                logged_in: true,
                credential_expires_at: None,
            },
            is_default,
            is_process_default: is_default,
            usage: None,
        }
    }

    fn fixture() -> Vec<Account> {
        vec![
            account("default", "/accounts/default", true),
            account("pinned", "/accounts/pinned", false),
            account("last", "/accounts/last", false),
            account("explicit", "/accounts/explicit", false),
        ]
    }

    #[test]
    fn account_origin_keeps_shipped_wire_names_and_adds_generic_memory_sources() {
        // Regression: commit d6839a3 shipped these launch-provenance strings;
        // renaming the enum for provider generalisation must not change them.
        assert_eq!(AccountOrigin::Request.as_str(), "request");
        assert_eq!(AccountOrigin::Session.as_str(), "session");
        assert_eq!(AccountOrigin::Project.as_str(), "project");
        assert_eq!(AccountOrigin::GlobalDefault.as_str(), "global_default");
        assert_eq!(AccountOrigin::SignedIn.as_str(), "signed_in");
        assert_eq!(
            AccountOrigin::DefaultConfigDir.as_str(),
            "default_config_dir"
        );
        assert_eq!(AccountOrigin::LastUsed.as_str(), "last_used");
        assert_eq!(AccountOrigin::BaseCommand.as_str(), "base_command");
    }

    #[test]
    fn resolution_precedence_starts_with_the_explicit_request() {
        // Regression: d6839a3 distributed account precedence between the
        // frontend, command layer, and Claude detector; the generic resolver
        // must keep one ordered contract.
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                requested_account_id: Some("explicit"),
                session_transcript: Some(Path::new("/accounts/last/session.jsonl")),
                pinned_account_id: Some("pinned"),
                last_used_account_id: Some("last"),
                default_account_id: Some("default"),
                ..Default::default()
            },
        );
        assert_eq!(
            resolved.account.as_ref().map(|account| account.id.as_str()),
            Some("explicit")
        );
        assert_eq!(resolved.origin, AccountOrigin::Request);
    }

    #[test]
    fn resolution_uses_the_session_before_project_memory() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                session_transcript: Some(Path::new("/accounts/last/session.jsonl")),
                pinned_account_id: Some("pinned"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "last");
        assert_eq!(resolved.origin, AccountOrigin::Session);
    }

    #[test]
    fn resolution_uses_pinned_before_last_used() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                pinned_account_id: Some("pinned"),
                last_used_account_id: Some("last"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "pinned");
        assert_eq!(resolved.origin, AccountOrigin::Project);
    }

    #[test]
    fn resolution_uses_last_used_before_the_global_default() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                last_used_account_id: Some("last"),
                default_account_id: Some("default"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "last");
        assert_eq!(resolved.origin, AccountOrigin::LastUsed);
    }

    #[test]
    fn resolution_uses_the_global_default_before_the_base_command() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                default_account_id: Some("pinned"),
                base_command: Some("CLAUDE_CONFIG_DIR=/accounts/last claude"),
                selector: Some("CLAUDE_CONFIG_DIR"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "pinned");
        assert_eq!(resolved.origin, AccountOrigin::GlobalDefault);
    }

    #[test]
    fn resolution_recognizes_a_selector_in_the_base_command() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                base_command: Some("CLAUDE_CONFIG_DIR='/accounts/last' claude"),
                selector: Some("CLAUDE_CONFIG_DIR"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "last");
        assert_eq!(resolved.origin, AccountOrigin::BaseCommand);
        assert_eq!(resolved.account_dir, None, "the base owns the selector");
    }

    #[test]
    fn resolution_falls_through_unavailable_targets_to_the_default_dir() {
        let resolved = resolve_launch_account(
            &fixture(),
            &FakeProvider,
            AccountRequest {
                requested_account_id: Some("missing"),
                ..Default::default()
            },
        );
        assert_eq!(resolved.account.unwrap().id, "default");
        assert_eq!(resolved.origin, AccountOrigin::DefaultConfigDir);
        assert_eq!(resolved.fallback_from.as_deref(), Some("missing"));
    }

    #[test]
    fn live_account_throttle_runs_before_scanner_io() {
        // Regression: 967f956 performed process inspection, account
        // canonicalization, and a full projects query on every scanner tick,
        // consulting the one-minute throttle only after all of that work.
        let now = Instant::now();
        let key = ("/projects/taurhaus".to_string(), CliTool::Claude);
        let checks = HashMap::from([(
            key.clone(),
            LastUsedWrite {
                selector_key: "/accounts/one".to_string(),
                checked_at: now,
            },
        )]);

        assert_eq!(checks[&key].selector_key, "/accounts/one");
        assert!(!live_account_check_is_due(
            &checks,
            &key,
            Some("/accounts/one"),
            now
        ));
        assert!(live_account_check_is_due(
            &checks,
            &key,
            Some("/accounts/two"),
            now
        ));
        assert!(live_account_check_is_due(
            &checks,
            &key,
            Some("/accounts/one"),
            now + Duration::from_secs(60)
        ));
    }

    #[test]
    fn unresolved_live_account_checks_are_throttled() {
        // Regression: 2f8246c recorded the scanner throttle only after a
        // selector dir resolved to a detected account, so an unresolvable
        // session repeated process inspection on every scanner cycle.
        let now = Instant::now();
        let key = ("/projects/taurhaus".to_string(), CliTool::Claude);
        let examined = vec![ExaminedLiveAccount {
            throttle_key: key.clone(),
            selector_key: "/accounts/not-detected".to_string(),
            observation: None,
        }];
        let mut checks = HashMap::new();

        assert!(finish_live_account_checks(&mut checks, examined, now).is_empty());
        assert!(!live_account_check_is_due(
            &checks,
            &key,
            Some("/accounts/not-detected"),
            now + Duration::from_secs(1),
        ));
    }

    #[test]
    fn live_account_observations_persist_only_on_change() {
        // Regression: 967f956 left the live account-memory write and its
        // one-minute throttle without an executable database seam in tests.
        let database = tempfile::NamedTempFile::new().unwrap();
        let connection = crate::db::init_db(database.path()).unwrap();
        connection
            .execute(
                "INSERT INTO projects
                 (id, name, path, description, last_activity_at, hero_preference, created_at, updated_at)
                 VALUES ('project-1', 'Project', '/projects/one', NULL, NULL, NULL, 'now', 'now')",
                [],
            )
            .unwrap();
        let mut observations = vec![LiveAccountObservation {
            project_path: "/projects/one".to_string(),
            tool: CliTool::Claude,
            account_id: "account-1".to_string(),
        }];
        let mut checks = HashMap::new();
        let now = Instant::now();

        assert_eq!(
            persist_live_account_observations_with_checks(
                &connection,
                &observations,
                &mut checks,
                now,
            )
            .unwrap(),
            1
        );
        assert_eq!(
            persist_live_account_observations_with_checks(
                &connection,
                &observations,
                &mut checks,
                now + Duration::from_secs(10),
            )
            .unwrap(),
            0
        );

        observations[0].account_id = "account-2".to_string();
        assert_eq!(
            persist_live_account_observations_with_checks(
                &connection,
                &observations,
                &mut checks,
                now + Duration::from_secs(10),
            )
            .unwrap(),
            1
        );
        let memory = crate::db::queries::project_account_memory(&connection, "project-1").unwrap();
        assert_eq!(memory["claude"].account_id, "account-2");
    }

    #[test]
    fn unknown_project_observations_are_still_throttled() {
        // Regression: 2f8246c recorded the throttle only after a project-row
        // match, so a session outside taurhaus reopened SQLite and rescanned
        // every project on every 500 ms daemon cycle forever.
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let project_key = crate::provider::path::normalize_project_path("/projects/unknown");
        let throttle_key = (project_key, CliTool::Claude);
        let observations = vec![LiveAccountObservation {
            project_path: "/projects/unknown".to_string(),
            tool: CliTool::Claude,
            account_id: "account-1".to_string(),
        }];
        let mut checks = HashMap::new();
        let now = Instant::now();

        assert_eq!(
            persist_live_account_observations_with_checks(
                &connection,
                &observations,
                &mut checks,
                now,
            )
            .unwrap(),
            0
        );
        assert!(checks.contains_key(&throttle_key));
        assert_eq!(
            persist_live_account_observations_with_checks(
                &connection,
                &observations,
                &mut checks,
                now + Duration::from_secs(10),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn session_account_label_comes_from_the_transcript_owner() {
        // Regression: 179a767 rendered `session.account_label` but no backend
        // model or mapper ever produced it, leaving the branch unreachable.
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join(".claude-account2");
        let project = "/projects/taurhaus";
        let session_id = "session-2";
        let transcript = config_dir
            .join("projects")
            .join(crate::session_scanner::idle::path_to_slug(project))
            .join(format!("{session_id}.jsonl"));
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, "{}\n").unwrap();
        let _guard = install_detection_override(
            CliTool::Claude,
            AccountScan {
                config_dirs: vec![config_dir.clone()],
                accounts: vec![Account {
                    tool: CliTool::Claude,
                    id: "account-2".to_string(),
                    dir: config_dir,
                    identity: AccountIdentity {
                        id: "account-2".to_string(),
                        label: "second@example.com".to_string(),
                        display_name: Some("Second".to_string()),
                        organization: None,
                        plan: None,
                        logged_in: true,
                        credential_expires_at: None,
                    },
                    is_default: false,
                    is_process_default: false,
                    usage: None,
                }],
            },
        );

        assert_eq!(
            account_label_for_session(CliTool::Claude, project, session_id).as_deref(),
            Some("Second")
        );
    }
}
