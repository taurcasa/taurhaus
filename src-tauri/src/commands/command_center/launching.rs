use crate::commands::accounts::{AccountsResult, TranscriptLookup};
use crate::commands::logging::LogFileState;
use crate::commands::terminal_settings::load_terminal_settings;
use crate::models::AccountMemoryOrigin;
use crate::session_scanner::accounts;
use crate::session_scanner::accounts::{AccountOrigin, AccountRequest, AccountResolution};
use crate::session_scanner::launch::{
    base_command, redact_command_for_logging, LaunchNote, LaunchSpec, ModelSpec,
};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn launch_cli_session_impl(
    db: &DbState,
    provider: &ProviderState,
    log_file: &LogFileState,
    coordination_state: Option<&CoordinationState>,
    project_id: String,
    mode: LaunchMode,
    cli_tool: Option<CliTool>,
    account_id: Option<String>,
) -> Result<protocol::LaunchSessionResult, String> {
    let tool = cli_tool.unwrap_or_default();

    let mut launch_fields = Map::new();
    launch_fields.insert("project_id".to_string(), Value::String(project_id.clone()));
    launch_fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
    launch_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
    launch_fields.insert(
        "caller".to_string(),
        Value::String("command_center.launch".to_string()),
    );
    log_file.emit(
        "info",
        "command_center",
        "command_center.launch.start",
        Some("Launching CLI session".to_string()),
        launch_fields,
    );

    let (project_path, project_account_memory) =
        resolve_project_launch_target(db, &project_id, tool)?;

    let linux_path = crate::provider::path::to_linux(&project_path).unwrap_or(project_path.clone());

    let mut path_fields = Map::new();
    path_fields.insert("db_path".to_string(), Value::String(project_path.clone()));
    path_fields.insert("linux_path".to_string(), Value::String(linux_path.clone()));
    path_fields.insert("project_id".to_string(), Value::String(project_id.clone()));
    log_file.emit(
        "debug",
        "command_center",
        "command_center.launch.path_resolved",
        Some("Resolved project path for launch".to_string()),
        path_fields,
    );

    if matches!(mode, LaunchMode::Continue | LaunchMode::Resume) {
        if let Some(coordination_state) = coordination_state {
            match find_unique_team_member_match(coordination_state.teams_dir(), &linux_path, tool) {
                TeamMemberMatchResult::Unique(target) => {
                    let mut delegated_fields = Map::new();
                    delegated_fields.insert(
                        "team_name".to_string(),
                        Value::String(target.team_name.clone()),
                    );
                    delegated_fields.insert(
                        "member_name".to_string(),
                        Value::String(target.member_name.clone()),
                    );
                    log_file.emit(
                        "info",
                        "command_center",
                        "command_center.launch.coordination_delegate",
                        Some("Delegating team-member resume to coordination pipeline".to_string()),
                        delegated_fields,
                    );
                    let mut result = delegate_launch_to_coordination_resume(
                        db,
                        coordination_state,
                        &target,
                        tool,
                    )?;
                    // The team's own config dir is what a member resumes in, so
                    // an account the user picked for this launch has nowhere to
                    // go. Per-team accounts are a follow-up; until then the
                    // launch says what it did instead of the pick.
                    if let Some(wanted) = requested_account(tool, account_id.as_deref()) {
                        note_team_account_ignored(&project_id, wanted);
                        result.account_applied = Some(false);
                        result.account_note = Some(TEAM_DEFAULT_ACCOUNT.to_string());
                    }
                    return Ok(result);
                }
                TeamMemberMatchResult::Ambiguous => {
                    let mut ambiguous_fields = Map::new();
                    ambiguous_fields.insert(
                        "project_path".to_string(),
                        Value::String(linux_path.clone()),
                    );
                    ambiguous_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.launch.team_match_ambiguous",
                        Some(
                            "Multiple team members matched generic resume; using raw launch"
                                .to_string(),
                        ),
                        ambiguous_fields,
                    );
                }
                TeamMemberMatchResult::None => {}
            }
        }
    }

    let terminal_settings = load_terminal_settings(db);
    let configured_base = base_command(&terminal_settings.cli_commands, tool, mode);
    let account = crate::session_scanner::cli_tool::spec(tool)
        .account_provider()
        .map(|_| {
            let launch = resolve_account(
                provider,
                &linux_path,
                tool,
                mode,
                ResolveAccountOptions {
                    requested_account_id: account_id.as_deref(),
                    project_memory: project_account_memory.as_ref(),
                    default_account_id: terminal_settings
                        .default_account_ids
                        .get(&tool.to_string())
                        .map(String::as_str),
                    base: configured_base,
                },
            );
            log_account_resolution(&project_id, tool, &launch);
            launch.resolution
        });
    let config_dir = account
        .as_ref()
        .and_then(|resolution| launch_account_dir(tool, resolution));
    let resolved_account_id = account
        .as_ref()
        .and_then(|resolution| resolution.account.as_ref())
        .map(|account| account.id.clone());
    // The session token is expanded after the account, not before: a harness
    // that scopes its whole history behind an account selector keeps a
    // different conversation in every home, so the launch has to look in the
    // one it is about to select.
    let base = resolved_launch_base(configured_base, tool, &linux_path, config_dir.as_deref())?;
    let rendered = LaunchSpec {
        tool,
        mode,
        base: base.as_ref(),
        model: ModelSpec::default(),
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
        account_dir: config_dir.as_deref(),
        selector: crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .account_selector,
        team: None,
    }
    .render();
    let tool_cmd = rendered.command;
    let mode_name = format!("{mode:?}").to_ascii_lowercase();
    let mut rendered_fields = Map::new();
    rendered_fields.insert("tool".to_string(), Value::String(tool.to_string()));
    rendered_fields.insert("mode".to_string(), Value::String(mode_name.clone()));
    rendered_fields.insert(
        "command".to_string(),
        Value::String(redact_command_for_logging(&tool_cmd)),
    );
    rendered_fields.insert(
        "account".to_string(),
        account
            .as_ref()
            .and_then(|resolution| resolution.account.as_ref())
            .map(|account| Value::String(account.identity.label.clone()))
            .unwrap_or(Value::Null),
    );
    crate::commands::logging::emit_global(
        "info",
        "command_center",
        "launch.command.rendered",
        Some("Rendered CLI launch command".to_string()),
        rendered_fields,
    );
    for note in rendered.notes {
        let event = note.event_name();
        let mut fields = Map::new();
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
        fields.insert("mode".to_string(), Value::String(mode_name.clone()));
        let message = match note {
            LaunchNote::CapabilityMissing { capability, found } => {
                fields.insert(
                    "capability".to_string(),
                    Value::String(capability.as_str().to_string()),
                );
                fields.insert("found".to_string(), Value::String(found));
                "Requested launch value has no declared harness capability"
            }
            LaunchNote::DeprecatedFlag { flag } => {
                fields.insert("flag".to_string(), Value::String(flag));
                "Configured launch base contains a deprecated flag"
            }
            LaunchNote::ModelIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides the requested model"
            }
            LaunchNote::NotifyIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides the managed Codex notifier"
            }
            LaunchNote::ModelDeprecated { found, replacement } => {
                fields.insert("found".to_string(), Value::String(found));
                fields.insert(
                    "replacement".to_string(),
                    replacement.map(Value::String).unwrap_or(Value::Null),
                );
                "Requested model is deprecated"
            }
            LaunchNote::EffortIgnored { found, .. } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base overrides or cannot use the requested reasoning effort"
            }
            LaunchNote::SelectorIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base selects its own account directory"
            }
        };
        crate::commands::logging::emit_global(
            "warn",
            "command_center",
            event,
            Some(message.to_string()),
            fields,
        );
    }

    if let Some(ref daemon) = provider.daemon {
        if daemon.is_connected() {
            let id = "launch-session";
            let daemon_method = protocol::method::LAUNCH_SESSION;
            let request = protocol::DaemonRequest::new(
                id,
                daemon_method,
                protocol::LaunchSessionParams {
                    project_path: linux_path.clone(),
                    mode,
                    cli_tool: tool,
                    tmux_layout: terminal_settings.tmux_layout.clone(),
                    command_override: Some(tool_cmd.clone()),
                },
            );
            let mut daemon_request_fields = Map::new();
            daemon_request_fields.insert(
                "caller".to_string(),
                Value::String("command_center.launch".to_string()),
            );
            daemon_request_fields.insert(
                "daemon_request_id".to_string(),
                Value::String(id.to_string()),
            );
            daemon_request_fields.insert(
                "daemon_method".to_string(),
                Value::String(daemon_method.to_string()),
            );
            daemon_request_fields
                .insert("project_id".to_string(), Value::String(project_id.clone()));
            daemon_request_fields.insert(
                "project_path".to_string(),
                Value::String(linux_path.clone()),
            );
            daemon_request_fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
            daemon_request_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
            log_file.emit(
                "info",
                "command_center",
                "command_center.launch.daemon_request",
                Some("Submitting launch request to daemon".to_string()),
                daemon_request_fields,
            );
            match daemon.send_status_request(&request) {
                Ok(response) if response.is_ok() => {
                    let result = decode_daemon_launch_result(response.result)?;

                    let mut success_fields = Map::new();
                    success_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.launch".to_string()),
                    );
                    success_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    success_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    success_fields
                        .insert("project_id".to_string(), Value::String(project_id.clone()));
                    success_fields.insert(
                        "project_path".to_string(),
                        Value::String(linux_path.clone()),
                    );
                    success_fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
                    success_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    if let Some(session) = result.tmux_session.as_ref() {
                        success_fields
                            .insert("tmux_session".to_string(), Value::String(session.clone()));
                    }
                    success_fields.insert(
                        "tmux_window".to_string(),
                        Value::String(result.tmux_window.clone()),
                    );
                    success_fields.insert(
                        "tmux_pane".to_string(),
                        Value::String(result.tmux_pane.clone()),
                    );
                    log_file.emit(
                        "info",
                        "command_center",
                        "command_center.launch.daemon_success",
                        Some("Launch succeeded via daemon".to_string()),
                        success_fields,
                    );

                    let tmux_session = result.tmux_session.as_deref().unwrap_or(TMUX_SESSION_NAME);
                    let _ = crate::terminal::handle_terminal(
                        crate::terminal::TerminalIntent::EnsureOpen {
                            distro: provider.wsl_distro.clone(),
                            tmux_session: tmux_session.to_string(),
                            emulator: terminal_settings.emulator,
                            custom_command: terminal_settings.custom_command,
                        },
                    );
                    remember_resolved_account(
                        db,
                        &project_id,
                        tool,
                        resolved_account_id.as_deref(),
                    );
                    return Ok(result);
                }
                Ok(response) => {
                    let msg = response
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    let mut fail_fields = Map::new();
                    fail_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.launch".to_string()),
                    );
                    fail_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    fail_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    fail_fields.insert("project_id".to_string(), Value::String(project_id.clone()));
                    fail_fields.insert(
                        "project_path".to_string(),
                        Value::String(linux_path.clone()),
                    );
                    fail_fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
                    fail_fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    fail_fields.insert("error".to_string(), Value::String(msg.clone()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.launch.daemon_failed",
                        Some("Launch failed via daemon".to_string()),
                        fail_fields,
                    );
                    return Err(format!("Failed to launch session: {msg}"));
                }
                Err(e) => {
                    let mut unreachable_fields = Map::new();
                    unreachable_fields.insert(
                        "caller".to_string(),
                        Value::String("command_center.launch".to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_request_id".to_string(),
                        Value::String(id.to_string()),
                    );
                    unreachable_fields.insert(
                        "daemon_method".to_string(),
                        Value::String(daemon_method.to_string()),
                    );
                    unreachable_fields
                        .insert("project_id".to_string(), Value::String(project_id.clone()));
                    unreachable_fields.insert(
                        "project_path".to_string(),
                        Value::String(linux_path.clone()),
                    );
                    unreachable_fields
                        .insert("mode".to_string(), Value::String(format!("{mode:?}")));
                    unreachable_fields
                        .insert("tool".to_string(), Value::String(format!("{tool:?}")));
                    unreachable_fields.insert("error".to_string(), Value::String(e.to_string()));
                    log_file.emit(
                        "warn",
                        "command_center",
                        "command_center.launch.daemon_unreachable",
                        Some("Daemon unreachable during launch".to_string()),
                        unreachable_fields,
                    );
                    tracing::warn!(error = %e, "Daemon unreachable for launch");
                }
            }
        }
    }

    log_file.emit(
        "info",
        "command_center",
        "command_center.launch.local_fallback",
        Some("Falling back to local tmux launch".to_string()),
        {
            let mut fields = Map::new();
            fields.insert(
                "caller".to_string(),
                Value::String("command_center.launch".to_string()),
            );
            fields.insert("project_id".to_string(), Value::String(project_id.clone()));
            fields.insert(
                "project_path".to_string(),
                Value::String(linux_path.clone()),
            );
            fields.insert("mode".to_string(), Value::String(format!("{mode:?}")));
            fields.insert("tool".to_string(), Value::String(format!("{tool:?}")));
            fields
        },
    );
    let (session, window, pane) = crate::session_scanner::control::launch_in_tmux_with_layout(
        &linux_path,
        mode,
        tool,
        &terminal_settings.tmux_layout,
        Some(&tool_cmd),
    )
    .map_err(|e| format!("Failed to launch session: {e}"))?;
    remember_resolved_account(db, &project_id, tool, resolved_account_id.as_deref());

    #[cfg(target_os = "macos")]
    {
        let _ = crate::terminal::handle_terminal(crate::terminal::TerminalIntent::EnsureOpen {
            distro: None,
            tmux_session: session.clone(),
            emulator: terminal_settings.emulator,
            custom_command: terminal_settings.custom_command,
        });
    }

    Ok(protocol::LaunchSessionResult {
        tmux_session: Some(session),
        tmux_window: window,
        tmux_pane: pane,
        ..Default::default()
    })
}

const SESSION_ID_PLACEHOLDER: &str = "{session_id}";

/// Resolve the Settings session token before the command crosses the daemon
/// boundary. The daemon deliberately executes command overrides verbatim.
///
/// `config_dir` is the account home this launch selected, so a harness whose
/// history is scoped by its account selector resolves the conversation from the
/// home the command will actually run against.
fn resolved_launch_base<'a>(
    base: &'a str,
    tool: CliTool,
    project_path: &str,
    config_dir: Option<&std::path::Path>,
) -> Result<Cow<'a, str>, String> {
    if !base.contains(SESSION_ID_PLACEHOLDER) {
        return Ok(Cow::Borrowed(base));
    }

    let session_id = crate::session_scanner::cli_tool::spec(tool)
        .session_resolver()
        .resume_session_id_in(project_path, config_dir)
        .ok_or_else(|| {
            format!(
                "No resumable {} conversation was found for this project",
                crate::session_scanner::cli_tool::spec(tool).label
            )
        })?;
    Ok(Cow::Owned(base.replace(
        SESSION_ID_PLACEHOLDER,
        &crate::session_scanner::launch::shell_escape(&session_id),
    )))
}

fn remember_resolved_account(
    db: &DbState,
    project_id: &str,
    tool: CliTool,
    account_id: Option<&str>,
) {
    remember_resolved_account_with(
        project_id,
        tool,
        account_id,
        |project_id, tool, account_id| {
            let connection = db.0.lock().map_err(|error| error.to_string())?;
            accounts::remember_last_used_in(&connection, project_id, tool, account_id)
        },
    );
}

pub(super) fn remember_resolved_account_with<F>(
    project_id: &str,
    tool: CliTool,
    account_id: Option<&str>,
    remember: F,
) where
    F: FnOnce(&str, CliTool, &str) -> Result<bool, String>,
{
    let Some(account_id) = account_id else {
        return;
    };
    if let Err(error) = remember(project_id, tool, account_id) {
        tracing::warn!(tool = %tool, error = %error, "failed to remember launched account");
    }
}

/// Detection could not run at all — no daemon, or one that never answered.
const DAEMON_UNAVAILABLE: &str = "daemon_unavailable";
/// The selected account is gone or signed out.
const ACCOUNT_UNAVAILABLE: &str = "account_unavailable";

/// The launch ran on a team's config dir, which is not anyone's subscription to
/// choose. A token, not a sentence: the frontend matches on it.
pub(super) const TEAM_DEFAULT_ACCOUNT: &str = "team_default";

/// The account this launch was explicitly asked to run on, if a tool that has
/// accounts was asked at all.
fn requested_account(tool: CliTool, account_id: Option<&str>) -> Option<&str> {
    crate::session_scanner::cli_tool::spec(tool).account_provider()?;
    account_id.map(str::trim).filter(|id| !id.is_empty())
}

/// Projects already told, this run, that a team resume runs on the team's
/// account.
static TEAM_ACCOUNT_NOTICES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Whether this project's team-account warning is the first of the run.
///
/// The menu offers the choice on every right-click, and a warning per click
/// says nothing the first one did not.
pub(super) fn first_team_account_notice(project_id: &str) -> bool {
    TEAM_ACCOUNT_NOTICES
        .get_or_init(Mutex::default)
        .lock()
        .map(|mut seen| seen.insert(project_id.to_string()))
        // A poisoned set has lost track of what was said; saying it again beats
        // swallowing the warning.
        .unwrap_or(true)
}

/// Say once that a team resume ignored the account the launch named.
fn note_team_account_ignored(project_id: &str, wanted: &str) {
    if !first_team_account_notice(project_id) {
        return;
    }
    let mut fields = Map::new();
    fields.insert(
        "project_id".to_string(),
        Value::String(project_id.to_string()),
    );
    fields.insert("wanted".to_string(), Value::String(wanted.to_string()));
    crate::commands::logging::emit_global(
        "warn",
        "command_center",
        "launch.account.ignored_for_team",
        Some(
            "Team resume runs on the team's config dir; the chosen account was not applied"
                .to_string(),
        ),
        fields,
    );
}

/// A launch's account, and what deciding it could not find out.
pub(super) struct LaunchAccount {
    pub resolution: AccountResolution,
    pub degraded: Option<DegradedDetection>,
}

/// Detection went unanswered and the launch went ahead anyway.
pub(super) struct DegradedDetection {
    pub reason: &'static str,
    /// The stored choice this launch could not honour — the answer it would
    /// have used had detection worked.
    pub wanted: Option<String>,
}

/// Resolve one provider-backed tool account for a launch.
struct ResolveAccountOptions<'a> {
    requested_account_id: Option<&'a str>,
    project_memory: Option<&'a crate::models::AccountMemory>,
    default_account_id: Option<&'a str>,
    base: &'a str,
}

fn resolve_account(
    provider: &ProviderState,
    linux_path: &str,
    tool: CliTool,
    mode: LaunchMode,
    options: ResolveAccountOptions<'_>,
) -> LaunchAccount {
    let ResolveAccountOptions {
        requested_account_id,
        project_memory,
        default_account_id,
        base,
    } = options;
    let asked_for_an_account = requested_account_id.is_some_and(|id| !id.trim().is_empty());
    let mut transcript = TranscriptLookup::default();
    if !asked_for_an_account && matches!(mode, LaunchMode::Continue | LaunchMode::Resume) {
        transcript = crate::commands::accounts::project_transcript(provider, tool, linux_path);
        if transcript.transcript.is_none() {
            transcript.transcript = accounts::remembered_transcript(tool, linux_path);
        }
    }

    let tool_spec = crate::session_scanner::cli_tool::spec(tool);
    let account_provider = tool_spec
        .account_provider()
        .expect("account resolver is called only for provider-backed tools");
    let detected = crate::commands::accounts::accounts_report(provider, tool);
    let pinned_account_id = project_memory
        .filter(|memory| memory.origin == AccountMemoryOrigin::Pinned)
        .map(|memory| memory.account_id.as_str());
    let last_used_account_id = project_memory
        .filter(|memory| memory.origin == AccountMemoryOrigin::LastUsed)
        .map(|memory| memory.account_id.as_str());
    decide_launch_account(
        &detected,
        &transcript,
        account_provider,
        AccountRequest {
            requested_account_id,
            session_transcript: transcript.transcript.as_deref(),
            pinned_account_id,
            last_used_account_id,
            default_account_id,
            base_command: Some(base),
            selector: tool_spec.capabilities.account_selector,
        },
    )
}

/// The account precedence, plus whether it was applied to a real answer.
///
/// An empty account list from a daemon that never replied is silence, and a
/// resume that falls through it lands on the config dir Claude Code reads on
/// its own — someone else's history. The launch still goes ahead; what it must
/// not do is go ahead quietly.
pub(super) fn decide_launch_account(
    detected: &AccountsResult,
    transcript: &TranscriptLookup,
    provider: &dyn accounts::AccountProvider,
    request: AccountRequest<'_>,
) -> LaunchAccount {
    let wanted = request
        .pinned_account_id
        .or(request.last_used_account_id)
        .or(request.default_account_id)
        .map(str::to_string);
    let resolution = accounts::resolve_launch_account(&detected.accounts, provider, request);
    // An explicit pick and a transcript both answer for themselves; only a
    // launch that fell through to the fallback lost something.
    let derived = matches!(
        resolution.origin,
        AccountOrigin::Request | AccountOrigin::Session
    );
    let unanswered = detected.degraded || transcript.unavailable.is_some();
    let degraded = (unanswered && !derived).then_some(DegradedDetection {
        reason: DAEMON_UNAVAILABLE,
        wanted,
    });
    LaunchAccount {
        resolution,
        degraded,
    }
}

/// Say in the log which subscription a launch ended up on, and why.
pub(super) fn log_account_resolution(project_id: &str, tool: CliTool, launch: &LaunchAccount) {
    let resolution = &launch.resolution;
    let used = || {
        resolution
            .account
            .as_ref()
            .map(|account| Value::String(account.identity.label.clone()))
            .unwrap_or(Value::Null)
    };

    if let Some(degraded) = launch.degraded.as_ref() {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
        fields.insert(
            "reason".to_string(),
            Value::String(degraded.reason.to_string()),
        );
        fields.insert(
            "wanted".to_string(),
            degraded
                .wanted
                .as_ref()
                .map(|id| Value::String(id.clone()))
                .unwrap_or(Value::Null),
        );
        fields.insert("used".to_string(), used());
        crate::commands::logging::emit_global(
            "warn",
            "command_center",
            "launch.account.fallback",
            Some("Account detection is unavailable; launching without it".to_string()),
            fields,
        );
    }

    if let Some(wanted) = resolution.fallback_from.as_deref() {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
        fields.insert(
            "reason".to_string(),
            Value::String(ACCOUNT_UNAVAILABLE.to_string()),
        );
        fields.insert("wanted".to_string(), Value::String(wanted.to_string()));
        fields.insert("used".to_string(), used());
        crate::commands::logging::emit_global(
            "warn",
            "command_center",
            "launch.account.fallback",
            Some("Selected account is unavailable; using the default".to_string()),
            fields,
        );
    }

    if resolution.origin == AccountOrigin::Session {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
        fields.insert(
            "account_dir".to_string(),
            resolution
                .account_dir
                .as_ref()
                .map(|dir| Value::String(dir.display().to_string()))
                .unwrap_or(Value::Null),
        );
        crate::commands::logging::emit_global(
            "info",
            "command_center",
            "launch.account.derived_from_session",
            Some("Resuming on the account that owns the session transcript".to_string()),
            fields,
        );
    }
}

/// Account dir rendered in the namespace of the launch shell.
fn launch_account_dir(tool: CliTool, resolution: &AccountResolution) -> Option<PathBuf> {
    let dir = resolution.account_dir.clone().or_else(|| {
        resolution
            .account
            .is_none()
            .then(|| accounts::configured_default_dir(tool))
            .flatten()
    })?;
    Some(accounts::to_launch_namespace(&dir))
}

/// What a launch would run on, without launching it.
///
/// The chooser exists for one question the frontend cannot answer on its own:
/// whether anything has already decided this launch's subscription. The
/// transcript of the project's last session is the interesting case — it decides
/// every resume, and only the backend can see it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAccountPreview {
    pub account_id: Option<String>,
    pub label: Option<String>,
    pub origin: String,
    /// Nothing decided the account and more than one subscription could run the
    /// launch: ask the user.
    pub needs_choice: bool,
}

pub(super) fn resolve_launch_account_preview_impl(
    db: &DbState,
    provider: &ProviderState,
    project_id: String,
    tool: CliTool,
    mode: LaunchMode,
    session_id: Option<&str>,
) -> Result<LaunchAccountPreview, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let project = crate::db::queries::get_project(&conn, &project_id)
        .sanitize_err()?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;
    drop(conn);

    let linux_path = crate::provider::path::to_linux(&project.path).unwrap_or(project.path);
    let terminal_settings = load_terminal_settings(db);
    let tool_spec = crate::session_scanner::cli_tool::spec(tool);
    let Some(account_provider) = tool_spec.account_provider() else {
        return Ok(LaunchAccountPreview {
            account_id: None,
            label: None,
            origin: AccountOrigin::DefaultConfigDir.as_str().to_string(),
            needs_choice: false,
        });
    };

    let explicit_transcript = session_id.and_then(|wanted| {
        crate::session_scanner::latest_compaction_runtime_sessions()
            .into_iter()
            .find(|session| {
                session.cli_tool == tool && session.session_id.as_deref() == Some(wanted)
            })
            .and_then(|session| session.jsonl_path)
            .map(PathBuf::from)
    });
    let transcript = if explicit_transcript.is_some() {
        TranscriptLookup {
            transcript: explicit_transcript,
            unavailable: None,
        }
    } else if matches!(mode, LaunchMode::Continue | LaunchMode::Resume) {
        let mut lookup = crate::commands::accounts::project_transcript(provider, tool, &linux_path);
        if lookup.transcript.is_none() {
            lookup.transcript = accounts::remembered_transcript(tool, &linux_path);
        }
        lookup
    } else {
        TranscriptLookup::default()
    };
    let memory = project.account_memory.get(&tool.to_string());
    let pinned_account_id = memory
        .filter(|memory| memory.origin == AccountMemoryOrigin::Pinned)
        .map(|memory| memory.account_id.as_str());
    let last_used_account_id = memory
        .filter(|memory| memory.origin == AccountMemoryOrigin::LastUsed)
        .map(|memory| memory.account_id.as_str());
    let base = base_command(&terminal_settings.cli_commands, tool, mode);
    let detected = crate::commands::accounts::accounts_report(provider, tool);

    let resolution = accounts::resolve_launch_account(
        &detected.accounts,
        account_provider,
        accounts::AccountRequest {
            session_transcript: transcript.transcript.as_deref(),
            pinned_account_id,
            last_used_account_id,
            default_account_id: terminal_settings
                .default_account_ids
                .get(&tool.to_string())
                .map(String::as_str),
            base_command: Some(base),
            selector: tool_spec.capabilities.account_selector,
            ..Default::default()
        },
    );

    Ok(LaunchAccountPreview {
        account_id: resolution
            .account
            .as_ref()
            .map(|account| account.id.clone()),
        label: resolution
            .account
            .as_ref()
            .map(|account| account.identity.label.clone()),
        origin: resolution.origin.as_str().to_string(),
        needs_choice: resolution.needs_choice,
    })
}

pub(super) fn decode_daemon_launch_result(
    payload: Option<serde_json::Value>,
) -> Result<protocol::LaunchSessionResult, String> {
    let value = payload.ok_or_else(|| "Invalid launch result from daemon".to_string())?;
    serde_json::from_value(value).map_err(|e| {
        tracing::warn!(error = %e, "Failed to deserialize launch result from daemon");
        format!("Invalid launch result from daemon: {e}")
    })
}
