// Deny unsafe code crate-wide. The one exception (libgit2 init) lives in
// git_init.rs with a scoped #[allow]. Any new `unsafe` block will fail compilation.
#![deny(unsafe_code)]

extern crate self as taurhaus_lib;

mod bootstrap;
mod commands;
mod config;
mod daemon_lifecycle;
pub mod db;
pub mod errors;
mod event_processor;
pub mod models;
mod sentinels;
pub mod services;
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

use std::collections::HashMap;
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

/// Managed state: last known good CLI session snapshot for fast foreground/session fallbacks.
pub struct SessionSnapshotCacheState(pub Mutex<Option<Vec<session_scanner::DisplaySession>>>);

/// Managed state: cached project/settings inputs for watcher reconciliation and fast path lookup.
pub struct ActivityWatchCacheState(pub Mutex<Option<ActivityWatchCacheSnapshot>>);

#[derive(Debug, Clone)]
pub struct ActivityWatchCacheSnapshot {
    pub projects: Vec<models::Project>,
    pub thresholds: models::ActivityThresholds,
    pub project_ids_by_path: HashMap<String, String>,
}

/// Managed state: holds the tantivy search index.
pub struct SearchState(pub Mutex<search::indexer::SearchIndex>);

/// Disable libgit2 ownership validation so repos on WSL filesystems
/// (accessed via `\\wsl$\` UNC paths) don't get rejected as "unsafe".
/// Safe for a desktop app where the user explicitly registers projects.
#[allow(unsafe_code)]
fn disable_git_owner_validation() {
    unsafe {
        let _ = git2::opts::set_verify_owner_validation(false);
    }
}

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
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    #[cfg(target_os = "macos")]
    inherit_macos_shell_env();

    disable_git_owner_validation();

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
            println!("{payload}");
        }
        Err(err) => {
            crate::coordination::claude_hooks::emit_claude_hook_cli_failed(&err.to_string());
            tracing::warn!(error = %err, "Claude compact hook bridge failed");
            println!("{{}}");
        }
    }

    0
}
