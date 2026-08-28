//! Antigravity CLI session identity from its cwd index and presence locks.

use std::collections::HashMap;
#[cfg(test)]
use std::fs::File;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use fs2::FileExt;

use super::{ActivitySource, AuthoritativeState, IdleResult, SessionResolver, SessionSource};
use crate::session_scanner::SessionState;

const APP_DATA_SUBDIR: &str = "antigravity-cli";
const LAST_CONVERSATIONS: &str = "cache/last_conversations.json";
const CONVERSATIONS_DIR: &str = "conversations";
const PRESENCE_DIR: &str = "presence";
const MAX_HOOK_RECORD_AGE: Duration = Duration::from_secs(5 * 60);

#[cfg(test)]
static BASE_DIR_FOR_TEST: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

#[cfg(test)]
struct BaseDirOverride;

#[cfg(test)]
impl Drop for BaseDirOverride {
    fn drop(&mut self) {
        *BASE_DIR_FOR_TEST.lock().expect("agy test root lock") = None;
    }
}

#[cfg(test)]
fn set_base_dir_for_test(base_dir: PathBuf) -> BaseDirOverride {
    *BASE_DIR_FOR_TEST.lock().expect("agy test root lock") = Some(base_dir);
    BaseDirOverride
}

/// Resolves a live agy conversation without interpreting its SQLite transcript.
pub struct AgyResolver {
    /// Shared Google tooling root (`~/.gemini`).
    base_dir: Option<PathBuf>,
}

impl AgyResolver {
    pub fn new() -> Self {
        Self {
            base_dir: dirs::home_dir().map(|home| home.join(".gemini")),
        }
    }

    fn resolved_base_dir(&self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(base_dir) = BASE_DIR_FOR_TEST
            .lock()
            .expect("agy test root lock")
            .clone()
        {
            return Some(base_dir);
        }

        self.base_dir.clone()
    }
}

impl Default for AgyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionResolver for AgyResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let Some(base_dir) = self.resolved_base_dir() else {
            return IdleResult::idle();
        };
        agy_session_for_cwd(Path::new(project_path), &base_dir)
    }
}

impl SessionSource for AgyResolver {
    fn resolve(&self, project_path: &str, pid: u32, _pane_id: Option<&str>) -> IdleResult {
        let cwd = crate::platform::process_cwd(pid).unwrap_or_else(|| PathBuf::from(project_path));
        let Some(base_dir) = self.resolved_base_dir() else {
            return IdleResult::idle();
        };
        agy_session_for_cwd(&cwd, &base_dir)
    }
}

pub struct AgyHooksActivitySource;

impl AgyHooksActivitySource {
    pub fn authoritative_state_at(
        resolved: &IdleResult,
        sink_path: &Path,
        agy_root: &Path,
    ) -> Option<AuthoritativeState> {
        agy_hook_state_at(resolved, sink_path, agy_root)
    }
}

impl ActivitySource for AgyHooksActivitySource {
    fn authoritative_state(
        &self,
        _project_path: &str,
        _pid: u32,
        resolved: &IdleResult,
    ) -> Option<AuthoritativeState> {
        Self::authoritative_state_at(
            resolved,
            &crate::provider::platform_paths::PlatformPaths::agy_hooks_path(),
            &crate::provider::platform_paths::PlatformPaths::agy_dir(),
        )
    }
}

fn agy_hook_state_at(
    resolved: &IdleResult,
    sink_path: &Path,
    agy_root: &Path,
) -> Option<AuthoritativeState> {
    let session_id = resolved.session_id.as_deref()?;
    if !crate::coordination::agy_hooks_installer::agy_hooks_installed_at(agy_root) {
        return None;
    }
    let freshness_floor = SystemTime::now()
        .checked_sub(MAX_HOOK_RECORD_AGE)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let transcript_mtime = resolved
        .jsonl_path
        .as_deref()
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let record = crate::daemon::agy_hooks::latest_record_for_session_after(
        sink_path,
        session_id,
        freshness_floor.max(transcript_mtime),
    )?;
    Some(AuthoritativeState {
        state: match record.state {
            crate::daemon::agy_hooks::AgyHookState::Busy => SessionState::Active,
            crate::daemon::agy_hooks::AgyHookState::Idle => SessionState::Idle,
        },
        source: "agy_hooks",
    })
}

fn agy_session_for_cwd(cwd: &Path, base_dir: &Path) -> IdleResult {
    let app_data = base_dir.join(APP_DATA_SUBDIR);
    let raw = match std::fs::read_to_string(app_data.join(LAST_CONVERSATIONS)) {
        Ok(raw) => raw,
        Err(_) => return IdleResult::idle(),
    };
    let conversations: HashMap<String, String> = match serde_json::from_str(&raw) {
        Ok(conversations) => conversations,
        Err(_) => return IdleResult::idle(),
    };
    let Some(conversation_id) = conversation_for_cwd(&conversations, cwd) else {
        return IdleResult::idle();
    };
    if !valid_conversation_id(conversation_id) {
        return IdleResult::idle();
    }

    let transcript = app_data
        .join(CONVERSATIONS_DIR)
        .join(format!("{conversation_id}.db"));
    let presence = app_data
        .join(PRESENCE_DIR)
        .join(format!("{conversation_id}.lock"));
    if !transcript.is_file() || !presence_lock_is_held(&presence) {
        return IdleResult::idle();
    }

    IdleResult {
        state: SessionState::Idle,
        session_id: Some(conversation_id.to_string()),
        jsonl_path: Some(transcript.to_string_lossy().into_owned()),
        last_output_age_secs: None,
        authoritative: false,
    }
}

fn conversation_for_cwd<'a>(
    conversations: &'a HashMap<String, String>,
    cwd: &Path,
) -> Option<&'a str> {
    let raw = cwd.to_string_lossy();
    conversations
        .get(raw.as_ref())
        .or_else(|| {
            std::fs::canonicalize(cwd)
                .ok()
                .and_then(|path| conversations.get(path.to_string_lossy().as_ref()))
        })
        .map(String::as_str)
}

fn valid_conversation_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// A held flock proves the conversation is live. File presence alone does not.
pub(crate) fn presence_lock_is_held(path: &Path) -> bool {
    let Ok(file) = OpenOptions::new().read(true).open(path) else {
        return false;
    };
    match FileExt::try_lock_shared(&file) {
        Ok(()) => {
            let _ = FileExt::unlock(&file);
            false
        }
        Err(error) => error.kind() == std::io::ErrorKind::WouldBlock,
    }
}

#[cfg(test)]
fn presence_path_for_transcript(transcript: &Path) -> Option<PathBuf> {
    let file_name = transcript.file_stem()?.to_str()?;
    let conversations = transcript.parent()?;
    if conversations.file_name()?.to_str()? != CONVERSATIONS_DIR {
        return None;
    }
    let app_data = conversations.parent()?;
    Some(
        app_data
            .join(PRESENCE_DIR)
            .join(format!("{file_name}.lock")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn live_fixture(tmp: &TempDir, project: &str, session_id: &str) -> (PathBuf, File) {
        let root = tmp.path().join(".gemini");
        let app_data = root.join(APP_DATA_SUBDIR);
        fs::create_dir_all(app_data.join("cache")).unwrap();
        fs::create_dir_all(app_data.join(CONVERSATIONS_DIR)).unwrap();
        fs::create_dir_all(app_data.join(PRESENCE_DIR)).unwrap();
        fs::write(
            app_data.join(LAST_CONVERSATIONS),
            serde_json::json!({ project: session_id }).to_string(),
        )
        .unwrap();
        let transcript = app_data
            .join(CONVERSATIONS_DIR)
            .join(format!("{session_id}.db"));
        File::create(&transcript).unwrap();
        let presence = File::create(
            app_data
                .join(PRESENCE_DIR)
                .join(format!("{session_id}.lock")),
        )
        .unwrap();
        presence.lock_exclusive().unwrap();
        (transcript, presence)
    }

    #[test]
    fn agy_runtime_source_uses_last_conversation_and_presence_lock() {
        // Regression: f90b362 replaced the third harness's project-scoped
        // source with NoSessionSource; agy identity must survive through the
        // registry using its cwd map and flock-held presence record.
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/runtime-agy";
        let session_id = "7f71fcb0-8a57-4f01-a3fd-a6f43cf70869";
        let (transcript, _presence) = live_fixture(&tmp, project, session_id);

        let _base_dir = set_base_dir_for_test(tmp.path().join(".gemini"));
        let result = crate::session_scanner::idle::detect_runtime_idle(
            project,
            u32::MAX,
            Some("%42"),
            crate::session_scanner::cli_tool::CliTool::Agy,
        );

        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(result.session_id.as_deref(), Some(session_id));
        assert_eq!(result.jsonl_path.as_deref(), transcript.to_str());
        assert_eq!(result.last_output_age_secs, None);
        assert!(!result.authoritative);
    }

    #[test]
    fn unlocked_presence_file_is_not_a_live_agy_session() {
        // Regression: commit 9a66d1c treated a matching transcript file as a
        // live session; agy intentionally leaves stale presence files behind.
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/stale-agy";
        let session_id = "a15f125f-4ac1-4ce8-a828-bcb057a12dac";
        let (transcript, presence) = live_fixture(&tmp, project, session_id);
        FileExt::unlock(&presence).unwrap();

        let result = agy_session_for_cwd(Path::new(project), &tmp.path().join(".gemini"));
        assert_eq!(result, IdleResult::idle());
        assert_eq!(
            presence_path_for_transcript(&transcript),
            Some(
                tmp.path()
                    .join(".gemini/antigravity-cli/presence")
                    .join(format!("{session_id}.lock"))
            )
        );
    }

    #[test]
    fn shared_presence_probe_does_not_claim_another_reader_is_live() {
        // Regression: commit efcd7d2 probed foreign presence state with an
        // exclusive lock, briefly competing with agy's own lock acquisition.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("conversation.lock");
        let reader = File::create(&path).unwrap();
        FileExt::lock_shared(&reader).unwrap();

        assert!(!presence_lock_is_held(&path));
        FileExt::unlock(&reader).unwrap();
    }

    #[test]
    fn enabled_agy_hooks_are_authoritative_but_disabled_hooks_use_the_floor() {
        // Regression: commit c0aa59a made native activity authoritative only
        // for always-on sources; agy's unverified hook loading must be opt-in.
        let _guard = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".gemini");
        let sink = tmp.path().join("agy-hooks.jsonl");
        let executable = tmp.path().join("taurhaus-daemon");
        fs::write(&executable, "fixture").unwrap();
        let resolved = IdleResult {
            state: SessionState::Idle,
            session_id: Some("conversation-1".to_string()),
            jsonl_path: Some("conversation-1.db".to_string()),
            last_output_age_secs: None,
            authoritative: false,
        };
        crate::daemon::agy_hooks::append_event_at(
            &sink,
            crate::daemon::agy_hooks::AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            chrono::Utc::now(),
        )
        .unwrap();

        assert!(agy_hook_state_at(&resolved, &sink, &root).is_none());
        crate::coordination::agy_hooks_installer::ensure_agy_hooks_installed_at(&root, &executable)
            .unwrap();
        assert_eq!(
            agy_hook_state_at(&resolved, &sink, &root)
                .expect("enabled hook state")
                .state,
            SessionState::Active
        );
    }

    #[test]
    fn stale_busy_hook_record_falls_back_to_process_activity() {
        // Regression: commit 4e9e2c5 treated the newest busy record as timeless,
        // so a crashed invocation pinned a resumed conversation Active forever.
        let _guard = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".gemini");
        let sink = tmp.path().join("agy-hooks.jsonl");
        let executable = tmp.path().join("taurhaus-daemon");
        fs::write(&executable, "fixture").unwrap();
        crate::coordination::agy_hooks_installer::ensure_agy_hooks_installed_at(&root, &executable)
            .unwrap();
        let transcript = tmp.path().join("conversation-1.db");
        fs::write(&transcript, "fixture").unwrap();
        let resolved = IdleResult {
            state: SessionState::Idle,
            session_id: Some("conversation-1".to_string()),
            jsonl_path: Some(transcript.to_string_lossy().into_owned()),
            last_output_age_secs: None,
            authoritative: false,
        };
        crate::daemon::agy_hooks::append_event_at(
            &sink,
            crate::daemon::agy_hooks::AgyHookEvent::Busy,
            r#"{"conversationId":"conversation-1"}"#,
            chrono::Utc::now() - chrono::Duration::hours(1),
        )
        .unwrap();

        assert!(agy_hook_state_at(&resolved, &sink, &root).is_none());
    }
}
