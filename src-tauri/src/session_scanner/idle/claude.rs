use super::claude_registry;
use super::*;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::{config_for, CliTool};
use std::path::{Path, PathBuf};

/// Environment variable that moves a Claude session's whole config root.
const CLAUDE_CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Resolves Claude Code session files from `<config dir>/projects/<slug>/`.
///
/// Claude Code writes session transcripts as JSONL files. During compaction,
/// a subagent writes to `<session-id>/subagents/agent-acompact-*.jsonl`.
/// Both locations are checked for activity.
pub struct ClaudeResolver {
    /// Transcript root for the app's own config dir.
    base_dir: Option<PathBuf>,
}

impl ClaudeResolver {
    pub fn new() -> Self {
        Self {
            base_dir: Some(PlatformPaths::tool_session_root(CliTool::Claude)),
        }
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

/// Per-process Claude detection.
///
/// Prefers the sessions registry the process itself writes; falls back to the
/// transcript heuristic under that process's own config root.
pub(super) fn claude_detect_runtime_idle(project_path: &str, pid: u32) -> IdleResult {
    claude_detect_runtime_idle_with(project_path, pid, &process_env_var)
}

fn process_env_var(pid: u32, name: &str) -> Option<String> {
    crate::platform::process_env_var(pid, name)
}

pub(super) fn claude_detect_runtime_idle_with<F>(
    project_path: &str,
    pid: u32,
    env_lookup: &F,
) -> IdleResult
where
    F: Fn(u32, &str) -> Option<String>,
{
    let config_dir = claude_config_dir_for_pid(pid, env_lookup);
    let source = claude_registry::ClaudeRegistryActivitySource {
        config_dir: &config_dir,
    };

    if let Some(result) = ActivitySource::activity(&source, project_path, pid, None) {
        return result;
    }

    claude_detect_idle(
        project_path,
        &config_dir.join(config_for(CliTool::Claude).projects_subdir),
    )
}

/// Config root a specific Claude process is using.
///
/// Sessions launched with `CLAUDE_CONFIG_DIR=<other root>` keep their registry
/// records *and* their transcripts under that root. Resolving it per process is
/// what makes a second-account session visible at all.
pub(super) fn claude_config_dir_for_pid<F>(pid: u32, env_lookup: &F) -> PathBuf
where
    F: Fn(u32, &str) -> Option<String>,
{
    env_lookup(pid, CLAUDE_CONFIG_DIR_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(PlatformPaths::claude_dir)
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
    let last_output_age_secs =
        most_recent_mtime(main_mtime, subagent_mtime).map(age_secs_since_mtime);

    IdleResult {
        state,
        session_id,
        jsonl_path: Some(jsonl_path),
        last_output_age_secs,
        authoritative: false,
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

    /// Env map stand-in for `/proc/<pid>/environ`.
    fn env_map(pairs: &[(u32, &str, &str)]) -> impl Fn(u32, &str) -> Option<String> + use<> {
        let owned: Vec<(u32, String, String)> = pairs
            .iter()
            .map(|(pid, key, value)| (*pid, key.to_string(), value.to_string()))
            .collect();
        move |pid, name| {
            owned
                .iter()
                .find(|(entry_pid, key, _)| *entry_pid == pid && key == name)
                .map(|(_, _, value)| value.clone())
        }
    }

    fn write_registry_record(
        config_dir: &Path,
        pid: u32,
        session_id: &str,
        cwd: &str,
        status: &str,
    ) {
        let dir = config_dir.join("sessions");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{pid}.json")),
            format!(
                r#"{{"pid":{pid},"sessionId":"{session_id}","cwd":"{cwd}","version":"2.1.238","tmux":"taurhaus:@3.%3","name":"taurhaus-00","status":"{status}","updatedAt":1787327562655}}"#
            ),
        )
        .unwrap();
    }

    fn write_transcript(config_dir: &Path, cwd: &str, session_id: &str) -> PathBuf {
        let dir = config_dir.join("projects").join(path_to_slug(cwd));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session_id}.jsonl"));
        let mut f = File::create(&path).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();
        path
    }

    // Regression: 9a66d1c shipped `ClaudeResolver` with `~/.claude/projects`
    // hardcoded (moved verbatim into this module by b7cf393). Every live Claude
    // session on this host runs with `CLAUDE_CONFIG_DIR=~/.claude-account2`, so
    // the transcript was never found and the session stayed permanently
    // "uncertain" (yellow) in the sidebar.
    #[test]
    fn runtime_idle_reads_the_transcript_under_the_process_claude_config_dir() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".claude-account2");
        let project = "/home/user/projects/foo";
        let transcript = write_transcript(&config_dir, project, "session-under-account2");

        let lookup = env_map(&[(
            4242,
            "CLAUDE_CONFIG_DIR",
            config_dir.to_string_lossy().as_ref(),
        )]);
        let result = claude_detect_runtime_idle_with(project, 4242, &lookup);

        assert_eq!(result.session_id.as_deref(), Some("session-under-account2"));
        assert_eq!(
            result.jsonl_path.as_deref(),
            Some(transcript.to_string_lossy().as_ref())
        );
        assert_eq!(result.state, SessionState::Active);
    }

    #[test]
    fn runtime_idle_prefers_the_sessions_registry_over_the_transcript_mtime() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".claude-account2");
        let project = "/home/user/projects/foo";
        // Fresh transcript: the mtime heuristic would say Active.
        write_transcript(&config_dir, project, "registry-session");
        write_registry_record(&config_dir, 4242, "registry-session", project, "idle");

        let lookup = env_map(&[(
            4242,
            "CLAUDE_CONFIG_DIR",
            config_dir.to_string_lossy().as_ref(),
        )]);
        let result = claude_detect_runtime_idle_with(project, 4242, &lookup);

        assert_eq!(result.state, SessionState::Idle);
        assert!(result.authoritative);
        assert_eq!(result.session_id.as_deref(), Some("registry-session"));
    }

    #[test]
    fn runtime_idle_without_a_registry_is_not_authoritative() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".claude-account2");
        let project = "/home/user/projects/foo";
        write_transcript(&config_dir, project, "heuristic-session");

        let lookup = env_map(&[(
            4242,
            "CLAUDE_CONFIG_DIR",
            config_dir.to_string_lossy().as_ref(),
        )]);
        let result = claude_detect_runtime_idle_with(project, 4242, &lookup);

        assert!(!result.authoritative);
    }

    // Regression: c9669ef dropped the whole registry record when its `status`
    // was one this build does not know, so `claude_detect_runtime_idle_with`
    // fell back to "newest transcript in the project" — which in a project with
    // two Claude panes hands this PID the *other* pane's session id and
    // transcript. The record's identity is PID-specific and stays; only the
    // activity falls back to the heuristic.
    #[test]
    fn unknown_status_keeps_this_pids_transcript_not_the_projects_newest() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".claude-account2");
        let project = "/home/user/projects/foo";

        let mine = write_transcript(&config_dir, project, "session-mine");
        filetime_set_mtime(&mine, SystemTime::now() - Duration::from_secs(30));
        // A second pane in the same project, writing right now.
        write_transcript(&config_dir, project, "session-other-pane");
        write_registry_record(&config_dir, 4242, "session-mine", project, "hibernating");

        let lookup = env_map(&[(
            4242,
            "CLAUDE_CONFIG_DIR",
            config_dir.to_string_lossy().as_ref(),
        )]);
        let result = claude_detect_runtime_idle_with(project, 4242, &lookup);

        assert_eq!(result.session_id.as_deref(), Some("session-mine"));
        assert_eq!(
            result.jsonl_path.as_deref(),
            Some(mine.to_string_lossy().as_ref())
        );
        assert!(!result.authoritative);
        assert_eq!(result.state, SessionState::Idle);
    }

    #[test]
    fn claude_config_dir_falls_back_to_the_app_root_when_unset() {
        let lookup = env_map(&[]);
        assert_eq!(
            claude_config_dir_for_pid(4242, &lookup),
            PlatformPaths::claude_dir()
        );

        let blank = env_map(&[(4242, "CLAUDE_CONFIG_DIR", "   ")]);
        assert_eq!(
            claude_config_dir_for_pid(4242, &blank),
            PlatformPaths::claude_dir()
        );
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
