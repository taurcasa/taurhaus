//! Tool-agnostic account and subscription-usage contracts.
//!
//! Per-tool implementations live in sibling modules. Consumers use the
//! registry-provided traits and these normalised wire types.

pub mod claude;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
#[cfg(not(test))]
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use super::cli_tool::SessionRoot;
use super::cli_tool::{spec, CliTool};

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
    use super::AccountOrigin;

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
}
