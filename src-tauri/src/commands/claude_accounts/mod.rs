//! Claude subscription accounts, as the frontend sees them.
//!
//! Detection runs where the config dirs are: in-process on Linux and macOS, in
//! the WSL daemon on Windows. A daemon that predates the additive
//! `list_claude_accounts` method answers `UNKNOWN_METHOD`, and an empty list is
//! the honest answer — the chooser and the chip stay hidden, and launches keep
//! rendering exactly as they did before this feature existed.

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::daemon::protocol;
use crate::db::queries;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::session_scanner::claude_accounts::{
    detect_claude_accounts_cached, newest_project_transcript, transcript_config_dirs, ClaudeAccount,
};
use crate::ProviderState;

#[tauri::command]
pub fn list_claude_accounts(provider: State<'_, ProviderState>) -> IpcResult<Vec<ClaudeAccount>> {
    let span = IpcCommandSpan::start("list_claude_accounts");
    let result = Ok::<_, String>(claude_accounts(provider.inner())).ipc_cmd("list_claude_accounts");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn set_project_claude_account(
    db: State<'_, DbState>,
    project_id: String,
    account_id: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("set_project_claude_account");
    let result = set_project_claude_account_impl(db.inner(), &project_id, account_id.as_deref())
        .ipc_cmd("set_project_claude_account");
    span.finish_result(&result);
    result
}

pub(crate) fn set_project_claude_account_impl(
    db: &DbState,
    project_id: &str,
    account_id: Option<&str>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let changed =
        queries::set_project_claude_account(&conn, project_id, account_id).sanitize_err()?;
    if !changed {
        return Err("Project not found".to_string());
    }
    Ok(())
}

/// The detected accounts, from whichever side of the WSL boundary can see them.
pub(crate) fn claude_accounts(provider: &ProviderState) -> Vec<ClaudeAccount> {
    if cfg!(target_os = "windows") {
        return daemon_claude_accounts(provider).unwrap_or_default();
    }
    detect_claude_accounts_cached()
}

/// The newest Claude transcript for a project, read where the transcripts are.
///
/// This is what makes `--resume` land on the subscription that owns a project's
/// history after the session that wrote it is gone — including after a restart,
/// and on Windows, where the app never sees the sessions the daemon scans.
pub(crate) fn claude_project_transcript(
    provider: &ProviderState,
    project_path: &str,
) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return daemon_claude_project_transcript(provider, project_path);
    }
    // The config dirs, not the accounts: a `.claude.json` caught mid-rewrite
    // names no account, and the history it sits next to must not disappear
    // with it.
    newest_project_transcript(&transcript_config_dirs(), project_path)
}

fn daemon_claude_project_transcript(
    provider: &ProviderState,
    project_path: &str,
) -> Option<PathBuf> {
    let daemon = provider.daemon.as_ref()?;
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return None;
    }

    let request = protocol::DaemonRequest::new(
        "claude-project-transcript",
        protocol::method::CLAUDE_PROJECT_TRANSCRIPT,
        protocol::ClaudeProjectTranscriptParams {
            project_path: project_path.to_string(),
        },
    );
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => response
            .result
            .and_then(|value| {
                serde_json::from_value::<protocol::ClaudeProjectTranscriptResult>(value)
                    .map_err(|error| {
                        tracing::warn!(error = %error, "Failed to decode Claude transcript from daemon");
                    })
                    .ok()
            })
            .and_then(|result| result.transcript)
            .map(PathBuf::from),
        Ok(response) => {
            tracing::debug!(
                error = ?response.error,
                "Daemon does not resolve Claude transcripts; resume keeps the project's own choice"
            );
            None
        }
        Err(error) => {
            tracing::warn!(error = %error, "Daemon unreachable for Claude transcript lookup");
            None
        }
    }
}

fn daemon_claude_accounts(provider: &ProviderState) -> Option<Vec<ClaudeAccount>> {
    let daemon = provider.daemon.as_ref()?;
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return None;
    }

    let request = protocol::DaemonRequest::new(
        "list-claude-accounts",
        protocol::method::LIST_CLAUDE_ACCOUNTS,
        serde_json::Value::Null,
    );
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => response
            .result
            .and_then(|value| {
                serde_json::from_value::<protocol::ClaudeAccountsResult>(value)
                    .map_err(|error| {
                        tracing::warn!(error = %error, "Failed to decode Claude accounts from daemon");
                    })
                    .ok()
            })
            .map(|result| result.accounts),
        Ok(response) => {
            tracing::debug!(
                error = ?response.error,
                "Daemon does not report Claude accounts; treating as a single default account"
            );
            None
        }
        Err(error) => {
            tracing::warn!(error = %error, "Daemon unreachable for Claude account detection");
            None
        }
    }
}
