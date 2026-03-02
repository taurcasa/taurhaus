use super::*;
use std::path::{Path, PathBuf};

/// Resolves Claude Code session files from `~/.claude/projects/<slug>/`.
///
/// Claude Code writes session transcripts as JSONL files. During compaction,
/// a subagent writes to `<session-id>/subagents/agent-acompact-*.jsonl`.
/// Both locations are checked for activity.
pub struct ClaudeResolver {
    /// `~/.claude/projects/` (or None if $HOME is unavailable).
    base_dir: Option<PathBuf>,
}

impl ClaudeResolver {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir().map(|h| h.join(".claude").join("projects"));
        Self { base_dir }
    }
}

impl Default for ClaudeResolver {
    fn default() -> Self {
        Self::new()
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
pub(super) fn claude_detect_idle(project_path: &str, projects_dir: &Path) -> IdleResult {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn filetime_set_mtime(path: &Path, time: SystemTime) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

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
}
