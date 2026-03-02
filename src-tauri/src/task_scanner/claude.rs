//! Claude Code task parser.
//!
//! Claude Code stores structured task JSON at `~/.claude/tasks/{session-id}/*.json`.
//! Each file contains a single task object with rich metadata including dependencies,
//! owners, and active forms.
//!
//! **Live session**: Use `session_id` from running sessions to find task directories.
//! **Offline fallback**: Scan `~/.claude/projects/{slug}/` for JSONL files, check which
//! UUIDs have task directories, use most recently modified.

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;
use crate::task_scanner::types::{TaskStatus, UnifiedTask};
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum file size to parse (1 MB). Skip larger files as a safety measure.
const MAX_FILE_SIZE: u64 = 1_024 * 1_024;

/// Raw Claude task JSON shape (matches disk format exactly).
#[derive(serde::Deserialize)]
struct RawClaudeTask {
    id: String,
    subject: String,
    description: Option<String>,
    #[serde(rename = "activeForm")]
    active_form: Option<String>,
    status: String,
    #[serde(default)]
    blocks: Vec<String>,
    #[serde(rename = "blockedBy", default)]
    blocked_by: Vec<String>,
    #[serde(default)]
    owner: Option<String>,
}

/// Get tasks for a project from Claude Code's task storage.
///
/// Strategy:
/// 1. Check live sessions for `session_id` → read `~/.claude/tasks/{session_id}/`
/// 2. If no live sessions, fall back to finding the most recent session with tasks
pub fn get_tasks(
    project_path: &str,
    sessions: &[&ClaudeSession],
) -> Result<Vec<UnifiedTask>, String> {
    let tasks_base = match claude_tasks_base_dir() {
        Some(dir) => dir,
        None => return Ok(vec![]),
    };

    // Try live sessions first — check each session_id for a tasks directory
    for session in sessions {
        if let Some(ref session_id) = session.session_id {
            let task_dir = tasks_base.join(session_id);
            if task_dir.is_dir() {
                return parse_task_directory(&task_dir);
            }
        }
    }

    // Offline fallback: find sessions for this project slug, check for tasks
    get_tasks_offline(project_path, &tasks_base)
}

/// Testable version that accepts custom base directories.
pub fn get_tasks_in(
    project_path: &str,
    sessions: &[&ClaudeSession],
    tasks_base: &Path,
    projects_base: &Path,
) -> Result<Vec<UnifiedTask>, String> {
    // Try live sessions first
    for session in sessions {
        if let Some(ref session_id) = session.session_id {
            let task_dir = tasks_base.join(session_id);
            if task_dir.is_dir() {
                return parse_task_directory(&task_dir);
            }
        }
    }

    // Offline fallback
    get_tasks_offline_in(project_path, tasks_base, projects_base)
}

/// Resolve `~/.claude/tasks/`.
fn claude_tasks_base_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("tasks"))
}

/// Offline fallback: scan the project's Claude directory for session IDs with tasks.
fn get_tasks_offline(project_path: &str, tasks_base: &Path) -> Result<Vec<UnifiedTask>, String> {
    let projects_base = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Ok(vec![]),
    };
    get_tasks_offline_in(project_path, tasks_base, &projects_base)
}

/// Offline fallback with injectable paths for testing.
fn get_tasks_offline_in(
    project_path: &str,
    tasks_base: &Path,
    projects_base: &Path,
) -> Result<Vec<UnifiedTask>, String> {
    let slug = crate::session_scanner::idle::path_to_slug(project_path);
    let project_dir = projects_base.join(&slug);

    if !project_dir.is_dir() {
        return Ok(vec![]);
    }

    // List JSONL files, extract session IDs (filenames without extension)
    let mut session_ids: Vec<(String, std::time::SystemTime)> = fs::read_dir(&project_dir)
        .map_err(|e| format!("Failed to read project dir: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| {
            let session_id = entry.path().file_stem()?.to_str()?.to_string();
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((session_id, mtime))
        })
        .collect();

    // Sort by mtime descending — try most recent first
    session_ids.sort_by(|a, b| b.1.cmp(&a.1));

    // Find the first session ID that has a tasks directory
    for (session_id, _) in &session_ids {
        let task_dir = tasks_base.join(session_id);
        if task_dir.is_dir() {
            return parse_task_directory(&task_dir);
        }
    }

    Ok(vec![])
}

/// Parse all task JSON files in a directory.
///
/// The `session_id` is extracted from the directory name (the parent UUID).
/// Tasks with `status: "deleted"` are silently excluded.
fn parse_task_directory(dir: &Path) -> Result<Vec<UnifiedTask>, String> {
    let session_id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read task dir: {e}"))?;

    let mut tasks = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Only parse .json files
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Skip oversized files
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > MAX_FILE_SIZE {
                tracing::warn!(path = %path.display(), "Skipping oversized task file (> 1MB)");
                continue;
            }
        }

        match parse_task_file(&path, session_id.clone()) {
            Ok(Some(task)) => tasks.push(task),
            Ok(None) => {} // Deleted task — silently skip
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Skipping malformed task file"
                );
            }
        }
    }

    // Sort by ID for stable ordering
    tasks.sort_by(|a, b| {
        let a_num: Option<u32> = a.id.parse().ok();
        let b_num: Option<u32> = b.id.parse().ok();
        match (a_num, b_num) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => a.id.cmp(&b.id),
        }
    });

    Ok(tasks)
}

/// Parse a single Claude task JSON file into a UnifiedTask.
///
/// Returns `Ok(None)` for deleted tasks (status: "deleted") so they are
/// silently excluded from the board without logging a warning.
fn parse_task_file(path: &Path, session_id: Option<String>) -> Result<Option<UnifiedTask>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Read error: {e}"))?;
    let raw: RawClaudeTask =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {e}"))?;

    // Deleted tasks are excluded entirely — they should not appear on the board
    if raw.status == "deleted" {
        return Ok(None);
    }

    let status = match raw.status.as_str() {
        "in_progress" => TaskStatus::InProgress,
        "completed" => TaskStatus::Completed,
        _ => TaskStatus::Pending, // "pending" and anything unknown → Pending
    };

    Ok(Some(UnifiedTask {
        id: raw.id,
        subject: raw.subject,
        description: raw.description,
        active_form: raw.active_form,
        status,
        source: CliTool::Claude,
        blocks: raw.blocks,
        blocked_by: raw.blocked_by,
        owner: raw.owner,
        session_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_task(dir: &Path, filename: &str, content: &str) {
        let path = dir.join(filename);
        let mut f = File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.sync_all().unwrap();
    }

    #[test]
    fn parse_well_formed_task() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-123");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{
                "id": "1",
                "subject": "Implement feature X",
                "description": "A longer description",
                "activeForm": "Implementing feature X",
                "status": "in_progress",
                "blocks": ["2"],
                "blockedBy": [],
                "owner": "agent-1"
            }"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[0].subject, "Implement feature X");
        assert_eq!(
            tasks[0].description.as_deref(),
            Some("A longer description")
        );
        assert_eq!(
            tasks[0].active_form.as_deref(),
            Some("Implementing feature X")
        );
        assert_eq!(tasks[0].status, TaskStatus::InProgress);
        assert_eq!(tasks[0].source, CliTool::Claude);
        assert_eq!(tasks[0].blocks, vec!["2"]);
        assert!(tasks[0].blocked_by.is_empty());
        assert_eq!(tasks[0].owner.as_deref(), Some("agent-1"));
        assert_eq!(tasks[0].session_id.as_deref(), Some("session-123"));
    }

    #[test]
    fn session_id_extracted_from_directory_name() {
        let tmp = TempDir::new().unwrap();
        let uuid = "a7a1946e-6c27-468b-a46b-0eb005992454";
        let task_dir = tmp.path().join(uuid);
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Test","status":"pending"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks[0].session_id.as_deref(), Some(uuid));
    }

    #[test]
    fn parse_multiple_tasks_sorted_numerically() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-456");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Third","status":"pending"}"#,
        );
        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"First","status":"completed"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Second","status":"in_progress"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "2");
        assert_eq!(tasks[2].id, "3");
    }

    #[test]
    fn empty_directory_returns_empty_vec() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("empty-session");
        fs::create_dir_all(&task_dir).unwrap();

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn malformed_json_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-bad");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(&task_dir, "1.json", "not valid json");
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Valid","status":"pending"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn status_mapping() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-status");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Pending","status":"pending"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"In progress","status":"in_progress"}"#,
        );
        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Completed","status":"completed"}"#,
        );
        write_task(
            &task_dir,
            "4.json",
            r#"{"id":"4","subject":"Unknown","status":"unknown_value"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks[0].status, TaskStatus::Pending);
        assert_eq!(tasks[1].status, TaskStatus::InProgress);
        assert_eq!(tasks[2].status, TaskStatus::Completed);
        assert_eq!(tasks[3].status, TaskStatus::Pending); // unknown → Pending
    }

    #[test]
    fn deleted_tasks_are_excluded() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-deleted");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Active task","status":"in_progress"}"#,
        );
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Deleted task","status":"deleted"}"#,
        );
        write_task(
            &task_dir,
            "3.json",
            r#"{"id":"3","subject":"Another active","status":"pending"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "1");
        assert_eq!(tasks[1].id, "3");
        // Deleted task #2 should not appear
    }

    #[test]
    fn preserves_dependency_arrays() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-deps");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Task","status":"pending","blocks":["2","3"],"blockedBy":["0"]}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks[0].blocks, vec!["2", "3"]);
        assert_eq!(tasks[0].blocked_by, vec!["0"]);
    }

    #[test]
    fn skips_non_json_files() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-mixed");
        fs::create_dir_all(&task_dir).unwrap();

        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Valid","status":"pending"}"#,
        );
        write_task(&task_dir, "notes.txt", "some notes");
        write_task(&task_dir, "data.jsonl", r#"{"line":1}"#);

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn skips_oversized_files() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-big");
        fs::create_dir_all(&task_dir).unwrap();

        // Create a file > 1MB
        let big_path = task_dir.join("1.json");
        let mut f = File::create(&big_path).unwrap();
        let padding = " ".repeat(1_100_000);
        write!(
            f,
            r#"{{"id":"1","subject":"Big","status":"pending","description":"{padding}"}}"#
        )
        .unwrap();
        f.sync_all().unwrap();

        // Also a normal-sized file
        write_task(
            &task_dir,
            "2.json",
            r#"{"id":"2","subject":"Small","status":"pending"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "2");
    }

    #[test]
    fn missing_optional_fields_default() {
        let tmp = TempDir::new().unwrap();
        let task_dir = tmp.path().join("session-minimal");
        fs::create_dir_all(&task_dir).unwrap();

        // Minimal valid task — only required fields
        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Minimal task","status":"pending"}"#,
        );

        let tasks = parse_task_directory(&task_dir).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].description.is_none());
        assert!(tasks[0].active_form.is_none());
        assert!(tasks[0].blocks.is_empty());
        assert!(tasks[0].blocked_by.is_empty());
        assert!(tasks[0].owner.is_none());
    }

    #[test]
    fn offline_fallback_finds_tasks_by_slug() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");

        // Create project directory with a JSONL file
        let slug = "-home-user-projects-myapp";
        let project_dir = projects_base.join(slug);
        fs::create_dir_all(&project_dir).unwrap();
        let mut f = File::create(project_dir.join("sess-abc.jsonl")).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        // Create tasks directory for that session
        let task_dir = tasks_base.join("sess-abc");
        fs::create_dir_all(&task_dir).unwrap();
        write_task(
            &task_dir,
            "1.json",
            r#"{"id":"1","subject":"Offline task","status":"pending"}"#,
        );

        let tasks = get_tasks_in(
            "/home/user/projects/myapp",
            &[],
            &tasks_base,
            &projects_base,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Offline task");
    }

    #[test]
    fn live_session_takes_priority_over_offline() {
        let tmp = TempDir::new().unwrap();
        let tasks_base = tmp.path().join("tasks");
        let projects_base = tmp.path().join("projects");

        // Set up offline tasks
        let slug = "-home-user-projects-myapp";
        let project_dir = projects_base.join(slug);
        fs::create_dir_all(&project_dir).unwrap();
        let mut f = File::create(project_dir.join("old-session.jsonl")).unwrap();
        writeln!(f, "{{}}").unwrap();
        f.sync_all().unwrap();

        let old_task_dir = tasks_base.join("old-session");
        fs::create_dir_all(&old_task_dir).unwrap();
        write_task(
            &old_task_dir,
            "1.json",
            r#"{"id":"1","subject":"Old offline task","status":"completed"}"#,
        );

        // Set up live session tasks
        let live_task_dir = tasks_base.join("live-session");
        fs::create_dir_all(&live_task_dir).unwrap();
        write_task(
            &live_task_dir,
            "1.json",
            r#"{"id":"1","subject":"Live task","status":"in_progress"}"#,
        );

        let live_session = ClaudeSession {
            pid: 1234,
            project_path: "/home/user/projects/myapp".to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tmux_window_name: None,
            state: crate::session_scanner::SessionState::Active,
            session_id: Some("live-session".to_string()),
            jsonl_path: None,
        };

        let tasks = get_tasks_in(
            "/home/user/projects/myapp",
            &[&live_session],
            &tasks_base,
            &projects_base,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Live task");
    }
}
