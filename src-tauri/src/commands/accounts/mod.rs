//! Tool accounts, as the frontend sees them.
//!
//! Detection runs where the config dirs are: in-process on Linux and macOS, in
//! the WSL daemon on Windows. Three answers come back from that daemon and they
//! mean different things. A daemon that predates the additive
//! `list_accounts` method answers `UNKNOWN_METHOD`, and an empty list is
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
use crate::session_scanner::accounts::{self, Account};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch_base::{self, ResolvedBase};
use crate::ProviderState;

/// Detection ran in this process.
pub(crate) const SOURCE_NATIVE: &str = "native";
/// Detection ran in the WSL daemon, where the config dirs are.
pub(crate) const SOURCE_DAEMON: &str = "daemon";

const UNKNOWN_METHOD: &str = "UNKNOWN_METHOD";

/// Detected accounts for one registry tool.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsResult {
    pub accounts: Vec<Account>,
    pub source: String,
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
pub fn list_accounts(
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<AccountsResult> {
    let span = IpcCommandSpan::start("list_accounts");
    let result = Ok::<_, String>(accounts_report(provider.inner(), tool)).ipc_cmd("list_accounts");
    span.finish_result(&result);
    result
}

#[tauri::command]
pub fn refresh_accounts_usage(
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<bool> {
    let span = IpcCommandSpan::start("refresh_accounts_usage");
    let result =
        refresh_accounts_usage_impl(provider.inner(), tool).ipc_cmd("refresh_accounts_usage");
    span.finish_result(&result);
    result
}

fn refresh_accounts_usage_impl(provider: &ProviderState, tool: CliTool) -> Result<bool, String> {
    if cfg!(target_os = "windows") {
        let Some(daemon) = provider.daemon.as_ref() else {
            return Ok(false);
        };
        let request = protocol::DaemonRequest::new(
            format!("refresh-usage-{tool}"),
            protocol::method::REFRESH_USAGE,
            protocol::ListAccountsParams { tool },
        );
        return Ok(daemon.send_status_request(&request).is_ok());
    }
    Ok(crate::daemon::usage_poller::refresh(tool))
}

/// Detected accounts, from whichever side of the WSL boundary can see them.
pub(crate) fn accounts_report(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    if cfg!(target_os = "windows") {
        return daemon_accounts_report(provider, tool);
    }
    let mut detected = accounts::detect(tool);
    crate::daemon::usage_poller::attach_usage(tool, &mut detected);
    AccountsResult {
        accounts: detected,
        source: SOURCE_NATIVE.to_string(),
        degraded: false,
        error: None,
    }
}

#[tauri::command]
pub fn set_project_account(
    db: State<'_, DbState>,
    project_id: String,
    tool: CliTool,
    account_id: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("set_project_account");
    let result = set_project_account_impl(db.inner(), &project_id, tool, account_id.as_deref())
        .ipc_cmd("set_project_account");
    span.finish_result(&result);
    result
}

pub(crate) fn set_project_account_impl(
    db: &DbState,
    project_id: &str,
    tool: CliTool,
    account_id: Option<&str>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let changed = queries::set_project_account(&conn, project_id, &tool.to_string(), account_id)
        .sanitize_err()?;
    if !changed {
        return Err("Project not found".to_string());
    }
    Ok(())
}

/// The newest transcript for one tool and project, read where that tool runs.
pub(crate) fn project_transcript(
    provider: &ProviderState,
    tool: CliTool,
    project_path: &str,
) -> TranscriptLookup {
    if cfg!(target_os = "windows") {
        return daemon_project_transcript_lookup(provider, tool, project_path);
    }
    TranscriptLookup {
        transcript: accounts::newest_project_transcript(
            tool,
            &accounts::transcript_dirs(tool),
            project_path,
        ),
        unavailable: None,
    }
}

/// What the shell that will run this launch makes of its base command.
///
/// Where the config dirs are, the aliases are: in-process on Linux and macOS,
/// in the WSL daemon on Windows. Every failure — no daemon, an older daemon, a
/// shell that never answered — leaves the base exactly as configured.
pub(crate) fn resolve_launch_base(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
) -> ResolvedBase {
    if cfg!(target_os = "windows") {
        return daemon_resolve_launch_base(provider, tool, base);
    }
    launch_base::resolve_base_command_cached(base, tool, &launch_base::ShellAliasProbe::for_pane())
}

fn daemon_resolve_launch_base(provider: &ProviderState, tool: CliTool, base: &str) -> ResolvedBase {
    let Some(daemon) = provider.daemon.as_ref() else {
        return literal_base(base);
    };
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return literal_base(base);
    }

    let request = protocol::DaemonRequest::new(
        format!("resolve-launch-base-{tool}"),
        protocol::method::RESOLVE_LAUNCH_BASE,
        protocol::ResolveLaunchBaseParams {
            tool,
            base: base.to_string(),
        },
    );
    resolved_base_from(
        daemon_answer(
            daemon.send_status_request(&request),
            "the resolved launch base",
        ),
        base,
    )
}

fn resolved_base_from(answer: DaemonAnswer<ResolvedBase>, base: &str) -> ResolvedBase {
    match answer {
        DaemonAnswer::Value(resolved) => resolved,
        DaemonAnswer::Unsupported | DaemonAnswer::Unavailable(_) => literal_base(base),
    }
}

/// The base command as configured: no expansion, nothing claimed about it.
fn literal_base(base: &str) -> ResolvedBase {
    ResolvedBase {
        command: base.to_string(),
        expansions: Vec::new(),
        opaque_head: None,
    }
}

fn daemon_accounts_report(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    daemon_accounts_report_from(daemon_accounts(provider, tool))
}

fn daemon_accounts_report_from(answer: DaemonAnswer<protocol::AccountsResult>) -> AccountsResult {
    let (accounts, degraded, error) = match answer {
        DaemonAnswer::Value(result) => (result.accounts, result.degraded, result.error),
        DaemonAnswer::Unsupported => (Vec::new(), false, None),
        DaemonAnswer::Unavailable(error) => (Vec::new(), true, Some(error)),
    };
    AccountsResult {
        accounts,
        source: SOURCE_DAEMON.to_string(),
        degraded,
        error,
    }
}

fn daemon_project_transcript_lookup(
    provider: &ProviderState,
    tool: CliTool,
    project_path: &str,
) -> TranscriptLookup {
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
        format!("project-transcript-{tool}"),
        protocol::method::PROJECT_TRANSCRIPT,
        protocol::ProjectTranscriptParams {
            tool,
            project: project_path.to_string(),
        },
    );
    transcript_lookup_from(daemon_answer(
        daemon.send_status_request(&request),
        "Project transcript",
    ))
}

fn transcript_lookup_from(
    answer: DaemonAnswer<protocol::ProjectTranscriptResult>,
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

fn daemon_accounts(
    provider: &ProviderState,
    tool: CliTool,
) -> DaemonAnswer<protocol::AccountsResult> {
    let Some(daemon) = provider.daemon.as_ref() else {
        return DaemonAnswer::Unavailable("The WSL daemon is not running".to_string());
    };
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return DaemonAnswer::Unavailable("The WSL daemon is not reachable".to_string());
    }

    let request = protocol::DaemonRequest::new(
        format!("list-accounts-{tool}"),
        protocol::method::LIST_ACCOUNTS,
        protocol::ListAccountsParams { tool },
    );
    daemon_answer(daemon.send_status_request(&request), "tool accounts")
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
