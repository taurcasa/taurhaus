use super::*;
use crate::provider::path::normalize_project_path;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::process::ProcessInfo;
use crate::session_scanner::tmux::TmuxPane;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use super::claude_registry::SessionSource;

/// Resolves Codex CLI session files from `~/.codex/sessions/YYYY/MM/DD/`.
///
/// Codex organizes sessions by date, not by project. To find the session
/// for a specific project, we scan recent JSONL files and read the
/// `session_meta` record to match the `cwd` field against the project path.
///
/// A path->file cache avoids re-scanning on every poll.
pub struct CodexResolver {
    /// `~/.codex/sessions/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
    notify_path: PathBuf,
}

pub struct CodexSessionSource;

impl SessionSource for CodexSessionSource {
    fn resolve(&self, project_path: &str, pid: u32, pane_id: Option<&str>) -> IdleResult {
        static RESOLVER: OnceLock<CodexResolver> = OnceLock::new();
        RESOLVER
            .get_or_init(CodexResolver::new)
            .detect_idle_for_pid(project_path, pid, pane_id)
    }
}

struct CodexNotifyActivitySource<'a> {
    notify_path: &'a Path,
}

impl ActivitySource for CodexNotifyActivitySource<'_> {
    fn activity(
        &self,
        _project_path: &str,
        _pid: u32,
        resolved: Option<&IdleResult>,
    ) -> Option<IdleResult> {
        let result = resolved?.clone();
        let result = apply_notify_edge(result, self.notify_path);
        result.authoritative.then_some(result)
    }
}

impl CodexResolver {
    pub fn new() -> Self {
        let base_dir = Some(PlatformPaths::codex_dir().join("sessions"));
        let notify_path = PlatformPaths::codex_notify_path();
        Self {
            base_dir,
            notify_path,
        }
    }

    pub fn detect_idle_for_pid(
        &self,
        project_path: &str,
        pid: u32,
        pane_id: Option<&str>,
    ) -> IdleResult {
        let Some(base) = self.base_dir.as_ref() else {
            return IdleResult::idle();
        };
        let result = codex_detect_idle_for_pid(project_path, pid, pane_id, base);
        let source = CodexNotifyActivitySource {
            notify_path: &self.notify_path,
        };
        ActivitySource::activity(&source, project_path, pid, Some(&result)).unwrap_or(result)
    }
}

fn apply_notify_edge(mut result: IdleResult, notify_path: &Path) -> IdleResult {
    let (Some(session_id), Some(transcript_path)) =
        (result.session_id.as_deref(), result.jsonl_path.as_deref())
    else {
        return result;
    };
    let Some(transcript_mtime) = fs::metadata(transcript_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
    else {
        return result;
    };
    let Some(record) = crate::daemon::codex_notify::latest_record_for_session_after(
        notify_path,
        session_id,
        "agent-turn-complete",
        transcript_mtime,
    ) else {
        return result;
    };
    if record.ts < DateTime::<Utc>::from(transcript_mtime) {
        return result;
    }

    result.state = SessionState::Idle;
    result.authoritative = true;
    result
}

impl Default for CodexResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionResolver for CodexResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let base = match self.base_dir.as_ref() {
            Some(dir) => dir,
            None => return IdleResult::idle(),
        };
        codex_detect_idle(project_path, base)
    }
}

const CODEX_BINDING_STORE_VERSION: u32 = 1;
const CODEX_BINDING_STORE_FILENAME: &str = "codex-transcript-bindings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CodexBindingRecord {
    project_path: String,
    pid: u32,
    pane_id: Option<String>,
    session_id: String,
    jsonl_path: String,
    resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CodexBindingStoreFile {
    version: u32,
    bindings: Vec<CodexBindingRecord>,
}

impl Default for CodexBindingStoreFile {
    fn default() -> Self {
        Self {
            version: CODEX_BINDING_STORE_VERSION,
            bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
struct CodexBindingStoreState {
    loaded_path: Option<PathBuf>,
    bindings: HashMap<String, CodexBindingRecord>,
}

#[cfg(test)]
static CODEX_BINDING_STORE_PATH_OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static CODEX_BINDING_STORE: OnceLock<Mutex<CodexBindingStoreState>> = OnceLock::new();

fn binding_store_path() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = CODEX_BINDING_STORE_PATH_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
    {
        return path;
    }

    PlatformPaths::app_data_root().join(CODEX_BINDING_STORE_FILENAME)
}

fn binding_key(project_path: &str, pid: u32, pane_id: Option<&str>) -> String {
    format!(
        "{}\n{}\n{}",
        normalize_project_path(project_path),
        pid,
        pane_id.unwrap_or("")
    )
}

fn load_binding_store_from_disk(path: &Path) -> HashMap<String, CodexBindingRecord> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return HashMap::new(),
    };
    let parsed: CodexBindingStoreFile = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to parse Codex transcript binding store; starting empty"
            );
            return HashMap::new();
        }
    };

    parsed
        .bindings
        .into_iter()
        .map(|binding| {
            (
                binding_key(
                    &binding.project_path,
                    binding.pid,
                    binding.pane_id.as_deref(),
                ),
                binding,
            )
        })
        .collect()
}

fn save_binding_store_to_disk(path: &Path, bindings: &HashMap<String, CodexBindingRecord>) {
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(error) = fs::create_dir_all(parent) {
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "failed to create Codex transcript binding store directory"
        );
        return;
    }

    let payload = CodexBindingStoreFile {
        version: CODEX_BINDING_STORE_VERSION,
        bindings: bindings.values().cloned().collect(),
    };
    let serialized = match serde_json::to_string_pretty(&payload) {
        Ok(serialized) => serialized,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to serialize Codex transcript binding store"
            );
            return;
        }
    };

    let tmp_path = path.with_extension("json.tmp");
    if let Err(error) = fs::write(&tmp_path, serialized).and_then(|_| fs::rename(&tmp_path, path)) {
        let _ = fs::remove_file(&tmp_path);
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "failed to persist Codex transcript binding store"
        );
    }
}

fn with_binding_store<R>(
    f: impl FnOnce(&mut HashMap<String, CodexBindingRecord>, &Path) -> R,
) -> R {
    let path = binding_store_path();
    let store = CODEX_BINDING_STORE.get_or_init(|| Mutex::new(CodexBindingStoreState::default()));
    let mut guard = store.lock().unwrap_or_else(|error| error.into_inner());
    if guard.loaded_path.as_ref() != Some(&path) {
        guard.bindings = load_binding_store_from_disk(&path);
        guard.loaded_path = Some(path.clone());
    }
    f(&mut guard.bindings, &path)
}

/// Serializes tests that redirect or inspect the process-global binding store.
#[cfg(test)]
pub(crate) static CODEX_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn set_binding_store_path_for_test(path: Option<PathBuf>) {
    *CODEX_BINDING_STORE_PATH_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = path;
    let store = CODEX_BINDING_STORE.get_or_init(|| Mutex::new(CodexBindingStoreState::default()));
    let mut guard = store.lock().unwrap_or_else(|error| error.into_inner());
    *guard = CodexBindingStoreState::default();
}

fn persist_binding(project_path: &str, pid: u32, pane_id: Option<&str>, result: &IdleResult) {
    let (Some(session_id), Some(jsonl_path)) =
        (result.session_id.clone(), result.jsonl_path.clone())
    else {
        return;
    };

    let record = CodexBindingRecord {
        project_path: normalize_project_path(project_path),
        pid,
        pane_id: pane_id.map(str::to_string),
        session_id,
        jsonl_path,
        resolved_at: Utc::now(),
    };
    let key = binding_key(project_path, pid, pane_id);

    with_binding_store(|bindings, path| {
        let changed = bindings.get(&key) != Some(&record);
        if changed {
            bindings.insert(key, record);
            save_binding_store_to_disk(path, bindings);
        }
    });
}

fn invalidate_binding(project_path: &str, pid: u32, pane_id: Option<&str>) {
    let key = binding_key(project_path, pid, pane_id);
    with_binding_store(|bindings, path| {
        if bindings.remove(&key).is_some() {
            save_binding_store_to_disk(path, bindings);
        }
    });
}

fn binding_result<F>(
    project_path: &str,
    pid: u32,
    pane_id: Option<&str>,
    file_open_by_pid: &F,
) -> Option<IdleResult>
where
    F: Fn(&Path) -> bool,
{
    let key = binding_key(project_path, pid, pane_id);
    let record = with_binding_store(|bindings, _| bindings.get(&key).cloned())?;
    let path = PathBuf::from(&record.jsonl_path);
    if !path.exists() {
        invalidate_binding(project_path, pid, pane_id);
        return None;
    }
    if !file_open_by_pid(&path) {
        invalidate_binding(project_path, pid, pane_id);
        return None;
    }

    let result = codex_result_from_file(&path);
    if result.session_id.as_deref() != Some(record.session_id.as_str()) {
        invalidate_binding(project_path, pid, pane_id);
        return None;
    }
    Some(result)
}

pub(super) fn reconcile_persisted_bindings(
    processes: &[ProcessInfo],
    pane_map: &HashMap<String, TmuxPane>,
) {
    let active_keys = processes
        .iter()
        .filter(|process| process.cli_tool == crate::session_scanner::cli_tool::CliTool::Codex)
        .map(|process| {
            let pane_id = pane_map.get(&process.tty).map(|pane| pane.pane_id.as_str());
            binding_key(&process.project_path, process.pid, pane_id)
        })
        .collect::<std::collections::HashSet<_>>();

    with_binding_store(|bindings, path| {
        let before = bindings.len();
        bindings.retain(|key, _| active_keys.contains(key));
        if bindings.len() != before {
            save_binding_store_to_disk(path, bindings);
        }
    });
}

/// Core Codex idle detection — testable with custom base dir.
pub(super) fn codex_detect_idle(project_path: &str, sessions_dir: &Path) -> IdleResult {
    match codex_find_session_for_project(project_path, sessions_dir) {
        Some(path) => codex_result_from_file(&path),
        None => IdleResult::idle(),
    }
}

pub(super) fn codex_detect_idle_for_pid(
    project_path: &str,
    pid: u32,
    pane_id: Option<&str>,
    sessions_dir: &Path,
) -> IdleResult {
    codex_detect_idle_for_pid_with(project_path, pid, pane_id, sessions_dir, &|path| {
        path.to_str()
            .is_some_and(|path_str| crate::platform::process_has_open_path(pid, path_str))
    })
}

fn codex_detect_idle_for_pid_with<F>(
    project_path: &str,
    pid: u32,
    pane_id: Option<&str>,
    sessions_dir: &Path,
    file_open_by_pid: &F,
) -> IdleResult
where
    F: Fn(&Path) -> bool,
{
    if let Some(result) = binding_result(project_path, pid, pane_id, file_open_by_pid) {
        return result;
    }

    let candidates = codex_find_sessions_for_project(project_path, sessions_dir);
    // `fd_proven` is the difference between "this PID has the transcript open"
    // and "this project has exactly one transcript". Only the former is durable
    // enough to persist: a second pane in the same project would otherwise
    // inherit the guess from the store.
    let (resolved, fd_proven) = match candidates.as_slice() {
        [] => (None, false),
        [only] => (Some(codex_result_from_file(only)), file_open_by_pid(only)),
        _ => match candidates.iter().find(|path| file_open_by_pid(path)) {
            Some(path) => (Some(codex_result_from_file(path)), true),
            None => (None, false),
        },
    };

    match resolved {
        Some(result) => {
            if fd_proven {
                persist_binding(project_path, pid, pane_id, &result);
            } else {
                invalidate_binding(project_path, pid, pane_id);
            }
            result
        }
        None => {
            invalidate_binding(project_path, pid, pane_id);
            IdleResult::idle()
        }
    }
}

/// Build an IdleResult from a Codex session file.
fn codex_result_from_file(path: &Path) -> IdleResult {
    // Extract session ID from filename: "rollout-2026-02-21T17-25-42-UUID.jsonl"
    // The UUID is the last segment of the stem after the timestamp portion.
    let session_id = path.file_stem().and_then(|s| s.to_str()).map(|stem| {
        // Find the UUID portion: everything after "rollout-YYYY-MM-DDTHH-MM-SS-"
        // "rollout-" (8) + "YYYY-MM-DDTHH-MM-SS" (19) + "-" (1) = 28
        if stem.len() > 28 && stem.starts_with("rollout-") {
            stem[28..].to_string()
        } else {
            stem.to_string()
        }
    });

    let file_path = path.to_string_lossy().to_string();

    let output_mtime = file_mtime(path);
    let state = output_mtime
        .map(|t| classify_mtime(t, CODEX_ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(file_path),
        last_output_age_secs: output_mtime.map(age_secs_since_mtime),
        authoritative: false,
    }
}

/// How many days back to scan for Codex session files.
///
/// Codex stores session files in date-organized directories (YYYY/MM/DD/).
/// When a session is resumed, Codex appends to the *original* file — which
/// stays in the date directory where it was first created. A session created
/// on Monday and resumed on Thursday would still live in Monday's directory
/// but with Thursday's mtime. We scan back far enough to catch these.
const CODEX_LOOKBACK_DAYS: i64 = 7;

/// Scan recent date directories to find the Codex session file for a project.
///
/// Checks the last [`CODEX_LOOKBACK_DAYS`] date directories. For each JSONL
/// file, reads the first line to extract `session_meta.payload.cwd` and
/// matches against the target project path. Files are checked newest-first
/// within each directory, and directories are checked newest-first, so the
/// active session is found quickly.
fn codex_find_session_for_project(project_path: &str, sessions_dir: &Path) -> Option<PathBuf> {
    codex_find_sessions_for_project(project_path, sessions_dir)
        .into_iter()
        .next()
}

fn codex_find_sessions_for_project(project_path: &str, sessions_dir: &Path) -> Vec<PathBuf> {
    use chrono::Local;

    let today = Local::now().date_naive();
    let mut matches = Vec::new();

    // Scan backwards from today — most likely hit is today, then yesterday, etc.
    for days_back in 0..CODEX_LOOKBACK_DAYS {
        let date = today - chrono::Duration::days(days_back);
        let date_dir = sessions_dir
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(date.format("%d").to_string());

        if !date_dir.is_dir() {
            continue;
        }

        // Scan JSONL files in reverse mtime order (newest first)
        let mut entries: Vec<_> = fs::read_dir(&date_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .collect();

        // Sort by mtime descending — check newest first for faster matching
        entries.sort_by(|a, b| {
            let mt_a = a
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let mt_b = b
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            mt_b.cmp(&mt_a)
        });

        for entry in entries {
            if codex_session_matches_project(&entry.path(), project_path) {
                matches.push(entry.path());
            }
        }
    }

    matches
}

/// Check if a Codex JSONL file's session_meta.payload.cwd matches a project path.
///
/// Reads only the first line of the file — the session_meta record.
fn codex_session_matches_project(jsonl_path: &Path, project_path: &str) -> bool {
    use std::io::{BufRead, BufReader};

    let file = match fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() || first_line.is_empty() {
        return false;
    }

    // Parse the first line and extract cwd from session_meta
    let parsed: serde_json::Value = match serde_json::from_str(&first_line) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Expected structure: {"type": "session_meta", "payload": {"cwd": "/path/..."}}
    if parsed.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return false;
    }

    let cwd = match parsed
        .get("payload")
        .and_then(|p| p.get("cwd"))
        .and_then(|c| c.as_str())
    {
        Some(cwd) => cwd,
        None => return false,
    };

    normalize_project_path(cwd) == normalize_project_path(project_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    fn setup_binding_store(tmp: &TempDir) {
        set_binding_store_path_for_test(Some(tmp.path().join("codex-bindings.json")));
    }

    /// Create a Codex JSONL session file with a session_meta record.
    fn create_codex_session(dir: &Path, filename: &str, cwd: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(filename);
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2026-02-21T16:00:00Z","type":"session_meta","payload":{{"cwd":"{cwd}","id":"test-id"}}}}"#,
        )
        .unwrap();
        writeln!(f, r#"{{"type":"response_item","payload":{{}}}}"#).unwrap();
        f.sync_all().unwrap();
        path
    }

    // Regression: c9669ef introduced authoritative Claude activity but left
    // Codex turn completion on the time-normalized fd/rchar fallback.
    #[test]
    fn fresh_turn_complete_is_authoritative_idle_for_the_bound_thread() {
        let tmp = TempDir::new().expect("tempdir");
        let transcript = tmp
            .path()
            .join("rollout-2026-08-26T12-00-00-01a03e54-7a7a-7fb3-85f5-24dfa739a2e1.jsonl");
        std::fs::write(&transcript, b"session\n").expect("transcript");
        let transcript_mtime = std::fs::metadata(&transcript)
            .and_then(|metadata| metadata.modified())
            .expect("transcript mtime");
        let notify_path = tmp.path().join("codex-notify.jsonl");
        let notify_ts =
            chrono::DateTime::<Utc>::from(transcript_mtime) + chrono::Duration::milliseconds(1);
        crate::daemon::codex_notify::append_event_at(
            &notify_path,
            include_str!("fixtures/codex-agent-turn-complete-0.149.0.json"),
            notify_ts,
        )
        .expect("notify fixture");
        let heuristic = IdleResult {
            state: SessionState::Active,
            session_id: Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1".to_string()),
            jsonl_path: Some(transcript.to_string_lossy().into_owned()),
            last_output_age_secs: Some(0),
            authoritative: false,
        };

        let result = apply_notify_edge(heuristic, &notify_path);

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
    }

    // Regression: 61e9a24 made fd introspection a notify precondition even
    // though macOS cannot probe open paths; the matching rollout UUID already
    // proves the native edge belongs to the resolved transcript.
    #[test]
    fn fresh_turn_complete_is_authoritative_when_fd_introspection_is_unavailable() {
        let tmp = TempDir::new().expect("tempdir");
        let transcript = tmp
            .path()
            .join("rollout-2026-08-26T12-00-00-01a03e54-7a7a-7fb3-85f5-24dfa739a2e1.jsonl");
        std::fs::write(&transcript, b"session\n").expect("transcript");
        let transcript_mtime = std::fs::metadata(&transcript)
            .and_then(|metadata| metadata.modified())
            .expect("transcript mtime");
        let notify_path = tmp.path().join("codex-notify.jsonl");
        crate::daemon::codex_notify::append_event_at(
            &notify_path,
            include_str!("fixtures/codex-agent-turn-complete-0.149.0.json"),
            chrono::DateTime::<Utc>::from(transcript_mtime) + chrono::Duration::milliseconds(1),
        )
        .expect("notify fixture");
        let heuristic = IdleResult {
            state: SessionState::Active,
            session_id: Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1".to_string()),
            jsonl_path: Some(transcript.to_string_lossy().into_owned()),
            last_output_age_secs: Some(0),
            authoritative: false,
        };

        let result = apply_notify_edge(heuristic, &notify_path);

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
    }

    // Regression: 61e9a24 only tested the edge helper, leaving resolver path
    // wiring and the platform fd-probe boundary uncovered.
    #[test]
    fn resolver_consumes_notify_edge_through_configured_paths() {
        let _guard = CODEX_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let tmp = TempDir::new().expect("tempdir");
        setup_binding_store(&tmp);
        let sessions_dir = tmp.path().join("sessions");
        let today = chrono::Local::now().date_naive();
        let date_dir = sessions_dir
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());
        let transcript = create_codex_session(
            &date_dir,
            "rollout-2026-08-26T12-00-00-01a03e54-7a7a-7fb3-85f5-24dfa739a2e1.jsonl",
            "/home/test/project",
        );
        let transcript_mtime = std::fs::metadata(&transcript)
            .and_then(|metadata| metadata.modified())
            .expect("transcript mtime");
        let notify_path = tmp.path().join("codex-notify.jsonl");
        crate::daemon::codex_notify::append_event_at(
            &notify_path,
            include_str!("fixtures/codex-agent-turn-complete-0.149.0.json"),
            chrono::DateTime::<Utc>::from(transcript_mtime) + chrono::Duration::milliseconds(1),
        )
        .expect("notify fixture");
        let resolver = CodexResolver {
            base_dir: Some(sessions_dir),
            notify_path,
        };

        let result = resolver.detect_idle_for_pid("/home/test/project", u32::MAX, Some("%99"));

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
        assert_eq!(
            result.session_id.as_deref(),
            Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1")
        );
    }

    #[test]
    fn stale_or_foreign_notify_falls_back_to_codex_heuristics() {
        let tmp = TempDir::new().expect("tempdir");
        let transcript = tmp.path().join("rollout.jsonl");
        let notify_path = tmp.path().join("codex-notify.jsonl");
        crate::daemon::codex_notify::append_event_at(
            &notify_path,
            include_str!("fixtures/codex-agent-turn-complete-0.149.0.json"),
            Utc::now() - chrono::Duration::seconds(5),
        )
        .expect("notify fixture");
        std::fs::write(&transcript, b"new turn\n").expect("newer transcript");
        let heuristic = IdleResult {
            state: SessionState::Active,
            session_id: Some("01a03e54-7a7a-7fb3-85f5-24dfa739a2e1".to_string()),
            jsonl_path: Some(transcript.to_string_lossy().into_owned()),
            last_output_age_secs: Some(0),
            authoritative: false,
        };

        assert_eq!(
            apply_notify_edge(heuristic.clone(), &notify_path),
            heuristic
        );

        let foreign = IdleResult {
            session_id: Some("different-thread".to_string()),
            ..heuristic.clone()
        };
        assert_eq!(apply_notify_edge(foreign.clone(), &notify_path), foreign);
    }

    #[test]
    fn codex_detect_idle_matches_project_by_cwd() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-test-uuid-1234.jsonl",
            "/home/user/projects/myapp",
        );

        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert!(result.session_id.is_some());
        assert!(result.jsonl_path.is_some());
    }

    #[test]
    fn codex_detect_idle_no_match_returns_idle() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        // Session for a different project
        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-other-uuid.jsonl",
            "/home/user/projects/other",
        );

        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn codex_detect_idle_old_session_file() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let project = "/home/user/projects/old-session-test";
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let path = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T10-00-00-old-session-id.jsonl",
            project,
        );
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&path, old_time);

        let result = codex_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_some());
    }

    #[test]
    fn codex_detect_idle_empty_sessions_dir() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn codex_detect_idle_malformed_jsonl_skipped() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());
        fs::create_dir_all(&date_dir).unwrap();

        // Create malformed file
        let path = date_dir.join("rollout-2026-02-21T10-00-00-bad.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "not valid json").unwrap();
        f.sync_all().unwrap();

        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn codex_normalizes_trailing_slash_in_cwd() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        // Session has trailing slash in cwd
        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-trailing-uuid.jsonl",
            "/home/user/projects/myapp/",
        );

        // Query without trailing slash — should still match
        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Active);
    }

    #[test]
    fn codex_normalizes_windows_style_project_paths() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-windows-uuid.jsonl",
            "/mnt/d/projects/taurhaus",
        );

        let result = codex_detect_idle("D:\\projects\\taurhaus\\", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("windows-uuid"));
    }

    #[test]
    fn codex_finds_session_from_days_ago() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let project = "/home/user/projects/days-ago-test";
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        // Place the session file 5 days ago (within 7-day lookback window)
        let five_days_ago = chrono::Local::now().date_naive() - chrono::Duration::days(5);
        let date_dir = tmp
            .path()
            .join(five_days_ago.format("%Y").to_string())
            .join(five_days_ago.format("%m").to_string())
            .join(five_days_ago.format("%d").to_string());

        let path = create_codex_session(
            &date_dir,
            "rollout-2026-02-16T10-00-00-resumed-uuid.jsonl",
            project,
        );

        // Even though the file is old by date directory, if mtime is recent
        // (because codex resume appended to it), it should be found AND active
        let result = codex_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert!(result.session_id.is_some());

        // Now make the mtime old — should still find it but report Idle
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&path, old_time);

        let result2 = codex_detect_idle(project, tmp.path());
        assert_eq!(result2.state, SessionState::Idle);
        assert!(result2.session_id.is_some());
    }

    #[test]
    fn codex_ignores_session_beyond_lookback_window() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        // Place session 10 days ago — beyond the 7-day window
        let ten_days_ago = chrono::Local::now().date_naive() - chrono::Duration::days(10);
        let date_dir = tmp
            .path()
            .join(ten_days_ago.format("%Y").to_string())
            .join(ten_days_ago.format("%m").to_string())
            .join(ten_days_ago.format("%d").to_string());

        create_codex_session(
            &date_dir,
            "rollout-2026-02-11T10-00-00-ancient-uuid.jsonl",
            "/home/user/projects/myapp",
        );

        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none()); // Not found — beyond lookback
    }

    #[test]
    fn codex_detect_idle_for_pid_prefers_open_jsonl_when_project_has_multiple_candidates() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let first = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-first-uuid.jsonl",
            project,
        );
        let second = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-second-uuid.jsonl",
            project,
        );

        let result = codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
            path == second
        });
        assert_eq!(result.session_id.as_deref(), Some("second-uuid"));
        assert_eq!(
            result.jsonl_path.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );

        let fallback =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
                path == first
            });
        assert_eq!(fallback.session_id.as_deref(), Some("first-uuid"));
        assert_eq!(
            fallback.jsonl_path.as_deref(),
            Some(first.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn codex_detect_idle_for_pid_refuses_project_level_guess_when_candidates_conflict() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-first-uuid.jsonl",
            project,
        );
        create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-second-uuid.jsonl",
            project,
        );

        let result =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|_| false);
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn codex_detect_idle_for_pid_reuses_persisted_binding_for_same_attachment() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let jsonl_path = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-second-uuid.jsonl",
            project,
        );

        let resolved =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
                path == jsonl_path
            });
        assert_eq!(resolved.session_id.as_deref(), Some("second-uuid"));

        let reused = codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
            path == jsonl_path
        });
        assert_eq!(reused.session_id.as_deref(), Some("second-uuid"));
        assert_eq!(
            reused.jsonl_path.as_deref(),
            Some(jsonl_path.to_string_lossy().as_ref())
        );
    }

    // Regression: a11c347 persisted a binding for any resolved transcript. With
    // a single candidate the resolution is a project-level guess, not proof
    // that this PID owns the file, so a second Codex pane in the same project
    // inherited the first pane's transcript from the store.
    #[test]
    fn codex_single_candidate_binding_is_not_persisted_without_fd_proof() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let jsonl_path = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-only-uuid.jsonl",
            project,
        );

        // The single candidate still answers this poll...
        let guessed =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|_| false);
        assert_eq!(guessed.session_id.as_deref(), Some("only-uuid"));

        // ...but nothing is written to the binding store without fd proof.
        let key = binding_key(project, 42, Some("%1"));
        with_binding_store(|bindings, _| {
            assert!(!bindings.contains_key(&key));
        });

        // fd proof persists it.
        let proven = codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
            path == jsonl_path
        });
        assert_eq!(proven.session_id.as_deref(), Some("only-uuid"));
        with_binding_store(|bindings, _| {
            assert!(bindings.contains_key(&key));
        });
    }

    #[test]
    fn codex_binding_is_invalidated_when_pid_changes() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let jsonl_path = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-second-uuid.jsonl",
            project,
        );
        let initial =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
                path == jsonl_path
            });
        assert_eq!(initial.session_id.as_deref(), Some("second-uuid"));

        let processes = vec![ProcessInfo {
            pid: 77,
            project_path: project.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex".to_string(),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
        }];
        let pane_map = HashMap::from([(
            "/dev/pts/1".to_string(),
            TmuxPane {
                pane_id: "%1".to_string(),
                tty: "/dev/pts/1".to_string(),
                window_index: "1".to_string(),
                window_name: "work".to_string(),
                session_name: "taurhaus".to_string(),
            },
        )]);
        reconcile_persisted_bindings(&processes, &pane_map);
        let stale_key = binding_key(project, 42, Some("%1"));
        let replacement_key = binding_key(project, 77, Some("%1"));
        with_binding_store(|bindings, _| {
            assert!(!bindings.contains_key(&stale_key));
            assert!(!bindings.contains_key(&replacement_key));
        });

        let rebound =
            codex_detect_idle_for_pid_with(project, 77, Some("%1"), tmp.path(), &|path| {
                path == jsonl_path
            });
        assert_eq!(rebound.session_id.as_deref(), Some("second-uuid"));
        with_binding_store(|bindings, _| {
            assert!(bindings.contains_key(&replacement_key));
        });
    }

    #[test]
    fn codex_binding_is_invalidated_when_same_pid_attaches_new_transcript() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = "/home/user/projects/myapp";
        let today = chrono::Local::now().date_naive();
        let date_dir = tmp
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let first = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-00-00-first-uuid.jsonl",
            project,
        );
        let second = create_codex_session(
            &date_dir,
            "rollout-2026-02-21T16-05-00-second-uuid.jsonl",
            project,
        );

        let initial =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
                path == first
            });
        assert_eq!(initial.session_id.as_deref(), Some("first-uuid"));

        let rebound =
            codex_detect_idle_for_pid_with(project, 42, Some("%1"), tmp.path(), &|path| {
                path == second
            });
        assert_eq!(rebound.session_id.as_deref(), Some("second-uuid"));
        assert_eq!(
            rebound.jsonl_path.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );
    }
}
