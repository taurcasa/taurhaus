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

pub mod platform;

pub mod templates;

#[cfg(test)]
mod test_support;

use std::sync::Mutex;
use std::{io, process};

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
    let env_cmd = r#"echo "PATH=$PATH"; echo "NODE_EXTRA_CA_CERTS=$NODE_EXTRA_CA_CERTS"; echo "ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY"; echo "OPENAI_API_KEY=$OPENAI_API_KEY"; echo "GEMINI_API_KEY=$GEMINI_API_KEY""#;
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
            commands::command_center::list_cli_sessions,
            commands::command_center::launch_cli_session,
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
        Some("--claude-compact-hook") => Some(run_claude_compact_hook_cli()),
        _ => None,
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn run_claude_compact_hook_cli() -> i32 {
    let _log_state = init_coordination_cli_log_sink();
    let response = (|| -> Result<String, crate::coordination::errors::CoordinationError> {
        let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
        let hook_response = crate::coordination::claude_hooks::handle_session_start_hook_stdin(
            io::stdin(),
            &teams_dir,
        )?;
        serde_json::to_string(&hook_response).map_err(|err| {
            crate::coordination::errors::CoordinationError::StoreError(format!(
                "failed to serialize Claude compact hook response: {err}"
            ))
        })
    })();

    match response {
        Ok(payload) => {
            if let Err(error) = write_claude_compact_hook_stdout(io::stdout(), &payload) {
                crate::coordination::claude_hooks::emit_claude_hook_cli_failed(&error.to_string());
                tracing::warn!(error = %error, "failed to write Claude compact hook response to stdout");
                return 1;
            }
        }
        Err(err) => {
            crate::coordination::claude_hooks::emit_claude_hook_cli_failed(&err.to_string());
            tracing::warn!(error = %err, "Claude compact hook bridge failed");
            if let Err(write_error) = write_claude_compact_hook_stdout(io::stdout(), "{}") {
                tracing::warn!(error = %write_error, "failed to write Claude compact hook fallback response to stdout");
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
        let _guard = acquire_env_guard();
        let temp = TempDir::new().expect("tempdir");
        let data_dir = temp.path().join("data");
        fs::create_dir_all(&data_dir).expect("data dir");
        std::env::set_var("TAURHAUS_DATA_DIR", &data_dir);

        let _state = init_coordination_cli_log_sink().expect("log state");
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "test.cli_hook_logging",
            Some("cli hook log test".to_string()),
            Default::default(),
        );

        let log_path = data_dir.join("taurhaus.log.jsonl");
        let mut contents = String::new();
        for _ in 0..50 {
            contents = fs::read_to_string(&log_path).unwrap_or_default();
            if contents
                .lines()
                .any(|line| line.contains("\"event\":\"test.cli_hook_logging\""))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
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
