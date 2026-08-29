// Deny unsafe code crate-wide. Any new `unsafe` block will fail compilation.
#![deny(unsafe_code)]

extern crate self as taurhaus_lib;

mod bootstrap;
mod commands;
mod config;
mod daemon_lifecycle;
pub mod db;
pub mod errors;
mod event_processor;
mod inotify_diagnostics;
pub mod models;
mod process_utils;
mod sentinels;
pub mod services;
mod session_snapshot_cache;
mod startup;
mod watch_targets;

pub mod git;

pub mod fs;

pub mod logging;

pub mod session;

pub mod search;

pub mod claude_code;

pub mod daemon;
pub mod daemon_api;
pub mod project_provider;
pub mod provider;

#[cfg(feature = "mesh-bridged-backend")]
pub mod coordination;

pub mod session_scanner;

pub mod task_scanner;

pub mod terminal;
pub mod tmux_layout;

pub mod platform;

pub mod templates;

pub mod workflow_runs;

pub mod test_support;

use std::io::{Read, Write};
use std::sync::Mutex;
use std::{io, process};

use serde::{Deserialize, Serialize};
use tauri_plugin_window_state::StateFlags;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

/// Managed state: holds the project provider for filesystem/git operations.
/// Routes operations to LocalProvider or DaemonProvider based on project path.
pub struct ProviderState {
    pub local: provider::local::LocalProvider,
    pub daemon: Option<provider::daemon_client::DaemonProvider>,
    /// Distro identifier for daemon management.
    /// On Windows: WSL distro name (e.g., "Ubuntu"), extracted from project paths.
    /// On macOS/Linux: synthetic `"native"` — daemon runs natively, no WSL needed.
    /// `None` only on Windows when no WSL projects are registered.
    pub wsl_distro: Option<String>,
}

impl ProviderState {
    /// Resolve the appropriate provider for a project path.
    /// WSL paths route through the daemon (when connected), everything else uses local.
    pub fn resolve(&self, project_path: &str) -> &dyn provider::ProjectProvider {
        let active_daemon = self
            .daemon
            .as_ref()
            .filter(|daemon| daemon.is_connected())
            .map(|daemon| daemon as &dyn provider::ProjectProvider);
        provider::provider_for(project_path, &self.local, active_daemon)
    }
}

/// Managed state: holds the file watcher so it lives for the app lifetime.
pub struct WatcherState(pub Mutex<fs::watcher::ProjectWatcher>);

/// Managed state: holds the tantivy search index.
pub struct SearchState(pub Mutex<search::indexer::SearchIndex>);

#[cfg(target_os = "macos")]
fn inherit_macos_shell_env() {
    // Print key env vars as key=value lines, one per line.
    let env_cmd = r#"echo "PATH=$PATH"; echo "NODE_EXTRA_CA_CERTS=$NODE_EXTRA_CA_CERTS"; echo "ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY"; echo "OPENAI_API_KEY=$OPENAI_API_KEY""#;
    if let Ok(output) = std::process::Command::new("/bin/zsh")
        .args(["-lc", env_cmd])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if !value.is_empty() {
                        std::env::set_var(key, value);
                        if key == "PATH" {
                            tracing::info!(path = %value, "Inherited PATH from login shell");
                        } else {
                            tracing::info!(key, "Inherited env var from login shell");
                        }
                    }
                }
            }
        }
    }
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::MAXIMIZED
                        | StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .menu(|app| {
            use tauri::menu::{MenuBuilder, PredefinedMenuItem, SubmenuBuilder};

            let app_menu = SubmenuBuilder::new(app, "taurhaus")
                .about(None)
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let window_menu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .item(&PredefinedMenuItem::close_window(app, None)?)
                .build()?;

            MenuBuilder::new(app)
                .item(&app_menu)
                .item(&edit_menu)
                .item(&window_menu)
                .build()
        })
        .setup(startup::setup)
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::register_project,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::remove_project,
            commands::projects::scan_directory,
            commands::projects::list_directory,
            commands::projects::validate_project_path,
            commands::projects::is_first_run,
            commands::projects::register_projects_batch,
            commands::projects::get_system_roots,
            commands::git::get_recent_commits,
            commands::git::get_all_commits,
            commands::git::get_git_status,
            commands::git::get_remote_url,
            commands::files::get_file_tree,
            commands::files::read_file,
            commands::files::get_readme,
            commands::files::read_project_asset,
            commands::files::check_path_type,
            commands::sessions::get_latest_session,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            workflow_runs::list_workflow_runs,
            workflow_runs::get_workflow_run,
            workflow_runs::workflow_ledger_row,
            commands::search::search,
            commands::search::get_index_status,
            commands::search::rebuild_index,
            commands::relationships::get_relationships,
            commands::relationships::dismiss_relationship,
            commands::relationships::create_relationship,
            commands::relationships::remove_relationship,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::daemon::get_platform,
            commands::daemon::get_daemon_status,
            commands::daemon::start_daemon,
            commands::daemon::check_daemon_install_status,
            commands::daemon::install_daemon,
            commands::mesh::check_mesh_install_status,
            commands::mesh::install_mesh,
            commands::logging::frontend_log,
            commands::accounts::list_accounts,
            commands::accounts::refresh_accounts_usage,
            commands::accounts::set_project_account,
            commands::command_center::list_cli_sessions,
            commands::command_center::list_cli_session_snapshot,
            commands::command_center::launch_cli_session,
            commands::command_center::resolve_launch_account,
            commands::command_center::stop_cli_session,
            commands::command_center::navigate_to_session,
            commands::command_center::record_session_activity,
            commands::command_center::get_project_activity,
            commands::command_center::get_foreground_project,
            commands::tasks::get_project_tasks,
            commands::tasks::get_task_detail,
            commands::tasks::get_archived_sessions,
            commands::tasks::get_commit_files,
            commands::tasks::get_commit_diff,
            commands::tasks::get_commits_in_range,
            // Kept for direct E2E IPC assertions.
            commands::templates::templates_list_roles_full,
            commands::templates::templates_get_role,
            commands::templates::templates_upsert_role,
            commands::templates::templates_delete_role,
            commands::templates::import_role_from_file,
            commands::templates::templates_list_presets_full,
            commands::templates::templates_get_preset,
            commands::templates::templates_upsert_preset,
            commands::templates::templates_delete_preset,
            commands::templates::templates_compose_team,
            commands::templates::templates_get_storage_status,
            commands::templates::templates_get_history,
            commands::templates::templates_get_diff,
            commands::templates::templates_revert,
            // Kept for direct E2E IPC assertions.
            commands::templates::templates_flush_pending,
            commands::templates::export_role_to_file,
            commands::templates::export_agent_definitions,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_create_team,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_disband_team,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_add_member,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_remove_member,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_list_teams,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_team_status,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_initialize_team,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_add_agent,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_resume_member,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_resume_team,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_reonboard,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_live_team_status,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_preflight_check,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_feature_availability,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_project_mesh_snapshot,
        ])
}

pub fn run() {
    let default_directive = if cfg!(debug_assertions) {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    let env_filter = EnvFilter::builder()
        .with_default_directive(default_directive.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(io::stderr)
        .init();

    // Resolve the interactive shell environment before coordination or app
    // startup needs native CLI paths.
    #[cfg(target_os = "macos")]
    inherit_macos_shell_env();

    #[cfg(feature = "mesh-bridged-backend")]
    if let Some(exit_code) = maybe_run_coordination_cli_mode() {
        process::exit(exit_code);
    }

    if let Err(error) = build_app().run(tauri::generate_context!()) {
        tracing::error!(error = %error, "error while running taurhaus");
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn maybe_run_coordination_cli_mode() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--compact-hook" | "--claude-compact-hook") => Some(run_compact_hook_cli()),
        Some("--launch-command") => Some(run_launch_command_cli(args.next().as_deref())),
        Some("--render-onboarding") => Some(run_render_onboarding_cli(args.next().as_deref())),
        Some("--export-agent-definitions") => {
            Some(run_export_agent_definitions_cli(args.next().as_deref()))
        }
        _ => None,
    }
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LaunchCommandCliRequest {
    tool: crate::session_scanner::cli_tool::CliTool,
    mode: crate::daemon::protocol::LaunchMode,
    #[serde(default)]
    base: Option<String>,
    model: Option<String>,
    #[serde(default, alias = "reasoning_effort")]
    reasoning_effort: Option<String>,
    team: Option<LaunchCommandTeamCliRequest>,
    #[serde(default, alias = "codex_bypass_hook_trust")]
    codex_bypass_hook_trust: bool,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LaunchCommandTeamCliRequest {
    #[serde(alias = "team_name")]
    team_name: String,
    #[serde(alias = "agent_name")]
    agent_name: String,
    role: crate::coordination::domain::MemberRole,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderOnboardingCliRequest {
    tool: crate::session_scanner::cli_tool::CliTool,
    #[serde(alias = "team_name")]
    team_name: String,
    #[serde(alias = "member_name")]
    member_name: String,
    #[serde(alias = "lead_name")]
    lead_name: String,
    role: crate::templates::types::RoleTemplate,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchCommandCliResponse {
    command: String,
    notes: Vec<LaunchCommandCliNote>,
}

#[cfg(feature = "mesh-bridged-backend")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchCommandCliNote {
    event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    found: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}

#[cfg(feature = "mesh-bridged-backend")]
impl From<crate::session_scanner::launch::LaunchNote> for LaunchCommandCliNote {
    fn from(note: crate::session_scanner::launch::LaunchNote) -> Self {
        use crate::session_scanner::launch::{EffortIgnoreReason, LaunchNote};

        let event = note.event_name();
        match note {
            LaunchNote::CapabilityMissing { capability, found } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement: None,
                reason: Some(capability.as_str()),
            },
            LaunchNote::DeprecatedFlag { flag } => Self {
                event,
                flag: Some(flag),
                found: None,
                replacement: None,
                reason: None,
            },
            LaunchNote::ModelIgnored { found } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement: None,
                reason: None,
            },
            LaunchNote::NotifyIgnored { found } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement: None,
                reason: None,
            },
            LaunchNote::ModelDeprecated { found, replacement } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement,
                reason: None,
            },
            LaunchNote::EffortIgnored { found, reason } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement: None,
                reason: Some(match reason {
                    EffortIgnoreReason::BaseOverride => "baseOverride",
                    EffortIgnoreReason::Invalid => "invalid",
                }),
            },
            LaunchNote::SelectorIgnored { found } => Self {
                event,
                flag: None,
                found: Some(found),
                replacement: None,
                reason: None,
            },
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_launch_command_cli(json_arg: Option<&str>) -> i32 {
    let _log_state = init_coordination_cli_log_sink();
    match render_launch_command_cli(json_arg, io::stdin()) {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(payload) => write_renderer_stdout(io::stdout(), &payload),
            Err(error) => {
                tracing::warn!(error = %error, "failed to serialize launch command response");
                1
            }
        },
        Err(error) => {
            tracing::warn!(error = %error, "launch command renderer failed");
            1
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn render_launch_command_cli<R: Read>(
    json_arg: Option<&str>,
    mut stdin: R,
) -> Result<LaunchCommandCliResponse, String> {
    use crate::models::{CliCommandSettings, ModelCatalog};
    use crate::session_scanner::control::validate_command_override;
    use crate::session_scanner::launch::{base_command, LaunchSpec, ModelSpec, TeamContext};

    let json = read_renderer_request(json_arg, &mut stdin)?;
    let request: LaunchCommandCliRequest =
        serde_json::from_str(&json).map_err(|error| format!("invalid launch request: {error}"))?;
    let mut model = request
        .model
        .as_deref()
        .map(ModelSpec::parse_legacy)
        .unwrap_or_default();
    if request.reasoning_effort.is_some() {
        model.reasoning_effort = request.reasoning_effort;
    }
    let mut notes = Vec::new();
    if let Some(requested_model) = model.model.clone() {
        let member_name = request
            .team
            .as_ref()
            .map(|team| team.agent_name.as_str())
            .unwrap_or("taureval");
        if let Some(validated) = crate::coordination::member_activation::validated_role_model(
            request.tool,
            &requested_model,
            member_name,
            "launch_renderer_cli",
        ) {
            model.model = Some(validated);
        } else {
            let replacement = ModelCatalog::default_for(request.tool).map(|entry| entry.id.clone());
            notes.push(LaunchCommandCliNote {
                event: "launch.model.invalid",
                flag: None,
                found: Some(requested_model),
                replacement: replacement.clone(),
                reason: None,
            });
            model.model = replacement;
        }
    }
    let team = request.team.as_ref().map(|team| TeamContext {
        team_name: &team.team_name,
        agent_name: &team.agent_name,
        role: team.role,
    });
    let defaults = CliCommandSettings::default();
    let base = request
        .base
        .as_deref()
        .unwrap_or_else(|| base_command(&defaults, request.tool, request.mode));

    let rendered = LaunchSpec {
        tool: request.tool,
        mode: request.mode,
        base,
        model: model.clone(),
        team,
        codex_bypass_hook_trust: request.codex_bypass_hook_trust,
        codex_notify_executable: None,
        account_dir: None,
        selector: None,
    }
    .render();
    validate_command_override(&rendered.command)?;

    let mode = format!("{:?}", request.mode).to_ascii_lowercase();
    let mut fields = serde_json::Map::new();
    fields.insert("tool".to_string(), request.tool.to_string().into());
    fields.insert("mode".to_string(), mode.into());
    fields.insert(
        "model".to_string(),
        model
            .model
            .map(serde_json::Value::String)
            .unwrap_or_default(),
    );
    fields.insert(
        "reasoning_effort".to_string(),
        model
            .reasoning_effort
            .map(serde_json::Value::String)
            .unwrap_or_default(),
    );
    fields.insert(
        "command".to_string(),
        crate::session_scanner::launch::redact_command_for_logging(&rendered.command).into(),
    );
    crate::commands::logging::emit_global(
        "info",
        "coordination",
        "launch.command.rendered",
        Some("Rendered CLI launch command".to_string()),
        fields,
    );

    for note in rendered.notes {
        let response_note = LaunchCommandCliNote::from(note);
        let mut fields = serde_json::Map::new();
        fields.insert("tool".to_string(), request.tool.to_string().into());
        if let Some(value) = response_note.flag.as_ref() {
            fields.insert("flag".to_string(), value.clone().into());
        }
        if let Some(value) = response_note.found.as_ref() {
            fields.insert("found".to_string(), value.clone().into());
        }
        if let Some(value) = response_note.replacement.as_ref() {
            fields.insert("replacement".to_string(), value.clone().into());
        }
        if let Some(value) = response_note.reason {
            fields.insert("reason".to_string(), value.into());
        }
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            response_note.event,
            Some("Launch renderer reported a configuration note".to_string()),
            fields,
        );
        notes.push(response_note);
    }

    Ok(LaunchCommandCliResponse {
        command: rendered.command,
        notes,
    })
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_render_onboarding_cli(json_arg: Option<&str>) -> i32 {
    match render_onboarding_cli(json_arg, io::stdin()) {
        Ok(onboarding) => write_renderer_stdout(io::stdout(), &onboarding),
        Err(error) => {
            tracing::warn!(error = %error, "onboarding renderer failed");
            1
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn render_onboarding_cli<R: Read>(json_arg: Option<&str>, mut stdin: R) -> Result<String, String> {
    use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
    let json = read_renderer_request(json_arg, &mut stdin)?;
    let request: RenderOnboardingCliRequest = serde_json::from_str(&json)
        .map_err(|error| format!("invalid onboarding request: {error}"))?;
    let role_context = RoleContext::from(&request.role);
    DeliveryRenderer::render_for_tool(
        request.tool,
        &request.team_name,
        &request.member_name,
        &request.lead_name,
        true,
        role_context,
    )
    .ok_or_else(|| "onboarding is not required for this harness".to_string())
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_export_agent_definitions_cli(project_dir: Option<&str>) -> i32 {
    match export_agent_definitions_cli(project_dir) {
        Ok(response) => write_renderer_stdout(io::stdout(), &response),
        Err(error) => {
            tracing::warn!(error = %error, "agent definition export failed");
            1
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn export_agent_definitions_cli(project_dir: Option<&str>) -> Result<String, String> {
    let project_dir = project_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "a project directory is required: --export-agent-definitions <dir>".to_string()
        })?;

    let store = crate::templates::storage::TemplateStore::new(
        crate::provider::platform_paths::PlatformPaths::app_data_root(),
    );
    let roles = store
        .list_roles()
        .map_err(|error| format!("failed to read the role catalog: {error}"))?
        .into_iter()
        .map(|record| record.template)
        .collect::<Vec<_>>();

    let export = crate::templates::agent_definitions::export_agent_definitions(
        &roles,
        std::path::Path::new(project_dir),
    )
    .map_err(|error| format!("failed to write agent definitions: {error}"))?;

    serde_json::to_string(&export)
        .map_err(|error| format!("failed to encode the export result: {error}"))
}

#[cfg(feature = "mesh-bridged-backend")]
fn read_renderer_request<R: Read>(json_arg: Option<&str>, stdin: &mut R) -> Result<String, String> {
    match json_arg {
        Some(json) if json != "-" => Ok(json.to_string()),
        _ => {
            let mut json = String::new();
            stdin
                .read_to_string(&mut json)
                .map_err(|error| format!("failed to read renderer request: {error}"))?;
            if json.trim().is_empty() {
                Err("renderer request JSON is required as an argument or on stdin".to_string())
            } else {
                Ok(json)
            }
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn write_renderer_stdout<W: Write>(mut stdout: W, payload: &str) -> i32 {
    if let Err(error) = writeln!(stdout, "{payload}").and_then(|()| stdout.flush()) {
        tracing::warn!(error = %error, "failed to write renderer output to stdout");
        return 1;
    }
    0
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_compact_hook_cli() -> i32 {
    let _log_state = init_coordination_cli_log_sink();
    let teams_dir = crate::provider::platform_paths::PlatformPaths::teams_dir();
    match crate::coordination::compact_hook::run_compact_hook_cli(
        io::stdin(),
        io::stdout(),
        &teams_dir,
    ) {
        Ok(()) => {}
        Err(err) => {
            crate::coordination::compact_hook::emit_compact_hook_cli_failed(&err.to_string());
            tracing::warn!(error = %err, "compact hook bridge failed");
            if let Err(write_error) = write_claude_compact_hook_stdout(io::stdout(), "{}") {
                tracing::warn!(error = %write_error, "failed to write compact hook fallback response to stdout");
                return 1;
            }
        }
    }

    0
}

#[cfg(feature = "mesh-bridged-backend")]
fn write_claude_compact_hook_stdout<W: io::Write>(mut stdout: W, payload: &str) -> io::Result<()> {
    stdout.write_all(payload.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

#[cfg(feature = "mesh-bridged-backend")]
fn init_coordination_cli_log_sink() -> Option<crate::commands::logging::LogFileState> {
    let log_path = crate::provider::platform_paths::PlatformPaths::log_path();
    match crate::commands::logging::LogFileState::new(log_path.clone()) {
        Ok(state) => {
            crate::commands::logging::install_global_sink(&state);
            Some(state)
        }
        Err(error) => {
            tracing::warn!(
                log_path = %log_path.display(),
                error = %error,
                "failed to initialize structured log sink for coordination CLI mode"
            );
            None
        }
    }
}

#[cfg(all(test, feature = "mesh-bridged-backend"))]
mod tests {
    use super::{init_coordination_cli_log_sink, write_claude_compact_hook_stdout};

    use serde_json::Value;
    use std::fs;
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use tempfile::TempDir;

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvGuard {}

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("TAURHAUS_DATA_DIR");
        }
    }

    fn acquire_env_guard() -> EnvGuard {
        EnvGuard {
            _lock: ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()),
        }
    }

    #[test]
    fn coordination_cli_log_sink_installs_jsonl_emitter() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let _guard = acquire_env_guard();
        let temp = TempDir::new().expect("tempdir");
        let data_dir = temp.path().join("data");
        fs::create_dir_all(&data_dir).expect("data dir");
        std::env::set_var("TAURHAUS_DATA_DIR", &data_dir);

        let state = init_coordination_cli_log_sink().expect("log state");
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "test.cli_hook_logging",
            Some("cli hook log test".to_string()),
            Default::default(),
        );

        state.flush_for_test().expect("flush structured log sink");
        let log_path = data_dir.join("taurhaus.log.jsonl");
        let contents = fs::read_to_string(&log_path).expect("read structured log");
        let entry: Value = serde_json::from_str(
            contents
                .lines()
                .find(|line| line.contains("\"event\":\"test.cli_hook_logging\""))
                .expect("one log entry written"),
        )
        .expect("json log");
        assert_eq!(entry["event"], "test.cli_hook_logging");
        assert_eq!(entry["component"], "coordination");
    }

    #[test]
    fn claude_compact_hook_stdout_writer_emits_only_json_payload() {
        let mut stdout = Vec::new();

        write_claude_compact_hook_stdout(&mut stdout, "{\"hookSpecificOutput\":null}")
            .expect("stdout write should succeed");

        assert_eq!(
            String::from_utf8(stdout).expect("utf8"),
            "{\"hookSpecificOutput\":null}\n"
        );
    }
}
