mod ledger;
mod scanner;

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::daemon::protocol;
use crate::errors::{sanitize_error, CommandResultExt, IpcResult};
use crate::ProviderState;

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

#[tauri::command]
pub fn list_workflow_runs(
    provider: State<'_, ProviderState>,
    session_id: String,
) -> IpcResult<Vec<WorkflowRunSummary>> {
    let span = IpcCommandSpan::start("list_workflow_runs");
    let result =
        list_workflow_runs_impl(provider.inner(), &session_id).ipc_cmd("list_workflow_runs");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn get_workflow_run(
    provider: State<'_, ProviderState>,
    session_id: String,
    run_id: String,
) -> IpcResult<WorkflowRun> {
    let span = IpcCommandSpan::start("get_workflow_run");
    let result =
        get_workflow_run_impl(provider.inner(), &session_id, &run_id).ipc_cmd("get_workflow_run");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn workflow_ledger_row(
    provider: State<'_, ProviderState>,
    session_id: String,
    run_id: String,
) -> IpcResult<Option<String>> {
    let span = IpcCommandSpan::start("workflow_ledger_row");
    let result = workflow_ledger_row_impl(provider.inner(), &session_id, &run_id)
        .ipc_cmd("workflow_ledger_row");
    span.finish_result(&result);
    result
}

fn list_workflow_runs_impl(
    provider: &ProviderState,
    session_id: &str,
) -> Result<Vec<WorkflowRunSummary>, String> {
    if cfg!(target_os = "windows") {
        return daemon_request(
            provider,
            protocol::method::LIST_WORKFLOW_RUNS,
            protocol::WorkflowSessionParams {
                session_id: session_id.to_string(),
            },
            "workflow run list",
        );
    }
    list_runs_for_session_id(session_id)
}

fn get_workflow_run_impl(
    provider: &ProviderState,
    session_id: &str,
    run_id: &str,
) -> Result<WorkflowRun, String> {
    if cfg!(target_os = "windows") {
        return daemon_request(
            provider,
            protocol::method::GET_WORKFLOW_RUN,
            protocol::WorkflowRunParams {
                session_id: session_id.to_string(),
                run_id: run_id.to_string(),
            },
            "workflow run",
        );
    }
    get_run_for_session_id(session_id, run_id)
}

fn workflow_ledger_row_impl(
    provider: &ProviderState,
    session_id: &str,
    run_id: &str,
) -> Result<Option<String>, String> {
    get_workflow_run_impl(provider, session_id, run_id).map(|run| ledger_row(&run))
}

pub(crate) fn list_runs_for_session_id(
    session_id: &str,
) -> Result<Vec<WorkflowRunSummary>, String> {
    // A session that has no directory yet has no runs; only an invalid
    // identifier is an error.
    let session_dir = match find_session_dir(session_id) {
        Ok(dir) => dir,
        Err(error) if error.starts_with("Session not found") => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(scan_session_runs(&session_dir)
        .iter()
        .map(WorkflowRunSummary::from)
        .collect())
}

pub(crate) fn get_run_for_session_id(
    session_id: &str,
    run_id: &str,
) -> Result<WorkflowRun, String> {
    if !valid_identifier(run_id) {
        return Err("Invalid workflow run ID".to_string());
    }
    let session_dir = find_session_dir(session_id)?;
    read_run(&session_dir, run_id).ok_or_else(|| format!("Workflow run not found: {run_id}"))
}

fn find_session_dir(session_id: &str) -> Result<PathBuf, String> {
    if !valid_identifier(session_id) {
        return Err("Invalid session ID".to_string());
    }
    let tool = crate::session_scanner::cli_tool::all()
        .iter()
        .find(|entry| entry.capabilities.workflow_runs)
        .map(|entry| entry.tool)
        .ok_or_else(|| "No workflow-capable harness is registered".to_string())?;
    let projects_subdir = crate::session_scanner::cli_tool::spec(tool).projects_subdir;
    for config_dir in crate::session_scanner::accounts::transcript_dirs(tool) {
        let Ok(projects) = std::fs::read_dir(config_dir.join(projects_subdir)) else {
            continue;
        };
        for project in projects.flatten() {
            if !project
                .file_type()
                .is_ok_and(|file_type| file_type.is_dir())
            {
                continue;
            }
            let candidate = project.path().join(session_id);
            if std::fs::symlink_metadata(&candidate)
                .is_ok_and(|metadata| metadata.file_type().is_dir())
            {
                return Ok(candidate);
            }
        }
    }
    Err(format!("Session not found: {session_id}"))
}

fn daemon_request<T, P>(
    provider: &ProviderState,
    method: &'static str,
    params: P,
    what: &str,
) -> Result<T, String>
where
    T: DeserializeOwned,
    P: Serialize,
{
    let daemon = provider
        .daemon
        .as_ref()
        .ok_or_else(|| "The WSL daemon is not running".to_string())?;
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return Err("The WSL daemon is not reachable".to_string());
    }
    let request = protocol::DaemonRequest::new(
        format!("{method}-{}", uuid::Uuid::new_v4().simple()),
        method,
        params,
    );
    let response = daemon
        .send_status_request(&request)
        .map_err(|error| sanitize_error(&error.to_string()))?;
    if let Some(error) = response.error {
        if error.code == "UNKNOWN_METHOD" {
            return Err(format!(
                "The WSL daemon does not support {what}; update the bundled daemon"
            ));
        }
        return Err(sanitize_error(&error.message));
    }
    let value = response
        .result
        .ok_or_else(|| format!("The WSL daemon returned no {what}"))?;
    serde_json::from_value(value)
        .map_err(|error| format!("The WSL daemon returned an invalid {what}: {error}"))
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests;
