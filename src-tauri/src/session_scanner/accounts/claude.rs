//! Claude account and subscription-usage provider.
//!
//! Claude Code selects an account through `CLAUDE_CONFIG_DIR`. Each config
//! root owns its identity, credentials, and transcripts. This module only
//! supplies Claude-specific filesystem and HTTP behavior; generic detection,
//! caching, launch resolution, and account memory live in `accounts`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{
    AccountIdentity, AccountProvider, HttpClient, Severity, UsageProvider, UsageSnapshot,
    UsageStatus, UsageWindow,
};

const CONFIG_FILENAME: &str = ".claude.json";
const CREDENTIALS_FILENAME: &str = ".credentials.json";
const DEFAULT_CONFIG_DIRNAME: &str = ".claude";
const CONFIG_DIRNAME_PREFIX: &str = ".claude-";
const PROJECTS_SUBDIR: &str = "projects";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialStore {
    File,
    Keychain,
}

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
        read_identity(dir, host_credential_store())
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

fn config_dir_candidates(home: &Path, extra_dirs: &[PathBuf], default_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![home.join(DEFAULT_CONFIG_DIRNAME), default_dir.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(home) {
        let mut siblings = entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(CONFIG_DIRNAME_PREFIX))
            })
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        siblings.sort();
        candidates.extend(siblings);
    }
    candidates.extend(extra_dirs.iter().cloned());

    let mut seen = Vec::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            let key = canonical_key(candidate);
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        })
        .collect()
}

fn read_identity(config_dir: &Path, store: CredentialStore) -> Option<AccountIdentity> {
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
    let label = non_empty(oauth.email_address)?;
    let id = non_empty(oauth.account_uuid).unwrap_or_else(|| config_dir.display().to_string());
    Some(AccountIdentity {
        id,
        label,
        display_name: non_empty(oauth.display_name),
        organization: non_empty(oauth.organization_name),
        plan: non_empty(oauth.seat_tier).or_else(|| non_empty(oauth.organization_type)),
        logged_in: signed_in(config_dir, store),
        usage_capable: true,
        credential_expires_at: credential_expires_at(config_dir),
    })
}

fn signed_in(config_dir: &Path, store: CredentialStore) -> bool {
    match store {
        CredentialStore::File => config_dir.join(CREDENTIALS_FILENAME).exists(),
        CredentialStore::Keychain => true,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
    serde_json::from_str::<CredentialsFile>(&raw)
        .ok()?
        .oauth?
        .expires_at
        .map(epoch_seconds)
}

pub struct ClaudeUsageProvider;

impl UsageProvider for ClaudeUsageProvider {
    fn credential_path(&self, dir: &Path) -> Option<PathBuf> {
        Some(dir.join(CREDENTIALS_FILENAME))
    }

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
        usage_snapshot(observed_at, UsageStatus::Ok, normalize_usage(payload))
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
    let mirrors_only = payload.limits.is_empty();
    let mut windows = if mirrors_only {
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
        .collect::<Vec<_>>()
    } else {
        payload
            .limits
            .into_iter()
            .enumerate()
            .filter_map(|(index, limit)| {
                let (key, title) = match limit.kind.as_str() {
                    "session" => (limit.kind, "Current session".to_string()),
                    "weekly_all" => (limit.kind, "Current week (all models)".to_string()),
                    "weekly_scoped" => {
                        let display_name = limit
                            .scope
                            .as_ref()
                            .and_then(|scope| scope.model.as_ref())
                            .and_then(|model| model.display_name.as_deref())
                            .unwrap_or("scoped");
                        (
                            format!(
                                "weekly_scoped:{}:{index}",
                                display_name.to_ascii_lowercase()
                            ),
                            format!("Current week ({display_name})"),
                        )
                    }
                    _ => return None,
                };
                Some(UsageWindow {
                    key,
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
    if mirrors_only {
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
    }
    windows
}

fn parse_reset(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::session_scanner::accounts::{HttpError, HttpResponse};

    fn config_json(id: &str, email: &str, display: &str) -> String {
        json!({
            "oauthAccount": {
                "accountUuid": id,
                "emailAddress": email,
                "displayName": display,
                "organizationName": "Example",
                "seatTier": null,
                "organizationType": "claude_max"
            },
            "projects": {}
        })
        .to_string()
    }

    fn write_account(home: &Path, name: &str, id: &str, logged_in: bool) -> PathBuf {
        let dir = home.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(CONFIG_FILENAME),
            config_json(id, &format!("{id}@example.com"), "Example User"),
        )
        .unwrap();
        if logged_in {
            fs::write(
                dir.join(CREDENTIALS_FILENAME),
                r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":4102444800000}}"#,
            )
            .unwrap();
        }
        dir
    }

    #[test]
    fn provider_detects_default_siblings_and_live_external_dirs() {
        let home = TempDir::new().unwrap();
        let default = write_account(home.path(), ".claude", "default", true);
        let sibling = write_account(home.path(), ".claude-second", "second", false);
        let external_root = TempDir::new().unwrap();
        let external = write_account(external_root.path(), "account", "external", true);
        let provider = ClaudeAccountProvider;

        let dirs = provider.candidate_dirs(home.path(), &[external.clone(), default.clone()]);

        assert_eq!(dirs, vec![default.clone(), sibling, external]);
        assert!(provider.identify(&default).unwrap().logged_in);
    }

    #[test]
    fn provider_identity_uses_safe_metadata_and_credential_expiry() {
        let home = TempDir::new().unwrap();
        let dir = write_account(home.path(), ".claude", "account-1", true);

        let identity = read_identity(&dir, CredentialStore::File).unwrap();

        assert_eq!(identity.id, "account-1");
        assert_eq!(identity.label, "account-1@example.com");
        assert_eq!(identity.display_name.as_deref(), Some("Example User"));
        assert_eq!(identity.plan.as_deref(), Some("claude_max"));
        assert_eq!(identity.credential_expires_at, Some(4_102_444_800));
    }

    #[test]
    fn provider_derives_the_config_dir_from_a_transcript() {
        let transcript = Path::new("/accounts/second/projects/-project/session.jsonl");
        assert_eq!(
            ClaudeAccountProvider.session_dir(transcript),
            Some(PathBuf::from("/accounts/second"))
        );
    }

    struct FakeHttp {
        status: u16,
        body: String,
        calls: AtomicUsize,
    }

    impl HttpClient for FakeHttp {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _timeout: std::time::Duration,
        ) -> Result<HttpResponse, HttpError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            })
        }
    }

    fn credentials_dir(expires_at: i64) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(CREDENTIALS_FILENAME),
            format!(
                r#"{{"claudeAiOauth":{{"accessToken":"fixture-token","expiresAt":{expires_at}}}}}"#
            ),
        )
        .unwrap();
        dir
    }

    #[test]
    fn oauth_usage_fixture_normalizes_windows_in_provider_order() {
        // Regression: a574720 modeled usage as two status-line fields; the
        // OAuth response adds an ordered scoped weekly window that must survive.
        let dir = credentials_dir(4_102_444_800_000);
        let http = FakeHttp {
            status: 200,
            body: include_str!("../../daemon/fixtures/claude-oauth-usage-2.1.247.json").to_string(),
            calls: AtomicUsize::new(0),
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
                ("weekly_scoped:fable:2", "Current week (Fable)", 29.0),
            ]
        );
        assert!(snapshot
            .windows
            .iter()
            .all(|window| window.resets_at.is_some()));
    }

    #[test]
    fn scoped_weekly_windows_have_distinct_render_keys() {
        // Regression: c11770e keyed every scoped limit as `weekly_scoped`, so
        // two buckets crashed the keyed Svelte meter and its parent surface.
        let payload = serde_json::from_value::<UsagePayload>(json!({
            "limits": [
                {"kind":"weekly_scoped","percent":29,"scope":{"model":{"display_name":"Fable"}}},
                {"kind":"weekly_scoped","percent":41,"scope":{"model":{"display_name":"Opus"}}}
            ]
        }))
        .unwrap();

        let windows = normalize_usage(payload);

        assert_eq!(windows.len(), 2);
        assert_ne!(windows[0].key, windows[1].key);
    }

    #[test]
    fn scoped_limits_do_not_duplicate_the_sonnet_mirror() {
        // Regression: c11770e appended the Sonnet mirror even when `limits`
        // already carried the same model-scoped weekly bucket.
        let payload = serde_json::from_value::<UsagePayload>(json!({
            "limits": [{
                "kind":"weekly_scoped",
                "percent":29,
                "scope":{"model":{"display_name":"Sonnet"}}
            }],
            "seven_day_sonnet":{"utilization":73,"resets_at":null}
        }))
        .unwrap();

        let windows = normalize_usage(payload);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "Current week (Sonnet)");
    }

    #[test]
    fn expired_credentials_return_unauthorized_without_http() {
        let dir = credentials_dir(1);
        let http = FakeHttp {
            status: 200,
            body: "{}".to_string(),
            calls: AtomicUsize::new(0),
        };

        let snapshot = ClaudeUsageProvider.fetch(dir.path(), &http);

        assert_eq!(snapshot.status, UsageStatus::Unauthorized);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn oauth_usage_without_limits_uses_mirrors() {
        let dir = credentials_dir(4_102_444_800_000);
        let http = FakeHttp {
            status: 200,
            body: r#"{"five_hour":{"utilization":7,"resets_at":null},"seven_day":{"utilization":14,"resets_at":null},"seven_day_sonnet":{"utilization":21,"resets_at":null}}"#.to_string(),
            calls: AtomicUsize::new(0),
        };

        let snapshot = ClaudeUsageProvider.fetch(dir.path(), &http);

        assert_eq!(
            snapshot
                .windows
                .iter()
                .map(|window| window.used_percentage)
                .collect::<Vec<_>>(),
            vec![7.0, 14.0, 21.0]
        );
    }

    #[test]
    fn oauth_usage_401_is_unauthorized() {
        let dir = credentials_dir(4_102_444_800_000);
        let http = FakeHttp {
            status: 401,
            body: "{}".to_string(),
            calls: AtomicUsize::new(0),
        };

        assert_eq!(
            ClaudeUsageProvider.fetch(dir.path(), &http).status,
            UsageStatus::Unauthorized
        );
    }
}
