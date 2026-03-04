//! Task scanner — reads task data from Claude, Codex, and Gemini CLI tools.
//!
//! Each tool tracks tasks differently:
//! - **Claude Code**: Structured JSON at `~/.claude/tasks/{session-id}/*.json`
//! - **Codex CLI**: `update_plan` function calls in session JSONL files
//! - **Gemini CLI**: `TODO.md` markdown checkboxes in the project directory
//!
//! The `get_tasks_for_project()` orchestrator calls all three parsers and
//! aggregates results. Partial failures are collected as errors — one source
//! failing doesn't prevent others from returning.

pub mod claude;
pub mod claude_index;
pub mod codex;
pub mod gemini;
pub mod types;

pub use types::{
    ArchivedSession, ArchivedSessionsResult, SessionInfo, TaskDetail, TaskResult, TaskStatus,
    UnifiedTask,
};

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;

/// Scan all task sources for a project and return a unified result.
///
/// Calls each tool-specific parser and aggregates tasks. If a parser fails,
/// its error is recorded but other parsers still contribute their results.
pub fn get_tasks_for_project(project_path: &str, sessions: &[ClaudeSession]) -> TaskResult {
    get_tasks_for_project_with(
        project_path,
        sessions,
        claude::get_tasks,
        codex::get_tasks,
        gemini::get_tasks,
    )
}

fn get_tasks_for_project_with<CF, XF, GF>(
    project_path: &str,
    sessions: &[ClaudeSession],
    get_claude_tasks: CF,
    get_codex_tasks: XF,
    get_gemini_tasks: GF,
) -> TaskResult
where
    CF: Fn(&str, &[&ClaudeSession]) -> Result<Vec<UnifiedTask>, String>,
    XF: Fn(&str, &[&ClaudeSession]) -> Result<Vec<UnifiedTask>, String>,
    GF: Fn(&str) -> Result<Vec<UnifiedTask>, String>,
{
    let mut result = TaskResult::empty();

    // Claude: structured task JSON
    let claude_sessions: Vec<&ClaudeSession> = sessions
        .iter()
        .filter(|s| s.cli_tool == CliTool::Claude)
        .collect();
    match get_claude_tasks(project_path, &claude_sessions) {
        Ok(tasks) => result.tasks.extend(tasks),
        Err(e) => result.errors.push(("claude".to_string(), e.to_string())),
    }

    // Codex: update_plan from JSONL
    let codex_sessions: Vec<&ClaudeSession> = sessions
        .iter()
        .filter(|s| s.cli_tool == CliTool::Codex)
        .collect();
    match get_codex_tasks(project_path, &codex_sessions) {
        Ok(tasks) => result.tasks.extend(tasks),
        Err(e) => result.errors.push(("codex".to_string(), e.to_string())),
    }

    // Gemini: TODO.md checkboxes (no session data needed)
    match get_gemini_tasks(project_path) {
        Ok(tasks) => result.tasks.extend(tasks),
        Err(e) => result.errors.push(("gemini".to_string(), e.to_string())),
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sessions_returns_empty_result() {
        let sessions: Vec<ClaudeSession> = Vec::new();
        let result = get_tasks_for_project_with(
            "/nonexistent/path",
            &sessions,
            |_project_path, _sessions| Ok(Vec::new()),
            |_project_path, _sessions| Ok(Vec::new()),
            |_project_path| Ok(Vec::new()),
        );
        // Should not error — parsers gracefully return empty vecs for missing data
        assert!(result.tasks.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn task_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Completed).unwrap(),
            "\"completed\""
        );
    }

    #[test]
    fn task_status_deserializes_snake_case() {
        let status: TaskStatus = serde_json::from_str("\"in_progress\"").unwrap();
        assert_eq!(status, TaskStatus::InProgress);
    }

    #[test]
    fn task_result_serializes_to_json() {
        let result = TaskResult {
            tasks: vec![UnifiedTask {
                id: "1".to_string(),
                subject: "Test task".to_string(),
                description: None,
                active_form: None,
                status: TaskStatus::Pending,
                source: CliTool::Claude,
                blocks: vec![],
                blocked_by: vec![],
                owner: None,
                session_id: None,
            }],
            errors: vec![],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["tasks"][0]["source"], "claude");
        assert_eq!(json["tasks"][0]["status"], "pending");
    }
}
