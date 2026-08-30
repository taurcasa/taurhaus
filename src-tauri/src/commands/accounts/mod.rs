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
use std::time::Duration;

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

/// How long the daemon gets to say what a launch command means.
///
/// The daemon answers this one by running an interactive shell, so the request
/// has to outlive the resolution's own budget with room for the transport. A
/// request that expires first is indistinguishable from a daemon that cannot
/// resolve anything: the literal base comes back, the alias goes unseen, and
/// its selector overrides the account the operator chose — the defect this
/// resolution exists to fix.
const RESOLVE_LAUNCH_BASE_TIMEOUT: Duration =
    Duration::from_secs(launch_base::RESOLUTION_BUDGET.as_secs() + 4);

/// Detected accounts for one registry tool.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsResult {
    pub accounts: Vec<Account>,
    pub source: String,
    pub degraded: bool,
    pub error: Option<String>,
    /// Retained for wire compatibility and always empty: what the pane shell
    /// makes of the configured commands comes from the dedicated
    /// `resolve_launch_bases` command, never from this report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_bases: Vec<ResolvedBase>,
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

#[tauri::command(async)]
pub fn list_accounts(
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<AccountsResult> {
    let span = IpcCommandSpan::start("list_accounts");
    let result =
        Ok::<_, String>(list_accounts_impl(provider.inner(), tool)).ipc_cmd("list_accounts");
    span.finish_result(&result);
    result
}

/// The accounts read path never asks an interactive shell what a launch means.
pub(crate) fn list_accounts_impl(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    accounts_report(provider, tool)
}

/// Resolve the launch commands only for the Settings accounts surface.
#[tauri::command(async)]
pub fn resolve_launch_bases(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    tool: CliTool,
    force: Option<bool>,
) -> IpcResult<Vec<ResolvedBase>> {
    let span = IpcCommandSpan::start("resolve_launch_bases");
    let result = Ok::<_, String>(resolve_launch_bases_impl(
        db.inner(),
        provider.inner(),
        tool,
        force.unwrap_or(false),
    ))
    .ipc_cmd("resolve_launch_bases");
    span.finish_result(&result);
    result
}

pub(crate) fn resolve_launch_bases_impl(
    db: &DbState,
    provider: &ProviderState,
    tool: CliTool,
    force: bool,
) -> Vec<ResolvedBase> {
    if force && !cfg!(target_os = "windows") {
        launch_base::invalidate_base_command_cache();
    }
    let commands = crate::commands::terminal_settings::load_terminal_settings(db).cli_commands;
    let mut seen = std::collections::HashSet::new();
    let bases = [
        protocol::LaunchMode::Fresh,
        protocol::LaunchMode::Continue,
        protocol::LaunchMode::Resume,
    ]
    .into_iter()
    .map(|mode| crate::session_scanner::launch::base_command(&commands, tool, mode))
    .filter(|base| seen.insert(base.to_string()))
    .collect::<Vec<_>>();
    resolve_bases_threading_force(&bases, force, |base, force| {
        resolve_launch_base_with_force_tracked(provider, tool, base, force)
    })
}

/// Resolve each base in order, carrying a forced invalidation forward until
/// one resolution actually consumed it.
///
/// A fail-soft literal answer — daemon absent, unreachable, or an
/// unsupported/unavailable reply — never consumed the force: the cache that
/// answers never heard it, so the next base must still carry it.
fn resolve_bases_threading_force(
    bases: &[&str],
    force: bool,
    mut resolve: impl FnMut(&str, bool) -> (ResolvedBase, bool),
) -> Vec<ResolvedBase> {
    let mut force_pending = force;
    bases
        .iter()
        .map(|base| {
            let (resolved, consumed) = resolve(base, force_pending);
            if consumed {
                force_pending = false;
            }
            resolved
        })
        .collect()
}

#[tauri::command(async)]
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
        resolved_bases: Vec::new(),
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
    resolve_launch_base_with_force(provider, tool, base, false)
}

fn resolve_launch_base_with_force(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> ResolvedBase {
    resolve_launch_base_with_force_tracked(provider, tool, base, force).0
}

/// Resolves one base and says whether a forced invalidation was consumed —
/// which it never is by a fail-soft literal answer.
fn resolve_launch_base_with_force_tracked(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> (ResolvedBase, bool) {
    #[cfg(test)]
    if let Some(resolved) = test_resolution_probe(base) {
        return (resolved, true);
    }
    if cfg!(target_os = "windows") {
        return daemon_resolve_launch_base_tracked(provider, tool, base, force);
    }
    (
        launch_base::resolve_base_command_cached(
            base,
            tool,
            &launch_base::ShellAliasProbe::for_pane(),
        ),
        true,
    )
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct TestResolutionProbe {
    delay: Duration,
    calls: usize,
}

#[cfg(test)]
thread_local! {
    static TEST_RESOLUTION_PROBE: std::cell::RefCell<Option<TestResolutionProbe>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
pub(crate) struct TestResolutionProbeGuard;

#[cfg(test)]
impl TestResolutionProbeGuard {
    pub(crate) fn calls(&self) -> usize {
        TEST_RESOLUTION_PROBE.with(|probe| probe.borrow().map_or(0, |probe| probe.calls))
    }
}

#[cfg(test)]
impl Drop for TestResolutionProbeGuard {
    fn drop(&mut self) {
        TEST_RESOLUTION_PROBE.with(|probe| *probe.borrow_mut() = None);
    }
}

/// Install a counting, delayed stand-in for daemon/shell resolution on this test thread.
#[cfg(test)]
pub(crate) fn install_test_resolution_probe(delay: Duration) -> TestResolutionProbeGuard {
    TEST_RESOLUTION_PROBE.with(|probe| {
        *probe.borrow_mut() = Some(TestResolutionProbe { delay, calls: 0 });
    });
    TestResolutionProbeGuard
}

#[cfg(test)]
fn test_resolution_probe(base: &str) -> Option<ResolvedBase> {
    let delay = TEST_RESOLUTION_PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let probe = probe.as_mut()?;
        probe.calls += 1;
        Some(probe.delay)
    })?;
    std::thread::sleep(delay);
    Some(literal_base(base))
}

fn daemon_resolve_launch_base_tracked(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> (ResolvedBase, bool) {
    let Some(daemon) = provider.daemon.as_ref() else {
        return (literal_base(base), false);
    };
    if !daemon.is_connected() && !daemon.try_reconnect() {
        return (literal_base(base), false);
    }

    let request = protocol::DaemonRequest::new(
        format!("resolve-launch-base-{tool}"),
        protocol::method::RESOLVE_LAUNCH_BASE,
        protocol::ResolveLaunchBaseParams {
            tool,
            base: base.to_string(),
            force,
        },
    );
    let answer = daemon_answer(
        daemon.send_status_request_within(&request, RESOLVE_LAUNCH_BASE_TIMEOUT),
        "the resolved launch base",
    );
    let answered = matches!(answer, DaemonAnswer::Value(_));
    (resolved_base_from(answer, base), answered)
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
        resolved_bases: Vec::new(),
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
