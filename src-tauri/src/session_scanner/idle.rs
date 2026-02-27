//! Idle detector — determine session activity state from session file mtimes.
//!
//! Each CLI tool stores session data differently:
//! - **Claude Code**: `~/.claude/projects/<slug>/<session-id>.jsonl`
//! - **Codex CLI**: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
//! - **Gemini CLI**: `~/.gemini/tmp/<sha256(path)>/chats/session-*.json`
//!
//! The `SessionResolver` trait abstracts per-tool file resolution and
//! activity detection. The `detect_idle()` entry point dispatches to
//! the correct resolver via `resolver_for(tool)`.

use crate::claude_code::resolver;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::SessionState;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Threshold: if any session file mtime is less than this, session is Active.
/// Used for Claude and Gemini where proc-level signals supplement the mtime.
const ACTIVE_THRESHOLD: Duration = Duration::from_secs(5);

/// Longer threshold for Codex — session file mtime is the ONLY activity signal
/// (TCP keep-alive makes socket detection unreliable). Codex has silent gaps
/// during API thinking where no file writes happen. A longer window reduces
/// false idle flashes during those gaps.
/// Total active→idle latency: 10s threshold + ~4s hysteresis = ~14s.
const CODEX_ACTIVE_THRESHOLD: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of idle detection for a project.
#[derive(Debug, Clone, PartialEq)]
pub struct IdleResult {
    /// Detected session state.
    pub state: SessionState,
    /// Session ID (from session filename).
    pub session_id: Option<String>,
    /// Full path to the active session file.
    pub jsonl_path: Option<String>,
}

impl IdleResult {
    /// Convenience constructor for the common "no data found" case.
    fn idle() -> Self {
        Self {
            state: SessionState::Idle,
            session_id: None,
            jsonl_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionResolver trait
// ---------------------------------------------------------------------------

/// Trait for tool-specific session file resolution and activity detection.
///
/// Each CLI tool stores session data in a different layout. Implementations
/// know how to locate session files for a given project path and determine
/// whether the session is currently active (writing) or idle (waiting).
pub trait SessionResolver: Send + Sync {
    /// Detect whether a session for `project_path` is active or idle.
    ///
    /// Checks tool-specific session files and returns activity state,
    /// session ID, and path to the active session file.
    fn detect_idle(&self, project_path: &str) -> IdleResult;
}

/// Get the resolver for a CLI tool.
///
/// Returns a `&'static` reference — resolvers are created once and reused.
/// Each resolver is initialized with its base directory derived from `$HOME`.
pub fn resolver_for(tool: CliTool) -> &'static dyn SessionResolver {
    static CLAUDE: OnceLock<ClaudeResolver> = OnceLock::new();
    static CODEX: OnceLock<CodexResolver> = OnceLock::new();
    static GEMINI: OnceLock<GeminiResolver> = OnceLock::new();

    match tool {
        CliTool::Claude => CLAUDE.get_or_init(ClaudeResolver::new),
        CliTool::Codex => CODEX.get_or_init(CodexResolver::new),
        CliTool::Gemini => GEMINI.get_or_init(GeminiResolver::new),
    }
}

// ---------------------------------------------------------------------------
// Public entry point (unchanged API)
// ---------------------------------------------------------------------------

/// Detect idle state for a project using the appropriate tool-specific resolver.
pub fn detect_idle(project_path: &str, tool: CliTool) -> IdleResult {
    resolver_for(tool).detect_idle(project_path)
}

/// Testable version: detect idle state for Claude using a custom projects directory.
///
/// Kept for backward compatibility with existing tests.
pub fn detect_idle_in(project_path: &str, claude_projects_dir: &Path) -> IdleResult {
    claude_detect_idle(project_path, claude_projects_dir)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Convert a project path to Claude Code's project slug format.
///
/// Delegates to `claude_code::resolver::project_slug()` — the canonical
/// implementation that handles both `/` and `\` separators.
pub fn path_to_slug(path: &str) -> String {
    resolver::project_slug(Path::new(path))
}

/// Classify mtime into Active or Idle based on how recent it is.
fn classify_mtime(mtime: SystemTime, threshold: Duration) -> SessionState {
    let elapsed = SystemTime::now()
        .duration_since(mtime)
        .unwrap_or(Duration::from_secs(999));

    if elapsed < threshold {
        SessionState::Active
    } else {
        SessionState::Idle
    }
}

/// Get a file's modification time.
fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

/// Get the most recent mtime of any file in a directory (non-recursive).
fn newest_file_mtime(dir: &Path) -> Option<SystemTime> {
    if !dir.is_dir() {
        return None;
    }
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().ok().is_some_and(|ft| ft.is_file()))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

/// Find the most recently modified file with a given extension in a directory.
///
/// Uses a directory-mtime cache: only rescans when the directory's mtime
/// has changed (Linux updates dir mtime on file creation/deletion).
/// In steady state this costs 1 stat() instead of N.
fn find_latest_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    let dir_mtime = fs::metadata(dir).ok()?.modified().ok()?;

    // Check cache
    {
        let guard = FILE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_ref() {
            if let Some(entry) = map.get(dir) {
                if entry.dir_mtime == dir_mtime {
                    return entry.latest_path.clone();
                }
            }
        }
    }

    // Cache miss — full scan
    let latest = scan_latest_file(dir, extension);

    // Update cache (evict all if over max — simple and avoids LRU complexity)
    {
        let mut guard = FILE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        if map.len() >= FILE_CACHE_MAX_ENTRIES {
            map.clear();
        }
        map.insert(
            dir.to_path_buf(),
            CachedFileEntry {
                dir_mtime,
                latest_path: latest.clone(),
            },
        );
    }

    latest
}

/// Scan a directory for the most recently modified file with a given extension.
fn scan_latest_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == extension)
        })
        .max_by_key(|entry| {
            entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .map(|entry| entry.path())
}

/// Compute the SHA-256 hex digest of a project path.
///
/// Used by Gemini CLI which stores sessions under `~/.gemini/tmp/<sha256>/`.
fn project_path_sha256(project_path: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(project_path.as_bytes()))
}

/// Pick the most recent of two optional timestamps.
fn most_recent_mtime(a: Option<SystemTime>, b: Option<SystemTime>) -> Option<SystemTime> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// File cache (generalized from JSONL-only cache)
// ---------------------------------------------------------------------------

/// Cached result of a directory scan for the latest file.
struct CachedFileEntry {
    /// Directory mtime at the time of the scan.
    dir_mtime: SystemTime,
    /// Path to the most recent file (None if directory was empty).
    latest_path: Option<PathBuf>,
}

/// Maximum entries before the file cache is swept.
const FILE_CACHE_MAX_ENTRIES: usize = 128;

/// Cache keyed by directory path.
static FILE_CACHE: Mutex<Option<HashMap<PathBuf, CachedFileEntry>>> = Mutex::new(None);

/// Clean up stale cache entries.
pub fn clear_jsonl_cache() {
    let mut guard = FILE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        map.clear();
    }
}

// ===========================================================================
// ClaudeResolver
// ===========================================================================

/// Resolves Claude Code session files from `~/.claude/projects/<slug>/`.
///
/// Claude Code writes session transcripts as JSONL files. During compaction,
/// a subagent writes to `<session-id>/subagents/agent-acompact-*.jsonl`.
/// Both locations are checked for activity.
struct ClaudeResolver {
    /// `~/.claude/projects/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
}

impl ClaudeResolver {
    fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".claude").join("projects"));
        Self { base_dir }
    }
}

impl SessionResolver for ClaudeResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let base = match self.base_dir.as_ref() {
            Some(dir) => dir,
            None => return IdleResult::idle(),
        };
        claude_detect_idle(project_path, base)
    }
}

/// Core Claude idle detection logic — shared by ClaudeResolver and detect_idle_in().
fn claude_detect_idle(project_path: &str, projects_dir: &Path) -> IdleResult {
    let slug = path_to_slug(project_path);
    let project_dir = projects_dir.join(&slug);

    // Find the most recently modified .jsonl file in the project directory
    let jsonl = match find_latest_file(&project_dir, "jsonl") {
        Some(entry) => entry,
        None => return IdleResult::idle(),
    };

    // Extract session ID from filename (UUID.jsonl → UUID)
    let session_id = jsonl
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    let jsonl_path = jsonl.to_string_lossy().to_string();

    // Check mtime of main JSONL
    let main_mtime = file_mtime(&jsonl);

    // Also check the subagents directory for this session.
    // During compaction, Claude writes to subagents/agent-acompact-*.jsonl
    // while the main JSONL goes quiet.
    let subagent_mtime = session_id.as_deref().and_then(|sid| {
        let subagents_dir = project_dir.join(sid).join("subagents");
        newest_file_mtime(&subagents_dir)
    });

    let state = most_recent_mtime(main_mtime, subagent_mtime)
        .map(|t| classify_mtime(t, ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(jsonl_path),
    }
}

// ===========================================================================
// GeminiResolver
// ===========================================================================

/// Resolves Gemini CLI session files from `~/.gemini/tmp/<sha256>/chats/`.
///
/// Gemini CLI uses SHA-256 of the project path as the directory name.
/// Chat sessions are stored as JSON files in the `chats/` subdirectory.
struct GeminiResolver {
    /// `~/.gemini/tmp/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
}

impl GeminiResolver {
    fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".gemini").join("tmp"));
        Self { base_dir }
    }
}

impl SessionResolver for GeminiResolver {
    fn detect_idle(&self, project_path: &str) -> IdleResult {
        let base = match self.base_dir.as_ref() {
            Some(dir) => dir,
            None => return IdleResult::idle(),
        };
        gemini_detect_idle(project_path, base)
    }
}

/// Core Gemini idle detection — testable with custom base dir.
fn gemini_detect_idle(project_path: &str, base_dir: &Path) -> IdleResult {
    // Gemini CLI has used two naming schemes for session directories:
    //   - Newer (0.29+): project directory name (e.g. "my-project")
    //   - Older: SHA-256 hash of the full project path
    // Try the directory name first, then fall back to the hash.
    let dir_name = Path::new(project_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let chats_dir_by_name = base_dir.join(dir_name).join("chats");
    let hash = project_path_sha256(project_path);
    let chats_dir_by_hash = base_dir.join(&hash).join("chats");

    let chats_dir = if chats_dir_by_name.is_dir() {
        &chats_dir_by_name
    } else {
        &chats_dir_by_hash
    };

    // Find the most recently modified .json file in the chats directory
    let session_file = match find_latest_file(chats_dir, "json") {
        Some(entry) => entry,
        None => return IdleResult::idle(),
    };

    // Extract session ID from filename: "session-2026-02-10T19-57-4574fc66.json" → "4574fc66"
    // The last segment after the final dash is the UUID prefix.
    let session_id = session_file
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.rsplit('-').next())
        .map(|s| s.to_string());

    let file_path = session_file.to_string_lossy().to_string();

    let state = file_mtime(&session_file)
        .map(|t| classify_mtime(t, ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(file_path),
    }
}

// ===========================================================================
// CodexResolver
// ===========================================================================

/// Resolves Codex CLI session files from `~/.codex/sessions/YYYY/MM/DD/`.
///
/// Codex organizes sessions by date, not by project. To find the session
/// for a specific project, we scan recent JSONL files and read the
/// `session_meta` record to match the `cwd` field against the project path.
///
/// A path→file cache avoids re-scanning on every poll.
struct CodexResolver {
    /// `~/.codex/sessions/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
}

impl CodexResolver {
    fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".codex").join("sessions"));
        Self { base_dir }
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
fn codex_detect_idle(project_path: &str, sessions_dir: &Path) -> IdleResult {
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

/// Build an IdleResult from a Codex session file.
fn codex_result_from_file(path: &Path) -> IdleResult {
    // Extract session ID from filename: "rollout-2026-02-21T17-25-42-UUID.jsonl"
    // The UUID is the last segment of the stem after the timestamp portion.
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| {
            // Find the UUID portion: everything after "rollout-YYYY-MM-DDTHH-MM-SS-"
            // "rollout-" (8) + "YYYY-MM-DDTHH-MM-SS" (19) + "-" (1) = 28
            if stem.len() > 28 && stem.starts_with("rollout-") {
                stem[28..].to_string()
            } else {
                stem.to_string()
            }
        });

    let file_path = path.to_string_lossy().to_string();

    let state = file_mtime(path)
        .map(|t| classify_mtime(t, CODEX_ACTIVE_THRESHOLD))
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(file_path),
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
    use chrono::Local;

    let today = Local::now().date_naive();

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
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "jsonl")
            })
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
                return Some(entry.path());
            }
        }
    }

    None
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

    // -----------------------------------------------------------------------
    // Shared helpers
    // -----------------------------------------------------------------------

    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    // -----------------------------------------------------------------------
    // Slug / classify_mtime tests
    // -----------------------------------------------------------------------

    #[test]
    fn slug_from_absolute_path() {
        assert_eq!(
            path_to_slug("/home/testuser/projects/taurhaus"),
            "-home-testuser-projects-taurhaus"
        );
    }

    #[test]
    fn slug_from_nested_path() {
        assert_eq!(
            path_to_slug("/mnt/d/ai_lab/creative/WE_IT"),
            "-mnt-d-ai_lab-creative-WE_IT"
        );
    }

    #[test]
    fn slug_preserves_case() {
        assert_eq!(path_to_slug("/home/User/MyProject"), "-home-User-MyProject");
    }

    #[test]
    fn slug_handles_backslashes() {
        assert_eq!(
            path_to_slug("C:\\Users\\test\\project"),
            "C:-Users-test-project"
        );
    }

    #[test]
    fn classify_mtime_active() {
        let recent = SystemTime::now() - Duration::from_secs(2);
        assert_eq!(classify_mtime(recent, ACTIVE_THRESHOLD), SessionState::Active);
    }

    #[test]
    fn classify_mtime_idle() {
        let old = SystemTime::now() - Duration::from_secs(30);
        assert_eq!(classify_mtime(old, ACTIVE_THRESHOLD), SessionState::Idle);
    }

    #[test]
    fn classify_mtime_transition_zone() {
        let mid = SystemTime::now() - Duration::from_secs(7);
        assert_eq!(classify_mtime(mid, ACTIVE_THRESHOLD), SessionState::Idle);
    }

    #[test]
    fn codex_threshold_is_longer() {
        // 7 seconds: idle with default threshold, still active with Codex threshold
        let mid = SystemTime::now() - Duration::from_secs(7);
        assert_eq!(classify_mtime(mid, ACTIVE_THRESHOLD), SessionState::Idle);
        assert_eq!(classify_mtime(mid, CODEX_ACTIVE_THRESHOLD), SessionState::Active);
    }

    // -----------------------------------------------------------------------
    // ClaudeResolver tests (via detect_idle_in)
    // -----------------------------------------------------------------------

    #[test]
    fn claude_detect_idle_no_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn claude_detect_idle_no_project_dir() {
        let tmp = TempDir::new().unwrap();
        let result = detect_idle_in("/home/user/projects/missing", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn claude_detect_idle_recent_jsonl_is_active() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl_path = project_dir.join("abc-123-def.jsonl");
        let mut f = File::create(&jsonl_path).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("abc-123-def"));
        assert!(result.jsonl_path.is_some());
    }

    #[test]
    fn claude_detect_idle_old_jsonl_is_idle() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl_path = project_dir.join("old-session.jsonl");
        File::create(&jsonl_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(60);
        filetime_set_mtime(&jsonl_path, old_time);

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(result.session_id.as_deref(), Some("old-session"));
    }

    #[test]
    fn claude_detect_idle_picks_most_recent_jsonl() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let old_path = project_dir.join("old-session.jsonl");
        File::create(&old_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old_path, old_time);

        let new_path = project_dir.join("new-session.jsonl");
        let mut f = File::create(&new_path).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("new-session"));
    }

    #[test]
    fn claude_detect_idle_ignores_non_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        File::create(project_dir.join("memory.md")).unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn claude_detect_idle_active_during_compaction_via_subagent() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl_path = project_dir.join("my-session-id.jsonl");
        File::create(&jsonl_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&jsonl_path, old_time);

        let subagents_dir = project_dir.join("my-session-id").join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        let compact_path = subagents_dir.join("agent-acompact-abc123.jsonl");
        let mut f = File::create(&compact_path).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("my-session-id"));
    }

    #[test]
    fn claude_detect_idle_idle_when_both_main_and_subagent_old() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl_path = project_dir.join("my-session-id.jsonl");
        File::create(&jsonl_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&jsonl_path, old_time);

        let subagents_dir = project_dir.join("my-session-id").join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        let compact_path = subagents_dir.join("agent-acompact-abc123.jsonl");
        File::create(&compact_path).unwrap();
        filetime_set_mtime(&compact_path, old_time);

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
    }

    // -----------------------------------------------------------------------
    // GeminiResolver tests
    // -----------------------------------------------------------------------

    #[test]
    fn gemini_detect_idle_active_session() {
        let tmp = TempDir::new().unwrap();

        // Create hash dir for project path
        let project = "/home/user/projects/myapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        // Create a recent session file
        let session = chats_dir.join("session-2026-02-21T10-30-abc12345.json");
        let mut f = File::create(&session).unwrap();
        writeln!(f, r#"{{"sessionId":"abc12345"}}"#).unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("abc12345"));
        assert!(result.jsonl_path.is_some());
    }

    #[test]
    fn gemini_detect_idle_old_session() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/oldapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let session = chats_dir.join("session-2026-01-01T00-00-deadbeef.json");
        File::create(&session).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&session, old_time);

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(result.session_id.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn gemini_detect_idle_no_hash_dir() {
        let tmp = TempDir::new().unwrap();
        let result = gemini_detect_idle("/nonexistent/project", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn gemini_detect_idle_empty_chats_dir() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/emptyapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn gemini_picks_most_recent_session() {
        let tmp = TempDir::new().unwrap();

        let project = "/home/user/projects/multiapp";
        let hash = project_path_sha256(project);
        let chats_dir = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        // Old session
        let old = chats_dir.join("session-2026-01-01T00-00-old11111.json");
        File::create(&old).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old, old_time);

        // Recent session
        let new = chats_dir.join("session-2026-02-21T12-00-new22222.json");
        let mut f = File::create(&new).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("new22222"));
    }

    #[test]
    fn gemini_detect_idle_by_dir_name() {
        // Newer Gemini CLI (0.29+) uses project directory name, not SHA-256 hash
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/tapcount-gemini";
        let chats_dir = tmp.path().join("tapcount-gemini").join("chats");
        fs::create_dir_all(&chats_dir).unwrap();

        let session = chats_dir.join("session-2026-02-23T22-17-80291013.json");
        let mut f = File::create(&session).unwrap();
        writeln!(f, r#"{{"sessionId":"80291013"}}"#).unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("80291013"));
    }

    #[test]
    fn gemini_prefers_dir_name_over_hash() {
        // When both exist, the directory-name version should win
        let tmp = TempDir::new().unwrap();
        let project = "/home/user/projects/myapp";

        // Create hash-based dir with old session
        let hash = project_path_sha256(project);
        let hash_chats = tmp.path().join(&hash).join("chats");
        fs::create_dir_all(&hash_chats).unwrap();
        let old = hash_chats.join("session-2026-01-01T00-00-oldhash1.json");
        File::create(&old).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old, old_time);

        // Create name-based dir with fresh session
        let name_chats = tmp.path().join("myapp").join("chats");
        fs::create_dir_all(&name_chats).unwrap();
        let new = name_chats.join("session-2026-02-23T12-00-newname1.json");
        let mut f = File::create(&new).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = gemini_detect_idle(project, tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("newname1"));
    }

    // -----------------------------------------------------------------------
    // CodexResolver tests
    // -----------------------------------------------------------------------

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
        // Unique project path to avoid CODEX_PATH_CACHE collisions with
        // other tests that run in parallel and use /home/user/projects/myapp.
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
        let tmp = TempDir::new().unwrap();
        let result = codex_detect_idle("/home/user/projects/myapp", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn codex_detect_idle_malformed_jsonl_skipped() {
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
        // Unique project path to avoid CODEX_PATH_CACHE collisions with
        // other tests that run in parallel and use /home/user/projects/myapp.
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

    // -----------------------------------------------------------------------
    // Resolver dispatch test
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_for_returns_correct_type() {
        // Just verify that resolver_for doesn't panic for any tool variant
        let _ = resolver_for(CliTool::Claude);
        let _ = resolver_for(CliTool::Codex);
        let _ = resolver_for(CliTool::Gemini);
    }

    // -----------------------------------------------------------------------
    // Live integration tests (require real sessions on disk)
    // -----------------------------------------------------------------------
    // Run with: cargo test -- --ignored idle::tests::live_

    #[test]
    #[ignore]
    fn live_codex_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Codex);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!("Codex: state={:?}, session_id={:?}, path={:?}", result.state, result.session_id, result.jsonl_path);
        assert!(result.jsonl_path.is_some(), "Codex resolver should find a session file");
        assert!(result.session_id.is_some(), "Codex resolver should extract session ID");
    }

    #[test]
    #[ignore]
    fn live_gemini_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Gemini);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!("Gemini: state={:?}, session_id={:?}, path={:?}", result.state, result.session_id, result.jsonl_path);
        assert!(result.jsonl_path.is_some(), "Gemini resolver should find a session file");
        assert!(result.session_id.is_some(), "Gemini resolver should extract session ID");
    }

    #[test]
    #[ignore]
    fn live_claude_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Claude);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!("Claude: state={:?}, session_id={:?}, path={:?}", result.state, result.session_id, result.jsonl_path);
        assert!(result.jsonl_path.is_some(), "Claude resolver should find a session file");
        assert!(result.session_id.is_some(), "Claude resolver should extract session ID");
    }
}
