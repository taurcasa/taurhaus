//! Grok account provider.
//!
//! grok isolates a whole account — credentials, config, sessions, the live
//! registry and the leader socket — behind `GROK_HOME`. `auth.json` (mode 0600)
//! is a JSON map keyed `<oidc_issuer>::<client_id>`; this reads only the display
//! names in a record and never a credential value.
//!
//! There is no usage provider: grok 1.0.5 exposes no subscription quota
//! endpoint, and per-turn cost and tokens arrive in-band. The registry says so
//! through `usage: false` and a note the UI shows where a meter would be.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{AccountIdentity, AccountProvider, TranscriptLocator};
use crate::session_scanner::idle::GROK_SESSION_FILES as SESSION_FILES;

const AUTH_FILENAME: &str = "auth.json";
const DEFAULT_HOME_NAME: &str = ".grok";
const HOME_PREFIX: &str = ".grok-";
const SESSIONS_DIR: &str = "sessions";

pub struct GrokAccountProvider;

/// grok files history as `<home>/sessions/<group>/<session-id>/<file>`, where
/// the group is its percent-encoded cwd until the cwd outgrows a path component
/// and becomes a slug plus hash. Only each session's own `summary.json` names
/// the project, so discovery reads the records rather than building a path.
pub struct GrokTranscriptLocator;

impl TranscriptLocator for GrokTranscriptLocator {
    fn newest_project_transcript(
        &self,
        config_dir: &Path,
        project_path: &str,
    ) -> Option<(std::time::SystemTime, PathBuf)> {
        crate::session_scanner::idle::grok_newest_session_transcript(config_dir, project_path)
    }

    fn session_transcript(&self, config_dir: &Path, session_id: &str) -> Option<PathBuf> {
        crate::session_scanner::idle::grok_session_transcript(config_dir, session_id)
    }
}

impl AccountProvider for GrokAccountProvider {
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
        let record = read_auth_record(dir)?;
        let expires_at = record.expires_at.as_deref().and_then(parse_timestamp);
        let has_credential = record
            .key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty());
        let logged_in = has_credential
            && expires_at.is_some_and(|expires_at| expires_at > chrono::Utc::now().timestamp());

        let label = non_empty(record.email.as_deref())
            .or_else(|| non_empty(record.user_id.as_deref()))
            .unwrap_or_else(|| "Grok account".to_string());
        let display_name = [record.first_name.as_deref(), record.last_name.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        Some(AccountIdentity {
            id: non_empty(record.user_id.as_deref())
                .or_else(|| non_empty(record.principal_id.as_deref()))
                .unwrap_or_else(|| canonical_path(dir).display().to_string()),
            label,
            display_name: (!display_name.is_empty()).then_some(display_name),
            // grok reports its subscription tier over ACP, never in a file.
            organization: None,
            plan: None,
            logged_in,
            usage_capable: false,
            credential_expires_at: expires_at,
        })
    }

    fn session_dir(&self, transcript: &Path) -> Option<PathBuf> {
        let file_name = transcript.file_name()?.to_str()?;
        if !SESSION_FILES.contains(&file_name) {
            return None;
        }
        transcript.ancestors().find_map(|ancestor| {
            (ancestor.file_name().and_then(|name| name.to_str()) == Some(SESSIONS_DIR))
                .then(|| ancestor.parent().map(Path::to_path_buf))
                .flatten()
        })
    }
}

/// The one record in a grok `auth.json`, read for display names only.
#[derive(Debug, Default, Deserialize)]
struct AuthRecord {
    key: Option<String>,
    email: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    user_id: Option<String>,
    principal_id: Option<String>,
    expires_at: Option<String>,
}

/// `auth.json` is a map keyed `<oidc_issuer>::<client_id>`. The build verified
/// here holds one record; a file that grew several is read as no account rather
/// than as an arbitrary pick, because grok's own selection rule is unverified.
fn read_auth_record(dir: &Path) -> Option<AuthRecord> {
    let raw = std::fs::read_to_string(dir.join(AUTH_FILENAME)).ok()?;
    let records: std::collections::BTreeMap<String, AuthRecord> =
        serde_json::from_str(&raw).ok()?;
    let mut records = records.into_values();
    let record = records.next()?;
    if records.next().is_some() {
        tracing::debug!(
            dir = %dir.display(),
            "grok auth store holds several credential records; account selection is unverified"
        );
        return None;
    }
    Some(record)
}

fn parse_timestamp(value: &str) -> Option<i64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<i64>() {
        return Some(seconds);
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.timestamp())
}

fn non_empty(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()).then(|| value.to_string())
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

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    const ISSUER_KEY: &str = "https://auth.x.ai::7f4d1b2c-0000-4000-8000-1c2d3e4f5a6b";

    fn write_auth(dir: &Path, record: serde_json::Value) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join(AUTH_FILENAME),
            json!({ ISSUER_KEY: record }).to_string(),
        )
        .unwrap();
    }

    fn signed_in_record(expires_at: &str) -> serde_json::Value {
        json!({
            "key": "fixture-credential",
            "auth_mode": "Oidc",
            "email": "grok@example.com",
            "first_name": "Ada",
            "last_name": "Lovelace",
            "user_id": "user-1",
            "principal_id": "principal-1",
            "team_id": "team-1",
            "expires_at": expires_at,
            "refresh_token": "fixture-refresh",
            "oidc_issuer": "https://auth.x.ai",
        })
    }

    #[test]
    fn identity_reads_display_names_and_never_a_credential() {
        // Regression: commit c1005ec left grok without an account provider, so
        // a second GROK_HOME could not be detected, chosen or resumed onto.
        let root = TempDir::new().unwrap();
        let dir = root.path().join(".grok-work");
        write_auth(&dir, signed_in_record("2099-01-01T00:00:00Z"));

        let identity = GrokAccountProvider.identify(&dir).expect("grok identity");

        assert_eq!(identity.id, "user-1");
        assert_eq!(identity.label, "grok@example.com");
        assert_eq!(identity.display_name.as_deref(), Some("Ada Lovelace"));
        assert!(identity.logged_in);
        assert!(!identity.usage_capable, "grok publishes no usage endpoint");
        assert_eq!(
            identity.credential_expires_at,
            Some(
                chrono::DateTime::parse_from_rfc3339("2099-01-01T00:00:00Z")
                    .unwrap()
                    .timestamp()
            )
        );
    }

    #[test]
    fn an_expired_or_keyless_record_is_a_detected_but_signed_out_account() {
        // Regression: commit c1005ec had no grok credential rules; taurhaus
        // never refreshes a token, so an expired one must present as signed out
        // rather than as a launchable account.
        let root = TempDir::new().unwrap();
        let expired = root.path().join(".grok-expired");
        write_auth(&expired, signed_in_record("2020-01-01T00:00:00Z"));
        let keyless = root.path().join(".grok-keyless");
        let mut record = signed_in_record("2099-01-01T00:00:00Z");
        record.as_object_mut().unwrap().remove("key");
        write_auth(&keyless, record);

        assert!(
            !GrokAccountProvider
                .identify(&expired)
                .expect("expired identity")
                .logged_in
        );
        assert!(
            !GrokAccountProvider
                .identify(&keyless)
                .expect("keyless identity")
                .logged_in
        );
    }

    #[test]
    fn an_empty_or_multi_record_auth_store_is_not_an_account() {
        // Regression: commit c1005ec would have had to guess which of several
        // stored issuer records grok itself selects, which is unverified.
        let root = TempDir::new().unwrap();
        let empty = root.path().join(".grok-empty");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(GrokAccountProvider.identify(&empty), None);

        fs::write(empty.join(AUTH_FILENAME), "{}").unwrap();
        assert_eq!(GrokAccountProvider.identify(&empty), None);

        let multi = root.path().join(".grok-multi");
        fs::create_dir_all(&multi).unwrap();
        fs::write(
            multi.join(AUTH_FILENAME),
            json!({
                ISSUER_KEY: signed_in_record("2099-01-01T00:00:00Z"),
                "https://auth.x.ai::second": signed_in_record("2099-01-01T00:00:00Z"),
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(GrokAccountProvider.identify(&multi), None);
    }

    #[test]
    fn candidates_cover_the_default_its_siblings_and_live_homes() {
        // Regression: commit c1005ec declared no GROK_HOME selector, so sibling
        // and live-process homes were undiscoverable.
        let home = TempDir::new().unwrap();
        let default = home.path().join(".grok");
        let sibling = home.path().join(".grok-work");
        let external_root = TempDir::new().unwrap();
        let external = external_root.path().join("account");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(home.path().join(".grok-not-a-dir"), "file").unwrap();

        let candidates = GrokAccountProvider.candidate_dirs(
            home.path(),
            &[external.clone(), external_root.path().join("missing")],
        );

        assert_eq!(
            candidates,
            vec![
                fs::canonicalize(default).unwrap(),
                fs::canonicalize(sibling).unwrap(),
                fs::canonicalize(external).unwrap(),
            ]
        );
    }

    fn write_session(home: &Path, group: &str, session_id: &str, cwd: &str) -> PathBuf {
        let dir = home.join(SESSIONS_DIR).join(group).join(session_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("summary.json"),
            json!({ "info": { "id": session_id, "cwd": cwd } }).to_string(),
        )
        .unwrap();
        let events = dir.join("events.jsonl");
        fs::write(&events, "{\"type\":\"turn_ended\"}\n").unwrap();
        events
    }

    #[test]
    fn cold_lookup_derives_the_home_that_owns_a_project_history() {
        // Regression: commit 8fcb5b3 left cold Continue/Resume account
        // derivation on the Claude `<projects>/<slug>/<id>.<ext>` layout, so a
        // grok history under `sessions/<group>/<id>/` was invisible and no grok
        // account could be derived from it.
        let root = TempDir::new().unwrap();
        let home_a = root.path().join(".grok");
        let home_b = root.path().join(".grok-work");
        let project = "/home/user/projects/grok";
        write_session(
            &home_a,
            "%2Fhome%2Fuser%2Fprojects%2Fgrok",
            "01a04585-2d53-7123-8000-00000000000a",
            project,
        );
        let newest = write_session(
            &home_b,
            "slug-9f2a1c",
            "01a04585-2d53-7123-8000-00000000000b",
            project,
        );
        let when = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        fs::OpenOptions::new()
            .write(true)
            .open(&newest)
            .unwrap()
            .set_modified(when)
            .unwrap();

        let transcript = crate::session_scanner::accounts::newest_project_transcript(
            crate::session_scanner::cli_tool::CliTool::Grok,
            &[home_a, home_b.clone()],
            project,
        )
        .expect("cold lookup finds grok history");

        assert_eq!(transcript, newest);
        assert_eq!(GrokAccountProvider.session_dir(&transcript), Some(home_b));
    }

    #[test]
    fn a_session_file_resolves_the_home_that_owns_it() {
        // Regression: commit c1005ec left resume derivation with no way back
        // from a grok transcript to the account whose history holds it.
        assert_eq!(
            GrokAccountProvider.session_dir(Path::new(
                "/accounts/work/sessions/%2Fhome%2Fuser/01a04585/events.jsonl"
            )),
            Some(PathBuf::from("/accounts/work"))
        );
        assert_eq!(
            GrokAccountProvider.session_dir(Path::new(
                "/accounts/work/sessions/2026/08/28/rollout-session.jsonl"
            )),
            None,
            "a foreign sessions layout is not a grok home"
        );
        assert_eq!(
            GrokAccountProvider.session_dir(Path::new("/accounts/work/notes.jsonl")),
            None
        );
    }
}
