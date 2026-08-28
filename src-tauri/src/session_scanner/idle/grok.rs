//! Grok CLI session identity and activity.
//!
//! Two files carry everything taurhaus needs, and each answers exactly one
//! question (verified against grok 1.0.5, 2026-08-28):
//!
//! - `<GROK_HOME>/active_sessions.json` is a live registry of **interactive**
//!   sessions: `[{session_id, pid, cwd, opened_at}]`. The row appears at the
//!   first prompt (not at process start), is removed on `/quit`, and survives a
//!   `SIGKILL` as a stale row until the next grok run prunes it. It proves
//!   residence, never busy state.
//! - `<GROK_HOME>/sessions/<encoded-cwd>/<session-id>/events.jsonl` is the turn
//!   lifecycle: `turn_started`, `loop_started`, `phase_changed`, `first_token`,
//!   `turn_ended`. The session is busy iff the newest lifecycle line is not
//!   `turn_ended`; `phase_changed` repeats once per streamed chunk, so the state
//!   is derived from the last line rather than from each event.
//!
//! The encoded-cwd group directory is never decoded back into a path — grok
//! falls back to a slug plus hash (with a `.cwd` marker file) once the encoded
//! name exceeds a path component. The session directory is found by its own id
//! and confirmed against `summary.json`'s authoritative `info.id`.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;

use super::{ActivitySource, AuthoritativeState, IdleResult, SessionResolver, SessionSource};
use crate::provider::path::normalize_project_path;
use crate::session_scanner::SessionState;

const ACTIVE_SESSIONS_FILE: &str = "active_sessions.json";
/// The files a grok session directory holds, newest-answer first. A path that
/// names none of them is not a grok transcript, so a foreign `<home>/sessions/…`
/// layout can never be attributed to a grok home.
pub(crate) const SESSION_FILES: &[&str] = &[
    EVENTS_FILE,
    "updates.jsonl",
    "chat_history.jsonl",
    SUMMARY_FILE,
    "signals.json",
];
const SESSIONS_DIR: &str = "sessions";
const EVENTS_FILE: &str = "events.jsonl";
const SUMMARY_FILE: &str = "summary.json";
const TURN_ENDED: &str = "turn_ended";
/// Lifecycle types this build understands. An unknown type is ignored so a new
/// grok event cannot be read as a turn boundary.
const LIFECYCLE_TYPES: &[&str] = &[
    "turn_started",
    "loop_started",
    "phase_changed",
    "first_token",
    TURN_ENDED,
];
/// Enough tail to hold the newest lifecycle line without reading a long turn's
/// whole `phase_changed` stream.
const EVENTS_TAIL_BYTES: u64 = 8 * 1024;
const EVENTS_CACHE_MAX_ENTRIES: usize = 128;

#[cfg(test)]
static BASE_DIR_FOR_TEST: Mutex<Option<PathBuf>> = Mutex::new(None);
#[cfg(test)]
pub(crate) static GROK_RESOLVER_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) struct BaseDirOverride;

#[cfg(test)]
impl Drop for BaseDirOverride {
    fn drop(&mut self) {
        *BASE_DIR_FOR_TEST.lock().expect("grok test root lock") = None;
    }
}

#[cfg(test)]
pub(crate) fn set_base_dir_for_test(base_dir: PathBuf) -> BaseDirOverride {
    *BASE_DIR_FOR_TEST.lock().expect("grok test root lock") = Some(base_dir);
    BaseDirOverride
}

/// One row of `active_sessions.json`.
#[derive(Debug, Clone, Deserialize)]
struct ActiveSession {
    session_id: String,
    pid: u32,
    cwd: String,
}

/// `summary.json`'s authoritative identity block.
#[derive(Debug, Deserialize)]
struct SessionSummary {
    info: SessionSummaryInfo,
}

#[derive(Debug, Deserialize)]
struct SessionSummaryInfo {
    id: String,
    /// The directory the session ran in. Authoritative — the encoded group
    /// directory name is a slug plus hash once the cwd outgrows a path
    /// component, so it is never decoded back into a path.
    #[serde(default)]
    cwd: Option<String>,
}

/// Resolves a live grok session without interpreting its transcript.
pub struct GrokResolver {
    /// Default grok home (`$GROK_HOME` or `~/.grok`).
    base_dir: Option<PathBuf>,
}

impl GrokResolver {
    pub fn new() -> Self {
        Self {
            base_dir: Some(crate::provider::platform_paths::PlatformPaths::grok_dir()),
        }
    }

    fn default_base_dir(&self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(base_dir) = BASE_DIR_FOR_TEST
            .lock()
            .expect("grok test root lock")
            .clone()
        {
            return Some(base_dir);
        }

        self.base_dir.clone()
    }

    /// The home this process actually runs on. grok isolates every account and
    /// its registry by `GROK_HOME`, so a session started with one must not be
    /// looked up in the default home.
    fn base_dir_for_pid(&self, pid: u32) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(base_dir) = BASE_DIR_FOR_TEST
            .lock()
            .expect("grok test root lock")
            .clone()
        {
            return Some(base_dir);
        }

        #[cfg(not(test))]
        if let Some(home) =
            crate::session_scanner::process::process_selector_value(pid, "GROK_HOME")
        {
            return Some(home);
        }
        #[cfg(test)]
        let _ = pid;

        self.base_dir.clone()
    }

    /// Resolve a session from an explicit grok home.
    ///
    /// Conformance and tests drive this instead of the ambient `GROK_HOME`, so
    /// no test run can read the developer's own `~/.grok`.
    pub fn resolve_at(base_dir: &Path, project_path: &str, pid: u32) -> IdleResult {
        let Some(row) = live_session_for_pid(base_dir, pid, project_path) else {
            return IdleResult::idle();
        };
        idle_result_for(base_dir, &row)
    }

    /// The conversation a `--resume` should reopen, inside one grok home.
    ///
    /// `active_sessions.json` proves residence, not history: grok removes the
    /// row on `/quit`, so the ordinary way to end a conversation also erases the
    /// only live evidence of it. The persisted session records answer instead —
    /// each carries its own authoritative `summary.json`.
    pub fn resume_session_id_at(base_dir: &Path, project_path: &str) -> Option<String> {
        newest_session_record(base_dir, project_path).map(|record| record.session_id)
    }
}

impl Default for GrokResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionResolver for GrokResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let Some(base_dir) = self.default_base_dir() else {
            return IdleResult::idle();
        };
        let Some(row) = live_session_for_project(&base_dir, project_path) else {
            return IdleResult::idle();
        };
        idle_result_for(&base_dir, &row)
    }

    fn resume_session_id_in(
        &self,
        project_path: &str,
        config_dir: Option<&Path>,
    ) -> Option<String> {
        let base_dir = config_dir
            .map(Path::to_path_buf)
            .or_else(|| self.default_base_dir())?;
        Self::resume_session_id_at(&base_dir, project_path)
    }
}

impl SessionSource for GrokResolver {
    fn resolve(&self, project_path: &str, pid: u32, _pane_id: Option<&str>) -> IdleResult {
        let Some(base_dir) = self.base_dir_for_pid(pid) else {
            return IdleResult::idle();
        };
        Self::resolve_at(&base_dir, project_path, pid)
    }
}

/// Whether this pid still holds a registry row — grok removes it on `/quit`.
pub(crate) fn session_registry_holds_pid(base_dir: &Path, pid: u32) -> bool {
    read_active_sessions(base_dir)
        .into_iter()
        .any(|row| row.pid == pid)
}

pub struct GrokEventsActivitySource;

impl GrokEventsActivitySource {
    pub fn authoritative_state_at(resolved: &IdleResult) -> Option<AuthoritativeState> {
        let events = resolved.jsonl_path.as_deref()?;
        events_state(Path::new(events)).map(|state| AuthoritativeState {
            state,
            source: "grok_events",
        })
    }
}

impl ActivitySource for GrokEventsActivitySource {
    fn authoritative_state(
        &self,
        _project_path: &str,
        _pid: u32,
        resolved: &IdleResult,
    ) -> Option<AuthoritativeState> {
        Self::authoritative_state_at(resolved)
    }
}

fn idle_result_for(base_dir: &Path, row: &ActiveSession) -> IdleResult {
    let events = session_events_path(base_dir, &row.session_id);
    let state = events
        .as_deref()
        .and_then(events_state)
        .unwrap_or(SessionState::Idle);
    IdleResult {
        state,
        session_id: Some(row.session_id.clone()),
        jsonl_path: events.map(|path| path.to_string_lossy().into_owned()),
        last_output_age_secs: None,
        authoritative: false,
    }
}

fn read_active_sessions(base_dir: &Path) -> Vec<ActiveSession> {
    let path = base_dir.join(ACTIVE_SESSIONS_FILE);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<ActiveSession>>(&raw) {
        Ok(rows) => rows,
        Err(error) => {
            tracing::debug!(
                path = %path.display(),
                error = %error,
                "unparsable grok active-session registry"
            );
            Vec::new()
        }
    }
}

/// The row this process wrote. The pid comes from the live inventory, so the
/// only failure mode left is a stale row for a recycled pid: the row's own cwd
/// has to agree with the process's, and where the platform cannot report a cwd
/// the row stands rather than being discarded on a guess.
fn live_session_for_pid(base_dir: &Path, pid: u32, project_path: &str) -> Option<ActiveSession> {
    let row = read_active_sessions(base_dir)
        .into_iter()
        .find(|row| row.pid == pid)?;
    let observed = crate::platform::process_cwd(pid)
        .map(|cwd| cwd.to_string_lossy().into_owned())
        .unwrap_or_else(|| project_path.to_string());
    (normalize_project_path(&row.cwd) == normalize_project_path(&observed)).then_some(row)
}

/// The newest live row for a project, used where no pid is known (resume).
fn live_session_for_project(base_dir: &Path, project_path: &str) -> Option<ActiveSession> {
    let wanted = normalize_project_path(project_path);
    read_active_sessions(base_dir)
        .into_iter()
        .filter(|row| normalize_project_path(&row.cwd) == wanted)
        .find(|row| process_is_live(row.pid))
}

/// A registry row outlives a `SIGKILL`, so residence is only proof while the
/// process it names is still there. Platforms that cannot answer do not vote.
fn process_is_live(pid: u32) -> bool {
    crate::platform::process_cwd(pid).is_some() || cfg!(not(target_os = "linux"))
}

/// `<GROK_HOME>/sessions/<group>/<session-id>/events.jsonl`.
///
/// The group directory is grok's percent-encoded cwd, or a slug plus hash for a
/// cwd too long to encode. Neither is decoded here: the session directory is
/// located by its own id and confirmed against `summary.json`'s `info.id`.
fn session_events_path(base_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let events = session_dir_for(base_dir, session_id)?.join(EVENTS_FILE);
    events.is_file().then_some(events)
}

/// The one session directory that claims this id, confirmed by its own summary.
fn session_dir_for(base_dir: &Path, session_id: &str) -> Option<PathBuf> {
    if !valid_session_id(session_id) {
        return None;
    }
    std::fs::read_dir(base_dir.join(SESSIONS_DIR))
        .ok()?
        .flatten()
        .map(|group| group.path().join(session_id))
        .find(|candidate| summary_confirms_session(candidate, session_id))
}

fn summary_confirms_session(session_dir: &Path, session_id: &str) -> bool {
    read_summary(session_dir).is_some_and(|info| info.id == session_id)
}

fn read_summary(session_dir: &Path) -> Option<SessionSummaryInfo> {
    let raw = std::fs::read_to_string(session_dir.join(SUMMARY_FILE)).ok()?;
    serde_json::from_str::<SessionSummary>(&raw)
        .ok()
        .map(|summary| summary.info)
}

/// One persisted session directory belonging to a project.
struct SessionRecord {
    session_id: String,
    dir: PathBuf,
    modified: std::time::SystemTime,
}

/// The newest persisted session a project owns inside one grok home.
///
/// Sessions live at `<home>/sessions/<group>/<session-id>/`, and only each
/// session's own `summary.json` says which directory it ran in — the group name
/// is grok's percent-encoded cwd until the cwd outgrows a path component, after
/// which it is a slug plus hash. Reading `info.cwd` works for both.
fn newest_session_record(base_dir: &Path, project_path: &str) -> Option<SessionRecord> {
    let wanted = normalize_project_path(project_path);
    let mut newest: Option<SessionRecord> = None;
    for group in std::fs::read_dir(base_dir.join(SESSIONS_DIR))
        .ok()?
        .flatten()
    {
        let Ok(sessions) = std::fs::read_dir(group.path()) else {
            continue;
        };
        for session in sessions.flatten() {
            let dir = session.path();
            let Some(info) = read_summary(&dir) else {
                continue;
            };
            if !valid_session_id(&info.id)
                || dir.file_name().and_then(|name| name.to_str()) != Some(info.id.as_str())
            {
                continue;
            }
            if info
                .cwd
                .as_deref()
                .map(normalize_project_path)
                .is_none_or(|cwd| cwd != wanted)
            {
                continue;
            }
            let Some(modified) = session_modified_at(&dir) else {
                continue;
            };
            if newest
                .as_ref()
                .is_none_or(|record| modified > record.modified)
            {
                newest = Some(SessionRecord {
                    session_id: info.id,
                    dir,
                    modified,
                });
            }
        }
    }
    newest
}

/// The newest write anywhere in a session directory that taurhaus recognises.
fn session_modified_at(session_dir: &Path) -> Option<std::time::SystemTime> {
    SESSION_FILES
        .iter()
        .filter_map(|name| std::fs::metadata(session_dir.join(name)).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .max()
}

/// The transcript of one named session inside one grok home, in the shape the
/// account provider maps back to the home that holds it.
pub(crate) fn session_transcript(base_dir: &Path, session_id: &str) -> Option<PathBuf> {
    recognised_transcript(&session_dir_for(base_dir, session_id)?)
}

fn recognised_transcript(session_dir: &Path) -> Option<PathBuf> {
    SESSION_FILES
        .iter()
        .map(|name| session_dir.join(name))
        .find(|path| path.is_file())
}

/// The newest persisted transcript a project owns inside one grok home, in the
/// shape the account provider maps back to the home that holds it.
pub(crate) fn newest_session_transcript(
    base_dir: &Path,
    project_path: &str,
) -> Option<(std::time::SystemTime, PathBuf)> {
    let record = newest_session_record(base_dir, project_path)?;
    Some((record.modified, recognised_transcript(&record.dir)?))
}

/// Session ids are UUIDv7; anything else is not used to build a path.
fn valid_session_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// Cached `(file length, state)` per events file. The file is append-only, so
/// an unchanged length means an unchanged answer and no read at all.
static EVENTS_STATE_CACHE: Mutex<Option<HashMap<PathBuf, (u64, SessionState)>>> = Mutex::new(None);

/// Busy iff the newest lifecycle line is not `turn_ended`.
///
/// `None` means the file cannot answer (missing, empty, or nothing recognised),
/// and the caller falls back to the process-IO floor.
fn events_state(path: &Path) -> Option<SessionState> {
    let length = std::fs::metadata(path).ok()?.len();
    if length == 0 {
        return None;
    }

    {
        let guard = EVENTS_STATE_CACHE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((cached_length, state)) = guard.as_ref().and_then(|cache| cache.get(path)) {
            if *cached_length == length {
                return Some(*state);
            }
        }
    }

    let state = read_tail_state(path, length)?;
    let mut guard = EVENTS_STATE_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if cache.len() >= EVENTS_CACHE_MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(path.to_path_buf(), (length, state));
    Some(state)
}

fn read_tail_state(path: &Path, length: u64) -> Option<SessionState> {
    let mut file = std::fs::File::open(path).ok()?;
    let offset = length.saturating_sub(EVENTS_TAIL_BYTES);
    file.seek(SeekFrom::Start(offset)).ok()?;
    // The seek lands mid-line, and on a long turn mid-character too, so the tail
    // is decoded lossily and the truncated first line simply fails to parse —
    // rather than the whole read failing and flapping a busy session back to the
    // process-IO floor for a poll.
    let mut tail = Vec::new();
    file.take(EVENTS_TAIL_BYTES).read_to_end(&mut tail).ok()?;
    let tail = String::from_utf8_lossy(&tail);

    tail.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|event| {
            event
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .find(|event_type| LIFECYCLE_TYPES.contains(&event_type.as_str()))
        .map(|event_type| {
            if event_type == TURN_ENDED {
                SessionState::Idle
            } else {
                SessionState::Active
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SESSION_ID: &str = "01a04585-2d53-7123-8000-9a0f4d0b21ce";

    fn grok_home(tmp: &TempDir) -> PathBuf {
        tmp.path().join(".grok")
    }

    fn write_registry(home: &Path, rows: serde_json::Value) {
        fs::create_dir_all(home).unwrap();
        fs::write(home.join(ACTIVE_SESSIONS_FILE), rows.to_string()).unwrap();
    }

    fn write_session(home: &Path, group: &str, session_id: &str, events: &[&str]) -> PathBuf {
        write_session_for(home, group, session_id, "/home/user/projects/grok", events)
    }

    fn write_session_for(
        home: &Path,
        group: &str,
        session_id: &str,
        cwd: &str,
        events: &[&str],
    ) -> PathBuf {
        let dir = home.join(SESSIONS_DIR).join(group).join(session_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(SUMMARY_FILE),
            serde_json::json!({ "info": { "id": session_id, "cwd": cwd } }).to_string(),
        )
        .unwrap();
        let events_path = dir.join(EVENTS_FILE);
        fs::write(&events_path, format!("{}\n", events.join("\n"))).unwrap();
        events_path
    }

    /// Give one session a newer mtime than any other without sleeping.
    fn touch_newer(path: &Path) {
        let when = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn a_cleanly_closed_conversation_is_still_resumable() {
        // Regression: commit 358a7c9 resolved a grok resume from the live
        // `active_sessions.json` registry alone. grok removes that row on
        // `/quit`, so the most common way to end a conversation also made it
        // impossible to resume.
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        let project = "/home/user/projects/grok";
        let older = "01a04585-2d53-7123-8000-000000000001";
        let newer = "01a04585-2d53-7123-8000-000000000002";
        write_session_for(&home, "%2Fp", older, project, &[r#"{"type":"turn_ended"}"#]);
        let newest =
            write_session_for(&home, "%2Fp", newer, project, &[r#"{"type":"turn_ended"}"#]);
        write_session_for(
            &home,
            "%2Fother",
            "01a04585-2d53-7123-8000-000000000003",
            "/home/user/projects/other",
            &[r#"{"type":"turn_ended"}"#],
        );
        touch_newer(&newest);
        // No registry at all: this is exactly the state a clean `/quit` leaves.
        assert!(!home.join(ACTIVE_SESSIONS_FILE).exists());

        assert_eq!(
            GrokResolver::resume_session_id_at(&home, project).as_deref(),
            Some(newer),
            "the newest persisted session for this project answers the resume"
        );
        assert_eq!(
            GrokResolver::resume_session_id_at(&home, "/home/user/projects/nothing-here"),
            None
        );
    }

    #[test]
    fn an_explicitly_selected_home_resolves_its_own_history() {
        // Regression: commit 358a7c9 resolved `{session_id}` before the launch
        // account, so a resume onto a second GROK_HOME searched the default
        // home and either failed or named a foreign account's conversation.
        let _guard = GROK_RESOLVER_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let default_home = tmp.path().join(".grok");
        let work_home = tmp.path().join(".grok-work");
        let project = "/home/user/projects/grok";
        let default_session = "01a04585-2d53-7123-8000-00000000000a";
        let work_session = "01a04585-2d53-7123-8000-00000000000b";
        write_session_for(
            &default_home,
            "%2Fp",
            default_session,
            project,
            &[r#"{"type":"turn_ended"}"#],
        );
        write_session_for(
            &work_home,
            "%2Fp",
            work_session,
            project,
            &[r#"{"type":"turn_ended"}"#],
        );

        let _base_dir = set_base_dir_for_test(default_home);
        let resolver = GrokResolver::new();

        assert_eq!(
            SessionResolver::resume_session_id_in(&resolver, project, Some(&work_home)).as_deref(),
            Some(work_session)
        );
        assert_eq!(
            SessionResolver::resume_session_id_in(&resolver, project, None).as_deref(),
            Some(default_session),
            "without an explicit home the default one still answers"
        );
    }

    #[test]
    fn cold_history_lookup_finds_the_newest_transcript_across_homes() {
        // Regression: commit 8fcb5b3 left cold Continue/Resume account
        // derivation on Claude's `<projects>/<slug>/<id>.<ext>` layout, which
        // cannot see grok's `sessions/<group>/<id>/events.jsonl`, so no grok
        // history could name the account that owns it.
        let tmp = TempDir::new().unwrap();
        let home_a = tmp.path().join(".grok");
        let home_b = tmp.path().join(".grok-work");
        let project = "/home/user/projects/grok";
        write_session_for(
            &home_a,
            "%2Fp",
            "01a04585-2d53-7123-8000-00000000000a",
            project,
            &[r#"{"type":"turn_ended"}"#],
        );
        let newest = write_session_for(
            &home_b,
            "%2Fp",
            "01a04585-2d53-7123-8000-00000000000b",
            project,
            &[r#"{"type":"turn_ended"}"#],
        );
        touch_newer(&newest);

        assert_eq!(
            newest_session_transcript(&home_b, project).map(|(_, path)| path),
            Some(newest)
        );
        assert_eq!(newest_session_transcript(&home_a, "/nowhere"), None);
    }

    #[test]
    fn runtime_source_binds_the_registry_row_of_this_process() {
        // Regression: commit 16de5ec registered grok on the session-source
        // floor, so a live grok pane had no session id, no transcript and no
        // way for a resume or an account to find its history.
        let _guard = GROK_RESOLVER_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        let project = std::env::current_dir().unwrap();
        let project_path = project.to_string_lossy().into_owned();
        write_registry(
            &home,
            serde_json::json!([{
                "session_id": SESSION_ID,
                "pid": std::process::id(),
                "cwd": project_path,
                "opened_at": "2026-08-27T23:22:06.993848110Z",
            }]),
        );
        let events = write_session(
            &home,
            "%2Fhome%2Fuser%2Fprojects%2Fgrok",
            SESSION_ID,
            &[r#"{"ts":"2026-08-27T23:19:11.067Z","type":"turn_started","turn_number":0}"#],
        );

        let _base_dir = set_base_dir_for_test(home);
        let result = crate::session_scanner::idle::detect_runtime_idle(
            &project_path,
            std::process::id(),
            None,
            crate::session_scanner::cli_tool::CliTool::Grok,
        );

        assert_eq!(result.session_id.as_deref(), Some(SESSION_ID));
        assert_eq!(result.jsonl_path.as_deref(), events.to_str());
        assert_eq!(result.state, SessionState::Active);
    }

    #[test]
    fn a_stale_registry_row_for_a_recycled_pid_is_ignored() {
        // Regression: commit 16de5ec had no grok identity at all; grok leaves a
        // row behind after SIGKILL, so a fresh pane landing on that pid must not
        // inherit the dead session's id.
        let _guard = GROK_RESOLVER_TEST_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        write_registry(
            &home,
            serde_json::json!([{
                "session_id": SESSION_ID,
                "pid": std::process::id(),
                "cwd": "/home/user/projects/another-project",
                "opened_at": "2026-08-27T23:22:06.993848110Z",
            }]),
        );

        let _base_dir = set_base_dir_for_test(home.clone());
        let resolver = GrokResolver::new();
        let result = SessionSource::resolve(
            &resolver,
            &std::env::current_dir().unwrap().to_string_lossy(),
            std::process::id(),
            None,
        );

        assert_eq!(result, IdleResult::idle());
    }

    #[test]
    fn a_session_directory_is_confirmed_by_its_own_summary() {
        // Regression: commit 16de5ec left no grok transcript resolution, and
        // the encoded-cwd group name must never be decoded back into a path —
        // grok slugs and hashes it once it outgrows a path component.
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        let events = write_session(
            &home,
            "slug-9f2a1c",
            SESSION_ID,
            &[r#"{"type":"turn_ended"}"#],
        );
        let foreign = home
            .join(SESSIONS_DIR)
            .join("%2Fother")
            .join("11111111-2222-7333-8444-555555555555");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(
            foreign.join(SUMMARY_FILE),
            serde_json::json!({ "info": { "id": "someone-else" } }).to_string(),
        )
        .unwrap();

        assert_eq!(session_events_path(&home, SESSION_ID), Some(events));
        assert_eq!(
            session_events_path(&home, "11111111-2222-7333-8444-555555555555"),
            None,
            "a directory whose summary names another session is not a match"
        );
        assert_eq!(session_events_path(&home, "../escape"), None);
    }

    #[test]
    fn activity_is_busy_until_the_turn_ends() {
        // Regression: commit 16de5ec left grok on the rchar floor, so a turn
        // that finished still read as working until the heuristic settled.
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        let events = write_session(
            &home,
            "%2Fhome%2Fuser",
            SESSION_ID,
            &[
                r#"{"ts":"1","type":"turn_started","turn_number":0}"#,
                r#"{"ts":"2","type":"phase_changed","phase":"streaming_reasoning"}"#,
                r#"{"ts":"3","type":"phase_changed","phase":"streaming_text"}"#,
            ],
        );
        let busy = IdleResult {
            state: SessionState::Idle,
            session_id: Some(SESSION_ID.to_string()),
            jsonl_path: Some(events.to_string_lossy().into_owned()),
            last_output_age_secs: None,
            authoritative: false,
        };

        assert_eq!(
            GrokEventsActivitySource::authoritative_state_at(&busy),
            Some(AuthoritativeState {
                state: SessionState::Active,
                source: "grok_events",
            })
        );

        fs::write(
            &events,
            concat!(
                "{\"ts\":\"1\",\"type\":\"turn_started\",\"turn_number\":0}\n",
                "{\"ts\":\"2\",\"type\":\"phase_changed\",\"phase\":\"streaming_text\"}\n",
                "{\"ts\":\"3\",\"type\":\"turn_ended\",\"outcome\":\"completed\"}\n",
            ),
        )
        .unwrap();

        assert_eq!(
            GrokEventsActivitySource::authoritative_state_at(&busy)
                .expect("settled turn")
                .state,
            SessionState::Idle
        );
    }

    #[test]
    fn a_missing_or_unrecognised_events_file_leaves_the_floor_in_charge() {
        // Regression: commit 16de5ec would have let an absent or future-shaped
        // events file claim authority for a state it cannot actually observe.
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        let events = write_session(
            &home,
            "%2Fhome%2Fuser",
            SESSION_ID,
            &[r#"{"type":"gossip"}"#],
        );
        let resolved = |path: Option<&Path>| IdleResult {
            state: SessionState::Idle,
            session_id: Some(SESSION_ID.to_string()),
            jsonl_path: path.map(|path| path.to_string_lossy().into_owned()),
            last_output_age_secs: None,
            authoritative: false,
        };

        assert_eq!(
            GrokEventsActivitySource::authoritative_state_at(&resolved(Some(&events))),
            None
        );
        assert_eq!(
            GrokEventsActivitySource::authoritative_state_at(&resolved(None)),
            None
        );
        assert_eq!(
            GrokEventsActivitySource::authoritative_state_at(&resolved(Some(
                &home.join("nothing-here.jsonl")
            ))),
            None
        );
    }

    #[test]
    fn the_registry_row_disappearing_is_the_clean_stop_proof() {
        // Regression: commit 16de5ec stopped a grok pane on the tmux floor
        // alone, so `/quit` was confirmed only by the pane returning to a shell.
        let tmp = TempDir::new().unwrap();
        let home = grok_home(&tmp);
        write_registry(
            &home,
            serde_json::json!([{
                "session_id": SESSION_ID,
                "pid": 4242,
                "cwd": "/home/user/projects/grok",
                "opened_at": "2026-08-27T23:22:06.993848110Z",
            }]),
        );

        assert!(session_registry_holds_pid(&home, 4242));

        write_registry(&home, serde_json::json!([]));
        assert!(!session_registry_holds_pid(&home, 4242));
    }
}
