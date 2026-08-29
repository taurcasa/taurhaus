//! Antigravity CLI session identity from its cwd index and presence locks.

use std::collections::{BTreeMap, HashMap};
#[cfg(test)]
use std::fs::File;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use fs2::FileExt;

use super::{ActivitySource, AuthoritativeState, IdleResult, SessionResolver, SessionSource};
use crate::session_scanner::SessionState;

const APP_DATA_SUBDIR: &str = "antigravity-cli";
const LAST_CONVERSATIONS: &str = "cache/last_conversations.json";
const CONVERSATIONS_DIR: &str = "conversations";
const PRESENCE_DIR: &str = "presence";
const MAX_HOOK_RECORD_AGE: Duration = Duration::from_secs(5 * 60);

/// The identity last logged per cwd, so a per-poll resolution only speaks up
/// when it changes.
static LAST_RESOLVED_IDENTITY: Mutex<BTreeMap<String, (String, &'static str)>> =
    Mutex::new(BTreeMap::new());

#[cfg(test)]
static BASE_DIR_FOR_TEST: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
#[cfg(test)]
pub(crate) static AGY_RESOLVER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) struct BaseDirOverride;

#[cfg(test)]
impl Drop for BaseDirOverride {
    fn drop(&mut self) {
        *BASE_DIR_FOR_TEST.lock().expect("agy test root lock") = None;
    }
}

#[cfg(test)]
pub(crate) fn set_base_dir_for_test(base_dir: PathBuf) -> BaseDirOverride {
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
            base_dir: Some(crate::provider::platform_paths::PlatformPaths::agy_dir()),
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
        agy_session_for_cwd(
            Path::new(project_path),
            &base_dir,
            &crate::provider::platform_paths::PlatformPaths::agy_hooks_path(),
        )
    }

    fn resume_session_id(&self, project_path: &str) -> Option<String> {
        let base_dir = self.resolved_base_dir()?;
        indexed_conversation_for_cwd(Path::new(project_path), &base_dir)
            .map(|(conversation_id, _)| conversation_id)
    }
}

impl SessionSource for AgyResolver {
    fn resolve(&self, project_path: &str, pid: u32, _pane_id: Option<&str>) -> IdleResult {
        let cwd = crate::platform::process_cwd(pid).unwrap_or_else(|| PathBuf::from(project_path));
        let Some(base_dir) = self.resolved_base_dir() else {
            return IdleResult::idle();
        };
        agy_session_for_cwd(
            &cwd,
            &base_dir,
            &crate::provider::platform_paths::PlatformPaths::agy_hooks_path(),
        )
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

fn agy_session_for_cwd(cwd: &Path, base_dir: &Path, hooks_path: &Path) -> IdleResult {
    if let Some((conversation_id, transcript)) = indexed_conversation_for_cwd(cwd, base_dir) {
        if presence_lock_is_held(&presence_lock_path(base_dir, &conversation_id)) {
            return live_session(cwd, conversation_id, Some(transcript), "index");
        }
    }

    // agy writes a conversation into its cwd index lazily, so a session it has
    // not indexed yet resolves to nothing at all — and a session with no
    // identity never consults its hook state. The hook stream names the
    // workspace of every turn it reports, which is the same claim the index
    // makes, one turn earlier.
    match hook_conversation_for_cwd(cwd, base_dir, hooks_path) {
        Some((conversation_id, transcript)) => {
            live_session(cwd, conversation_id, transcript, "hooks")
        }
        None => IdleResult::idle(),
    }
}

fn live_session(
    cwd: &Path,
    conversation_id: String,
    transcript: Option<PathBuf>,
    source: &'static str,
) -> IdleResult {
    log_identity_resolved(cwd, &conversation_id, source);
    IdleResult {
        state: SessionState::Idle,
        session_id: Some(conversation_id),
        jsonl_path: transcript.map(|path| path.to_string_lossy().into_owned()),
        last_output_age_secs: None,
        authoritative: false,
    }
}

/// The newest live conversation the hook stream reports for this cwd.
fn hook_conversation_for_cwd(
    cwd: &Path,
    base_dir: &Path,
    hooks_path: &Path,
) -> Option<(String, Option<PathBuf>)> {
    workspace_forms(cwd)
        .iter()
        .flat_map(|workspace| {
            crate::daemon::agy_hooks::records_for_workspace(hooks_path, workspace)
        })
        .find(|record| {
            valid_conversation_id(&record.conversation_id)
                && presence_lock_is_held(&presence_lock_path(base_dir, &record.conversation_id))
        })
        .map(|record| {
            let transcript = base_dir
                .join(APP_DATA_SUBDIR)
                .join(CONVERSATIONS_DIR)
                .join(format!("{}.db", record.conversation_id));
            let transcript = transcript.is_file().then_some(transcript);
            (record.conversation_id, transcript)
        })
}

/// The cwd in the normalized form hook records are written in, followed by its
/// canonicalized form when a symlink makes that a different path — the same two
/// spellings [`conversation_for_cwd`] looks the index up under.
fn workspace_forms(cwd: &Path) -> Vec<String> {
    let raw = crate::provider::path::normalize_project_path(&cwd.to_string_lossy());
    let canonical = std::fs::canonicalize(cwd)
        .ok()
        .map(|path| crate::provider::path::normalize_project_path(&path.to_string_lossy()))
        .filter(|path| *path != raw);
    std::iter::once(raw).chain(canonical).collect()
}

fn presence_lock_path(base_dir: &Path, conversation_id: &str) -> PathBuf {
    base_dir
        .join(APP_DATA_SUBDIR)
        .join(PRESENCE_DIR)
        .join(format!("{conversation_id}.lock"))
}

/// Identity resolution runs on every scan poll; only a change is worth a line.
fn log_identity_resolved(cwd: &Path, conversation_id: &str, source: &'static str) {
    let project_path = cwd.to_string_lossy().into_owned();
    {
        let mut resolved = LAST_RESOLVED_IDENTITY
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if resolved
            .get(&project_path)
            .is_some_and(|(previous_id, previous_source)| {
                previous_id == conversation_id && *previous_source == source
            })
        {
            return;
        }
        resolved.insert(project_path.clone(), (conversation_id.to_string(), source));
    }

    tracing::debug!(
        project_path = %project_path,
        session_id = conversation_id,
        source,
        "Antigravity session identity resolved"
    );
    let mut fields = serde_json::Map::new();
    fields.insert(
        "project_path".to_string(),
        serde_json::Value::String(project_path),
    );
    fields.insert(
        "session_id".to_string(),
        serde_json::Value::String(conversation_id.to_string()),
    );
    fields.insert(
        "source".to_string(),
        serde_json::Value::String(source.to_string()),
    );
    crate::commands::logging::emit_global(
        "debug",
        "backend",
        "agy.identity.resolved",
        Some("Antigravity session identity resolved".to_string()),
        fields,
    );
}

fn indexed_conversation_for_cwd(cwd: &Path, base_dir: &Path) -> Option<(String, PathBuf)> {
    let app_data = base_dir.join(APP_DATA_SUBDIR);
    let raw = std::fs::read_to_string(app_data.join(LAST_CONVERSATIONS)).ok()?;
    let conversations: HashMap<String, String> = serde_json::from_str(&raw).ok()?;
    let conversation_id = conversation_for_cwd(&conversations, cwd)?;
    if !valid_conversation_id(conversation_id) {
        return None;
    }

    let transcript = app_data
        .join(CONVERSATIONS_DIR)
        .join(format!("{conversation_id}.db"));
    if !transcript.is_file() {
        return None;
    }
    Some((conversation_id.to_string(), transcript))
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

    fn agy_root(tmp: &TempDir) -> PathBuf {
        let root = tmp.path().join(".gemini");
        let app_data = root.join(APP_DATA_SUBDIR);
        fs::create_dir_all(app_data.join("cache")).unwrap();
        fs::create_dir_all(app_data.join(CONVERSATIONS_DIR)).unwrap();
        fs::create_dir_all(app_data.join(PRESENCE_DIR)).unwrap();
        root
    }

    fn write_index(root: &Path, entries: serde_json::Value) {
        fs::write(
            root.join(APP_DATA_SUBDIR).join(LAST_CONVERSATIONS),
            entries.to_string(),
        )
        .unwrap();
    }

    fn conversation_db(root: &Path, session_id: &str) -> PathBuf {
        let transcript = root
            .join(APP_DATA_SUBDIR)
            .join(CONVERSATIONS_DIR)
            .join(format!("{session_id}.db"));
        File::create(&transcript).unwrap();
        transcript
    }

    fn hold_presence(root: &Path, session_id: &str) -> File {
        let presence = File::create(
            root.join(APP_DATA_SUBDIR)
                .join(PRESENCE_DIR)
                .join(format!("{session_id}.lock")),
        )
        .unwrap();
        presence.lock_exclusive().unwrap();
        presence
    }

    fn append_hook_busy(sink: &Path, session_id: &str, workspace: Option<&str>) {
        let payload = match workspace {
            Some(workspace) => serde_json::json!({
                "conversationId": session_id,
                "invocationNum": 0,
                "modelName": "gemini-3.7-flash-high",
                "workspacePaths": [workspace],
            }),
            None => serde_json::json!({
                "conversationId": session_id,
                "invocationNum": 0,
            }),
        };
        crate::daemon::agy_hooks::append_event_at(
            sink,
            crate::daemon::agy_hooks::AgyHookEvent::Busy,
            &payload.to_string(),
            chrono::Utc::now(),
        )
        .unwrap();
    }

    fn install_hooks(tmp: &TempDir, root: &Path) {
        let executable = tmp.path().join("taurhaus-daemon");
        fs::write(&executable, "fixture").unwrap();
        crate::coordination::agy_hooks_installer::ensure_agy_hooks_installed_at(root, &executable)
            .unwrap();
    }

    fn live_fixture(tmp: &TempDir, project: &str, session_id: &str) -> (PathBuf, File) {
        let root = agy_root(tmp);
        write_index(&root, serde_json::json!({ project: session_id }));
        let transcript = conversation_db(&root, session_id);
        let presence = hold_presence(&root, session_id);
        (transcript, presence)
    }

    #[test]
    fn agy_runtime_source_uses_last_conversation_and_presence_lock() {
        // Regression: f90b362 replaced the third harness's project-scoped
        // source with NoSessionSource; agy identity must survive through the
        // registry using its cwd map and flock-held presence record.
        let _guard = AGY_RESOLVER_TEST_LOCK.lock().unwrap();
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

        let result = agy_session_for_cwd(
            Path::new(project),
            &tmp.path().join(".gemini"),
            &tmp.path().join("agy-hooks.jsonl"),
        );
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

    #[test]
    fn hook_workspace_attaches_a_conversation_agy_has_not_indexed_yet() {
        // Regression: commit 54c9103 resolved agy identity only through
        // `cache/last_conversations.json`. Seen live 2026-08-29: an agy session
        // in a trusted workspace fired hooks for conversation d888d2e9… while
        // agy had not yet written that cwd into the index, so identity resolved
        // to nothing, the hook state was never consulted, and the session
        // blinked idle/active off the rchar heuristic instead.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let cwd = "/home/user/projects/QueenUI";
        let conversation = "d888d2e9-9a2c-4c6d-8f21-6c4f0f0f5a10";
        write_index(
            &root,
            serde_json::json!({ "/home/user/projects/other": "0c1e8fd0-other" }),
        );
        let transcript = conversation_db(&root, conversation);
        let _presence = hold_presence(&root, conversation);
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, conversation, Some(cwd));
        install_hooks(&tmp, &root);

        let resolved = agy_session_for_cwd(Path::new(cwd), &root, &sink);

        assert_eq!(resolved.session_id.as_deref(), Some(conversation));
        assert_eq!(resolved.jsonl_path.as_deref(), transcript.to_str());
        assert!(!resolved.authoritative);
        assert_eq!(
            agy_hook_state_at(&resolved, &sink, &root)
                .expect("hook state for a hook-attached session")
                .state,
            SessionState::Active
        );
    }

    #[test]
    fn the_cwd_index_wins_when_it_and_the_hook_stream_agree_on_the_cwd() {
        // The index is agy's own record of the cwd; the hook stream is only the
        // fallback for what the index has not caught up with yet.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let cwd = "/home/user/projects/indexed";
        let indexed = "11111111-1111-4111-8111-111111111111";
        let from_hooks = "22222222-2222-4222-8222-222222222222";
        write_index(&root, serde_json::json!({ cwd: indexed }));
        let transcript = conversation_db(&root, indexed);
        conversation_db(&root, from_hooks);
        let _indexed_presence = hold_presence(&root, indexed);
        let _hook_presence = hold_presence(&root, from_hooks);
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, from_hooks, Some(cwd));

        let resolved = agy_session_for_cwd(Path::new(cwd), &root, &sink);

        assert_eq!(resolved.session_id.as_deref(), Some(indexed));
        assert_eq!(resolved.jsonl_path.as_deref(), transcript.to_str());
    }

    #[test]
    fn a_stale_indexed_conversation_gives_way_to_the_live_hook_conversation() {
        // agy leaves the previous conversation in the index after it ends; a
        // released presence lock is exactly the case the fallback exists for.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let cwd = "/home/user/projects/resumed";
        let stale = "33333333-3333-4333-8333-333333333333";
        let live = "44444444-4444-4444-8444-444444444444";
        write_index(&root, serde_json::json!({ cwd: stale }));
        conversation_db(&root, stale);
        let stale_presence = hold_presence(&root, stale);
        FileExt::unlock(&stale_presence).unwrap();
        let transcript = conversation_db(&root, live);
        let _live_presence = hold_presence(&root, live);
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, live, Some(cwd));

        let resolved = agy_session_for_cwd(Path::new(cwd), &root, &sink);

        assert_eq!(resolved.session_id.as_deref(), Some(live));
        assert_eq!(resolved.jsonl_path.as_deref(), transcript.to_str());
    }

    #[test]
    fn a_hook_record_without_a_workspace_attaches_to_nothing() {
        // Records the 0.8.2 sink wrote carry no workspace at all: they must not
        // attach to whichever cwd happens to be asking.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let cwd = "/home/user/projects/unlabelled";
        let conversation = "55555555-5555-4555-8555-555555555555";
        write_index(&root, serde_json::json!({}));
        conversation_db(&root, conversation);
        let _presence = hold_presence(&root, conversation);
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, conversation, None);

        assert_eq!(
            agy_session_for_cwd(Path::new(cwd), &root, &sink),
            IdleResult::idle()
        );
    }

    #[test]
    fn a_hook_workspace_never_attaches_to_another_projects_cwd() {
        // Two agy sessions run side by side; the fallback matches the workspace
        // the record names and nothing else.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let first = "66666666-6666-4666-8666-666666666666";
        let second = "77777777-7777-4777-8777-777777777777";
        write_index(&root, serde_json::json!({}));
        conversation_db(&root, first);
        let second_transcript = conversation_db(&root, second);
        let _first_presence = hold_presence(&root, first);
        let _second_presence = hold_presence(&root, second);
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, first, Some("/home/user/projects/first"));
        append_hook_busy(&sink, second, Some("/home/user/projects/second"));

        let resolved = agy_session_for_cwd(Path::new("/home/user/projects/second"), &root, &sink);
        assert_eq!(resolved.session_id.as_deref(), Some(second));
        assert_eq!(resolved.jsonl_path.as_deref(), second_transcript.to_str());
        assert_eq!(
            agy_session_for_cwd(Path::new("/home/user/projects/third"), &root, &sink),
            IdleResult::idle()
        );
    }

    #[test]
    fn a_hook_workspace_without_a_held_presence_lock_is_not_a_live_session() {
        // The presence lock is what proves the conversation is still running;
        // the hook stream keeps its last record long after the session ended.
        let _hooks = crate::daemon::agy_hooks::AGY_HOOK_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let root = agy_root(&tmp);
        let cwd = "/home/user/projects/ended";
        let conversation = "88888888-8888-4888-8888-888888888888";
        write_index(&root, serde_json::json!({}));
        conversation_db(&root, conversation);
        let presence = hold_presence(&root, conversation);
        FileExt::unlock(&presence).unwrap();
        let sink = tmp.path().join("agy-hooks.jsonl");
        append_hook_busy(&sink, conversation, Some(cwd));

        assert_eq!(
            agy_session_for_cwd(Path::new(cwd), &root, &sink),
            IdleResult::idle()
        );
    }
}
