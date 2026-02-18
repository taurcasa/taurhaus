mod commands;
mod config;
pub mod db;
pub mod errors;
pub mod models;
pub mod services;

pub mod git;

pub mod fs;

pub mod session;

pub mod search;

pub mod claude_code;

pub mod daemon;
pub mod provider;

use std::sync::Mutex;

use commands::projects::DbState;
use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

/// Managed state: holds the project provider for filesystem/git operations.
/// Routes operations to LocalProvider or DaemonProvider based on project path.
pub struct ProviderState {
    pub local: provider::local::LocalProvider,
    pub daemon: Option<provider::daemon_client::DaemonProvider>,
    /// WSL distro name (extracted from first WSL project). Used for daemon restarts.
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

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Disable libgit2 ownership validation so repos on WSL filesystems
    // (accessed via \\wsl$\ UNC paths) don't get rejected as "unsafe".
    // Safe for a desktop app where the user explicitly registers projects.
    unsafe {
        let _ = git2::opts::set_verify_owner_validation(false);
    }

    tauri::Builder::default()
        .setup(|app| {
            tracing::info!("taurhaus starting");

            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app_data_dir");
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

            // Check for WSL projects and try daemon connection before managing state.
            // This happens synchronously (max ~3s) while we still have direct conn access.
            let (daemon_provider, wsl_distro) = {
                let projects = db::queries::list_projects(&conn).unwrap_or_default();
                let distro = projects
                    .iter()
                    .find_map(|p| provider::path::wsl_distro_from_path(&p.path));
                let daemon = daemon::launcher::try_connect_daemon(
                    distro.as_deref(),
                    daemon::server::DEFAULT_PORT,
                );
                (daemon, distro)
            };

            app.manage(DbState(Mutex::new(conn)));

            // Extract daemon addr before moving into managed state
            let daemon_addr = daemon_provider.as_ref().map(|d| d.addr().to_string());

            app.manage(ProviderState {
                local: provider::local::LocalProvider,
                daemon: daemon_provider,
                wsl_distro: wsl_distro.clone(),
            });

            // Start daemon health check if daemon is configured
            if wsl_distro.is_some() {
                let health_handle = app.handle().clone();
                std::thread::spawn(move || {
                    daemon_health_check(health_handle);
                });
            }

            // Start file watcher — events from both local and daemon watchers
            // are funneled through the same channel and processor.
            let (watcher, rx) = fs::watcher::ProjectWatcher::new();
            let event_tx = watcher.event_sender();
            app.manage(WatcherState(Mutex::new(watcher)));

            // If daemon is connected, start an event listener for WSL projects.
            // This opens a second TCP connection dedicated to receiving watch events.
            if let Some(daemon_addr) = daemon_addr {
                let distro = wsl_distro.clone();
                let event_tx_clone = event_tx.clone();
                let db_projects = db::queries::list_projects(
                    &app.state::<commands::projects::DbState>().0.lock().unwrap(),
                )
                .unwrap_or_default();

                std::thread::spawn(move || {
                    start_daemon_watches(daemon_addr, event_tx_clone, distro, db_projects);
                });
            }

            // Spawn background thread to process watcher events
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                process_watch_events(rx, handle);
            });

            // Initialize search index
            let index_dir = data_dir.join("search_index");
            let search_index = search::indexer::SearchIndex::open(&index_dir)
                .expect("failed to initialize search index");
            app.manage(SearchState(Mutex::new(search_index)));

            // Run slow startup tasks on a background thread so the window
            // appears immediately.  These involve git operations that can take
            // seconds per project over cross-filesystem paths (WSL UNC).
            let bg_handle = app.handle().clone();
            std::thread::spawn(move || {
                // Re-seed activity timestamps from git
                startup_reseed_activity(&bg_handle);

                // Import any unimported sessions
                startup_session_scan(&bg_handle);

                // Build search index if empty
                startup_search_index(&bg_handle);
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
            commands::files::get_file_tree,
            commands::files::read_file,
            commands::files::get_readme,
            commands::files::read_project_asset,
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
            commands::daemon::get_daemon_status,
            commands::daemon::start_daemon,
            commands::daemon::stop_daemon,
            commands::logging::frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running taurhaus");
}

/// Look up a project's path from the database, returning None on any error.
fn get_project_path(app: &tauri::AppHandle, project_id: &str) -> Option<String> {
    let db_state = app.state::<DbState>();
    let conn = db_state.0.lock().ok()?;
    let project = db::queries::get_project(&conn, project_id).ok()??;
    Some(project.path)
}

/// Process file watcher events on a background thread.
/// Emits Tauri events to the frontend and triggers session imports + search index updates.
fn process_watch_events(
    rx: std::sync::mpsc::Receiver<fs::watcher::WatchEvent>,
    app: tauri::AppHandle,
) {
    use fs::watcher::WatchEvent;

    for event in rx {
        // Bump last_activity_at for any file/git/session activity
        let activity_project_id = match &event {
            WatchEvent::GitChanged { project_id }
            | WatchEvent::FileChanged { project_id, .. }
            | WatchEvent::SessionFileCreated { project_id, .. } => Some(project_id.clone()),
            _ => None,
        };
        if let Some(pid) = activity_project_id {
            let db_state = app.state::<DbState>();
            if let Ok(conn) = db_state.0.lock() {
                let _ = services::project::touch_activity(&conn, &pid);
            };
        }

        match event {
            WatchEvent::GitChanged { project_id } => {
                let Some(project_path) = get_project_path(&app, &project_id) else {
                    continue;
                };
                let path = std::path::Path::new(&project_path);

                // Re-read git status via provider and emit to frontend
                let provider_state = app.state::<ProviderState>();
                let provider = provider_state.resolve(&project_path);
                if let Ok(status) = provider.git_status(&project_path) {
                    // Update cached git status in SQLite
                    let db_state = app.state::<DbState>();
                    if let Ok(conn) = db_state.0.lock() {
                        let _ = db::queries::update_cached_git_status(
                            &conn,
                            &project_id,
                            status.branch.as_deref(),
                            status.is_dirty,
                        );
                    }

                    let _ = app.emit(
                        "project-git-changed",
                        serde_json::json!({
                            "project_id": project_id,
                            "branch": status.branch,
                            "is_dirty": status.is_dirty,
                        }),
                    );
                }

                // Re-index recent commits
                let search_state = app.state::<SearchState>();
                let mut index = match search_state.0.lock() {
                    Ok(i) => i,
                    Err(_) => continue,
                };
                match search::indexer::reindex_commits(&mut index, &project_id, path, 50) {
                    Ok(count) if count > 0 => {
                        let _ = app.emit("search-index-updated", serde_json::json!({
                            "project_id": project_id,
                            "reason": "git_changed",
                            "docs_updated": count,
                        }));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to reindex commits on git change");
                    }
                    _ => {}
                }
            }
            WatchEvent::SessionFileCreated { project_id, path } => {
                let db_state = app.state::<DbState>();
                let conn = match db_state.0.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                match services::session_import::import_handoff(&conn, &project_id, &path) {
                    Ok(Some(session_id)) => {
                        let _ = app.emit(
                            "session-imported",
                            serde_json::json!({
                                "project_id": project_id,
                                "session_id": session_id,
                            }),
                        );

                        // Index the newly imported session
                        let search_state = app.state::<SearchState>();
                        let mut index = match search_state.0.lock() {
                            Ok(i) => i,
                            Err(_) => continue,
                        };
                        match search::indexer::index_session(&mut index, &project_id, &session_id, &conn) {
                            Ok(true) => {
                                let _ = app.emit("search-index-updated", serde_json::json!({
                                    "project_id": project_id,
                                    "reason": "session_imported",
                                    "docs_updated": 1,
                                }));
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to index imported session");
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => {} // already imported
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to import session from watcher event"
                        );
                    }
                }
            }
            WatchEvent::FileChanged { project_id, paths } => {
                let path_strs: Vec<String> =
                    paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
                let _ = app.emit(
                    "project-files-changed",
                    serde_json::json!({
                        "project_id": project_id,
                        "paths": path_strs,
                    }),
                );

                // Incrementally update search index for changed files
                let Some(project_path) = get_project_path(&app, &project_id) else {
                    continue;
                };
                let project_root = std::path::Path::new(&project_path);

                let search_state = app.state::<SearchState>();
                let mut index = match search_state.0.lock() {
                    Ok(i) => i,
                    Err(_) => continue,
                };
                let mut updated = 0;
                for path in &paths {
                    match search::indexer::update_file(&mut index, &project_id, project_root, path) {
                        Ok(true) => updated += 1,
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to update search index for file"
                            );
                        }
                        _ => {}
                    }
                }
                drop(index);
                if updated > 0 {
                    let _ = app.emit("search-index-updated", serde_json::json!({
                        "project_id": project_id,
                        "reason": "file_changed",
                        "docs_updated": updated,
                    }));
                }
            }
            WatchEvent::GitignoreChanged { project_id } => {
                tracing::info!(project_id, "gitignore changed — watch rebuild not yet implemented");
            }
        }
    }
}

/// On startup, re-seed last_activity_at from each project's latest git commit.
/// This corrects projects whose activity timestamp was incorrectly set to
/// registration time instead of actual last-commit time.
///
/// IMPORTANT: The DB lock is released between projects so frontend IPC commands
/// are not blocked during slow git operations (especially over the daemon).
fn startup_reseed_activity(app: &tauri::AppHandle) {
    // Snapshot the project list, then release the DB lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for activity reseed: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for activity reseed: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let provider_state = app.state::<ProviderState>();
    let db_state = app.state::<DbState>();

    let mut updated = 0;
    for project in &projects {
        let provider = provider_state.resolve(&project.path);

        // Do git I/O WITHOUT holding the DB lock
        let git_status = provider.git_status(&project.path).ok();
        let commit_time = provider.latest_commit_time(&project.path).ok().flatten();

        // Brief DB lock per project to write results
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(status) = git_status {
            let _ = db::queries::update_cached_git_status(
                &conn,
                &project.id,
                status.branch.as_deref(),
                status.is_dirty,
            );
        }

        if let Some(commit_time) = commit_time {
            let commit_ts = commit_time.to_rfc3339();
            if project.last_activity_at.as_deref() != Some(&commit_ts) {
                let _ = db::queries::update_project(
                    &conn,
                    &project.id,
                    None,
                    None,
                    None,
                    Some(Some(&commit_ts)),
                    None,
                );
                updated += 1;
            }
        }
        // conn lock dropped here — frontend can interleave
    }

    if updated > 0 {
        tracing::info!(updated, "Re-seeded activity timestamps from git");
    }
}

/// On startup, build the search index if it's empty.
///
/// Only holds locks briefly: checks doc count with search lock, then acquires
/// both locks for the rebuild if needed. The rebuild is a one-time operation
/// (subsequent startups skip it), so the longer hold is acceptable.
fn startup_search_index(app: &tauri::AppHandle) {
    // Check if index is already populated — brief lock
    {
        let search_state = app.state::<SearchState>();
        let index = match search_state.0.lock() {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("Failed to lock search index for startup build: {e}");
                return;
            }
        };

        let doc_count = index.doc_count().unwrap_or(0);
        if doc_count > 0 {
            tracing::info!(doc_count, "Search index already populated, skipping rebuild");
            return;
        }
        // search lock dropped here
    }

    // Index is empty — need to rebuild. This holds both locks but only happens
    // on first run (or after index wipe), so the brief block is acceptable.
    let search_state = app.state::<SearchState>();
    let mut index = match search_state.0.lock() {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("Failed to lock search index for rebuild: {e}");
            return;
        }
    };

    let db_state = app.state::<DbState>();
    let conn = match db_state.0.lock() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to lock DB for startup index build: {e}");
            return;
        }
    };

    match search::indexer::rebuild_all(&mut index, &conn) {
        Ok(total) => {
            tracing::info!(total, "Built initial search index");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build initial search index");
        }
    }
}

/// On startup, scan all registered projects for unimported session handoffs.
///
/// IMPORTANT: DB lock released between projects to avoid blocking frontend IPC.
fn startup_session_scan(app: &tauri::AppHandle) {
    // Snapshot project list, release lock immediately.
    let projects = {
        let db_state = app.state::<DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to lock DB for startup session scan: {e}");
                return;
            }
        };
        match db::queries::list_projects(&conn) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to list projects for startup scan: {e}");
                return;
            }
        }
        // conn lock dropped here
    };

    let db_state = app.state::<DbState>();

    for project in &projects {
        let project_root = std::path::Path::new(&project.path);

        // Brief lock per project for the import operation
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => continue,
        };

        match services::session_import::scan_and_import_sessions(&conn, &project.id, project_root) {
            Ok(imported) if !imported.is_empty() => {
                tracing::info!(
                    project = project.name,
                    count = imported.len(),
                    "Imported sessions on startup"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    project = project.name,
                    error = %e,
                    "Failed to scan sessions on startup"
                );
            }
        }
        // conn lock dropped here — frontend can interleave
    }
}

/// Start daemon event listener for WSL projects.
///
/// Opens a dedicated TCP connection to the daemon, sends `watch` commands for
/// each WSL project, then runs the event loop. Events are forwarded to the
/// shared watcher channel, where `process_watch_events` handles them identically
/// to local watcher events.
fn start_daemon_watches(
    daemon_addr: String,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    wsl_distro: Option<String>,
    projects: Vec<models::Project>,
) {
    let mut listener = match daemon::event_listener::DaemonEventListener::connect(
        &daemon_addr,
        event_tx,
    ) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to connect daemon event listener");
            return;
        }
    };

    // Register watches for all WSL projects
    let mut count = 0;
    for project in &projects {
        if !provider::path::is_wsl_path(&project.path) {
            continue;
        }

        // Convert UNC path to Linux path for the daemon
        let linux_path = match provider::path::wsl_unc_to_linux(&project.path) {
            Some(p) => p,
            None => {
                tracing::warn!(path = %project.path, "Cannot convert WSL path to Linux");
                continue;
            }
        };

        if let Err(e) = listener.watch(&project.id, &linux_path) {
            tracing::warn!(
                project = project.name,
                error = %e,
                "Failed to register daemon watch"
            );
        } else {
            count += 1;
        }
    }

    if count > 0 {
        tracing::info!(
            count,
            distro = ?wsl_distro,
            "Daemon watching WSL projects"
        );
        // Run blocks until daemon disconnects
        listener.run();
    }
}

/// Background thread that monitors daemon health via periodic pings.
///
/// On disconnect: attempts restart and reconnection (max 3 attempts per session).
/// Emits `daemon-status` events to the frontend for UI indicators.
fn daemon_health_check(app: tauri::AppHandle) {
    use std::time::Duration;

    const CHECK_INTERVAL: Duration = Duration::from_secs(30);
    const MAX_RESTART_ATTEMPTS: u32 = 3;

    let mut consecutive_failures: u32 = 0;
    let mut restart_attempts: u32 = 0;

    // Initial delay — let the app finish starting
    std::thread::sleep(Duration::from_secs(5));

    loop {
        std::thread::sleep(CHECK_INTERVAL);

        let provider_state = app.state::<ProviderState>();
        let Some(ref daemon) = provider_state.daemon else {
            return;
        };

        if daemon.is_connected() {
            match daemon.ping() {
                Ok(()) => {
                    if consecutive_failures > 0 {
                        tracing::debug!("Daemon health check recovered");
                    }
                    consecutive_failures = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        failures = consecutive_failures,
                        error = %e,
                        "Daemon health check failed"
                    );
                    if consecutive_failures >= 3 {
                        let _ = app.emit(
                            "daemon-status",
                            serde_json::json!({ "status": "disconnected" }),
                        );
                    }
                }
            }
        } else {
            // Daemon is disconnected — try to reconnect
            if restart_attempts >= MAX_RESTART_ATTEMPTS {
                tracing::warn!(
                    "Max daemon restart attempts reached ({MAX_RESTART_ATTEMPTS}), giving up"
                );
                let _ = app.emit(
                    "daemon-status",
                    serde_json::json!({ "status": "failed" }),
                );
                return;
            }

            let _ = app.emit(
                "daemon-status",
                serde_json::json!({ "status": "reconnecting" }),
            );

            // Try reconnecting to existing daemon first
            if daemon.reconnect().is_ok() {
                tracing::info!("Reconnected to daemon");
                consecutive_failures = 0;
                let _ = app.emit(
                    "daemon-status",
                    serde_json::json!({ "status": "connected" }),
                );
                continue;
            }

            // Try restarting daemon process
            restart_attempts += 1;
            tracing::info!(
                attempt = restart_attempts,
                max = MAX_RESTART_ATTEMPTS,
                "Attempting daemon restart"
            );

            let distro = provider_state.wsl_distro.as_deref();
            let port = daemon::server::DEFAULT_PORT;

            if let Some(d) = distro {
                if daemon::launcher::try_restart_daemon(d, port).is_ok() {
                    std::thread::sleep(Duration::from_secs(2));
                    if daemon.reconnect().is_ok() {
                        tracing::info!("Reconnected after daemon restart");
                        consecutive_failures = 0;
                        let _ = app.emit(
                            "daemon-status",
                            serde_json::json!({ "status": "connected" }),
                        );
                        continue;
                    }
                }
            }

            tracing::warn!(attempt = restart_attempts, "Daemon restart attempt failed");
        }
    }
}
