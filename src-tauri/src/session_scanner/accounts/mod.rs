//! Tool-agnostic account and subscription-usage contracts.
//!
//! Per-tool implementations live in sibling modules. Consumers use the
//! registry-provided traits and these normalised wire types.

pub mod claude;
pub mod legacy_statusline;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
#[cfg(not(test))]
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
            crate::provider::platform_paths::PlatformPaths::claude_dir()
        }
        SessionRoot::ToolHome => process_default.clone(),
    };
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

#[cfg(not(test))]
struct LastUsedWrite {
    account_id: String,
    written_at: Instant,
}

#[cfg(not(test))]
static LAST_USED_WRITES: Mutex<Option<HashMap<(String, CliTool), LastUsedWrite>>> =
    Mutex::new(None);

/// Persist a scanner or launch observation without disturbing a user pin.
#[cfg(not(test))]
pub fn remember_last_used(
    project_id: &str,
    tool: CliTool,
    account_id: &str,
) -> Result<bool, String> {
    let db_path =
        crate::provider::platform_paths::PlatformPaths::app_data_root().join("taurhaus.db");
    if !db_path.exists() {
        return Ok(false);
    }
    let connection = rusqlite::Connection::open(db_path).map_err(|error| error.to_string())?;
    crate::db::queries::remember_last_used_account(
        &connection,
        project_id,
        &tool.to_string(),
        account_id,
    )
    .map_err(|error| error.to_string())
}

/// Unit tests exercise the connection-scoped query and never open app data.
#[cfg(test)]
pub fn remember_last_used(
    _project_id: &str,
    _tool: CliTool,
    _account_id: &str,
) -> Result<bool, String> {
    Ok(false)
}

/// Bind live selector values back to project memory. This runs on the scanner
/// thread, where per-process environments are available.
#[cfg(not(test))]
pub(crate) fn record_live_session_accounts(sessions: &[RuntimeSession]) {
    if cfg!(target_os = "windows") {
        return;
    }

    for session in sessions {
        let tool = session.cli_tool;
        let tool_spec = spec(tool);
        let (Some(selector), Some(provider)) = (
            tool_spec.capabilities.account_selector,
            tool_spec.account_provider(),
        ) else {
            continue;
        };
        let selected_dir = super::process::process_selector_value(session.pid, selector)
            .or_else(|| dirs::home_dir().map(|home| provider.default_dir(&home)));
        let Some(selected_dir) = selected_dir else {
            continue;
        };
        let selected_key = canonical_key(&selected_dir);
        let Some(account) = detect(tool)
            .into_iter()
            .find(|account| canonical_key(&account.dir) == selected_key)
        else {
            continue;
        };
        let Some(project_id) = project_id_for_path(&session.project_path) else {
            continue;
        };
        let throttle_key = (
            crate::provider::path::normalize_project_path(&session.project_path),
            tool,
        );
        let should_write = {
            let mut writes = LAST_USED_WRITES
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let writes = writes.get_or_insert_with(HashMap::new);
            match writes.get(&throttle_key) {
                Some(write) if write.account_id == account.id => false,
                Some(write) if write.written_at.elapsed() < Duration::from_secs(60) => false,
                _ => true,
            }
        };
        if !should_write {
            continue;
        }
        match remember_last_used(&project_id, tool, &account.id) {
            Ok(_) => {
                LAST_USED_WRITES
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_or_insert_with(HashMap::new)
                    .insert(
                        throttle_key,
                        LastUsedWrite {
                            account_id: account.id,
                            written_at: Instant::now(),
                        },
                    );
            }
            Err(error) => {
                tracing::warn!(tool = %tool, error = %error, "failed to remember live session account")
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn record_live_session_accounts(_sessions: &[RuntimeSession]) {}

#[cfg(not(test))]
fn project_id_for_path(project_path: &str) -> Option<String> {
    let db_path =
        crate::provider::platform_paths::PlatformPaths::app_data_root().join("taurhaus.db");
    let connection = rusqlite::Connection::open(db_path).ok()?;
    let wanted = crate::provider::path::normalize_project_path(project_path);
    let mut statement = connection.prepare("SELECT id, path FROM projects").ok()?;
    let project_id = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()?
        .filter_map(Result::ok)
        .find(|(_, path)| crate::provider::path::normalize_project_path(path) == wanted)
        .map(|(id, _)| id);
    project_id
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

impl HttpClient for ReqwestHttpClient {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<HttpResponse, HttpError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| HttpError {
                kind: HttpErrorKind::Network,
            })?;
        let mut request = client.get(url);
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
}
