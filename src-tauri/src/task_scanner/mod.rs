//! Task scanner — reads task data from harnesses with verified task sources.
//!
//! Each tool tracks tasks differently:
//! - **Claude Code**: Structured JSON at `~/.claude/tasks/{session-id}/*.json`
//! - **Codex CLI**: `update_plan` function calls in session JSONL files
//!
//! The `get_tasks_for_project()` orchestrator calls the registered parsers and
//! aggregates results. Partial failures are collected as errors — one source
//! failing doesn't prevent others from returning.

pub mod claude;
pub mod claude_index;
pub mod codex;
pub mod types;

pub use claude::TranscriptParser;
pub use types::{
    ArchivedSession, ArchivedSessionsResult, ScanOutcome, SessionInfo, SourceScanOutcome,
    TaskDetail, TaskResult, TaskStatus, UnifiedTask,
};

use crate::session_scanner::cli_tool::{all, CliToolSpec};
use crate::session_scanner::RuntimeSession;
use crate::task_scanner::claude_index::ClaudeSourceIndex;

/// Scan all task sources for a project and return a unified result.
///
/// Calls each tool-specific parser and aggregates tasks. If a parser fails,
/// its error is recorded but other parsers still contribute their results.
pub fn get_tasks_for_project(project_path: &str, sessions: &[RuntimeSession]) -> TaskResult {
    get_tasks_for_project_with_index(project_path, sessions, None)
}

/// Scan all task sources for a project and return a unified result, optionally
/// reusing a pre-built Claude source index for this scan cycle.
pub fn get_tasks_for_project_with_index(
    project_path: &str,
    sessions: &[RuntimeSession],
    claude_index: Option<&ClaudeSourceIndex>,
) -> TaskResult {
    get_tasks_for_project_with(
        project_path,
        sessions,
        |entry, tool_sessions, index| {
            entry
                .transcript_parser()
                .map(|parser| parser.get_tasks(project_path, tool_sessions, index))
                .unwrap_or(ScanOutcome::DefinitivelyEmpty)
        },
        claude_index,
    )
}

fn get_tasks_for_project_with<TF>(
    _project_path: &str,
    sessions: &[RuntimeSession],
    mut get_transcript_tasks: TF,
    claude_index: Option<&ClaudeSourceIndex>,
) -> TaskResult
where
    TF: FnMut(&CliToolSpec, &[&RuntimeSession], Option<&ClaudeSourceIndex>) -> ScanOutcome,
{
    let mut result = TaskResult::empty();

    for entry in all() {
        if entry.transcript_parser().is_none() {
            continue;
        }
        let tool_sessions = sessions
            .iter()
            .filter(|session| session.cli_tool == entry.tool)
            .collect::<Vec<_>>();
        apply_source_outcome(
            &mut result,
            entry.name,
            get_transcript_tasks(entry, &tool_sessions, claude_index),
        );
    }

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
    use crate::session_scanner::cli_tool::CliTool;

    #[test]
    fn empty_sessions_returns_empty_result() {
        // Regression: e17f3eb deleted the injected scanner seam, making this
        // unit test read live ~/.claude* and walk ~/.codex/sessions.
        let sessions: Vec<RuntimeSession> = Vec::new();
        let result = get_tasks_for_project_with(
            "/nonexistent/task-scanner-test-project",
            &sessions,
            |_entry, _sessions, _index| ScanOutcome::DefinitivelyEmpty,
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
        assert_eq!(
            serde_json::to_string(&TaskStatus::Stale).unwrap(),
            "\"stale\""
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
                effort: None,
                effort_why: None,
            }],
            errors: vec![],
            source_outcomes: vec![],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["tasks"][0]["source"], "claude");
        assert_eq!(json["tasks"][0]["status"], "pending");
    }
}
