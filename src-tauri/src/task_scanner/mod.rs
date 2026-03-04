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
    ArchivedSession, ArchivedSessionsResult, ScanOutcome, SessionInfo, SourceScanOutcome,
    TaskDetail, TaskResult, TaskStatus, UnifiedTask,
};

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::ClaudeSession;
use crate::task_scanner::claude_index::ClaudeSourceIndex;

/// Scan all task sources for a project and return a unified result.
///
/// Calls each tool-specific parser and aggregates tasks. If a parser fails,
/// its error is recorded but other parsers still contribute their results.
pub fn get_tasks_for_project(project_path: &str, sessions: &[ClaudeSession]) -> TaskResult {
    get_tasks_for_project_with_index(project_path, sessions, None)
}

/// Scan all task sources for a project and return a unified result, optionally
/// reusing a pre-built Claude source index for this scan cycle.
pub fn get_tasks_for_project_with_index(
    project_path: &str,
    sessions: &[ClaudeSession],
    claude_index: Option<&ClaudeSourceIndex>,
) -> TaskResult {
    get_tasks_for_project_with(
        project_path,
        sessions,
        claude::get_tasks_with_index,
        codex::get_tasks,
        gemini::get_tasks,
        claude_index,
    )
}

fn get_tasks_for_project_with<CF, XF, GF>(
    project_path: &str,
    sessions: &[ClaudeSession],
    get_claude_tasks: CF,
    get_codex_tasks: XF,
    get_gemini_tasks: GF,
    claude_index: Option<&ClaudeSourceIndex>,
) -> TaskResult
where
    CF: Fn(&str, &[&ClaudeSession], Option<&ClaudeSourceIndex>) -> ScanOutcome,
    XF: Fn(&str, &[&ClaudeSession]) -> ScanOutcome,
    GF: Fn(&str) -> ScanOutcome,
{
    let mut result = TaskResult::empty();

    // Claude: structured task JSON
    let claude_sessions: Vec<&ClaudeSession> = sessions
        .iter()
        .filter(|s| s.cli_tool == CliTool::Claude)
        .collect();
    apply_source_outcome(
        &mut result,
        "claude",
        get_claude_tasks(project_path, &claude_sessions, claude_index),
    );

    // Codex: update_plan from JSONL
    let codex_sessions: Vec<&ClaudeSession> = sessions
        .iter()
        .filter(|s| s.cli_tool == CliTool::Codex)
        .collect();
    apply_source_outcome(
        &mut result,
        "codex",
        get_codex_tasks(project_path, &codex_sessions),
    );

    // Gemini: TODO.md checkboxes (no session data needed)
    apply_source_outcome(&mut result, "gemini", get_gemini_tasks(project_path));

    result
}

fn apply_source_outcome(result: &mut TaskResult, source: &str, outcome: ScanOutcome) {
    if let ScanOutcome::Data(tasks) = &outcome {
        result.tasks.extend(tasks.iter().cloned());
    }
    if let ScanOutcome::Unavailable(reason) = &outcome {
        result.errors.push((source.to_string(), reason.clone()));
    }
    result.source_outcomes.push(SourceScanOutcome {
        source: source.to_string(),
        outcome,
    });
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
            |_project_path, _sessions, _index| ScanOutcome::DefinitivelyEmpty,
            |_project_path, _sessions| ScanOutcome::DefinitivelyEmpty,
            |_project_path| ScanOutcome::DefinitivelyEmpty,
            None,
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
                source_key: "source-1".to_string(),
                subject: "Test task".to_string(),
                description: None,
                active_form: None,
                status: TaskStatus::Pending,
                source: CliTool::Claude,
                blocks: vec![],
                blocked_by: vec![],
                owner: None,
                session_id: None,
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
            }],
            errors: vec![],
            source_outcomes: vec![],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["tasks"][0]["source"], "claude");
        assert_eq!(json["tasks"][0]["status"], "pending");
    }
}
