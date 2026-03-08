use super::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

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
}

impl CodexResolver {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".codex").join("sessions"));
        Self { base_dir }
    }

    pub fn detect_idle_for_pid(&self, project_path: &str, pid: u32) -> IdleResult {
        let Some(base) = self.base_dir.as_ref() else {
            return IdleResult::idle();
        };
        codex_detect_idle_for_pid(project_path, pid, base)
    }
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

/// Cached mapping from project path to the JSONL file that serves it.
struct CodexCacheEntry {
    /// The JSONL file serving this project.
    file_path: PathBuf,
    /// When we last scanned (to expire stale entries).
    scanned_at: SystemTime,
}

/// How long a Codex cache entry is valid before we rescan.
const CODEX_CACHE_TTL: Duration = Duration::from_secs(30);

static CODEX_PATH_CACHE: Mutex<Option<HashMap<String, CodexCacheEntry>>> = Mutex::new(None);

/// Core Codex idle detection — testable with custom base dir.
pub(super) fn codex_detect_idle(project_path: &str, sessions_dir: &Path) -> IdleResult {
    // Check cache first
    {
        let guard = CODEX_PATH_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_ref() {
            if let Some(entry) = map.get(project_path) {
                let age = SystemTime::now()
                    .duration_since(entry.scanned_at)
                    .unwrap_or(Duration::ZERO);
                if age < CODEX_CACHE_TTL && entry.file_path.exists() {
                    return codex_result_from_file(&entry.file_path);
                }
            }
        }
    }

    // Cache miss — scan recent date directories
    let matching_file = codex_find_session_for_project(project_path, sessions_dir);

    // Update cache (evict expired entries on each insert)
    if let Some(ref path) = matching_file {
        let mut guard = CODEX_PATH_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        let now = SystemTime::now();
        map.retain(|_, entry| {
            now.duration_since(entry.scanned_at)
                .unwrap_or(Duration::ZERO)
                < CODEX_CACHE_TTL * 2
        });
        map.insert(
            project_path.to_string(),
            CodexCacheEntry {
                file_path: path.clone(),
                scanned_at: now,
            },
        );
    }

    match matching_file {
        Some(path) => codex_result_from_file(&path),
        None => IdleResult::idle(),
    }
}

pub(super) fn codex_detect_idle_for_pid(
    project_path: &str,
    pid: u32,
    sessions_dir: &Path,
) -> IdleResult {
    codex_detect_idle_for_pid_with(project_path, sessions_dir, &|path| {
        path.to_str()
            .is_some_and(|path_str| crate::platform::process_has_open_path(pid, path_str))
    })
}

fn codex_detect_idle_for_pid_with<F>(
    project_path: &str,
    sessions_dir: &Path,
    file_open_by_pid: &F,
) -> IdleResult
where
    F: Fn(&Path) -> bool,
{
    let candidates = codex_find_sessions_for_project(project_path, sessions_dir);
    match candidates.as_slice() {
        [] => IdleResult::idle(),
        [only] => codex_result_from_file(only),
        _ => candidates
            .into_iter()
            .find(|path| file_open_by_pid(path))
            .map(|path| codex_result_from_file(&path))
            .unwrap_or_else(IdleResult::idle),
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

    // Normalize trailing slashes for comparison
    let norm_cwd = cwd.trim_end_matches('/');
    let norm_target = project_path.trim_end_matches('/');
    norm_cwd == norm_target
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

    fn clear_codex_cache() {
        let mut guard = CODEX_PATH_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_mut() {
            map.clear();
        }
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

    #[test]
    fn codex_detect_idle_matches_project_by_cwd() {
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let project = "/home/user/projects/old-session-test";
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn codex_detect_idle_malformed_jsonl_skipped() {
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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
    fn codex_finds_session_from_days_ago() {
        clear_codex_cache();
        let project = "/home/user/projects/days-ago-test";
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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

        let result = codex_detect_idle_for_pid_with(project, tmp.path(), &|path| path == second);
        assert_eq!(result.session_id.as_deref(), Some("second-uuid"));
        assert_eq!(
            result.jsonl_path.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );

        let fallback = codex_detect_idle_for_pid_with(project, tmp.path(), &|path| path == first);
        assert_eq!(fallback.session_id.as_deref(), Some("first-uuid"));
        assert_eq!(
            fallback.jsonl_path.as_deref(),
            Some(first.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn codex_detect_idle_for_pid_refuses_project_level_guess_when_candidates_conflict() {
        clear_codex_cache();
        let tmp = TempDir::new().unwrap();
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

        let result = codex_detect_idle_for_pid_with(project, tmp.path(), &|_| false);
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }
}
