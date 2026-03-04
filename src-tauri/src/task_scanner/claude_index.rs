//! Claude task source index builder.
//!
//! Builds a unified index that maps Claude task directory keys to project paths:
//! - Session sources: `~/.claude/tasks/{session-id}/` via live + offline session discovery
//! - Team sources: `~/.claude/tasks/{team-name}/` via `~/.claude/teams/{team}/config.json`

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;

/// Unified Claude source index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeSourceIndex {
    /// Session source key (session id) -> project path.
    pub sessions: HashMap<String, PathBuf>,
    /// Team source key (team name) -> one or more project paths.
    pub teams: HashMap<String, Vec<PathBuf>>,
}

/// Build Claude source index using default user directories and live sessions.
pub fn build_claude_source_index() -> ClaudeSourceIndex {
    let Some(home) = dirs::home_dir() else {
        return ClaudeSourceIndex::default();
    };

    let tasks_base = home.join(".claude").join("tasks");
    let projects_base = home.join(".claude").join("projects");
    let teams_base = home.join(".claude").join("teams");
    let live_sessions = crate::session_scanner::scan_sessions();

    build_claude_source_index_in(&live_sessions, &tasks_base, &projects_base, &teams_base)
}

/// Build Claude source index from injectable inputs (testable variant).
pub fn build_claude_source_index_in(
    live_sessions: &[ClaudeSession],
    tasks_base: &Path,
    projects_base: &Path,
    teams_base: &Path,
) -> ClaudeSourceIndex {
    let mut sessions = HashMap::new();

    merge_live_session_map(live_sessions, &mut sessions);
    merge_offline_session_map(projects_base, &mut sessions);
    let teams = build_team_map(tasks_base, teams_base);

    ClaudeSourceIndex { sessions, teams }
}

fn merge_live_session_map(
    live_sessions: &[ClaudeSession],
    sessions: &mut HashMap<String, PathBuf>,
) {
    for session in live_sessions
        .iter()
        .filter(|s| s.cli_tool == CliTool::Claude)
    {
        let Some(session_id) = session.session_id.as_ref() else {
            continue;
        };
        let id = session_id.trim();
        if id.is_empty() {
            continue;
        }

        sessions.insert(id.to_string(), PathBuf::from(session.project_path.as_str()));
    }
}

fn merge_offline_session_map(projects_base: &Path, sessions: &mut HashMap<String, PathBuf>) {
    let Ok(project_dirs) = fs::read_dir(projects_base) else {
        return;
    };

    for project_dir in project_dirs.filter_map(|e| e.ok()) {
        let project_path = project_dir.path();
        if !project_path.is_dir() {
            continue;
        }

        let Ok(entries) = fs::read_dir(&project_path) else {
            continue;
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }

            let Some(session_id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };

            // Live sessions are fresher and should win on key collisions.
            if sessions.contains_key(session_id) {
                continue;
            }

            if let Some(project_path) = extract_project_path_from_jsonl(&path) {
                sessions.insert(session_id.to_string(), project_path);
            }
        }
    }
}

fn extract_project_path_from_jsonl(jsonl_path: &Path) -> Option<PathBuf> {
    const MAX_LINES: usize = 40;

    let file = fs::File::open(jsonl_path).ok()?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines().take(MAX_LINES) {
        let Ok(line) = line else {
            continue;
        };
        let line = line.trim();
        if line.is_empty() || !line.contains("cwd") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(cwd) = extract_cwd_from_value(&value) {
            let cwd = cwd.trim();
            if !cwd.is_empty() {
                return Some(PathBuf::from(cwd));
            }
        }
    }

    None
}

fn extract_cwd_from_value(value: &Value) -> Option<&str> {
    value
        .get("cwd")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("payload").and_then(|p| p.get("cwd")).and_then(|v| v.as_str()))
}

fn build_team_map(tasks_base: &Path, teams_base: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut teams = HashMap::new();

    let Ok(team_dirs) = fs::read_dir(teams_base) else {
        return teams;
    };

    for team_dir in team_dirs.filter_map(|e| e.ok()) {
        let path = team_dir.path();
        if !path.is_dir() {
            continue;
        }

        let Some(team_name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };

        // Include only teams that have a corresponding task directory.
        if !tasks_base.join(team_name).is_dir() {
            continue;
        }

        let config_path = path.join("config.json");
        let Ok(content) = fs::read_to_string(&config_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&content) else {
            tracing::warn!(
                path = %config_path.display(),
                "Skipping malformed Claude team config while building source index"
            );
            continue;
        };

        let Some(members) = value.get("members").and_then(|m| m.as_array()) else {
            continue;
        };

        let mut project_paths = BTreeSet::new();
        for member in members {
            let maybe_path = member
                .get("projectPath")
                .and_then(|v| v.as_str())
                .or_else(|| member.get("project_path").and_then(|v| v.as_str()))
                .or_else(|| member.get("cwd").and_then(|v| v.as_str()));

            if let Some(path) = maybe_path.map(str::trim).filter(|p| !p.is_empty()) {
                project_paths.insert(PathBuf::from(path));
            }
        }

        if !project_paths.is_empty() {
            teams.insert(team_name.to_string(), project_paths.into_iter().collect());
        }
    }

    teams
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{ActivityAttribution, ActivityConfidence, SessionState};
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f.sync_all().unwrap();
    }

    fn make_live_claude_session(session_id: &str, project_path: &str) -> ClaudeSession {
        ClaudeSession {
            pid: 1,
            project_path: project_path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: SessionState::Active,
            session_id: Some(session_id.to_string()),
            jsonl_path: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
        }
    }

    #[test]
    fn index_builds_sessions_map_from_live_and_offline_sources() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        // Offline session jsonl with cwd in early metadata line.
        let project_slug = projects_base.join("-home-user-projects-offline");
        fs::create_dir_all(&project_slug).unwrap();
        write_file(
            &project_slug.join("offline-session.jsonl"),
            r#"{"type":"file-history-snapshot","snapshot":{}}
{"type":"user","sessionId":"offline-session","cwd":"/home/user/projects/offline"}"#,
        );

        let live_sessions = vec![make_live_claude_session(
            "live-session",
            "/home/user/projects/live",
        )];
        let index =
            build_claude_source_index_in(&live_sessions, &tasks_base, &projects_base, &teams_base);

        assert_eq!(
            index.sessions.get("live-session"),
            Some(&PathBuf::from("/home/user/projects/live"))
        );
        assert_eq!(
            index.sessions.get("offline-session"),
            Some(&PathBuf::from("/home/user/projects/offline"))
        );
    }

    #[test]
    fn index_builds_team_map_from_config_variants_and_dedupes_paths() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        fs::create_dir_all(tasks_base.join("team-a")).unwrap();
        fs::create_dir_all(teams_base.join("team-a")).unwrap();
        write_file(
            &teams_base.join("team-a").join("config.json"),
            r#"{
  "name": "team-a",
  "members": [
    {"projectPath": "/projects/a"},
    {"project_path": "/projects/b"},
    {"cwd": "/projects/c"},
    {"projectPath": "/projects/a"}
  ]
}"#,
        );

        let index = build_claude_source_index_in(&[], &tasks_base, &projects_base, &teams_base);
        assert_eq!(
            index.teams.get("team-a"),
            Some(&vec![
                PathBuf::from("/projects/a"),
                PathBuf::from("/projects/b"),
                PathBuf::from("/projects/c"),
            ])
        );
    }

    #[test]
    fn index_skips_malformed_or_missing_team_configs_without_crashing() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        fs::create_dir_all(tasks_base.join("team-bad")).unwrap();
        fs::create_dir_all(teams_base.join("team-bad")).unwrap();
        write_file(&teams_base.join("team-bad").join("config.json"), "not json");

        fs::create_dir_all(tasks_base.join("team-missing-config")).unwrap();
        fs::create_dir_all(teams_base.join("team-missing-config")).unwrap();

        let index = build_claude_source_index_in(&[], &tasks_base, &projects_base, &teams_base);
        assert!(!index.teams.contains_key("team-bad"));
        assert!(!index.teams.contains_key("team-missing-config"));
    }

    #[test]
    fn index_excludes_teams_without_corresponding_tasks_directory() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");
        let teams_base = tmp.path().join("teams");
        fs::create_dir_all(&tasks_base).unwrap();
        fs::create_dir_all(&projects_base).unwrap();
        fs::create_dir_all(&teams_base).unwrap();

        fs::create_dir_all(teams_base.join("team-no-tasks")).unwrap();
        write_file(
            &teams_base.join("team-no-tasks").join("config.json"),
            r#"{
  "name": "team-no-tasks",
  "members": [{"projectPath": "/projects/ghost"}]
}"#,
        );

        let index = build_claude_source_index_in(&[], &tasks_base, &projects_base, &teams_base);
        assert!(!index.teams.contains_key("team-no-tasks"));
    }
}
