//! Unified task types for the compound task board.
//!
//! These types represent tasks from any CLI tool (Claude, Codex, Gemini)
//! in a normalized format suitable for frontend display.

use crate::session_scanner::cli_tool::CliTool;
use serde::{Deserialize, Serialize};

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
        }
    }
}

/// A task normalized from any CLI tool's native format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedTask {
    /// Tool-specific ID: "1", "codex-0", "todo-3".
    pub id: String,
    /// Short task description.
    pub subject: String,
    /// Longer description (Claude only).
    pub description: Option<String>,
    /// Present continuous form shown while in progress (Claude only).
    pub active_form: Option<String>,
    /// Task status.
    pub status: TaskStatus,
    /// Which CLI tool this task came from.
    pub source: CliTool,
    /// IDs of tasks this one blocks (Claude only).
    pub blocks: Vec<String>,
    /// IDs of tasks that block this one (Claude only).
    pub blocked_by: Vec<String>,
    /// Agent name for team tasks (Claude only).
    pub owner: Option<String>,
    /// Session UUID that created this task (Claude only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Result of scanning tasks for a project.
///
/// Supports partial results: if one source fails, the others still return.
/// Errors are collected per-source so the frontend can show targeted warnings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    /// All tasks from all sources, combined.
    pub tasks: Vec<UnifiedTask>,
    /// Per-source errors: (source_name, error_message).
    pub errors: Vec<(String, String)>,
}

impl TaskResult {
    /// Create an empty result.
    pub fn empty() -> Self {
        Self {
            tasks: vec![],
            errors: vec![],
        }
    }
}

/// Enriched task detail with session context, commits, and files changed.
///
/// Returned by the `get_task_detail` IPC command when the user clicks a task card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    /// Full task data.
    pub task: UnifiedTask,
    /// Session info (if a session_id was associated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionInfo>,
    /// Commits made during the session window.
    pub commits: Vec<crate::models::Commit>,
    /// Deduplicated file paths changed during the session window.
    pub files_changed: Vec<String>,
}

/// Lightweight session metadata for the task detail panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub started_at: String,
    pub ended_at: String,
}

/// A group of archived tasks from a single session, enriched with context.
///
/// Returned by the `get_archived_sessions` IPC command for the History sub-tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSession {
    /// Session UUID (or "ungrouped" for tasks without a session_id).
    pub session_id: String,
    /// Session start time (ISO 8601).
    pub started_at: Option<String>,
    /// Session end time (ISO 8601).
    pub ended_at: Option<String>,
    /// Session duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Archived tasks in this session.
    pub tasks: Vec<UnifiedTask>,
    /// Number of commits made during the session window.
    pub commit_count: usize,
    /// Number of files changed during the session window.
    pub file_count: usize,
    /// Which CLI tools contributed tasks to this session.
    pub sources: Vec<String>,
    /// When tasks were most recently archived into this session group (ISO 8601).
    pub last_archived_at: Option<String>,
}

/// Result of querying archived sessions for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSessionsResult {
    /// Sessions sorted reverse-chronological (newest first).
    pub sessions: Vec<ArchivedSession>,
    /// Errors encountered during enrichment.
    pub errors: Vec<String>,
}
