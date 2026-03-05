// Deny unsafe code crate-wide. The one exception (libgit2 init) lives in
// git_init.rs with a scoped #[allow]. Any new `unsafe` block will fail compilation.
#![deny(unsafe_code)]

mod bootstrap;
mod commands;
mod config;
mod daemon_lifecycle;
pub mod db;
pub mod errors;
mod event_processor;
pub mod models;
pub mod services;

pub mod git;

pub mod fs;

pub mod session;

pub mod search;

pub mod claude_code;

pub mod daemon;
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

use commands::projects::DbState;
use tauri::{Emitter, Manager};
use tauri_plugin_window_state::StateFlags;
use tracing_subscriber::EnvFilter;

const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

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
        // Only pass daemon to the router if it's actually connected
        let active_daemon = self
            .daemon
            .as_ref()
            .filter(|d| d.is_connected())
            .map(|d| d as &dyn provider::ProjectProvider);
        provider::provider_for(project_path, &self.local, active_daemon)
    }
}

/// Managed state: holds the file watcher so it lives for the app lifetime.
pub struct WatcherState(pub Mutex<fs::watcher::ProjectWatcher>);

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

fn env_path_override(var: &str) -> Option<std::path::PathBuf> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(value))
}

fn resolve_app_data_dir(app: tauri::AppHandle) -> std::path::PathBuf {
    if let Some(path) = env_path_override(DATA_DIR_OVERRIDE_ENV) {
        tracing::info!(
            env = DATA_DIR_OVERRIDE_ENV,
            path = %path.display(),
            "Using app data dir override"
        );
        return path;
    }
    app.path()
        .app_data_dir()
        .expect("failed to resolve app_data_dir")
}

fn resolve_claude_tasks_dir() -> Option<std::path::PathBuf> {
    if let Some(path) = env_path_override(CLAUDE_DIR_OVERRIDE_ENV) {
        return Some(path.join("tasks"));
    }
    dirs::home_dir().map(|home| home.join(".claude").join("tasks"))
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // macOS: Finder-launched apps get a minimal env that doesn't include
    // homebrew, cargo, fnm, etc. Resolve the user's real environment from
    // their login shell so Command::new("tmux"), "claude", etc. all work.
    // Also inherits NODE_EXTRA_CA_CERTS (Homebrew Node.js needs this for TLS).
    #[cfg(target_os = "macos")]
    {
        // Print key env vars as key=value lines, one per line.
        let env_cmd = r#"echo "PATH=$PATH"; echo "NODE_EXTRA_CA_CERTS=$NODE_EXTRA_CA_CERTS"; echo "ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY"; echo "OPENAI_API_KEY=$OPENAI_API_KEY"; echo "GEMINI_API_KEY=$GEMINI_API_KEY""#;
        if let Ok(output) = std::process::Command::new("/bin/zsh")
            .args(["-lc", env_cmd])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some((key, val)) = line.split_once('=') {
                        if !val.is_empty() {
                            std::env::set_var(key, val);
                            if key == "PATH" {
                                tracing::info!(path = %val, "Inherited PATH from login shell");
                            } else {
                                tracing::info!(key, "Inherited env var from login shell");
                            }
                        }
                    }
                }
            }
        }
    }

    disable_git_owner_validation();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            // Persist only geometry/fullscreen state. Decorations are static
            // (`decorations: false` in tauri.conf) and restoring them can cause
            // platform-specific client-height drift on reopen.
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
            use tauri::menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem};

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
        .setup(|app| {
            tracing::info!("taurhaus starting");

            let data_dir = resolve_app_data_dir(app.handle().clone());
            std::fs::create_dir_all(&data_dir).expect("failed to create data directory");

            // Open append-only log file for frontend + backend logs.
            // Truncate on each launch so the file stays manageable.
            let log_path = data_dir.join("taurhaus.log");
            let log_file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&log_path)
                .expect("failed to open log file");
            tracing::info!(?log_path, "Log file ready");
            app.manage(commands::logging::LogFileState(
                std::sync::Mutex::new(log_file),
            ));

            let db_path = data_dir.join("taurhaus.db");
            let conn = db::init_db(&db_path).expect("failed to initialize database");

            // Fast daemon probe: try connecting to an already-running daemon.
            // This is instant for localhost (connection refused = immediate fail).
            // We do NOT spawn or poll here — that moves to the background thread.
            let projects = db::queries::list_projects(&conn).unwrap_or_default();
            let wsl_distro = if daemon::launcher::is_native_daemon() {
                Some("native".to_string())
            } else {
                projects
                    .iter()
                    .find_map(|p| provider::path::wsl_distro_from_path(&p.path))
            };

            let port = daemon::server::DEFAULT_PORT;
            let (daemon_provider, daemon_connected_at_startup) = if wsl_distro.is_some() {
                let addr = format!("127.0.0.1:{port}");
                match provider::daemon_client::DaemonProvider::connect(&addr) {
                    Ok(provider) => {
                        tracing::info!("Connected to existing daemon (fast path)");
                        (Some(provider), true)
                    }
                    Err(_) => {
                        tracing::info!(addr, "Daemon not running — will start in background");
                        (Some(provider::daemon_client::DaemonProvider::new_disconnected(&addr)), false)
                    }
                }
            } else {
                (None, false)
            };

            app.manage(DbState(Mutex::new(conn)));
            app.manage(commands::templates::TemplateStoreState::new(
                data_dir.clone(),
            ));

            let daemon_addr = daemon_provider.as_ref().map(|d| d.addr().to_string());

            app.manage(ProviderState {
                local: provider::local::LocalProvider,
                daemon: daemon_provider,
                wsl_distro: wsl_distro.clone(),
            });

            #[cfg(feature = "mesh-bridged-backend")]
            app.manage(coordination::state::CoordinationState::for_app_startup());

            // Background bootstrap: daemon spawn, tmux, protocol check, file watchers.
            // Runs AFTER setup returns so the webview + splash screen render immediately.
            {
                let boot_handle = app.handle().clone();
                let boot_distro = wsl_distro.clone();
                let boot_log_path = log_path.clone();
                let boot_connected = daemon_connected_at_startup;
                let _boot_addr = daemon_addr.clone();

                std::thread::spawn(move || {
                    // If daemon wasn't running, spawn it and connect.
                    if !boot_connected {
                        if let Some(ref distro) = boot_distro {
                            tracing::info!("Background bootstrap: starting daemon");
                            let port = daemon::server::DEFAULT_PORT;

                            // Spawn daemon process (fire-and-forget).
                            if let Err(e) = daemon::launcher::try_restart_daemon(distro, port) {
                                tracing::warn!(error = %e, "Failed to start daemon in background");
                            } else {
                                // Give the daemon a moment to bind the port, then reconnect.
                                std::thread::sleep(std::time::Duration::from_secs(2));
                                let provider_state = boot_handle.state::<ProviderState>();
                                if let Some(ref daemon) = provider_state.daemon {
                                    if daemon.reconnect().is_ok() {
                                        tracing::info!("Background bootstrap: daemon connected");
                                        daemon_lifecycle::respawn_daemon_watches(&boot_handle);
                                        let _ = boot_handle.emit(
                                            "daemon-status",
                                            serde_json::json!({ "status": "connected" }),
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Protocol version check (non-blocking, informational).
                    {
                        let provider_state = boot_handle.state::<ProviderState>();
                        if let Some(ref daemon) = provider_state.daemon {
                            if daemon.is_connected() {
                                let expected = daemon::protocol::PROTOCOL_VERSION;
                                match daemon.ping_protocol_version() {
                                    Ok(v) if v < expected => {
                                        tracing::error!(
                                            daemon_version = v,
                                            expected = expected,
                                            "DAEMON IS OUTDATED — rebuild with `just install-daemon`"
                                        );
                                    }
                                    Ok(v) => {
                                        tracing::info!(protocol_version = v, "Daemon protocol version OK");
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Could not check daemon protocol version");
                                    }
                                }
                            }
                        }
                    }

                    // Ensure tmux session exists.
                    if let Some(ref distro) = boot_distro {
                        daemon::launcher::ensure_tmux_session(distro, &boot_log_path);
                    }
                });
            }

            // Start daemon health check (handles reconnection if background
            // bootstrap didn't connect, plus ongoing monitoring).
            if wsl_distro.is_some() {
                let health_handle = app.handle().clone();
                let connected_at_startup = daemon_connected_at_startup;
                std::thread::spawn(move || {
                    daemon_lifecycle::daemon_health_check(health_handle, connected_at_startup);
                });

                // Session activity stream: daemon-owned scanning + app event bridge.
                daemon_lifecycle::start_session_updates_bridge(app.handle().clone());
            }

            // Start file watcher — events from both local and daemon watchers
            // are funneled through the same channel and processor.
            let (watcher, rx) = fs::watcher::ProjectWatcher::new();
            let event_tx = watcher.event_sender();
            app.manage(WatcherState(Mutex::new(watcher)));

            // If daemon is connected at startup, start an event listener for WSL projects.
            // This opens a second TCP connection dedicated to receiving watch events.
            // The daemon also watches ~/.claude/tasks/ for event-driven task sync.
            // If daemon wasn't connected, respawn_daemon_watches handles this on late-connect.
            let has_daemon = daemon_connected_at_startup;
            if daemon_connected_at_startup {
                let daemon_addr = daemon_addr.expect("daemon_addr must be set when connected");
                let distro = wsl_distro.clone();
                let event_tx_clone = event_tx.clone();
                let db_state = app.state::<commands::projects::DbState>();
                let db_projects_guard = db_state.0.lock().unwrap_or_else(|err| {
                    tracing::warn!(
                        error = %err,
                        "DB lock poisoned while collecting projects for daemon watch bootstrap; recovering"
                    );
                    err.into_inner()
                });
                let db_projects = db::queries::list_projects(&db_projects_guard).unwrap_or_else(|err| {
                    tracing::warn!(
                        error = %err,
                        "Failed to list projects for daemon watch bootstrap"
                    );
                    Vec::new()
                });

                std::thread::spawn(move || {
                    daemon_lifecycle::start_daemon_watches(
                        daemon_addr,
                        event_tx_clone,
                        distro,
                        db_projects,
                    );
                });
            }

            // Spawn background thread to process watcher events
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                event_processor::process_watch_events(rx, handle);
            });

            // Initialize search index — fall back to in-memory if the on-disk
            // index is locked by another running instance.
            let index_dir = data_dir.join("search_index");
            let search_index = match search::indexer::SearchIndex::open(&index_dir) {
                Ok(idx) => idx,
                Err(e) => {
                    tracing::warn!(
                        "Search index unavailable (another instance running?): {e}. \
                         Falling back to in-memory index."
                    );
                    search::indexer::SearchIndex::open_in_memory()
                        .expect("failed to create in-memory search index")
                }
            };
            app.manage(SearchState(Mutex::new(search_index)));

            // Register local file watches for projects not covered by daemon.
            // In daemon mode: daemon watches WSL projects, so only non-WSL
            // projects need local watches.
            // Without daemon: ALL projects need local watches.
            {
                let db_state = app.state::<DbState>();
                let db_guard = db_state.0.lock().unwrap_or_else(|err| {
                    tracing::warn!(
                        error = %err,
                        "DB lock poisoned while collecting projects for local watch bootstrap; recovering"
                    );
                    err.into_inner()
                });
                let projects = db::queries::list_projects(&db_guard).unwrap_or_else(|err| {
                    tracing::warn!(
                        error = %err,
                        "Failed to list projects for local watch bootstrap"
                    );
                    Vec::new()
                });

                let watcher_state = app.state::<WatcherState>();
                let mut watcher_guard = watcher_state.0.lock().unwrap();
                let mut count = 0;

                let mut watch_limit_hit = false;
                for project in &projects {
                    // Skip WSL projects when daemon handles them
                    if has_daemon && provider::path::is_wsl_path(&project.path) {
                        continue;
                    }
                    let path = std::path::Path::new(&project.path);
                    if path.is_dir() {
                        match watcher_guard.watch_project(
                            project.id.clone(),
                            path.to_path_buf(),
                        ) {
                            Ok(()) => count += 1,
                            Err(e) => {
                                let msg = e.to_string();
                                if platform::is_watch_limit_error(&msg) {
                                    tracing::warn!(
                                        project = project.name,
                                        error = %e,
                                        "Watch limit reached — skipping project"
                                    );
                                    watch_limit_hit = true;
                                } else {
                                    tracing::debug!(
                                        project = project.name,
                                        error = %e,
                                        "Could not watch project directory (local)"
                                    );
                                }
                            }
                        }
                    }
                }
                if count > 0 {
                    tracing::info!(count, "Watching project directories (local)");
                }
                if watch_limit_hit {
                    tracing::warn!(
                        "Some projects could not be watched — watch limit reached. \
                         File changes in those projects won't be detected. {}",
                        platform::watch_limit_help()
                    );
                }

                // Also watch Claude task directories locally.
                // On Windows in daemon mode, start_daemon_watches handles
                // this via WSL. On macOS/Linux, always watch locally because
                // the daemon doesn't have WSL path context.
                if !has_daemon || daemon::launcher::is_native_daemon() {
                    if let Some(tasks_dir) = resolve_claude_tasks_dir() {
                        if tasks_dir.is_dir() {
                            if let Err(e) = watcher_guard.watch_project(
                                "__claude_tasks__".to_string(),
                                tasks_dir.clone(),
                            ) {
                                tracing::debug!(
                                    error = %e,
                                    path = %tasks_dir.display(),
                                    "Could not watch Claude tasks directory"
                                );
                            } else {
                                tracing::info!(
                                    path = %tasks_dir.display(),
                                    "Watching Claude tasks directory (local)"
                                );
                            }
                        }
                    }
                }
            }

            // Run slow startup tasks on a background thread so the window
            // appears immediately.  These involve git operations that can take
            // seconds per project over cross-filesystem paths (WSL UNC).
            let bg_handle = app.handle().clone();
            std::thread::spawn(move || {
                // Re-seed activity timestamps from git
                bootstrap::startup_reseed_activity(&bg_handle);

                // Import any unimported sessions
                bootstrap::startup_session_scan(&bg_handle);

                // Build search index if empty
                bootstrap::startup_search_index(&bg_handle);

                // Seed task database from live sources
                bootstrap::startup_task_scan(&bg_handle);
            });

            tracing::info!(?db_path, "database initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::register_project,
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
            commands::daemon::stop_daemon,
            commands::daemon::check_daemon_install_status,
            commands::daemon::install_daemon,
            commands::mesh::check_mesh_install_status,
            commands::mesh::install_mesh,
            commands::logging::frontend_log,
            commands::command_center::list_claude_sessions,
            commands::command_center::launch_claude_session,
            commands::command_center::stop_claude_session,
            commands::command_center::navigate_to_session,
            commands::command_center::record_session_activity,
            commands::command_center::get_project_activity,
            commands::tasks::get_project_tasks,
            commands::tasks::get_task_detail,
            commands::tasks::get_archived_sessions,
            commands::tasks::get_commit_files,
            commands::tasks::get_commit_diff,
            commands::tasks::get_commits_in_range,
            commands::templates::templates_list_roles,
            commands::templates::templates_list_roles_full,
            commands::templates::templates_get_role,
            commands::templates::templates_upsert_role,
            commands::templates::templates_delete_role,
            commands::templates::templates_list_presets,
            commands::templates::templates_list_presets_full,
            commands::templates::templates_get_preset,
            commands::templates::templates_upsert_preset,
            commands::templates::templates_delete_preset,
            commands::templates::templates_compose_team,
            commands::templates::templates_validate_composition,
            commands::templates::templates_get_storage_status,
            commands::templates::templates_get_history,
            commands::templates::templates_get_diff,
            commands::templates::templates_revert,
            commands::templates::templates_flush_pending,
            commands::templates::templates_import,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::templates::templates_apply_composition,
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
            commands::coordination::coordination_reonboard,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_live_team_status,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_preflight_check,
            #[cfg(feature = "mesh-bridged-backend")]
            commands::coordination::coordination_get_feature_availability,
        ])
        .run(tauri::generate_context!())
        .expect("error while running taurhaus");
}
