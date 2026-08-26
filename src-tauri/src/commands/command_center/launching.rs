use std::path::PathBuf;

use crate::commands::logging::LogFileState;
use crate::commands::terminal_settings::load_terminal_settings;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::claude_accounts::{
    resolve_launch_account, AccountRequest, AccountResolution, AccountSource,
};
use crate::session_scanner::launch::{
    base_command, redact_command_for_logging, LaunchNote, LaunchSpec, ModelSpec,
};
use serde_json::{Map, Value};

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
    let tool = cli_tool.unwrap_or(CliTool::Claude);

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
    let account = (tool == CliTool::Claude).then(|| {
        resolve_claude_account(
            provider,
            &project_id,
            &linux_path,
            mode,
            claude_account_id.as_deref(),
            project_account_id.as_deref(),
            terminal_settings.claude_default_account_id.as_deref(),
        )
    });
    let rendered = LaunchSpec {
        tool,
        mode,
        base: base_command(&terminal_settings.cli_commands, tool, mode),
        model: ModelSpec::default(),
        codex_bypass_hook_trust: false,
        codex_notify_executable: None,
        claude_config_dir: account
            .as_ref()
            .and_then(|resolution| resolution.config_dir.as_deref()),
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

/// Pick the Claude subscription this launch runs on, and say so in the log.
#[allow(clippy::too_many_arguments)]
fn resolve_claude_account(
    provider: &ProviderState,
    project_id: &str,
    linux_path: &str,
    mode: LaunchMode,
    requested_account_id: Option<&str>,
    project_account_id: Option<&str>,
    default_account_id: Option<&str>,
) -> AccountResolution {
    // `--continue`/`--resume` only see the history of the config dir they run
    // in, so a session already known for this project decides the account.
    let session_transcript = matches!(mode, LaunchMode::Continue | LaunchMode::Resume)
        .then(|| live_session_transcript(linux_path))
        .flatten();

    let accounts = crate::commands::claude_accounts::claude_accounts(provider);
    let resolution = resolve_launch_account(
        &accounts,
        &PlatformPaths::claude_dir(),
        AccountRequest {
            requested_account_id,
            session_transcript: session_transcript.as_deref(),
            project_account_id,
            default_account_id,
        },
    );

    if let Some(wanted) = resolution.fallback_from.as_deref() {
        let mut fields = Map::new();
        fields.insert("project".to_string(), Value::String(project_id.to_string()));
        fields.insert("wanted".to_string(), Value::String(wanted.to_string()));
        fields.insert(
            "used".to_string(),
            resolution
                .account
                .as_ref()
                .map(|account| Value::String(account.email.clone()))
                .unwrap_or(Value::Null),
        );
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

    resolution
}

/// Transcript of the Claude session this project already has, if the last
/// runtime snapshot saw one. That transcript names its own config dir.
fn live_session_transcript(linux_path: &str) -> Option<PathBuf> {
    let project_key = crate::provider::path::normalize_project_path(linux_path);
    let snapshot = crate::session_snapshot_cache::load()?;
    snapshot
        .runtime_sessions
        .into_iter()
        .filter(|session| {
            session.cli_tool == CliTool::Claude
                && crate::provider::path::normalize_project_path(&session.project_path)
                    == project_key
        })
        .filter_map(|session| session.jsonl_path)
        .map(PathBuf::from)
        .max_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
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
