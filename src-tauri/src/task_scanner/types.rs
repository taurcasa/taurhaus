//! Unified task types for the compound task board.
//!
//! These types represent tasks from any supported CLI harness.
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
    Stale,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Stale => write!(f, "stale"),
            TaskStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// A task normalized from any CLI tool's native format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScannedTask {
    /// Tool-specific ID: "1", "codex-0", "todo-3".
    pub id: String,
    /// Source-directory/session key used for identity disambiguation.
    pub source_key: String,
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
    /// Latest status transition time (ISO 8601), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_changed_at: Option<String>,
    /// Last update/write timestamp from source persistence (ISO 8601), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Archive timestamp if task was archived (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    /// Last persisted status before archival.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    /// Reason code for archival.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_reason: Option<String>,
    /// Reasoning effort the lead attached when assigning this task.
    ///
    /// Written by `mesh task assign` into the task record's metadata. Absent
    /// for a task no lead assigned, and for every source that has no
    /// assignment contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Why the lead chose that level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort_why: Option<String>,
}

/// Backward-compatible alias used by existing command/frontend code.
pub type UnifiedTask = ScannedTask;

/// Tri-state scanner outcome for one tool source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ScanOutcome {
    /// Scanner successfully loaded task data for this source.
    Data(Vec<UnifiedTask>),
    /// Scanner successfully checked this source and found no tasks.
    DefinitivelyEmpty,
    /// Scanner could not reliably inspect this source (I/O, permissions, etc.).
    Unavailable(String),
}

/// One source's scan outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceScanOutcome {
    pub source: String,
    pub outcome: ScanOutcome,
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
    /// Per-source tri-state outcomes for pruning and reconciliation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_outcomes: Vec<SourceScanOutcome>,
}

impl TaskResult {
    /// Create an empty result.
    pub fn empty() -> Self {
        Self {
            tasks: vec![],
            errors: vec![],
            source_outcomes: vec![],
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

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: 04bda5ec added a task-status wire token without an unknown
    // landing pad, so a newer daemon status rejected the entire task result.
    #[test]
    fn unknown_task_status_decodes_without_rejecting_the_payload() {
        let status: TaskStatus =
            serde_json::from_str("\"a_future_status\"").expect("decode future task status");

        assert_eq!(status.to_string(), "unknown");
    }
}

/// A group of archived tasks from a single session, enriched with context.
///
/// Returned by the `get_archived_sessions` IPC command for the History sub-tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSession {
    /// Session UUID when available; null for tasks without a session grouping key.
    pub session_id: Option<String>,
    /// Account display label inferred from the transcript's owning config dir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
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
    /// Per-session enrichment warnings (timeline/commit lookup fallbacks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrichment_warnings: Vec<String>,
}

/// Result of querying archived sessions for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSessionsResult {
    /// Sessions sorted reverse-chronological (newest first).
    pub sessions: Vec<ArchivedSession>,
    /// Errors encountered during enrichment.
    pub errors: Vec<String>,
}
