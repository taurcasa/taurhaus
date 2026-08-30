//! Antigravity's single implicit account and command-backed usage provider.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{
    AccountIdentity, AccountProvider, ProviderEnv, Severity, UsageProvider, UsageSnapshot,
    UsageStatus, UsageWindow,
};

const GOOGLE_ACCOUNTS: &str = "google_accounts.json";
const SHARED_CREDENTIALS: &str = "oauth_creds.json";
const APP_DIR: &str = "antigravity-cli";
const APP_CREDENTIALS: &str = "antigravity-oauth-token";
const USAGE_ARGS: &[&str] = &["agy", "-p", "/usage", "--output-format", "json"];
const USAGE_ENV: &[(&str, &str)] = &[("AGY_CLI_DISABLE_AUTO_UPDATE", "true")];

pub struct AgyAccountProvider;

impl AccountProvider for AgyAccountProvider {
    fn default_dir(&self, home: &Path) -> PathBuf {
        crate::provider::platform_paths::PlatformPaths::agy_dir_override()
            .unwrap_or_else(|| home.join(".gemini"))
    }

    fn candidate_dirs(&self, home: &Path, _live_selector_values: &[PathBuf]) -> Vec<PathBuf> {
        vec![self.default_dir(home)]
    }

    fn identify(&self, dir: &Path) -> Option<AccountIdentity> {
        let raw = std::fs::read_to_string(dir.join(GOOGLE_ACCOUNTS)).ok()?;
        let active = serde_json::from_str::<GoogleAccounts>(&raw)
            .ok()?
            .active
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())?;
        let logged_in = dir.join(SHARED_CREDENTIALS).is_file()
            || dir.join(APP_DIR).join(APP_CREDENTIALS).is_file();
        Some(AccountIdentity {
            id: active.clone(),
            label: active,
            display_name: None,
            organization: None,
            plan: None,
            logged_in,
            usage_capable: true,
            credential_expires_at: None,
        })
    }

    fn session_dir(&self, _transcript: &Path) -> Option<PathBuf> {
        None
    }
}

#[derive(Deserialize)]
struct GoogleAccounts {
    active: Option<String>,
}

pub struct AgyUsageProvider;

impl UsageProvider for AgyUsageProvider {
    fn credential_path(&self, dir: &Path) -> Option<PathBuf> {
        let app_token = dir.join(APP_DIR).join(APP_CREDENTIALS);
        Some(if app_token.is_file() {
            app_token
        } else {
            dir.join(SHARED_CREDENTIALS)
        })
    }

    fn fetch(&self, dir: &Path, env: &dyn ProviderEnv) -> UsageSnapshot {
        let observed_at = Utc::now();
        if AgyAccountProvider
            .identify(dir)
            .is_none_or(|identity| !identity.logged_in)
        {
            return snapshot(observed_at, UsageStatus::Unauthorized, Vec::new(), None);
        }

        let output = match env.run_command(
            USAGE_ARGS,
            &dir.join(APP_DIR),
            Duration::from_secs(10),
            USAGE_ENV,
        ) {
            Ok(output) => output,
            Err(_) => return snapshot(observed_at, UsageStatus::Stale, Vec::new(), None),
        };
        if !output.success {
            let diagnostic = format!("{} {}", output.stdout, output.stderr).to_ascii_lowercase();
            let status = if diagnostic.contains("sign in")
                || diagnostic.contains("not signed")
                || diagnostic.contains("authentication")
                || diagnostic.contains("no valid auth")
            {
                UsageStatus::Unauthorized
            } else {
                UsageStatus::Stale
            };
            return snapshot(observed_at, status, Vec::new(), None);
        }

        let Ok(payload) = serde_json::from_str::<UsagePayload>(&output.stdout) else {
            return snapshot(observed_at, UsageStatus::Stale, Vec::new(), None);
        };
        if payload.status.as_deref() != Some("SUCCESS") {
            return snapshot(observed_at, UsageStatus::Stale, Vec::new(), None);
        }
        let Some(command) = payload.command.filter(|command| command.name == "usage") else {
            return snapshot(observed_at, UsageStatus::Stale, Vec::new(), None);
        };
        let Some(windows) = normalize_usage(&command.data.groups) else {
            return snapshot(observed_at, UsageStatus::Stale, Vec::new(), None);
        };
        snapshot(
            observed_at,
            UsageStatus::Ok,
            windows,
            non_empty(command.data.description),
        )
    }
}

#[derive(Deserialize)]
struct UsagePayload {
    status: Option<String>,
    command: Option<UsageCommand>,
}

#[derive(Deserialize)]
struct UsageCommand {
    name: String,
    data: UsageData,
}

#[derive(Deserialize)]
struct UsageData {
    description: Option<String>,
    groups: Vec<UsageGroup>,
}

#[derive(Deserialize)]
struct UsageGroup {
    name: String,
    buckets: Vec<UsageBucket>,
}

#[derive(Deserialize)]
struct UsageBucket {
    id: String,
    remaining_fraction: f64,
    reset_time: Option<String>,
}

fn normalize_usage(groups: &[UsageGroup]) -> Option<Vec<UsageWindow>> {
    let buckets = groups
        .iter()
        .flat_map(|group| {
            group
                .buckets
                .iter()
                .map(move |bucket| (bucket.id.as_str(), (group.name.as_str(), bucket)))
        })
        .collect::<HashMap<_, _>>();
    let windows = [
        ("gemini-weekly", "Gemini Models · Weekly", false),
        ("gemini-5h", "Gemini Models · 5h", true),
        ("3p-weekly", "Claude and GPT models · Weekly", false),
        ("3p-5h", "Claude and GPT models · 5h", true),
    ]
    .into_iter()
    .filter_map(|(key, title, compact)| {
        let (_, bucket) = buckets.get(key)?;
        let used_percentage =
            (100.0 - bucket.remaining_fraction.clamp(0.0, 1.0) * 100.0).clamp(0.0, 100.0);
        Some(UsageWindow {
            key: key.to_string(),
            title: title.to_string(),
            used_percentage,
            resets_at: bucket
                .reset_time
                .as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.timestamp()),
            severity: if used_percentage >= 100.0 {
                Severity::Critical
            } else if used_percentage >= 80.0 {
                Severity::Warning
            } else {
                Severity::Normal
            },
            is_active: false,
            compact,
        })
    })
    .collect::<Vec<_>>();
    (!windows.is_empty()).then_some(windows)
}

fn snapshot(
    observed_at: DateTime<Utc>,
    status: UsageStatus,
    windows: Vec<UsageWindow>,
    note: Option<String>,
) -> UsageSnapshot {
    UsageSnapshot {
        observed_at,
        status,
        windows,
        note,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::accounts::{
        AccountProvider, CommandError, CommandOutput, HttpClient, HttpError, HttpResponse,
        ProviderEnv, UsageProvider, UsageStatus,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;

    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }

        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct UnusedHttp;

    impl HttpClient for UnusedHttp {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _timeout: Duration,
        ) -> Result<HttpResponse, HttpError> {
            panic!("agy usage must not use HTTP")
        }
    }

    type CommandCall = (Vec<String>, PathBuf, Duration, Vec<(String, String)>);

    #[derive(Default)]
    struct FakeEnv {
        calls: Mutex<Vec<CommandCall>>,
        output: Mutex<Option<Result<CommandOutput, CommandError>>>,
    }

    impl ProviderEnv for FakeEnv {
        fn http(&self) -> &dyn HttpClient {
            &UnusedHttp
        }

        fn run_command(
            &self,
            argv: &[&str],
            cwd: &Path,
            timeout: Duration,
            env: &[(&str, &str)],
        ) -> Result<CommandOutput, CommandError> {
            self.calls.lock().unwrap().push((
                argv.iter().map(|value| (*value).to_string()).collect(),
                cwd.to_path_buf(),
                timeout,
                env.iter()
                    .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                    .collect(),
            ));
            self.output.lock().unwrap().take().unwrap()
        }
    }

    #[test]
    fn agy_account_provider_detects_single_implicit_account() {
        // Regression: commit 5680a7a only registered selector-based providers,
        // so Antigravity's single machine account was invisible in Settings.
        // Regression: commit 4a02fe0a made account detection consult the new
        // process override, so this ambient-default test raced the override test.
        let _guard = crate::test_support::acquire_env_test_guard();
        let _env = EnvRestore::remove("TAURHAUS_AGY_DIR");
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".gemini");
        std::fs::create_dir_all(root.join("antigravity-cli")).unwrap();
        std::fs::write(
            root.join("google_accounts.json"),
            r#"{"active":"person@example.com","old":[]}"#,
        )
        .unwrap();
        std::fs::write(root.join("antigravity-cli/antigravity-oauth-token"), "{}").unwrap();

        let provider = AgyAccountProvider;
        assert_eq!(
            provider.candidate_dirs(home.path(), &[]),
            vec![root.clone()]
        );
        let identity = provider.identify(&root).expect("active account");
        assert_eq!(identity.id, "person@example.com");
        assert!(identity.logged_in);
        assert_eq!(provider.session_dir(Path::new("conversation.db")), None);
    }

    // Regression: commit 4e9e2c54 put Antigravity hook and session paths behind
    // `PlatformPaths`, but account detection still read the operator's real
    // `~/.gemini` during an otherwise isolated E2E run.
    #[test]
    fn agy_account_provider_honours_the_taurhaus_root_override() {
        let _guard = crate::test_support::acquire_env_test_guard();
        let process_home = tempfile::tempdir().unwrap();
        let isolated_root = tempfile::tempdir().unwrap();
        let _env = EnvRestore::set("TAURHAUS_AGY_DIR", isolated_root.path());

        let resolved = AgyAccountProvider.default_dir(process_home.path());

        assert_eq!(resolved, isolated_root.path());
    }

    #[test]
    fn agy_usage_provider_parses_real_1_1_22_fixture() {
        // Regression: commit 5680a7a assumed every usage source was HTTP and
        // could not parse Antigravity's structured `/usage` command result.
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("antigravity-cli")).unwrap();
        std::fs::write(
            root.path().join("google_accounts.json"),
            r#"{"active":"fixture@example.com"}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("antigravity-cli/antigravity-oauth-token"),
            "{}",
        )
        .unwrap();
        let env = FakeEnv {
            output: Mutex::new(Some(Ok(CommandOutput {
                success: true,
                stdout: include_str!("../../daemon/fixtures/agy-usage-1.1.22.json").to_string(),
                stderr: String::new(),
            }))),
            ..Default::default()
        };

        let snapshot = AgyUsageProvider.fetch(root.path(), &env);
        assert_eq!(snapshot.status, UsageStatus::Ok);
        assert_eq!(snapshot.windows.len(), 4);
        assert_eq!(snapshot.windows[0].title, "Gemini Models · Weekly");
        assert!((snapshot.windows[0].used_percentage - 0.11993050575256).abs() < 0.000001);

        let calls = env.calls.lock().unwrap();
        assert_eq!(
            calls[0].0,
            ["agy", "-p", "/usage", "--output-format", "json"]
        );
        assert_eq!(calls[0].1, root.path().join("antigravity-cli"));
        assert_eq!(calls[0].2, Duration::from_secs(10));
        assert_eq!(
            calls[0].3,
            [(
                "AGY_CLI_DISABLE_AUTO_UPDATE".to_string(),
                "true".to_string()
            )]
        );
    }

    #[test]
    fn agy_usage_preserves_available_windows_when_a_group_is_absent() {
        // Regression: commit 56b8bb8 collected four buckets into Option<Vec>,
        // so one absent plan-specific group discarded every reported window.
        let groups = vec![UsageGroup {
            name: "Gemini Models".to_string(),
            buckets: vec![UsageBucket {
                id: "gemini-weekly".to_string(),
                remaining_fraction: 0.75,
                reset_time: None,
            }],
        }];

        let windows = normalize_usage(&groups).expect("one known bucket");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].key, "gemini-weekly");
        assert_eq!(windows[0].used_percentage, 25.0);
    }
}
