//! Antigravity CLI session identity from its cwd index and presence locks.

use std::collections::HashMap;
#[cfg(test)]
use std::fs::File;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use fs2::FileExt;

use super::{IdleResult, SessionResolver, SessionSource};
use crate::session_scanner::SessionState;

const APP_DATA_SUBDIR: &str = "antigravity-cli";
const LAST_CONVERSATIONS: &str = "cache/last_conversations.json";
const CONVERSATIONS_DIR: &str = "conversations";
const PRESENCE_DIR: &str = "presence";

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
    let Ok(file) = OpenOptions::new().read(true).write(true).open(path) else {
        return false;
    };
    match file.try_lock_exclusive() {
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
            4242,
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
}
