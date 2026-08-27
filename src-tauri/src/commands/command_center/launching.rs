use crate::commands::claude_accounts::{ClaudeAccountsResult, TranscriptLookup};
use crate::commands::logging::LogFileState;
use crate::commands::terminal_settings::load_terminal_settings;
use crate::session_scanner::claude_accounts::{
    configured_root_to_name, remembered_claude_transcript, resolve_launch_account,
    to_launch_namespace, AccountRequest, AccountResolution, AccountSource,
};
use crate::session_scanner::launch::{
    base_command, redact_command_for_logging, LaunchNote, LaunchSpec, ModelSpec,
};
use serde_json::{Map, Value};
use std::path::PathBuf;

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
    claude_account_id: Option<String>,
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

    let (project_path, project_account_id) = resolve_project_launch_target(db, &project_id)?;

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
                    return delegate_launch_to_coordination_resume(
                        db,
                        coordination_state,
                        &target,
                        tool,
                    );
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
    let account = crate::session_scanner::cli_tool::spec(tool)
        .capabilities
        .config_dir_env
        .is_some()
        .then(|| {
            let launch = resolve_claude_account(
                provider,
                &linux_path,
                mode,
                claude_account_id.as_deref(),
                project_account_id.as_deref(),
                terminal_settings.claude_default_account_id.as_deref(),
            );
            log_account_resolution(&project_id, &launch);
            launch.resolution
        });
    let config_dir = account.as_ref().and_then(launch_config_dir);
    let rendered = LaunchSpec {
        tool,
        mode,
        base: base_command(&terminal_settings.cli_commands, tool, mode),
        model: ModelSpec::default(),
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
        claude_config_dir: config_dir.as_deref(),
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
        "claude_account".to_string(),
        account
            .as_ref()
            .and_then(|resolution| resolution.account.as_ref())
            .map(|account| Value::String(account.email.clone()))
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
            LaunchNote::ConfigDirIgnored { found } => {
                fields.insert("found".to_string(), Value::String(found));
                "Configured launch base selects its own Claude config dir"
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
    })
}

/// Detection could not run at all — no daemon, or one that never answered.
const DAEMON_UNAVAILABLE: &str = "daemon_unavailable";
/// The selected account is gone or signed out.
const ACCOUNT_UNAVAILABLE: &str = "account_unavailable";

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

/// Pick the Claude subscription this launch runs on.
fn resolve_claude_account(
    provider: &ProviderState,
    linux_path: &str,
    mode: LaunchMode,
    requested_account_id: Option<&str>,
    project_account_id: Option<&str>,
    default_account_id: Option<&str>,
) -> LaunchAccount {
    // `--continue`/`--resume` only see the history of the config dir they run
    // in, so the session this project used last decides the account. The
    // transcripts on disk answer that even after a restart — and on Windows
    // they are the only answer, because the app never scans the sessions the
    // WSL daemon reports. A sighting from this process's own scans stands in
    // when the transcripts cannot be read (an older daemon, say).
    let asked_for_an_account = requested_account_id.is_some_and(|id| !id.trim().is_empty());
    let mut transcript = TranscriptLookup::default();
    if !asked_for_an_account && matches!(mode, LaunchMode::Continue | LaunchMode::Resume) {
        transcript =
            crate::commands::claude_accounts::claude_project_transcript(provider, linux_path);
        if transcript.transcript.is_none() {
            transcript.transcript = remembered_claude_transcript(linux_path);
        }
    }

    let accounts = crate::commands::claude_accounts::claude_accounts_report(provider);
    decide_launch_account(
        &accounts,
        &transcript,
        AccountRequest {
            requested_account_id,
            session_transcript: transcript.transcript.as_deref(),
            project_account_id,
            default_account_id,
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
    accounts: &ClaudeAccountsResult,
    transcript: &TranscriptLookup,
    request: AccountRequest<'_>,
) -> LaunchAccount {
    let wanted = request
        .project_account_id
        .or(request.default_account_id)
        .map(str::to_string);
    let resolution = resolve_launch_account(&accounts.accounts, request);
    // An explicit pick and a transcript both answer for themselves; only a
    // launch that fell through to the fallback lost something.
    let derived = matches!(
        resolution.source,
        AccountSource::Request | AccountSource::Session
    );
    let unanswered = accounts.degraded || transcript.unavailable.is_some();
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
pub(super) fn log_account_resolution(project_id: &str, launch: &LaunchAccount) {
    let resolution = &launch.resolution;
    let used = || {
        resolution
            .account
            .as_ref()
            .map(|account| Value::String(account.email.clone()))
            .unwrap_or(Value::Null)
    };

    if let Some(degraded) = launch.degraded.as_ref() {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
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
            Some("Claude account detection is unavailable; launching without it".to_string()),
            fields,
        );
    }

    if let Some(wanted) = resolution.fallback_from.as_deref() {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
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
            Some("Selected Claude account is unavailable; using the default".to_string()),
            fields,
        );
    }

    if resolution.source == AccountSource::Session {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
        fields.insert(
            "config_dir".to_string(),
            resolution
                .config_dir
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

/// The `CLAUDE_CONFIG_DIR` this launch renders, in the form the shell that runs
/// it will read.
fn launch_config_dir(resolution: &AccountResolution) -> Option<PathBuf> {
    let dir = resolution.config_dir.clone().or_else(|| {
        // Detection found no account at all — an isolated run (E2E) with an
        // empty Claude root, typically. The configured root is still where this
        // process was told to keep Claude state, and Claude Code reads only
        // `CLAUDE_CONFIG_DIR`: leaving it unset would send the launch into the
        // real `~/.claude`.
        resolution
            .account
            .is_none()
            .then(configured_root_to_name)
            .flatten()
    })?;
    Some(to_launch_namespace(&dir))
}

/// What a launch would run on, without launching it.
///
/// The chooser exists for one question the frontend cannot answer on its own:
/// whether anything has already decided this launch's subscription. The
/// transcript of the project's last session is the interesting case — it decides
/// every resume, and only the backend can see it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeLaunchAccount {
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub source: String,
    /// Nothing decided the account and more than one subscription could run the
    /// launch: ask the user.
    pub needs_choice: bool,
}

pub(super) fn resolve_claude_launch_account_impl(
    db: &DbState,
    provider: &ProviderState,
    project_id: String,
    mode: LaunchMode,
) -> Result<ClaudeLaunchAccount, String> {
    let (project_path, project_account_id) = resolve_project_launch_target(db, &project_id)?;
    let linux_path = crate::provider::path::to_linux(&project_path).unwrap_or(project_path);
    let terminal_settings = load_terminal_settings(db);

    let resolution = resolve_claude_account(
        provider,
        &linux_path,
        mode,
        None,
        project_account_id.as_deref(),
        terminal_settings.claude_default_account_id.as_deref(),
    )
    .resolution;

    Ok(ClaudeLaunchAccount {
        account_id: resolution
            .account
            .as_ref()
            .map(|account| account.id.clone()),
        email: resolution
            .account
            .as_ref()
            .map(|account| account.email.clone()),
        source: resolution.source.as_str().to_string(),
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
