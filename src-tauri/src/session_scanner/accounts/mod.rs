//! Tool-agnostic account and subscription-usage contracts.
//!
//! Per-tool implementations live in sibling modules. Consumers use the
//! registry-provided traits and these normalised wire types.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::cli_tool::CliTool;

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
