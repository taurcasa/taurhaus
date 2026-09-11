use super::*;
use crate::daemon::session_activity::{hosted_activity::remote_session, SessionActivityHub};
use crate::provider::path::normalize_project_path;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::tmux::TmuxPane;
use crate::session_scanner::{process::ProcessInfo, RuntimeSession};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use super::claude_registry::{ActivitySource, AuthoritativeState, SessionSource};

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
    fn process_session(&self, process: &ProcessInfo, pane: Option<&str>) -> Option<RuntimeSession> {
        let words = crate::session_scanner::shell_words::words(&process.args);
        let socket = words.windows(2).find(|w| w[0].text == "--remote")?[1]
            .text
            .strip_prefix("unix://")?;
        let thread = &words.windows(2).find(|w| w[0].text == "resume")?[1].text;
        let hub = SessionActivityHub::shared();
        let Some((mut session, account)) =
            hub.host_session(Path::new(socket), thread, process, pane)
        else {
            return Some(remote_session(process, pane, thread));
        };
        if let Some(account) = account {
            let suffix = format!("-{thread}.jsonl");
            session.jsonl_path = ignore::WalkBuilder::new(account.join("sessions"))
                .standard_filters(false)
                .max_depth(Some(4))
                .build()
                .filter_map(Result::ok)
                .find(|entry| {
                    entry.file_type().is_some_and(|ty| ty.is_file())
                        && entry
                            .file_name()
                            .to_str()
                            .is_some_and(|name| name.ends_with(&suffix))
                        && codex_session_matches_project(entry.path(), &process.project_path)
                })
                .map(|entry| entry.path().to_string_lossy().into_owned());
        }
        Some(session)
    }

    fn resolve(&self, project_path: &str, pid: u32, pane_id: Option<&str>) -> IdleResult {
        static RESOLVER: OnceLock<CodexResolver> = OnceLock::new();
        RESOLVER
            .get_or_init(CodexResolver::new)
            .detect_idle_for_pid(project_path, pid, pane_id)
    }
}

pub struct CodexNotifyActivitySource;

impl ActivitySource for CodexNotifyActivitySource {
    fn observation(
        &self,
        pid: u32,
        project: &str,
        pane: Option<&str>,
    ) -> Option<ActivityObservation> {
        super::codex_readiness::observation(pid, project, pane)
    }

    fn authoritative_state(
        &self,
        _project_path: &str,
        _pid: u32,
        resolved: &IdleResult,
    ) -> Option<AuthoritativeState> {
        resolved.authoritative.then_some(AuthoritativeState {
            state: resolved.state,
            source: "notify",
        })
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
        self.detect_idle_for_pid_at(project_path, pid, pane_id, &PlatformPaths::teams_dir())
    }

    fn detect_idle_for_pid_at(
        &self,
        project_path: &str,
        pid: u32,
        pane_id: Option<&str>,
        teams_dir: &Path,
    ) -> IdleResult {
        let records = crate::coordination::stores::TeamRootRegistry::read_runtime_records(teams_dir.to_path_buf())
            .unwrap_or_else(|error| {
                static LAST_WARNING: Mutex<Option<std::time::Instant>> = Mutex::new(None);
                let mut last = LAST_WARNING.lock().unwrap_or_else(|e| e.into_inner());
                if last.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
                    tracing::warn!(%error, source = "codex_runtime_scope_unavailable", "Codex runtime scope unavailable; retaining rollout identity without team readiness");
                    *last = Some(std::time::Instant::now());
                }
                Vec::new()
            });
        self.detect_idle_for_pid_in(project_path, pid, pane_id, &records)
    }

    fn detect_idle_for_pid_in(
        &self,
        project_path: &str,
        pid: u32,
        pane_id: Option<&str>,
        records: &[crate::coordination::stores::MemberRuntimeRecord],
    ) -> IdleResult {
        super::codex_readiness::invalidate(pid);
        #[cfg(target_os = "linux")]
        if matches!(fs::metadata(format!("/proc/{pid}")), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
        {
            invalidate_binding(project_path, pid, pane_id);
            return unresolved_identity(pid, "codex_identity_process_gone");
        }
        let Some((seat_root, excluded)) = codex_identity_scope(project_path, pid, pane_id, records)
        else {
            return unresolved_identity(pid, "codex_identity_ambiguous_account");
        };
        let base = seat_root
            .or_else(|| {
                crate::platform::process_env_var(pid, "CODEX_HOME")
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| {
                crate::platform::process_env_var(pid, "HOME")
                    .map(|home| PathBuf::from(home).join(".codex"))
            })
            .map(|root| root.join("sessions"))
            .or_else(|| self.base_dir.clone());
        let Some(base) = base else {
            return IdleResult::idle();
        };
        let open_paths = codex_open_paths(pid);
        let result = codex_detect_idle_scoped_with_paths(
            project_path,
            pid,
            pane_id,
            &base,
            &excluded,
            open_paths.as_ref(),
            &|path| {
                open_paths.as_ref().map_or_else(
                    || {
                        path.to_str()
                            .is_some_and(|p| crate::platform::process_has_open_path(pid, p))
                    },
                    |paths| {
                        paths.contains(&path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
                    },
                )
            },
        );
        let mut result = apply_notify_edge(result, &self.notify_path);
        super::codex_readiness::refresh(&mut result, project_path, pid, records, &self.notify_path);
        result
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
        SystemTime::UNIX_EPOCH,
    ) else {
        return result;
    };
    if record.ts < DateTime::<Utc>::from(transcript_mtime)
        && !codex_completed_tail(Path::new(transcript_path), &record)
    {
        return result;
    }

    result.state = SessionState::Idle;
    result.authoritative = true;
    result
}

// Codex writes the notify hook BEFORE it flushes the task_complete row, so the
// row's own timestamp lands a few milliseconds after the notify record (e2e lane
// 6 run 2: notify 23:59:44.3099Z, row 23:59:44.319Z). A matching final row inside
// this window is the same completion, not a newer turn; beyond it the row is
// treated as unrelated. Only the final row can bridge the ordering; a newer row
// of any other kind means the seat moved on.
const COMPLETION_FLUSH_TOLERANCE: chrono::Duration = chrono::Duration::seconds(10);

fn codex_completed_tail(
    path: &Path,
    notify: &crate::daemon::codex_notify::CodexNotifyRecord,
) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let read = || -> Option<bool> {
        let turn = notify.turn_id.as_deref().filter(|id| !id.is_empty())?;
        let mut file = fs::File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(65_536)))
            .ok()?;
        let mut tail = Vec::new();
        file.take(65_537).read_to_end(&mut tail).ok()?;
        if tail.len() > 65_536 || !tail.ends_with(b"\n") {
            return None;
        }
        let line = tail.rsplit(|b| *b == b'\n').find(|line| !line.is_empty())?;
        let row: serde_json::Value = serde_json::from_slice(line).ok()?;
        let at = DateTime::parse_from_rfc3339(row["timestamp"].as_str()?).ok()?;
        Some(
            row["type"] == "event_msg"
                && row["payload"]["type"] == "task_complete"
                && row["payload"]["turn_id"].as_str() == Some(turn)
                && at <= notify.ts + COMPLETION_FLUSH_TOLERANCE,
        )
    };
    read().unwrap_or(false)
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

pub(super) fn is_codex(tool: crate::session_scanner::cli_tool::CliTool) -> bool {
    tool == crate::session_scanner::cli_tool::CliTool::Codex
}

pub(super) fn reconcile_persisted_bindings(
    processes: &[ProcessInfo],
    pane_map: &HashMap<String, TmuxPane>,
) {
    let active_keys = processes
        .iter()
        .filter(|process| is_codex(process.cli_tool))
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

pub(super) fn codex_process_ancestors(pid: u32) -> std::collections::HashSet<u32> {
    let mut ancestors = std::collections::HashSet::new();
    let mut parent = pid;
    for _ in 0..64 {
        if parent == 0 || !ancestors.insert(parent) {
            break;
        }
        let Some((next, _)) = crate::platform::process_parent_and_tty(parent) else {
            break;
        };
        parent = next;
    }
    ancestors
}

pub(super) fn codex_runtime_matches(
    record: &crate::coordination::stores::MemberRuntimeRecord,
    project: &str,
    ancestors: &std::collections::HashSet<u32>,
    pane: Option<&str>,
) -> bool {
    record.app_server.is_none()
        && record.cli_tool.is_some_and(is_codex)
        && record.project_path.as_deref().is_some_and(|p| {
            normalize_project_path(&p.to_string_lossy()) == normalize_project_path(project)
        })
        && pane.is_none_or(|p| record.pane_id.as_deref() == Some(p))
        && record.pane_pid.is_some_and(|p| {
            ancestors.contains(&p)
                && record.pane_start_time.is_some()
                && crate::platform::process_start_ticks(p) == record.pane_start_time
        })
}

/// Scope a TUI by the recorded launch, never by the newest project transcript.
/// PID ancestry plus start ticks also disambiguates equal pane IDs on private servers.
fn codex_identity_scope(
    project: &str,
    pid: u32,
    pane: Option<&str>,
    records: &[crate::coordination::stores::MemberRuntimeRecord],
) -> Option<(Option<PathBuf>, std::collections::HashSet<String>)> {
    let ancestors = codex_process_ancestors(pid);
    let mut account = None;
    let mut excluded = std::collections::HashSet::new();
    for record in records {
        if let Some(host) = &record.app_server {
            excluded.insert(host.thread_id.clone());
            continue;
        }
        let bound = codex_runtime_matches(record, project, &ancestors, pane);
        if bound {
            if let Some(root) = record
                .recovery
                .harness_account_root
                .clone()
                .filter(|r| !r.is_empty())
                .map(PathBuf::from)
            {
                if account.as_ref().is_some_and(|previous| previous != &root) {
                    return None;
                }
                account = Some(root);
            }
        }
    }
    Some((account, excluded))
}

fn unresolved_identity(pid: u32, source: &'static str) -> IdleResult {
    super::codex_readiness::invalidate(pid);
    static REASONS: OnceLock<Mutex<HashMap<u32, &'static str>>> = OnceLock::new();
    let reason = source.strip_prefix("codex_identity_").unwrap_or(source);
    let changed = REASONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(pid, reason)
        != Some(reason);
    if changed {
        tracing::warn!(
            event = "codex.identity.unresolved",
            pid,
            reason,
            "Codex identity uncertain"
        );
        taurhaus_lib::logging::emit_global(
            "warn",
            "session_scanner",
            "codex.identity.unresolved",
            Some("Codex identity uncertain".into()),
            serde_json::Map::from_iter([
                ("pid".into(), pid.into()),
                ("reason".into(), reason.into()),
            ]),
        );
    }
    IdleResult::idle()
}

/// One descriptor inventory per Linux PID, shared by rollout and lock checks.
fn codex_open_paths(pid: u32) -> Option<std::collections::HashSet<PathBuf>> {
    #[cfg(target_os = "linux")]
    {
        let entries = fs::read_dir(format!("/proc/{pid}/fd")).ok()?;
        Some(
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| fs::read_link(entry.path()).ok())
                .filter(|path| path.is_absolute())
                .map(|path| path.canonicalize().unwrap_or(path))
                .collect(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
fn codex_detect_idle_scoped(
    project_path: &str,
    pid: u32,
    pane_id: Option<&str>,
    sessions_dir: &Path,
    excluded: &std::collections::HashSet<String>,
    file_open_by_pid: &dyn Fn(&Path) -> bool,
) -> IdleResult {
    codex_detect_idle_scoped_with_paths(
        project_path,
        pid,
        pane_id,
        sessions_dir,
        excluded,
        None,
        file_open_by_pid,
    )
}

#[allow(clippy::too_many_arguments)]
fn codex_detect_idle_scoped_with_paths(
    project_path: &str,
    pid: u32,
    pane_id: Option<&str>,
    sessions_dir: &Path,
    excluded: &std::collections::HashSet<String>,
    open_paths: Option<&std::collections::HashSet<PathBuf>>,
    file_open_by_pid: &dyn Fn(&Path) -> bool,
) -> IdleResult {
    let sessions_dir = sessions_dir
        .canonicalize()
        .unwrap_or_else(|_| sessions_dir.to_path_buf());
    let eligible = |path: &PathBuf| {
        codex_result_from_file(path)
            .session_id
            .is_some_and(|id| !excluded.contains(&id))
    };
    let scan_candidates = || {
        codex_find_sessions_for_project(project_path, &sessions_dir)
            .into_iter()
            .filter(eligible)
            .collect::<Vec<_>>()
    };
    let mut candidates: Vec<_> = open_paths
        .map(|paths| {
            paths
                .iter()
                .filter(|path| {
                    path.starts_with(&sessions_dir)
                        && path.extension().is_some_and(|ext| ext == "jsonl")
                })
                .filter(|path| eligible(path) && codex_session_matches_project(path, project_path))
                .cloned()
                .collect()
        })
        .unwrap_or_else(scan_candidates);
    // 0.153.4 creates this per-thread file before the first rollout. An open
    // descriptor belongs to this process; a filename or mtime alone proves nothing.
    let locks: Vec<_> = sessions_dir
        .parent()
        .into_iter()
        .flat_map(|root| fs::read_dir(root.join("thread-writer-locks")).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "lock"))
        .filter_map(|entry| {
            let path = entry.path();
            let id = path.file_stem()?.to_str()?.to_owned();
            (uuid::Uuid::parse_str(&id).is_ok() && !excluded.contains(&id)).then_some((path, id))
        })
        .filter(|(path, _)| file_open_by_pid(path))
        .map(|(_, id)| id)
        .collect();
    if locks.len() > 1 {
        // An in-process ephemeral thread must not displace a still-owned seat.
        if let Some(result) = binding_result(project_path, pid, pane_id, &|path| {
            path.starts_with(&sessions_dir)
                && codex_result_from_file(path)
                    .session_id
                    .is_some_and(|id| locks.contains(&id))
        }) {
            return result;
        }
        invalidate_binding(project_path, pid, pane_id);
        return unresolved_identity(pid, "codex_identity_ambiguous_writer_locks");
    }
    let owned: Vec<_> = candidates
        .iter()
        .filter(|p| file_open_by_pid(p))
        .cloned()
        .collect();
    if locks.is_empty() && owned.len() > 1 {
        invalidate_binding(project_path, pid, pane_id);
        return unresolved_identity(pid, "codex_identity_ambiguous");
    }
    let allowed = |path: &Path| {
        path.starts_with(&sessions_dir)
            && codex_result_from_file(path).session_id.is_some_and(|id| {
                !excluded.contains(&id) && locks.first().is_none_or(|lock| *lock == id)
            })
    };
    if let Some(result) = binding_result(project_path, pid, pane_id, &|path| {
        allowed(path) && (!locks.is_empty() || file_open_by_pid(path))
    }) {
        return result;
    }
    if open_paths.is_some()
        && (candidates.is_empty()
            || locks.first().is_some_and(|id| {
                !candidates
                    .iter()
                    .any(|path| codex_result_from_file(path).session_id.as_ref() == Some(id))
            }))
    {
        candidates = scan_candidates();
    }
    if let Some(id) = locks.first() {
        let result = candidates
            .iter()
            .find(|path| codex_result_from_file(path).session_id.as_ref() == Some(id))
            .map(|path| codex_result_from_file(path))
            .unwrap_or_else(|| IdleResult {
                session_id: Some(id.clone()),
                ..IdleResult::idle()
            });
        persist_binding(project_path, pid, pane_id, &result);
        return result;
    }
    let path = match owned.as_slice() {
        [only] => Some(only.as_path()),
        [] if candidates.len() == 1 => Some(candidates[0].as_path()),
        _ => None,
    };
    match path {
        Some(path) => {
            let result = codex_result_from_file(path);
            if file_open_by_pid(path) {
                persist_binding(project_path, pid, pane_id, &result);
            } else {
                invalidate_binding(project_path, pid, pane_id);
            }
            result
        }
        None => {
            invalidate_binding(project_path, pid, pane_id);
            unresolved_identity(
                pid,
                if candidates.is_empty() {
                    "codex_identity_missing"
                } else {
                    "codex_identity_ambiguous"
                },
            )
        }
    }
}

#[cfg(test)]
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
    codex_detect_idle_scoped(
        project_path,
        pid,
        pane_id,
        sessions_dir,
        &Default::default(),
        file_open_by_pid,
    )
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
/// Older versions may append on resume; 0.153.4 can create a new rollout.
/// Retained files are discovery candidates, never stronger than writer ownership.
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

#[cfg(test)]
thread_local! { static ROLLOUT_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn codex_find_sessions_for_project(project_path: &str, sessions_dir: &Path) -> Vec<PathBuf> {
    #[cfg(test)]
    ROLLOUT_SCANS.with(|count| count.set(count.get() + 1));
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
    use std::io::{BufRead, BufReader, Read};
    const HEADER_LIMIT: usize = 64 * 1024;

    let file = match fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut reader = BufReader::new(file.take(HEADER_LIMIT as u64 + 1));
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err()
        || first_line.is_empty()
        || first_line.len() > HEADER_LIMIT
    {
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
    use super::super::codex_readiness::observation;
    use super::*;
    use crate::daemon::codex_notify::append_event_at;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn rollout_identity_rejects_an_oversized_first_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let header = r#"{"type":"session_meta","payload":{"cwd":"/project"}}"#;
        for newline in ["", "\n"] {
            fs::write(
                &path,
                format!(
                    "{header}{}{newline}",
                    " ".repeat(64 * 1024 - header.len() - newline.len())
                ),
            )
            .unwrap();
            assert!(codex_session_matches_project(&path, "/project"));
            fs::write(
                &path,
                format!(
                    "{header}{}{newline}",
                    " ".repeat(64 * 1024 + 1 - header.len() - newline.len())
                ),
            )
            .unwrap();
            assert!(!codex_session_matches_project(&path, "/project"));
        }
    }

    #[test]
    fn unresolved_identity_warns_once_per_pid_reason_change_without_paths() {
        let _guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.jsonl");
        let sink = taurhaus_lib::logging::LogFileState::new(path.clone()).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        for (pid, reason) in [
            (4294900001, "ambiguous_account"),
            (4294900001, "ambiguous_account"),
            (4294900001, "ambiguous_writer_locks"),
            (4294900002, "ambiguous_account"),
        ] {
            let row = unresolved_identity(pid, reason);
            assert!(row.session_id.is_none());
            assert_eq!(row.state, SessionState::Idle);
        }
        sink.flush_for_test().unwrap();
        let events: Vec<serde_json::Value> = fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|e: &serde_json::Value| e["event"] == "codex.identity.unresolved")
            .collect();
        assert_eq!(events.len(), 3);
        for (event, (pid, reason)) in events.iter().zip([
            (4294900001_u32, "ambiguous_account"),
            (4294900001, "ambiguous_writer_locks"),
            (4294900002, "ambiguous_account"),
        ]) {
            assert_eq!(event["level"], "WARN");
            assert_eq!(event["pid"], pid);
            assert_eq!(event["reason"], reason);
            assert!(!event.to_string().contains('/'));
        }
    }

    #[cfg(target_os = "linux")]
    struct ResumeProcess(std::process::Child);

    #[cfg(target_os = "linux")]
    impl ResumeProcess {
        fn start(root: &Path, lock: &Path, coordination: &Path) -> Self {
            let writer = File::open(lock).unwrap();
            fs2::FileExt::try_lock_exclusive(&writer).unwrap();
            let child = std::process::Command::new("/bin/sleep")
                .arg("60")
                .env_clear()
                .envs([("CODEX_HOME", root), ("HOME", root)])
                .stdin(writer)
                .stdout(File::open(coordination).unwrap())
                .spawn()
                .unwrap();
            Self(child)
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for ResumeProcess {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    // Regression: 61e9a247's mtime floor, L2 run 4f: resumed notify lost
    // authority despite the unique writer lock and rebound runtime identity.
    #[test]
    #[cfg(target_os = "linux")]
    fn codex_resume_run4f_writer_identity_and_notify() {
        use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore};
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let cwd = tmp.path().to_str().unwrap();
        let sessions = tmp.path().join("sessions");
        let date = sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string());
        fs::create_dir_all(&date).unwrap();
        let locks = tmp.path().join("thread-writer-locks");
        fs::create_dir(&locks).unwrap();
        let coordination = locks.join(".coordination.lock");
        fs::write(&coordination, "").unwrap();
        fs::write(locks.join("not-a-session.lock"), "").unwrap();
        let ids = [
            "01a08c7e-f709-7152-92ae-8fdb97f3f3d8",
            "01a08c80-6beb-7433-8d1c-4b4c15e7f335",
        ];
        let mut paths = Vec::new();
        for (id, time) in ids.iter().zip(["20-05-26", "20-07-01"]) {
            let path = date.join(format!("rollout-2026-09-10T{time}-{id}.jsonl"));
            // Run-4f session_meta shape; instruction prose is irrelevant to identity.
            let meta = serde_json::json!({"timestamp":"2026-09-10T18:07:01.638Z",
                "ordinal":0,"type":"session_meta","payload":{"session_id":id,"id":id,
                "timestamp":"2026-09-10T18:07:01.638Z","cwd":cwd,"originator":"codex-tui",
                "cli_version":"0.153.4","source":"cli","thread_source":"user",
                "model_provider":"openai","history_mode":"paginated",
                "base_instructions":{"text":"fixture"},"context_window":{"window_id":"01a08c80-6beb-7433-8d1c-4b500b45a039"},
                "git":{"commit_hash":"29686d797ccd89b253b8a6ec225dfa4d353496eb","branch":"master"}}});
            fs::write(&path, format!("{meta}\n")).unwrap();
            paths.push(path);
        }
        let old_lock = locks.join(format!("{}.lock", ids[0]));
        fs::write(&old_lock, "").unwrap();
        let old = ResumeProcess::start(tmp.path(), &old_lock, &coordination);
        let resolver = CodexResolver {
            base_dir: Some(sessions),
            notify_path: tmp.path().join("notify.jsonl"),
        };
        let before = resolver.detect_idle_for_pid_in(cwd, old.0.id(), Some("%5"), &[]);
        assert_eq!(before.session_id.as_deref(), Some(ids[0]));
        let old_pid = old.0.id();
        drop(old);
        fs::remove_file(old_lock).unwrap();
        // Regression: c9669ef8, L2 run 4f: no fallback to a dead process's retained rollout.
        let new_meta = fs::read(&paths[1]).unwrap();
        fs::remove_file(&paths[1]).unwrap();
        let dead = resolver.detect_idle_for_pid_in(cwd, old_pid, Some("%5"), &[]);
        assert!(dead.session_id.is_none() && dead.jsonl_path.is_none());
        fs::write(&paths[1], new_meta).unwrap();
        let new_lock = locks.join(format!("{}.lock", ids[1]));
        fs::write(&new_lock, "").unwrap();
        let resumed = ResumeProcess::start(tmp.path(), &new_lock, &coordination);
        let pid = resumed.0.id();
        assert_ne!(pid, old_pid);
        let mut record: MemberRuntimeRecord = serde_json::from_value(serde_json::json!({
            "paneId":"%5","panePid":pid,"paneStartTime":crate::platform::process_start_ticks(pid).unwrap().to_string(),
            "cli_tool":"codex","project_path":cwd,"session_id":ids[0],"jsonl_path":paths[0],
            "attached_at":Utc::now() - chrono::Duration::seconds(30),
            "recovery":{"harness_account_root":tmp.path()}
        })).unwrap();
        for recorded_id in ids {
            record.session_id = Some(recorded_id.into());
            let result = resolver.detect_idle_for_pid_in(cwd, pid, Some("%5"), &[record.clone()]);
            assert_eq!(result.session_id.as_deref(), Some(ids[1]));
            assert_eq!(result.jsonl_path.as_deref(), paths[1].to_str());
        }
        // Same pane/new PID re-resolves; even a stale entry for that PID loses to its lock.
        persist_binding(cwd, pid, Some("%5"), &before);
        let rebound = resolver.detect_idle_for_pid_in(cwd, pid, Some("%5"), &[record.clone()]);
        record.session_id = rebound.session_id.clone();
        record.jsonl_path = rebound.jsonl_path.as_ref().map(PathBuf::from);
        let teams = tmp.path().join("teams");
        MemberRuntimeStore::save(&teams, "trial", "alpha", &record).unwrap();
        let saved = fs::read(teams.join("trial/runtime/alpha.json")).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        assert_eq!(saved["hookSessionId"], ids[1]);
        assert_eq!(saved["session_id"], ids[1]);
        assert_eq!(saved["jsonl_path"].as_str(), rebound.jsonl_path.as_deref());
        // Exact final run-4f row and notify, with its timestamp rebased for freshness.
        let completed = Utc::now() - chrono::Duration::seconds(1);
        let turn = "01a08c80-769a-7683-ad44-8243b2ad9945";
        let row = serde_json::json!({"timestamp":completed.to_rfc3339(),"ordinal":49,"type":"event_msg",
            "payload":{"type":"task_complete","turn_id":turn,"last_agent_message":"Q2-cc0dd185",
            "started_at":1789063624,"completed_at":1789063642,"duration_ms":18075,"time_to_first_token_ms":2760}});
        let mut rollout = fs::OpenOptions::new().append(true).open(&paths[1]).unwrap();
        writeln!(rollout, "{row}").unwrap();
        let event =
            serde_json::json!({"type":"agent-turn-complete","thread-id":ids[1],"turn-id":turn});
        append_event_at(&resolver.notify_path, &event.to_string(), completed).unwrap();
        record.tmux_socket = Some(tmp.path().join("never-probed.sock"));
        let result = resolver.detect_idle_for_pid_in(cwd, pid, Some("%5"), &[record.clone()]);
        assert_eq!(result.session_id, rebound.session_id);
        assert!(result.authoritative, "notify.ts < transcript_mtime");
        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(observation(pid, cwd, Some("%5")).unwrap().source, "notify");
        // Regression: e2e lane 6 run 2 — Codex flushed the task_complete row 9 ms
        // AFTER the notify record (notify 23:59:44.3099Z, row 23:59:44.319Z) and the
        // edge refused every later turn, so the seat decayed to source none and
        // deliveries stalled on "activity not freshly idle". The same turn's final
        // row inside the flush window is the completion itself.
        let flushed_late = serde_json::json!({"timestamp":(completed + chrono::Duration::milliseconds(9)).to_rfc3339(),
            "type":"event_msg","payload":{"type":"task_complete","turn_id":turn}});
        writeln!(rollout, "{flushed_late}").unwrap();
        let late = resolver.detect_idle_for_pid_in(cwd, pid, Some("%5"), &[record.clone()]);
        assert!(
            late.authoritative,
            "task_complete flushed after notify: {flushed_late}"
        );
        assert_eq!(late.state, SessionState::Idle);
        // Each guard must independently reject a complete, timestamped final row.
        for (kind, event, turn_id, at) in [
            ("response_item", "task_complete", turn, completed),
            ("event_msg", "task_started", turn, completed),
            ("event_msg", "turn_aborted", turn, completed),
            ("event_msg", "settings", turn, completed),
            ("event_msg", "task_complete", "other-turn", completed),
            (
                "event_msg",
                "task_complete",
                turn,
                completed + chrono::Duration::seconds(30),
            ),
        ] {
            let later = serde_json::json!({"timestamp":at.to_rfc3339(),"type":kind,
                "payload":{"type":event,"turn_id":turn_id}});
            writeln!(rollout, "{later}").unwrap();
            let busy = resolver.detect_idle_for_pid_in(cwd, pid, Some("%5"), &[record.clone()]);
            assert!(!busy.authoritative, "accepted {later}");
        }
    }

    // Regression: 6398bfa3's multi-lock refusal, L1 run 4d: an ephemeral
    // thread beside the bound seat must not erase identity or consume its notify.
    #[test]
    #[cfg(target_os = "linux")]
    fn codex_run4d_ephemeral_lock_preserves_seat_notify() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let cwd = tmp.path().to_str().unwrap();
        let pid = std::process::id();
        let ids = [
            "01a08c76-8916-7d71-96e5-2331f4ce7dcb",
            "01a08c76-9142-7c22-991c-59c32c30344a",
        ];
        let sessions = tmp.path().join("sessions");
        let date = sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string());
        let locks = tmp.path().join("thread-writer-locks");
        fs::create_dir_all(&date).unwrap();
        fs::create_dir(&locks).unwrap();
        let now = Utc::now() - chrono::Duration::seconds(1);
        let resolver = CodexResolver {
            base_dir: Some(sessions),
            notify_path: tmp.path().join("notify.jsonl"),
        };
        for id in ids {
            // Recorded seat metadata shape; the ephemeral metadata was not retained.
            let meta = serde_json::json!({"timestamp":"2026-09-10T17:56:14.970Z","ordinal":0,"type":"session_meta",
                "payload":{"id":id,"session_id":id,"cwd":cwd,"originator":"codex-tui","cli_version":"0.153.4","source":"cli","thread_source":"user"}});
            let row = serde_json::json!({"timestamp":now.to_rfc3339(),"type":"event_msg","payload":{"type":"task_complete","turn_id":id}});
            let path = date.join(format!("rollout-2026-09-10T19-56-14-{id}.jsonl"));
            fs::write(path, format!("{meta}\n{row}\n")).unwrap();
        }
        let lock = |id| {
            let file = File::create(locks.join(format!("{id}.lock"))).unwrap();
            fs2::FileExt::try_lock_exclusive(&file).unwrap();
            file
        };
        let _seat = lock(ids[0]);
        let mut record = serde_json::from_value::<crate::coordination::stores::MemberRuntimeRecord>(serde_json::json!({
            "paneId":"%4d","panePid":pid,"paneStartTime":crate::platform::process_start_ticks(pid).unwrap().to_string(),
            "cli_tool":"codex","project_path":cwd,"attached_at":now - chrono::Duration::seconds(30),
            "recovery":{"harness_account_root":tmp.path()}
        })).unwrap();
        let resolve = |record| resolver.detect_idle_for_pid_in(cwd, pid, Some("%4d"), &[record]);
        assert_eq!(resolve(record.clone()).session_id.as_deref(), Some(ids[0]));
        let ephemeral = lock(ids[1]);
        let notify = |id, at| {
            let event =
                serde_json::json!({"type":"agent-turn-complete","thread-id":id,"turn-id":id});
            append_event_at(&resolver.notify_path, &event.to_string(), at).unwrap();
        };
        notify(ids[1], now);
        let result = resolve(record.clone());
        assert_eq!(result.session_id.as_deref(), Some(ids[0]));
        assert!(!result.authoritative, "foreign notify");
        notify(ids[0], now);
        record.tmux_socket = Some(tmp.path().join("never-probed.sock"));
        for held in [Some(ephemeral), None] {
            let result = resolve(record.clone());
            assert_eq!(result.session_id.as_deref(), Some(ids[0]));
            assert!(result.authoritative);
            assert_eq!(observation(pid, cwd, Some("%4d")).unwrap().source, "notify");
            drop(held);
        }
        super::super::codex_readiness::invalidate(pid);
    }

    // Regression: 41ef6b21 made unrelated corrupt team bookkeeping erase TUI identity.
    #[test]
    fn codex_review_registry_failure_preserves_rollout_identity() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let sessions = tmp.path().join("sessions");
        let file = create_codex_session(
            &sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string()),
            "rollout-2026-09-10T08-30-00-seat-thread.jsonl",
            "/scratch/project",
        );
        let teams = tmp.path().join("teams");
        fs::create_dir_all(&teams).unwrap();
        let registry_dir = tmp.path().join(".taurhaus");
        fs::create_dir_all(&registry_dir).unwrap();
        fs::write(registry_dir.join("team-roots.json"), "{broken").unwrap();
        assert!(
            crate::coordination::stores::TeamRootRegistry::read_runtime_records(teams.clone())
                .is_err()
        );
        let resolver = CodexResolver {
            base_dir: Some(sessions),
            notify_path: tmp.path().join("notify.jsonl"),
        };
        #[cfg(target_os = "linux")]
        let process = ResumeProcess::start(tmp.path(), &file, &file);
        #[cfg(target_os = "linux")]
        let pid = process.0.id();
        #[cfg(not(target_os = "linux"))]
        let pid = u32::MAX;
        let result = resolver.detect_idle_for_pid_at("/scratch/project", pid, None, &teams);
        assert_eq!(result.session_id.as_deref(), Some("seat-thread"));
        assert!(result.jsonl_path.is_some());
    }

    // Regression: b9e4a855 probed every lock before rejecting invalid or hosted IDs.
    #[test]
    fn codex_review_irrelevant_writer_locks_never_probe_process() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let locks = tmp.path().join("thread-writer-locks");
        fs::create_dir(&locks).unwrap();
        let hosted = "01a08a70-8595-7ea0-a004-4ae0385d4e2f";
        for id in ["invalid", ".coordination", "not-a-session", hosted] {
            fs::write(locks.join(format!("{id}.lock")), "").unwrap();
        }
        let result = codex_detect_idle_scoped(
            "/scratch/project",
            u32::MAX,
            None,
            &tmp.path().join("sessions"),
            &std::collections::HashSet::from([hosted.into()]),
            &|_| panic!("irrelevant lock must not inspect process descriptors"),
        );
        assert!(result.session_id.is_none());
    }

    // Regression: a1fc452b restored ambiguity refusal by rescanning every rollout
    // even when a persisted descriptor-owned binding could have returned immediately.
    #[test]
    fn codex_review_bound_descriptor_skips_rollout_rescan() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let sessions = tmp.path().join("sessions");
        let date = sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string());
        create_codex_session(
            &date,
            "rollout-2026-09-10T08-30-00-seat-thread.jsonl",
            "/scratch/project",
        );
        let path = date.join("rollout-2026-09-10T08-30-00-seat-thread.jsonl");
        let result = codex_detect_idle_scoped_with_paths(
            "/scratch/project",
            u32::MAX,
            None,
            &sessions,
            &Default::default(),
            Some(&std::collections::HashSet::from([path.clone()])),
            &|p| p == path,
        );
        assert_eq!(result.session_id.as_deref(), Some("seat-thread"));
        ROLLOUT_SCANS.with(|count| count.set(0));
        let result = codex_detect_idle_scoped_with_paths(
            "/scratch/project",
            u32::MAX,
            None,
            &sessions,
            &Default::default(),
            Some(&std::collections::HashSet::from([path.clone()])),
            &|p| p == path,
        );
        assert_eq!(result.session_id.as_deref(), Some("seat-thread"));
        ROLLOUT_SCANS.with(|count| {
            assert_eq!(
                count.get(),
                0,
                "cached ownership must avoid date directory enumeration"
            )
        });
    }

    // Regression: f74d2aa1 narrowed candidate discovery to open descriptors,
    // omitting a closed rollout selected by a unique writer lock if another was open.
    #[test]
    fn codex_review_writer_lock_finds_closed_rollout_beside_open_other_thread() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let sessions = tmp.path().join("sessions");
        let date = sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string());
        let id = "01a08a70-8595-7ea0-a004-4ae0385d4e2f";
        let name = format!("rollout-2026-09-10T08-30-00-{id}.jsonl");
        create_codex_session(&date, &name, "/scratch/project");
        create_codex_session(
            &date,
            "rollout-2026-09-10T08-30-00-other.jsonl",
            "/scratch/project",
        );
        let lock = tmp
            .path()
            .join("thread-writer-locks")
            .join(format!("{id}.lock"));
        fs::create_dir(lock.parent().unwrap()).unwrap();
        fs::write(&lock, "").unwrap();
        let open = std::collections::HashSet::from([
            lock,
            date.join("rollout-2026-09-10T08-30-00-other.jsonl"),
        ]);
        let result = codex_detect_idle_scoped_with_paths(
            "/scratch/project",
            u32::MAX,
            None,
            &sessions,
            &Default::default(),
            Some(&open),
            &|p| open.contains(p),
        );
        assert_eq!(result.session_id.as_deref(), Some(id));
        assert_eq!(result.jsonl_path.as_deref(), date.join(name).to_str());
        ROLLOUT_SCANS.with(|count| count.set(0));
        let again = codex_detect_idle_scoped_with_paths(
            "/scratch/project",
            u32::MAX,
            None,
            &sessions,
            &Default::default(),
            Some(&open),
            &|p| open.contains(p),
        );
        assert_eq!(again.session_id, result.session_id);
        assert_eq!(again.jsonl_path, result.jsonl_path);
        ROLLOUT_SCANS.with(|count| assert_eq!(count.get(), 0));
    }

    // Regression: 5188e732 bound per-PID resolution to the daemon account, losing a seat
    // launched with its own CODEX_HOME (the trial's non-default root layout).
    #[test]
    #[cfg(target_os = "linux")]
    fn codex_identity_uses_process_account_before_daemon_default() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let home = tmp.path().join("seat-account");
        let date = chrono::Local::now().format("%Y/%m/%d").to_string();
        create_codex_session(
            &home.join("sessions").join(date),
            "rollout-2026-09-10T08-30-00-seat-thread.jsonl",
            "/scratch/project",
        );
        let mut child = std::process::Command::new("/bin/sleep")
            .env_clear()
            .env("CODEX_HOME", &home)
            .arg("60")
            .spawn()
            .unwrap();
        let resolver = CodexResolver {
            base_dir: Some(tmp.path().join("daemon-account/sessions")),
            notify_path: tmp.path().join("notify.jsonl"),
        };
        let result = resolver.detect_idle_for_pid_in("/scratch/project", child.id(), None, &[]);
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(result.session_id.as_deref(), Some("seat-thread"));
    }
    use tempfile::TempDir;

    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    // Regression: 80290f36 added hosted seats while c9669ef8's lone-rollout
    // fallback still admitted their threads as TUI candidates (seen at 06031992).
    // The trial retained cwd/id/version in thread/read, but no session_meta bytes;
    // use those observed fields without inventing a PID field in session_meta.
    #[test]
    fn codex_identity_excludes_hosted_and_refuses_ambiguous_rollouts() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let date = chrono::Local::now().format("%Y/%m/%d").to_string();
        let dir = tmp.path().join("sessions").join(date);
        let tui = create_codex_session(
            &dir,
            "rollout-2026-09-10T09-42-26-tui.jsonl",
            "/scratch/project",
        );
        let hosted = create_codex_session(
            &dir,
            "rollout-2026-09-10T09-42-27-hosted.jsonl",
            "/scratch/project",
        );
        let excluded = std::collections::HashSet::from(["hosted".to_string()]);
        let resolve = |open: &dyn Fn(&Path) -> bool| {
            codex_detect_idle_scoped(
                "/scratch/project",
                42,
                Some("%2"),
                &tmp.path().join("sessions"),
                &excluded,
                open,
            )
        };
        assert_eq!(resolve(&|_| false).session_id.as_deref(), Some("tui"));
        assert_eq!(resolve(&|p| p == hosted).session_id.as_deref(), Some("tui"));
        create_codex_session(
            &dir,
            "rollout-2026-09-10T09-42-28-peer.jsonl",
            "/scratch/project",
        );
        assert!(resolve(&|_| false).session_id.is_none());
        assert_eq!(resolve(&|p| p == tui).session_id.as_deref(), Some("tui"));
        // A formerly persisted fd binding cannot override a new hosted exclusion.
        let excluded = std::collections::HashSet::from(["hosted".into(), "tui".into()]);
        let result = codex_detect_idle_scoped(
            "/scratch/project",
            42,
            Some("%2"),
            &tmp.path().join("sessions"),
            &excluded,
            &|p| p == tui,
        );
        assert_ne!(result.session_id.as_deref(), Some("tui"));
    }

    // Regression: 41ef6b21 checked cached ownership before detecting multiple
    // open rollouts, allowing yesterday's binding to hide today's ambiguity.
    #[test]
    fn codex_identity_cache_cannot_override_ambiguous_ownership() {
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let sessions = tmp.path().join("sessions");
        let dir = sessions.join(chrono::Local::now().format("%Y/%m/%d").to_string());
        let first = create_codex_session(
            &dir,
            "rollout-2026-09-10T09-42-26-first.jsonl",
            "/scratch/project",
        );
        let resolve = |open: &dyn Fn(&Path) -> bool| {
            codex_detect_idle_scoped(
                "/scratch/project",
                42,
                Some("%2"),
                &sessions,
                &Default::default(),
                open,
            )
        };
        assert_eq!(
            resolve(&|p| p == first).session_id.as_deref(),
            Some("first")
        );
        let second = create_codex_session(
            &dir,
            "rollout-2026-09-10T09-42-27-second.jsonl",
            "/scratch/project",
        );
        assert!(resolve(&|p| p == first || p == second).session_id.is_none());
        assert_eq!(
            resolve(&|p| p == second).session_id.as_deref(),
            Some("second")
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn codex_identity_private_tmux_runtime_account_and_symlink() {
        // Regression: 5188e732 used daemon CODEX_HOME, not the pane launch root.
        use crate::coordination::stores::{MemberRuntimeStore, TeamRootRegistry};
        use std::process::Command;
        struct Server(PathBuf);
        impl Drop for Server {
            fn drop(&mut self) {
                let _ = Command::new("tmux")
                    .args(["-S", self.0.to_str().unwrap(), "kill-server"])
                    .env_remove("TMUX")
                    .output();
            }
        }
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let project = tmp.path().join("project");
        fs::create_dir(&project).unwrap();
        let home = tmp.path().join("seat-account");
        let date = chrono::Local::now().format("%Y/%m/%d").to_string();
        let file = create_codex_session(
            &home.join("sessions").join(date),
            "rollout-2026-09-10T08-30-00-seat-thread.jsonl",
            project.to_str().unwrap(),
        );
        let metadata = fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        fs::write(&file, metadata).unwrap();
        filetime_set_mtime(&file, SystemTime::now() - Duration::from_secs(120));
        let executable = tmp.path().join("codex");
        std::os::unix::fs::symlink("/bin/sleep", &executable).unwrap();
        let socket = tmp.path().join("private.sock");
        let server = Server(socket.clone());
        let output = Command::new("tmux")
            .env_clear()
            .env("HOME", tmp.path())
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("CODEX_HOME", tmp.path().join("wrong-process-account"))
            .args([
                "-S",
                socket.to_str().unwrap(),
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-P",
                "-F",
                "#{pane_pid} #{pane_id} #{pane_tty}",
                "-c",
                project.to_str().unwrap(),
            ])
            .arg(format!(
                "printf '› Ask Codex to do anything\n\n  gpt-5.6-luna low · <scratch>\n'; exec {} 60 3<{}",
                executable.display(),
                file.display()
            ))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let line = String::from_utf8(output.stdout).unwrap();
        let parts: Vec<_> = line.split_whitespace().collect();
        let pid: u32 = parts[0].parse().unwrap();
        let teams = tmp.path().join("teams");
        fs::create_dir_all(teams.join("trial")).unwrap();
        fs::write(
            teams.join("trial/config.json"),
            r#"{"name":"trial","members":[]}"#,
        )
        .unwrap();
        let record = serde_json::from_value(serde_json::json!({
            "paneId":parts[1],"panePid":pid,"paneStartTime":crate::platform::process_start_ticks(pid).unwrap().to_string(),
            "tmuxSocket":socket,"cli_tool":"codex","project_path":project,
            "attachedAt": Utc::now() - chrono::Duration::seconds(30),
            "recovery":{"harness_account_root":home,"reserved_attachment":Utc::now() - chrono::Duration::seconds(10)}
        })).unwrap();
        MemberRuntimeStore::save(&teams, "trial", "seat", &record).unwrap();
        let hosted = serde_json::from_value(serde_json::json!({"appServer":{
            "contract":1,"socketPath":tmp.path().join("host.sock"),"threadId":"hosted",
            "memberId":"beta@trial","accountRoot":home,"processId":999999,
            "processStart":"1","hostGeneration":"generation","build":"0.153.4",
            "host":"taurhaus-daemon-owned-thread/1","configuration":"strict-config/1",
            "trust":"daemon-owned/1","transport":"unix-websocket"
        }}))
        .unwrap();
        MemberRuntimeStore::save(&teams, "trial", "beta", &hosted).unwrap();
        create_codex_session(
            file.parent().unwrap(),
            "rollout-2026-09-10T09-42-27-hosted.jsonl",
            project.to_str().unwrap(),
        );
        let resolver = CodexResolver {
            base_dir: Some(tmp.path().join("daemon-account/sessions")),
            notify_path: tmp.path().join("notify.jsonl"),
        };
        let records = TeamRootRegistry::read_runtime_records(teams).unwrap();
        assert!(
            codex_identity_scope(project.to_str().unwrap(), pid, Some(parts[1]), &records)
                .unwrap()
                .1
                .contains("hosted")
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let process = loop {
            let argv = crate::platform::list_processes()
                .unwrap()
                .into_iter()
                .find(|(p, _)| *p == pid)
                .unwrap()
                .1;
            if crate::session_scanner::process::detect_cli_tool_argv(&argv)
                == Some(crate::session_scanner::CliTool::Codex)
            {
                break argv;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(process[0].ends_with("/codex"));
        let inventory = crate::session_scanner::process::scan_processes();
        let detected = inventory.processes.iter().find(|p| p.pid == pid).unwrap();
        assert_eq!(detected.cli_tool, crate::session_scanner::CliTool::Codex);
        assert_eq!(detected.project_path, project.to_str().unwrap());
        assert_eq!(
            crate::session_scanner::process::detect_cli_tool_argv(&[
                "/usr/bin/node".into(),
                executable.to_string_lossy().into_owned(),
                "--yolo".into()
            ]),
            Some(crate::session_scanner::CliTool::Codex)
        );
        assert_eq!(crate::platform::process_tty(pid).as_deref(), Some(parts[2]));
        let result = resolver.detect_idle_for_pid_in(
            project.to_str().unwrap(),
            pid,
            Some(parts[1]),
            &records,
        );
        assert_eq!(result.session_id.as_deref(), Some("seat-thread"));
        assert_eq!(result.state, SessionState::Idle);
        // Regression: 6398bfa3 (#163), L2 run 3: an attributed loaded prompt
        // must admit onboarding on this first scan, without an IO quiet window.
        let observed = super::super::codex_readiness::observation(
            pid,
            project.to_str().unwrap(),
            Some(parts[1]),
        )
        .unwrap();
        assert_eq!(observed.source, "launch_ready");
        assert_eq!(observed.state, SessionState::Idle);
        assert!(
            Utc::now()
                .signed_duration_since(observed.last_observed_at)
                .num_seconds()
                < 2
        );
        // Regression: b9e4a855 let an old completion override a newer turn's rollout.
        let event = r#"{"type":"agent-turn-complete","thread-id":"seat-thread"}"#;
        crate::daemon::codex_notify::append_event_at(
            &resolver.notify_path,
            event,
            Utc::now() - chrono::Duration::seconds(1),
        )
        .unwrap();
        fs::write(&file, r#"{"type":"response_item"}"#).unwrap();
        let result = resolver.detect_idle_for_pid_in(
            project.to_str().unwrap(),
            pid,
            Some(parts[1]),
            &records,
        );
        assert!(!result.authoritative);
        drop(server);
    }

    #[test]
    fn codex_identity_writer_lock_binds_before_first_rollout() {
        // Regression: 5188e732 required a rollout that 0.153.4's empty prompt
        // has not created; the l2 trial retained this exact lock-name shape.
        let _guard = CODEX_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        setup_binding_store(&tmp);
        let id = "01a08a70-8595-7ea0-a004-4ae0385d4e2f";
        let locks = tmp.path().join("thread-writer-locks");
        fs::create_dir(&locks).unwrap();
        let lock = locks.join(format!("{id}.lock"));
        fs::write(&lock, "").unwrap();
        let result = codex_detect_idle_scoped(
            "/scratch/project",
            42,
            Some("%2"),
            &tmp.path().join("sessions"),
            &Default::default(),
            &|p| p == lock,
        );
        assert_eq!(result.session_id.as_deref(), Some(id));
        assert!(result.jsonl_path.is_none());
        assert_eq!(result.state, SessionState::Idle);
        let foreign = codex_detect_idle_scoped(
            "/scratch/project",
            43,
            Some("%3"),
            &tmp.path().join("sessions"),
            &Default::default(),
            &|_| false,
        );
        assert!(foreign.session_id.is_none());
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

        #[cfg(target_os = "linux")]
        let process = ResumeProcess::start(tmp.path(), &transcript, &transcript);
        #[cfg(target_os = "linux")]
        let pid = process.0.id();
        #[cfg(not(target_os = "linux"))]
        let pid = u32::MAX;
        let result = resolver.detect_idle_for_pid_in("/home/test/project", pid, Some("%99"), &[]);

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
