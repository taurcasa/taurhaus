//! Claude subscription accounts, as the frontend sees them.
//!
//! Detection runs where the config dirs are: in-process on Linux and macOS, in
//! the WSL daemon on Windows. Three answers come back from that daemon and they
//! mean different things. A daemon that predates the additive
//! `list_claude_accounts` method answers `UNKNOWN_METHOD`, and an empty list is
//! then the honest answer — the chooser and the chip stay hidden, and launches
//! keep rendering exactly as they did before this feature existed. A daemon
//! that is *gone* has answered nothing at all: that empty list is silence, it
//! is reported as degraded, and the frontend keeps showing the last accounts it
//! knew rather than pretending the subscriptions vanished.

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use serde::de::DeserializeOwned;
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::daemon::protocol;
use crate::db::queries;
use crate::errors::{sanitize_error, AppError, CommandResultExt, IpcResult, SanitizeErr};
use crate::session_scanner::claude_accounts::{
    detect_claude_accounts_cached, newest_project_transcript, transcript_config_dirs, ClaudeAccount,
};
use crate::ProviderState;

/// Detection ran in this process.
pub(crate) const SOURCE_NATIVE: &str = "native";
/// Detection ran in the WSL daemon, where the config dirs are.
pub(crate) const SOURCE_DAEMON: &str = "daemon";

const UNKNOWN_METHOD: &str = "UNKNOWN_METHOD";

/// The detected accounts, and whether they are an answer at all.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountsResult {
    pub accounts: Vec<ClaudeAccount>,
    /// Where detection ran: `daemon` on Windows, `native` everywhere else.
    pub source: String,
    /// Detection could not run. `accounts` is empty because nothing answered,
    /// not because nobody is signed in.
    pub degraded: bool,
    pub error: Option<String>,
}

/// The transcript that owns a project's history, and whether the lookup ran.
///
/// A resume that falls through an *unavailable* lookup is not choosing an
/// account; it is proceeding without the one fact that decides one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptLookup {
    pub transcript: Option<PathBuf>,
    pub unavailable: Option<String>,
}

/// One daemon answer, told apart by what it means.
pub(crate) enum DaemonAnswer<T> {
    Value(T),
    /// A daemon built before the method existed. Nothing is the right answer.
    Unsupported,
    /// Nothing answered: no daemon, a dropped connection, a timeout, or a
    /// payload this build cannot read.
    Unavailable(String),
}

#[tauri::command]
pub fn list_claude_accounts(provider: State<'_, ProviderState>) -> IpcResult<ClaudeAccountsResult> {
    let span = IpcCommandSpan::start("list_claude_accounts");
    let result =
        Ok::<_, String>(claude_accounts_report(provider.inner())).ipc_cmd("list_claude_accounts");
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
pub(crate) fn claude_accounts_report(provider: &ProviderState) -> ClaudeAccountsResult {
    if cfg!(target_os = "windows") {
        return daemon_accounts_report(provider);
    }
    let mut accounts = detect_claude_accounts_cached();
    // Detection is cached for a minute; usage is read fresh, because the whole
    // point of it is to be current when the chooser opens.
    crate::daemon::claude_usage::attach_usage(&mut accounts);
    ClaudeAccountsResult {
        accounts,
        source: SOURCE_NATIVE.to_string(),
        degraded: false,
        error: None,
    }
}

/// The newest Claude transcript for a project, read where the transcripts are.
///
/// This is what makes `--resume` land on the subscription that owns a project's
/// history after the session that wrote it is gone — including after a restart,
/// and on Windows, where the app never sees the sessions the daemon scans.
pub(crate) fn claude_project_transcript(
    provider: &ProviderState,
    project_path: &str,
) -> TranscriptLookup {
    if cfg!(target_os = "windows") {
        return daemon_transcript_lookup(provider, project_path);
    }
    // The config dirs, not the accounts: a `.claude.json` caught mid-rewrite
    // names no account, and the history it sits next to must not disappear
    // with it.
    TranscriptLookup {
        transcript: newest_project_transcript(&transcript_config_dirs(), project_path),
        unavailable: None,
    }
}

fn daemon_accounts_report(provider: &ProviderState) -> ClaudeAccountsResult {
    daemon_accounts_report_from(daemon_claude_accounts(provider))
}

fn daemon_accounts_report_from(
    answer: DaemonAnswer<protocol::ClaudeAccountsResult>,
) -> ClaudeAccountsResult {
    let (accounts, degraded, error) = match answer {
        DaemonAnswer::Value(result) => (result.accounts, false, None),
        DaemonAnswer::Unsupported => (Vec::new(), false, None),
        DaemonAnswer::Unavailable(error) => (Vec::new(), true, Some(error)),
    };
    ClaudeAccountsResult {
        accounts,
        source: SOURCE_DAEMON.to_string(),
        degraded,
        error,
    }
}

fn daemon_transcript_lookup(provider: &ProviderState, project_path: &str) -> TranscriptLookup {
    let Some(daemon) = provider.daemon.as_ref() else {
        return TranscriptLookup {
            transcript: None,
            unavailable: Some("The WSL daemon is not running".to_string()),
        };
    };
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return TranscriptLookup {
            transcript: None,
            unavailable: Some("The WSL daemon is not reachable".to_string()),
        };
    }

    let request = protocol::DaemonRequest::new(
        "claude-project-transcript",
        protocol::method::CLAUDE_PROJECT_TRANSCRIPT,
        protocol::ClaudeProjectTranscriptParams {
            project_path: project_path.to_string(),
        },
    );
    transcript_lookup_from(daemon_answer(
        daemon.send_status_request(&request),
        "Claude transcript",
    ))
}

fn transcript_lookup_from(
    answer: DaemonAnswer<protocol::ClaudeProjectTranscriptResult>,
) -> TranscriptLookup {
    match answer {
        DaemonAnswer::Value(result) => TranscriptLookup {
            transcript: result.transcript.map(PathBuf::from),
            unavailable: None,
        },
        // A daemon that cannot resolve transcripts never could: the resume
        // keeps the project's own choice, exactly as it did before.
        DaemonAnswer::Unsupported => TranscriptLookup::default(),
        DaemonAnswer::Unavailable(error) => TranscriptLookup {
            transcript: None,
            unavailable: Some(error),
        },
    }
}

fn daemon_claude_accounts(
    provider: &ProviderState,
) -> DaemonAnswer<protocol::ClaudeAccountsResult> {
    let Some(daemon) = provider.daemon.as_ref() else {
        return DaemonAnswer::Unavailable("The WSL daemon is not running".to_string());
    };
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return DaemonAnswer::Unavailable("The WSL daemon is not reachable".to_string());
    }

    let request = protocol::DaemonRequest::new(
        "list-claude-accounts",
        protocol::method::LIST_CLAUDE_ACCOUNTS,
        serde_json::Value::Null,
    );
    daemon_answer(daemon.send_status_request(&request), "Claude accounts")
}

/// Read one daemon response for what it means, not just for what it carries.
fn daemon_answer<T: DeserializeOwned>(
    outcome: Result<protocol::DaemonResponse, AppError>,
    what: &str,
) -> DaemonAnswer<T> {
    match outcome {
        Ok(response) if response.is_ok() => match response.result {
            Some(value) => match serde_json::from_value::<T>(value) {
                Ok(decoded) => DaemonAnswer::Value(decoded),
                Err(error) => {
                    tracing::warn!(error = %error, what, "Failed to decode daemon answer");
                    DaemonAnswer::Unavailable(sanitize_error(&format!(
                        "The daemon sent {what} this build cannot read: {error}"
                    )))
                }
            },
            None => DaemonAnswer::Unavailable(format!("The daemon returned no {what}")),
        },
        Ok(response) => {
            let error = response.error.unwrap_or(protocol::DaemonError {
                code: "DAEMON_ERROR".to_string(),
                message: format!("The daemon could not report {what}"),
            });
            if error.code == UNKNOWN_METHOD {
                tracing::debug!(
                    what,
                    "Daemon predates this method; treating the empty answer as honest"
                );
                return DaemonAnswer::Unsupported;
            }
            tracing::warn!(code = %error.code, message = %error.message, what, "Daemon error");
            DaemonAnswer::Unavailable(sanitize_error(&error.message))
        }
        Err(error) => {
            tracing::warn!(error = %error, what, "Daemon unreachable");
            DaemonAnswer::Unavailable(sanitize_error(&error.to_string()))
        }
    }
}
