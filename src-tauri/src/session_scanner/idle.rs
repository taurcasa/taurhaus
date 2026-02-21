//! Idle detector — check JSONL transcript mtime to determine session state.
//!
//! Claude Code writes session transcripts to:
//!   `~/.claude/projects/<slug>/<session-id>.jsonl`
//!
//! During normal work, the main JSONL updates every ~1 second. During
//! compaction, a subagent writes to:
//!   `~/.claude/projects/<slug>/<session-id>/subagents/agent-acompact-*.jsonl`
//!
//! We check the most recent mtime across both the main JSONL and the
//! subagents directory to cover all active phases including compaction.

use crate::session_scanner::SessionState;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Threshold: if any session file mtime is less than this, session is Active.
const ACTIVE_THRESHOLD: Duration = Duration::from_secs(5);

/// Result of idle detection for a project.
#[derive(Debug, Clone, PartialEq)]
pub struct IdleResult {
    /// Detected session state.
    pub state: SessionState,
    /// Session ID (UUID from JSONL filename).
    pub session_id: Option<String>,
    /// Full path to the JSONL file.
    pub jsonl_path: Option<String>,
}

/// Detect idle state for a project by checking JSONL transcript mtime.
///
/// Uses the default Claude Code projects directory (~/.claude/projects/).
pub fn detect_idle(project_path: &str) -> IdleResult {
    let claude_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => {
            return IdleResult {
                state: SessionState::Idle,
                session_id: None,
                jsonl_path: None,
            }
        }
    };
    detect_idle_in(project_path, &claude_dir)
}

/// Testable version: detect idle state using a custom Claude projects directory.
pub fn detect_idle_in(project_path: &str, claude_projects_dir: &Path) -> IdleResult {
    let slug = path_to_slug(project_path);
    let project_dir = claude_projects_dir.join(&slug);

    // Find the most recently modified .jsonl file in the project directory
    let jsonl = match find_latest_jsonl(&project_dir) {
        Some(entry) => entry,
        None => {
            // No JSONL files found — can't determine state, default to Idle
            return IdleResult {
                state: SessionState::Idle,
                session_id: None,
                jsonl_path: None,
            };
        }
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

    // Active if EITHER the main JSONL or any subagent file was recently modified
    let most_recent = match (main_mtime, subagent_mtime) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    let state = most_recent
        .map(classify_mtime)
        .unwrap_or(SessionState::Idle);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(jsonl_path),
    }
}

/// Convert a project path to Claude Code's project slug format.
///
/// The slug replaces `/` with `-` and drops the leading `/`:
/// `/home/mstie/projects/taurhaus` → `-home-mstie-projects-taurhaus`
pub fn path_to_slug(path: &str) -> String {
    path.replace('/', "-")
}

/// Classify mtime into Active or Idle based on how recent it is.
fn classify_mtime(mtime: SystemTime) -> SessionState {
    let elapsed = SystemTime::now()
        .duration_since(mtime)
        .unwrap_or(Duration::from_secs(999));

    if elapsed < ACTIVE_THRESHOLD {
        SessionState::Active
    } else {
        // Both the 5-10s gap and >10s map to Idle.
        // The 5-10s gap is a brief transition zone that avoids flickering.
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

/// Find the most recently modified .jsonl file in a directory.
fn find_latest_jsonl(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "jsonl")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn slug_from_absolute_path() {
        assert_eq!(
            path_to_slug("/home/mstie/projects/taurhaus"),
            "-home-mstie-projects-taurhaus"
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
    fn detect_idle_no_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
        assert!(result.jsonl_path.is_none());
    }

    #[test]
    fn detect_idle_no_project_dir() {
        let tmp = TempDir::new().unwrap();
        // Don't create the project directory
        let result = detect_idle_in("/home/user/projects/missing", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn detect_idle_recent_jsonl_is_active() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        // Create a JSONL file with current mtime (just written = active)
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
    fn detect_idle_old_jsonl_is_idle() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl_path = project_dir.join("old-session.jsonl");
        File::create(&jsonl_path).unwrap();

        // Set mtime to 60 seconds ago
        let old_time = SystemTime::now() - Duration::from_secs(60);
        filetime_set_mtime(&jsonl_path, old_time);

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert_eq!(result.session_id.as_deref(), Some("old-session"));
    }

    #[test]
    fn detect_idle_picks_most_recent_jsonl() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        // Create old file
        let old_path = project_dir.join("old-session.jsonl");
        File::create(&old_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(3600);
        filetime_set_mtime(&old_path, old_time);

        // Create recent file
        let new_path = project_dir.join("new-session.jsonl");
        let mut f = File::create(&new_path).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Active);
        assert_eq!(result.session_id.as_deref(), Some("new-session"));
    }

    #[test]
    fn detect_idle_ignores_non_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        // Create a non-jsonl file
        File::create(project_dir.join("memory.md")).unwrap();

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
        assert!(result.session_id.is_none());
    }

    #[test]
    fn classify_mtime_active() {
        let recent = SystemTime::now() - Duration::from_secs(2);
        assert_eq!(classify_mtime(recent), SessionState::Active);
    }

    #[test]
    fn classify_mtime_idle() {
        let old = SystemTime::now() - Duration::from_secs(30);
        assert_eq!(classify_mtime(old), SessionState::Idle);
    }

    #[test]
    fn classify_mtime_transition_zone() {
        // 7s is beyond threshold — should be Idle
        let mid = SystemTime::now() - Duration::from_secs(7);
        assert_eq!(classify_mtime(mid), SessionState::Idle);
    }

    #[test]
    fn detect_idle_active_during_compaction_via_subagent() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        // Main JSONL is old (compaction started, main transcript quiet)
        let jsonl_path = project_dir.join("my-session-id.jsonl");
        File::create(&jsonl_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&jsonl_path, old_time);

        // But subagent compaction file is fresh (written by compaction agent)
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
    fn detect_idle_idle_when_both_main_and_subagent_old() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-home-user-projects-foo");
        fs::create_dir_all(&project_dir).unwrap();

        // Main JSONL is old
        let jsonl_path = project_dir.join("my-session-id.jsonl");
        File::create(&jsonl_path).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(120);
        filetime_set_mtime(&jsonl_path, old_time);

        // Subagent file also old
        let subagents_dir = project_dir.join("my-session-id").join("subagents");
        fs::create_dir_all(&subagents_dir).unwrap();
        let compact_path = subagents_dir.join("agent-acompact-abc123.jsonl");
        File::create(&compact_path).unwrap();
        filetime_set_mtime(&compact_path, old_time);

        let result = detect_idle_in("/home/user/projects/foo", tmp.path());
        assert_eq!(result.state, SessionState::Idle);
    }

    /// Helper: set file mtime using std::fs.
    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        // Use filetime crate if available, otherwise use a workaround
        // For now, use std::fs::File::set_modified (available on Linux)
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }
}
