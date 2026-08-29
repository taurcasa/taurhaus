mod ledger;
mod scanner;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use ledger::ledger_row;
pub use scanner::{read_run, scan_session_runs, workflow_activity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowRunStatus {
    Live,
    Completed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowAgentState {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAgent {
    pub agent_id: String,
    pub label: Option<String>,
    pub phase: Option<String>,
    pub model: Option<String>,
    pub state: WorkflowAgentState,
    pub prompt_preview: String,
    pub last_tool: Option<String>,
    pub tokens: Option<u64>,
    pub tool_calls: Option<u32>,
    pub last_write_at: i64,
    pub result_preview: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunTotals {
    pub agents: u32,
    pub done: u32,
    pub tokens: Option<u64>,
    pub tool_calls: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub run_id: String,
    pub name: String,
    pub description: String,
    pub phases: Vec<String>,
    pub status: WorkflowRunStatus,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub agents: Vec<WorkflowAgent>,
    pub totals: WorkflowRunTotals,
    pub result: Option<serde_json::Value>,
    pub script_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunSummary {
    pub run_id: String,
    pub name: String,
    pub description: String,
    pub phases: Vec<String>,
    pub status: WorkflowRunStatus,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub totals: WorkflowRunTotals,
    pub script_path: PathBuf,
}

impl From<&WorkflowRun> for WorkflowRunSummary {
    fn from(run: &WorkflowRun) -> Self {
        Self {
            run_id: run.run_id.clone(),
            name: run.name.clone(),
            description: run.description.clone(),
            phases: run.phases.clone(),
            status: run.status,
            started_at: run.started_at,
            finished_at: run.finished_at,
            totals: run.totals.clone(),
            script_path: run.script_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowActivity {
    pub live_runs: u32,
    pub last_write_at: i64,
}

pub(crate) fn activity_for_transcript(
    tool: crate::session_scanner::CliTool,
    transcript: Option<&str>,
    now: std::time::SystemTime,
) -> Option<WorkflowActivity> {
    if !crate::session_scanner::cli_tool::spec(tool)
        .capabilities
        .workflow_runs
    {
        return None;
    }
    let transcript = std::path::Path::new(transcript?);
    let session_dir = transcript.with_extension("");
    workflow_activity(&session_dir, now)
}

#[cfg(test)]
mod tests;
