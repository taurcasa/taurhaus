//! Codex account and subscription-usage provider.
//!
//! Codex isolates credentials and session history by `CODEX_HOME`. JWT payloads
//! are decoded without signature verification only to obtain display metadata;
//! they are never treated as proof of identity or authorization.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AccountIdentity, AccountProvider, HttpClient, Severity, UsageProvider, UsageSnapshot,
    UsageStatus, UsageWindow,
};

const AUTH_FILENAME: &str = "auth.json";
const DEFAULT_HOME_NAME: &str = ".codex";
const HOME_PREFIX: &str = ".codex-";
const SESSIONS_DIR: &str = "sessions";
const PROFILE_CLAIMS: &str = "https://api.openai.com/profile";
const AUTH_CLAIMS: &str = "https://api.openai.com/auth";
const USAGE_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";

pub struct CodexAccountProvider;

impl AccountProvider for CodexAccountProvider {
    fn default_dir(&self, home: &Path) -> PathBuf {
        home.join(DEFAULT_HOME_NAME)
    }

    fn candidate_dirs(&self, home: &Path, live_selector_values: &[PathBuf]) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        push_candidate(&mut candidates, self.default_dir(home));

        if let Ok(entries) = std::fs::read_dir(home) {
            let mut siblings = entries
                .flatten()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with(HOME_PREFIX))
                })
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            siblings.sort();
            for sibling in siblings {
                push_candidate(&mut candidates, sibling);
            }
        }

        for live in live_selector_values
            .iter()
            .filter(|candidate| candidate.is_dir())
        {
            push_candidate(&mut candidates, live.clone());
        }
        candidates
    }

    fn identify(&self, dir: &Path) -> Option<AccountIdentity> {
        let auth = read_auth(dir)?;
        let fallback_id = canonical_path(dir).display().to_string();
        let Some(tokens) = auth.tokens.filter(|_| auth.auth_mode == "chatgpt") else {
            return Some(AccountIdentity {
                id: fallback_id,
                label: "API key".to_string(),
                display_name: None,
                organization: None,
                plan: None,
                logged_in: true,
                usage_capable: false,
                credential_expires_at: None,
            });
        };

        let claims = tokens
            .id_token
            .as_deref()
            .and_then(decode_jwt_payload)
            .unwrap_or_default();
        let auth_claims = claims.get(AUTH_CLAIMS).and_then(Value::as_object);
        let profile_claims = claims.get(PROFILE_CLAIMS).and_then(Value::as_object);
        let label = string_claim(&claims, "email")
            .or_else(|| profile_claims.and_then(|claims| object_string(claims, "email")))
            .unwrap_or_else(|| "ChatGPT account".to_string());
        let plan = auth_claims.and_then(|claims| object_string(claims, "chatgpt_plan_type"));
        let id = auth_claims
            .and_then(|claims| object_string(claims, "chatgpt_account_id"))
            .or_else(|| non_empty(tokens.account_id.clone()))
            .unwrap_or(fallback_id);
        let credential_expires_at = tokens.access_token.as_deref().and_then(jwt_expiration);

        Some(AccountIdentity {
            id,
            label,
            display_name: None,
            organization: None,
            plan,
            logged_in: true,
            usage_capable: true,
            credential_expires_at,
        })
    }

    fn session_dir(&self, transcript: &Path) -> Option<PathBuf> {
        let file_name = transcript.file_name()?.to_str()?;
        if !file_name.starts_with("rollout-") || transcript.extension()?.to_str()? != "jsonl" {
            return None;
        }
        transcript.ancestors().find_map(|ancestor| {
            (ancestor.file_name().and_then(|name| name.to_str()) == Some(SESSIONS_DIR))
                .then(|| ancestor.parent().map(Path::to_path_buf))
                .flatten()
        })
    }
}

fn push_candidate(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    let candidate = canonical_path(&candidate);
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Deserialize)]
struct AuthFile {
    #[serde(default)]
    auth_mode: String,
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: Option<String>,
    id_token: Option<String>,
    account_id: Option<String>,
}

fn read_auth(dir: &Path) -> Option<AuthFile> {
    let raw = std::fs::read_to_string(dir.join(AUTH_FILENAME)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn jwt_expiration(token: &str) -> Option<i64> {
    decode_jwt_payload(token)?.get("exp")?.as_i64()
}

fn string_claim(claims: &Value, key: &str) -> Option<String> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .and_then(non_empty_str)
}

fn object_string(claims: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .and_then(non_empty_str)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| non_empty_str(&value))
}

fn non_empty_str(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub struct CodexUsageProvider;

impl UsageProvider for CodexUsageProvider {
    fn credential_path(&self, dir: &Path) -> Option<PathBuf> {
        Some(dir.join(AUTH_FILENAME))
    }

    fn fetch(&self, dir: &Path, http: &dyn HttpClient) -> UsageSnapshot {
        let observed_at = chrono::Utc::now();
        let unauthorized = || usage_snapshot(observed_at, UsageStatus::Unauthorized, Vec::new());
        let Some(auth) = read_auth(dir) else {
            return unauthorized();
        };
        if auth.auth_mode != "chatgpt" {
            return unauthorized();
        }
        let Some(tokens) = auth.tokens else {
            return unauthorized();
        };
        let Some(access_token) = tokens.access_token else {
            return unauthorized();
        };
        if jwt_expiration(&access_token).is_some_and(|expires| expires <= observed_at.timestamp()) {
            return unauthorized();
        }
        let Some(account_id) = non_empty(tokens.account_id) else {
            return unauthorized();
        };

        let authorization = format!("Bearer {access_token}");
        let user_agent = format!("taurhaus/{}", env!("CARGO_PKG_VERSION"));
        let headers = [
            ("Authorization", authorization.as_str()),
            ("ChatGPT-Account-ID", account_id.as_str()),
            ("User-Agent", user_agent.as_str()),
        ];
        let response = match http.get(USAGE_ENDPOINT, &headers, Duration::from_secs(5)) {
            Ok(response) => response,
            Err(_) => return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new()),
        };
        if matches!(response.status, 401 | 403) {
            return unauthorized();
        }
        if response.status != 200 {
            return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new());
        }
        let Ok(payload) = serde_json::from_str::<UsagePayload>(&response.body) else {
            return usage_snapshot(observed_at, UsageStatus::Stale, Vec::new());
        };
        let (windows, note) = normalize_usage(payload);
        UsageSnapshot {
            observed_at,
            status: UsageStatus::Ok,
            windows,
            note,
        }
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
    rate_limit: RateLimit,
    #[serde(default)]
    additional_rate_limits: Vec<AdditionalRateLimit>,
    credits: Option<Credits>,
    spend_control: Option<SpendControl>,
    rate_limit_reached_type: Option<Value>,
    promo: Option<Value>,
}

#[derive(Deserialize)]
struct AdditionalRateLimit {
    limit_name: String,
    metered_feature: String,
    rate_limit: RateLimit,
}

#[derive(Deserialize)]
struct RateLimit {
    #[serde(default)]
    limit_reached: bool,
    primary_window: Option<RateLimitWindow>,
    secondary_window: Option<RateLimitWindow>,
}

#[derive(Deserialize)]
struct RateLimitWindow {
    used_percent: f64,
    limit_window_seconds: u64,
    reset_at: Option<i64>,
}

#[derive(Deserialize)]
struct Credits {
    #[serde(default)]
    has_credits: bool,
    balance: Option<String>,
}

#[derive(Deserialize)]
struct SpendControl {
    #[serde(default)]
    reached: bool,
}

fn normalize_usage(payload: UsagePayload) -> (Vec<UsageWindow>, Option<String>) {
    let active_limit = payload.rate_limit_reached_type.is_some();
    let mut windows = rate_limit_windows("codex", None, payload.rate_limit, active_limit);
    for additional in payload.additional_rate_limits {
        windows.extend(rate_limit_windows(
            &additional.metered_feature,
            Some(&additional.limit_name),
            additional.rate_limit,
            active_limit,
        ));
    }

    let mut notes = Vec::new();
    if let Some(credits) = payload.credits.filter(|credits| credits.has_credits) {
        if let Some(balance) = credits.balance.and_then(non_empty_str_owned) {
            notes.push(format!("credits balance {balance}"));
        }
    }
    if payload
        .spend_control
        .is_some_and(|spend_control| spend_control.reached)
    {
        notes.push("spend limit reached".to_string());
    }
    if let Some(promo) = payload.promo.as_ref().and_then(promo_text) {
        notes.push(promo);
    }
    let note = (!notes.is_empty()).then(|| notes.join(" · "));
    (windows, note)
}

fn rate_limit_windows(
    key_prefix: &str,
    title_prefix: Option<&str>,
    rate_limit: RateLimit,
    active_limit: bool,
) -> Vec<UsageWindow> {
    [rate_limit.primary_window, rate_limit.secondary_window]
        .into_iter()
        .flatten()
        .map(|window| {
            let (kind, title) = window_kind(window.limit_window_seconds);
            UsageWindow {
                key: format!("{key_prefix}.{kind}"),
                title: title_prefix
                    .map(|prefix| format!("{prefix} · {title}"))
                    .unwrap_or(title),
                used_percentage: window.used_percent,
                resets_at: window.reset_at,
                severity: if rate_limit.limit_reached {
                    Severity::Critical
                } else if window.used_percent >= 80.0 {
                    Severity::Warning
                } else {
                    Severity::Normal
                },
                is_active: rate_limit.limit_reached && active_limit,
            }
        })
        .collect()
}

fn window_kind(seconds: u64) -> (String, String) {
    if seconds <= 21_600 {
        return ("5h".to_string(), "5h limit".to_string());
    }
    if seconds == 604_800 {
        return ("weekly".to_string(), "Weekly limit".to_string());
    }
    let hours = seconds.div_ceil(3_600);
    (format!("{hours}h"), format!("{hours}h limit"))
}

fn promo_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str().and_then(non_empty_str) {
        return Some(text);
    }
    let object = value.as_object()?;
    ["message", "text", "title"]
        .into_iter()
        .find_map(|key| object_string(object, key))
}

fn non_empty_str_owned(value: String) -> Option<String> {
    non_empty_str(&value)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    use base64::Engine as _;
    use serde_json::{json, Value};
    use tempfile::TempDir;

    use super::*;
    use crate::session_scanner::accounts::{
        AccountProvider, HttpClient, HttpError, HttpErrorKind, HttpResponse, Severity,
        UsageProvider, UsageStatus,
    };

    fn jwt(payload: Value) -> String {
        let encoder = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = encoder.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = encoder.encode(serde_json::to_vec(&payload).unwrap());
        format!("{header}.{payload}.fixture")
    }

    fn write_auth(dir: &Path, value: Value) {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join("auth.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    fn chatgpt_auth(expires_at: i64) -> Value {
        json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": jwt(json!({"exp": expires_at})),
                "id_token": jwt(json!({
                    "email": "codex@example.com",
                    "https://api.openai.com/auth": {
                        "chatgpt_plan_type": "pro",
                        "chatgpt_account_id": "workspace-from-token"
                    }
                })),
                "refresh_token": "fixture-refresh",
                "account_id": "workspace-from-auth"
            }
        })
    }

    type CapturedRequest = (String, Vec<(String, String)>, Duration);

    #[derive(Default)]
    struct FakeHttp {
        status: Option<u16>,
        body: String,
        calls: AtomicUsize,
        request: Mutex<Option<CapturedRequest>>,
    }

    impl FakeHttp {
        fn response(status: u16, body: impl Into<String>) -> Self {
            Self {
                status: Some(status),
                body: body.into(),
                ..Default::default()
            }
        }
    }

    impl HttpClient for FakeHttp {
        fn get(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            timeout: Duration,
        ) -> Result<HttpResponse, HttpError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.request.lock().unwrap() = Some((
                url.to_string(),
                headers
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                    .collect(),
                timeout,
            ));
            match self.status {
                Some(status) => Ok(HttpResponse {
                    status,
                    body: self.body.clone(),
                }),
                None => Err(HttpError {
                    kind: HttpErrorKind::Network,
                }),
            }
        }
    }

    #[test]
    fn identity_decodes_display_claims_without_verifying_the_jwt() {
        // Regression: 08c3961 left Codex on the provider floor, so account
        // identity could not be derived from its display-only id_token claims.
        let root = TempDir::new().unwrap();
        let dir = root.path().join(".codex-work");
        write_auth(&dir, chatgpt_auth(4_102_444_800));

        let identity = CodexAccountProvider.identify(&dir).unwrap();

        assert_eq!(identity.id, "workspace-from-token");
        assert_eq!(identity.label, "codex@example.com");
        assert_eq!(identity.plan.as_deref(), Some("pro"));
        assert!(identity.logged_in);
        assert!(identity.usage_capable);
        assert_eq!(identity.credential_expires_at, Some(4_102_444_800));
    }

    #[test]
    fn expired_access_token_is_unauthorized_without_an_http_request() {
        // Regression: 08c3961 had no Codex usage slice; an expired credential
        // must never be sent or refreshed by the generic poller.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(1));
        let http = FakeHttp::response(200, "{}");

        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(snapshot.status, UsageStatus::Unauthorized);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn api_key_mode_is_an_account_without_subscription_usage() {
        // Regression: 08c3961 had no per-account usage capability, so an API
        // key login could only be misrepresented as ChatGPT subscription usage.
        let root = TempDir::new().unwrap();
        write_auth(
            root.path(),
            json!({"auth_mode":"apikey","OPENAI_API_KEY":"fixture-key"}),
        );
        let http = FakeHttp::response(200, "{}");

        let identity = CodexAccountProvider.identify(root.path()).unwrap();
        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(identity.label, "API key");
        assert!(identity.logged_in);
        assert!(!identity.usage_capable);
        assert_eq!(snapshot.status, UsageStatus::Unauthorized);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn candidates_include_default_siblings_and_existing_live_homes() {
        // Regression: 08c3961 declared CODEX_HOME but registered no provider,
        // leaving sibling and live-process homes undiscoverable.
        let home = TempDir::new().unwrap();
        let default = home.path().join(".codex");
        let sibling = home.path().join(".codex-work");
        let sibling_file = home.path().join(".codex-not-a-dir");
        let external_root = TempDir::new().unwrap();
        let external = external_root.path().join("account");
        let missing = external_root.path().join("missing");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(sibling_file, "not a directory").unwrap();

        let candidates = CodexAccountProvider
            .candidate_dirs(home.path(), &[external.clone(), missing, default.clone()]);

        assert_eq!(
            candidates,
            vec![
                fs::canonicalize(default).unwrap(),
                fs::canonicalize(sibling).unwrap(),
                fs::canonicalize(external).unwrap(),
            ]
        );
    }

    #[test]
    fn rollout_path_resolves_its_codex_home() {
        // Regression: 08c3961 left resume derivation without a Codex account
        // provider even though rollout paths carry the owning home.
        let transcript =
            Path::new("/accounts/work/sessions/2026/08/27/rollout-2026-08-27T12-00-00-id.jsonl");

        assert_eq!(
            CodexAccountProvider.session_dir(transcript),
            Some(PathBuf::from("/accounts/work"))
        );
    }

    #[test]
    fn wham_fixture_normalizes_dynamic_windows_in_provider_order() {
        // Regression: 08c3961 exposed no Codex usage provider, including no
        // ordered model-family windows from the live-verified wham payload.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(4_102_444_800));
        let http = FakeHttp::response(
            200,
            include_str!("../../daemon/fixtures/codex-wham-usage-0.149.json"),
        );

        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(snapshot.status, UsageStatus::Ok);
        assert_eq!(
            snapshot
                .windows
                .iter()
                .map(|window| (
                    window.key.as_str(),
                    window.title.as_str(),
                    window.used_percentage,
                    window.resets_at,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("codex.weekly", "Weekly limit", 50.0, Some(1_788_283_433)),
                (
                    "codex_bengalfox.5h",
                    "GPT-5.3-Codex-Spark · 5h limit",
                    0.0,
                    Some(1_787_860_379),
                ),
                (
                    "codex_bengalfox.weekly",
                    "GPT-5.3-Codex-Spark · Weekly limit",
                    0.0,
                    Some(1_788_447_179),
                ),
            ]
        );
        assert!(snapshot
            .windows
            .iter()
            .all(|window| window.severity == Severity::Normal));

        let (url, headers, timeout) = http.request.lock().unwrap().clone().unwrap();
        assert_eq!(url, "https://chatgpt.com/backend-api/wham/usage");
        assert_eq!(timeout, Duration::from_secs(5));
        assert!(headers.contains(&(
            "ChatGPT-Account-ID".to_string(),
            "workspace-from-auth".to_string()
        )));
        assert!(headers.contains(&(
            "User-Agent".to_string(),
            format!("taurhaus/{}", env!("CARGO_PKG_VERSION"))
        )));
    }

    #[test]
    fn null_secondary_window_yields_one_base_window() {
        // Regression: 08c3961 had no parser contract for nullable Codex usage
        // windows, risking rejection of the live response shape.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(4_102_444_800));
        let http = FakeHttp::response(
            200,
            json!({
                "rate_limit": {
                    "allowed": true,
                    "limit_reached": false,
                    "primary_window": {
                        "used_percent": 12,
                        "limit_window_seconds": 18000,
                        "reset_at": 2000000000
                    },
                    "secondary_window": null
                },
                "additional_rate_limits": [],
                "rate_limit_reached_type": null
            })
            .to_string(),
        );

        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].key, "codex.5h");
        assert_eq!(snapshot.windows[0].title, "5h limit");
    }

    #[test]
    fn rejected_wham_credential_is_unauthorized() {
        // Regression: 08c3961 had no Codex usage error classifier; rejected
        // credentials must pause until auth.json changes instead of going stale.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(4_102_444_800));
        let http = FakeHttp::response(401, "{}");

        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(snapshot.status, UsageStatus::Unauthorized);
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn network_and_parse_failures_are_stale() {
        // Regression: 08c3961 left the generic stale/backoff contract without
        // a Codex implementation or fake-client coverage.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(4_102_444_800));

        assert_eq!(
            CodexUsageProvider
                .fetch(root.path(), &FakeHttp::default())
                .status,
            UsageStatus::Stale
        );
        assert_eq!(
            CodexUsageProvider
                .fetch(root.path(), &FakeHttp::response(200, "not json"))
                .status,
            UsageStatus::Stale
        );
    }

    #[test]
    fn limit_state_sets_severity_activity_and_account_note() {
        // Regression: 08c3961 had no Codex normalization for reached limits,
        // credits, spend control, or provider notices.
        let root = TempDir::new().unwrap();
        write_auth(root.path(), chatgpt_auth(4_102_444_800));
        let http = FakeHttp::response(
            200,
            json!({
                "rate_limit": {
                    "allowed": false,
                    "limit_reached": true,
                    "primary_window": {
                        "used_percent": 100,
                        "limit_window_seconds": 18000,
                        "reset_at": 2000000000
                    },
                    "secondary_window": null
                },
                "additional_rate_limits": [],
                "credits": {"has_credits": true, "balance": "12.5"},
                "spend_control": {"reached": true},
                "rate_limit_reached_type": "rate_limit_reached",
                "promo": "Try Codex credits"
            })
            .to_string(),
        );

        let snapshot = CodexUsageProvider.fetch(root.path(), &http);

        assert_eq!(snapshot.windows[0].severity, Severity::Critical);
        assert!(snapshot.windows[0].is_active);
        assert_eq!(
            snapshot.note.as_deref(),
            Some("credits balance 12.5 · spend limit reached · Try Codex credits")
        );
    }
}
