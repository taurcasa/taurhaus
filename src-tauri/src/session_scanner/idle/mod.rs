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

mod claude;
mod codex;
mod gemini;

pub use claude::ClaudeResolver;
pub use codex::CodexResolver;
pub use gemini::GeminiResolver;

/// Threshold: if any session file mtime is less than this, session is Active.
/// Used for Claude and Gemini where proc-level signals supplement the mtime.
pub(super) const ACTIVE_THRESHOLD: Duration = Duration::from_secs(5);

/// Longer threshold for Codex — session file mtime is the ONLY activity signal
/// (TCP keep-alive makes socket detection unreliable). Codex has silent gaps
/// during API thinking where no file writes happen. A longer window reduces
/// false idle flashes during those gaps.
/// Total active→idle latency: 10s threshold + ~4s hysteresis = ~14s.
pub(super) const CODEX_ACTIVE_THRESHOLD: Duration = Duration::from_secs(10);

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
    /// Seconds since the latest observed session output file changed.
    pub last_output_age_secs: Option<u64>,
}

impl IdleResult {
    /// Convenience constructor for the common "no data found" case.
    pub(super) fn idle() -> Self {
        Self {
            state: SessionState::Idle,
            session_id: None,
            jsonl_path: None,
            last_output_age_secs: None,
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
// Public entry points (unchanged API)
// ---------------------------------------------------------------------------

/// Detect idle state for a project using the appropriate tool-specific resolver.
pub fn detect_idle(project_path: &str, tool: CliTool) -> IdleResult {
    resolver_for(tool).detect_idle(project_path)
}

/// Testable version: detect idle state for Claude using a custom projects directory.
///
/// Kept for backward compatibility with existing tests.
pub fn detect_idle_in(project_path: &str, claude_projects_dir: &Path) -> IdleResult {
    claude::claude_detect_idle(project_path, claude_projects_dir)
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
pub(super) fn classify_mtime(mtime: SystemTime, threshold: Duration) -> SessionState {
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
pub(super) fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

pub(super) fn age_secs_since_mtime(mtime: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(mtime)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Get the most recent mtime of any file in a directory (non-recursive).
pub(super) fn newest_file_mtime(dir: &Path) -> Option<SystemTime> {
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
pub(super) fn find_latest_file(dir: &Path, extension: &str) -> Option<PathBuf> {
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
pub(super) fn scan_latest_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == extension))
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
pub(super) fn project_path_sha256(project_path: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(project_path.as_bytes()))
}

/// Pick the most recent of two optional timestamps.
pub(super) fn most_recent_mtime(
    a: Option<SystemTime>,
    b: Option<SystemTime>,
) -> Option<SystemTime> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            classify_mtime(recent, ACTIVE_THRESHOLD),
            SessionState::Active
        );
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
        assert_eq!(
            classify_mtime(mid, CODEX_ACTIVE_THRESHOLD),
            SessionState::Active
        );
    }

    #[test]
    fn resolver_for_returns_correct_type() {
        // Just verify that resolver_for doesn't panic for any tool variant
        let _ = resolver_for(CliTool::Claude);
        let _ = resolver_for(CliTool::Codex);
        let _ = resolver_for(CliTool::Gemini);
    }

    // Run with: cargo test -- --ignored session_scanner::idle::tests::live_
    #[test]
    #[ignore]
    fn live_codex_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Codex);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!(
            "Codex: state={:?}, session_id={:?}, path={:?}",
            result.state, result.session_id, result.jsonl_path
        );
        assert!(
            result.jsonl_path.is_some(),
            "Codex resolver should find a session file"
        );
        assert!(
            result.session_id.is_some(),
            "Codex resolver should extract session ID"
        );
    }

    #[test]
    #[ignore]
    fn live_gemini_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Gemini);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!(
            "Gemini: state={:?}, session_id={:?}, path={:?}",
            result.state, result.session_id, result.jsonl_path
        );
        assert!(
            result.jsonl_path.is_some(),
            "Gemini resolver should find a session file"
        );
        assert!(
            result.session_id.is_some(),
            "Gemini resolver should extract session ID"
        );
    }

    #[test]
    #[ignore]
    fn live_claude_resolver_finds_session() {
        let resolver = resolver_for(CliTool::Claude);
        let result = resolver.detect_idle("/home/testuser/projects/taurhaus");
        println!(
            "Claude: state={:?}, session_id={:?}, path={:?}",
            result.state, result.session_id, result.jsonl_path
        );
        assert!(
            result.jsonl_path.is_some(),
            "Claude resolver should find a session file"
        );
        assert!(
            result.session_id.is_some(),
            "Claude resolver should extract session ID"
        );
    }
}
