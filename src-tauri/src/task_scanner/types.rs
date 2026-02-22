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
