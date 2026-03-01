// Deny unsafe code crate-wide. The one exception (libgit2 init) lives in
// git_init.rs with a scoped #[allow]. Any new `unsafe` block will fail compilation.
#![deny(unsafe_code)]

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

#[cfg(feature = "mesh-bridged-backend")]
pub mod coordination;

pub mod session_scanner;

pub mod task_scanner;

pub mod terminal;

pub mod platform;

use std::sync::Mutex;

use commands::projects::DbState;
use tauri::{Emitter, Manager};
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
        .plugin(tauri_plugin_window_state::Builder::new().build())
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

            let daemon_addr = daemon_provider.as_ref().map(|d| d.addr().to_string());

            app.manage(ProviderState {
                local: provider::local::LocalProvider,
                daemon: daemon_provider,
                wsl_distro: wsl_distro.clone(),
            });

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
                                        respawn_daemon_watches(&boot_handle);
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
                    daemon_health_check(health_handle, connected_at_startup);
                });
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
                let projects = db::queries::list_projects(
                    &db_state.0.lock().unwrap(),
                )
                .unwrap_or_default();

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
                    if let Some(home) = dirs::home_dir() {
                        let tasks_dir = home.join(".claude").join("tasks");
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
                startup_reseed_activity(&bg_handle);

                // Import any unimported sessions
                startup_session_scan(&bg_handle);

                // Build search index if empty
                startup_search_index(&bg_handle);

                // Seed task database from live sources
                startup_task_scan(&bg_handle);
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
            commands::daemon::get_platform,
            commands::daemon::get_daemon_status,
            commands::daemon::start_daemon,
            commands::daemon::stop_daemon,
            commands::daemon::check_daemon_install_status,
            commands::daemon::install_daemon,
            commands::logging::frontend_log,
            commands::command_center::list_claude_sessions,
            commands::command_center::launch_claude_session,
            commands::command_center::stop_claude_session,
            commands::command_center::navigate_to_session,
            commands::command_center::record_session_activity,
            commands::command_center::get_project_activity,
            commands::command_center::get_project_tasks,
            commands::command_center::get_task_detail,
            commands::command_center::get_archived_sessions,
            commands::command_center::get_commit_files,
            commands::command_center::get_commit_diff,
            commands::command_center::get_commits_in_range,
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
///
/// Uses **batch-and-flush** to coalesce the rapid event bursts that `notify`
/// produces for a single file edit (temp create → write → rename → cleanup).
///
/// Timing model:
/// - **Quiet window** (300 ms): if no new events arrive within this period
///   the batch is flushed. Resets with each incoming event.
/// - **Max-wait ceiling** (2 s): even if events keep arriving the batch is
///   flushed after this absolute deadline, preventing starvation.
///
/// The result: one `project-files-changed` Tauri event per edit operation
/// instead of 5–8, one search-index lock instead of 5–8, etc.
fn process_watch_events(
    rx: std::sync::mpsc::Receiver<fs::watcher::WatchEvent>,
    app: tauri::AppHandle,
) {
    use std::collections::{HashMap, HashSet};
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    use fs::watcher::WatchEvent;

    const QUIET_WINDOW: Duration = Duration::from_millis(300);
    const MAX_WAIT: Duration = Duration::from_millis(2000);

    // Spawn task scan thread with trailing-edge debounce (unchanged).
    let (task_trigger_tx, task_trigger_rx) = std::sync::mpsc::channel::<()>();
    let app_for_tasks = app.clone();
    std::thread::spawn(move || {
        task_scan_loop(task_trigger_rx, app_for_tasks);
    });

    // ── Batch accumulators ──────────────────────────────────────────────
    // Keyed by project_id. Cleared after each flush.
    struct Batch {
        file_paths: HashMap<String, Vec<std::path::PathBuf>>,
        git_projects: HashSet<String>,
        session_files: Vec<(String, std::path::PathBuf)>,
        gitignore_projects: HashSet<String>,
        activity_projects: HashSet<String>,
    }

    impl Batch {
        fn new() -> Self {
            Self {
                file_paths: HashMap::new(),
                git_projects: HashSet::new(),
                session_files: Vec::new(),
                gitignore_projects: HashSet::new(),
                activity_projects: HashSet::new(),
            }
        }

        fn is_empty(&self) -> bool {
            self.file_paths.is_empty()
                && self.git_projects.is_empty()
                && self.session_files.is_empty()
                && self.gitignore_projects.is_empty()
        }

        fn accumulate(&mut self, event: WatchEvent) {
            match event {
                WatchEvent::FileChanged { project_id, paths } => {
                    self.activity_projects.insert(project_id.clone());
                    self.file_paths.entry(project_id).or_default().extend(paths);
                }
                WatchEvent::GitChanged { project_id } => {
                    self.activity_projects.insert(project_id.clone());
                    self.git_projects.insert(project_id);
                }
                WatchEvent::SessionFileCreated { project_id, path } => {
                    self.activity_projects.insert(project_id.clone());
                    self.session_files.push((project_id, path));
                }
                WatchEvent::GitignoreChanged { project_id } => {
                    self.gitignore_projects.insert(project_id);
                }
            }
        }
    }

    // ── Main event loop ─────────────────────────────────────────────────
    loop {
        // Block until the first event arrives.
        let Ok(first) = rx.recv() else {
            break; // channel closed
        };

        // Fast-path: internal watch events (task directory) bypass batching.
        if is_internal_event(&first) {
            let _ = task_trigger_tx.send(());
            continue;
        }

        let mut batch = Batch::new();
        batch.accumulate(first);

        // Collect more events within the batch window.
        let batch_start = Instant::now();
        loop {
            let elapsed = batch_start.elapsed();
            if elapsed >= MAX_WAIT {
                break; // ceiling reached — flush regardless
            }
            let timeout = (MAX_WAIT - elapsed).min(QUIET_WINDOW);
            match rx.recv_timeout(timeout) {
                Ok(event) => {
                    if is_internal_event(&event) {
                        let _ = task_trigger_tx.send(());
                    } else {
                        batch.accumulate(event);
                    }
                }
                Err(RecvTimeoutError::Timeout) => break, // quiet window or ceiling
                Err(RecvTimeoutError::Disconnected) => break, // channel closed
            }
        }

        if batch.is_empty() {
            continue;
        }

        let batch_size: usize = batch.file_paths.values().map(|v| v.len()).sum::<usize>()
            + batch.git_projects.len()
            + batch.session_files.len();
        tracing::debug!(
            batch_size,
            file_projects = batch.file_paths.len(),
            git_projects = batch.git_projects.len(),
            sessions = batch.session_files.len(),
            elapsed_ms = batch_start.elapsed().as_millis() as u64,
            "flushing watch event batch"
        );

        // ── Bump last_activity_at once per project ──────────────────
        let db = app.state::<DbState>();
        if let Ok(conn) = db.0.lock() {
            for pid in &batch.activity_projects {
                let _ = services::project::touch_activity(&conn, pid);
            }
        }
        // db (tauri::State) is Copy — no need to drop

        // ── Git events (one per project) ────────────────────────────
        for project_id in &batch.git_projects {
            let Some(project_path) = get_project_path(&app, project_id) else {
                continue;
            };
            let path = std::path::Path::new(&project_path);

            let provider_state = app.state::<ProviderState>();
            let provider = provider_state.resolve(&project_path);
            if let Ok(status) = provider.git_status(&project_path) {
                let db = app.state::<DbState>();
                let conn = match db.0.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let _ = db::queries::update_cached_git_status(
                    &conn,
                    project_id,
                    status.branch.as_deref(),
                    status.is_dirty,
                );
                drop(conn);
                // db (tauri::State) is Copy — no need to drop
                let _ = app.emit(
                    "project-git-changed",
                    serde_json::json!({
                        "project_id": project_id,
                        "branch": status.branch,
                        "is_dirty": status.is_dirty,
                    }),
                );
            }

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(_) => continue,
            };
            match search::indexer::reindex_commits(&mut index, project_id, path, 50) {
                Ok(count) if count > 0 => {
                    let _ = app.emit(
                        "search-index-updated",
                        serde_json::json!({
                            "project_id": project_id,
                            "reason": "git_changed",
                            "docs_updated": count,
                        }),
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to reindex commits on git change");
                }
                _ => {}
            }
        }

        // ── Session file events ─────────────────────────────────────
        for (project_id, path) in &batch.session_files {
            let db = app.state::<DbState>();
            let conn = match db.0.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            match services::session_import::import_handoff(&conn, project_id, path) {
                Ok(Some(session_id)) => {
                    let _ = app.emit(
                        "session-imported",
                        serde_json::json!({
                            "project_id": project_id,
                            "session_id": session_id,
                        }),
                    );
                    let ss = app.state::<SearchState>();
                    let mut index = match ss.0.lock() {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    match search::indexer::index_session(&mut index, project_id, &session_id, &conn)
                    {
                        Ok(true) => {
                            let _ = app.emit(
                                "search-index-updated",
                                serde_json::json!({
                                    "project_id": project_id,
                                    "reason": "session_imported",
                                    "docs_updated": 1,
                                }),
                            );
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

        // ── File change events (one per project, all paths merged) ──
        for (project_id, paths) in &batch.file_paths {
            // Dedup paths within the batch (same file may appear in multiple events)
            let mut unique: Vec<&std::path::PathBuf> = paths.iter().collect();
            unique.sort();
            unique.dedup();

            let path_strs: Vec<String> = unique
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            let _ = app.emit(
                "project-files-changed",
                serde_json::json!({
                    "project_id": project_id,
                    "paths": path_strs,
                }),
            );

            // Incrementally update search index (one lock per project batch)
            let Some(project_path) = get_project_path(&app, project_id) else {
                continue;
            };
            let project_root = std::path::Path::new(&project_path);

            let ss = app.state::<SearchState>();
            let mut index = match ss.0.lock() {
                Ok(i) => i,
                Err(_) => continue,
            };
            let mut updated = 0;
            for path in &unique {
                match search::indexer::update_file(&mut index, project_id, project_root, path) {
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
                let _ = app.emit(
                    "search-index-updated",
                    serde_json::json!({
                        "project_id": project_id,
                        "reason": "file_changed",
                        "docs_updated": updated,
                    }),
                );
            }
        }

        // ── Gitignore changes ───────────────────────────────────────
        for project_id in &batch.gitignore_projects {
            tracing::info!(
                project_id,
                "gitignore changed — watch rebuild not yet implemented"
            );
        }
    }
}

/// Check if a watch event is an internal event (task directory etc.) that
/// should bypass batching and trigger the task scan thread instead.
fn is_internal_event(event: &fs::watcher::WatchEvent) -> bool {
    use fs::watcher::WatchEvent;
    matches!(
        event,
        WatchEvent::FileChanged { project_id, .. }
        | WatchEvent::GitChanged { project_id }
            if project_id.starts_with("__")
    )
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

    // Notify frontend that cached git data is now fresh — it may have loaded
    // the project list before the reseed completed (race on first launch).
    let _ = app.emit("projects-reseed-complete", ());
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
            tracing::info!(
                doc_count,
                "Search index already populated, skipping rebuild"
            );
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

/// On startup, scan all registered projects' tasks and seed the SQLite database.
///
/// This ensures the first frontend read has data. Subsequent updates are
/// event-driven (daemon watches `~/.claude/tasks/`).
fn startup_task_scan(app: &tauri::AppHandle) {
    sync_all_project_tasks(app);
}

/// Background thread that handles task re-scanning with trailing-edge debounce.
///
/// Waits for a trigger signal (from file watcher events), then drains additional
/// signals for 2 seconds. After the debounce window closes, scans all projects'
/// tasks and persists to SQLite. This ensures rapid task file changes (e.g.,
/// Claude creating 4 tasks at once) result in only one scan.
fn task_scan_loop(rx: std::sync::mpsc::Receiver<()>, app: tauri::AppHandle) {
    use std::time::{Duration, Instant};
    const DEBOUNCE: Duration = Duration::from_secs(2);

    loop {
        // Wait for first trigger
        if rx.recv().is_err() {
            break;
        }

        // Trailing-edge debounce: drain for 2 seconds
        let deadline = Instant::now() + DEBOUNCE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(()) => {} // More triggers, keep draining
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }

        // Scan all projects' tasks
        sync_all_project_tasks(&app);
    }
}

/// Scan tasks for all registered projects, persist to SQLite, and notify frontend.
///
/// Called from both the startup seed and the event-driven scan loop.
fn sync_all_project_tasks(app: &tauri::AppHandle) {
    let db_state = app.state::<DbState>();
    let provider_state = app.state::<ProviderState>();

    // Snapshot project list (brief DB lock)
    let projects = {
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        db::queries::list_projects(&conn).unwrap_or_default()
    };

    let mut total_tasks = 0;
    for project in &projects {
        // Scan tasks from files (daemon or local)
        let scan_result =
            commands::command_center::scan_tasks_from_files(&provider_state, &project.path);

        if scan_result.tasks.is_empty() {
            continue;
        }

        // Normalize path for DB storage
        let normalized_path =
            provider::path::to_linux(&project.path).unwrap_or_else(|| project.path.clone());

        // Persist to SQLite (brief DB lock per project)
        {
            let conn = match db_state.0.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            commands::command_center::persist_task_scan(&conn, &normalized_path, &scan_result);
        }

        total_tasks += scan_result.tasks.len();

        // Emit per-project event to frontend
        let _ = app.emit(
            "project-tasks-changed",
            serde_json::json!({
                "project_id": project.id,
                "task_count": scan_result.tasks.len(),
            }),
        );
    }

    if total_tasks > 0 {
        tracing::debug!(total_tasks, projects = projects.len(), "Task sync complete");
    }
}

/// Extract the WSL home directory from a Linux path.
///
/// `/home/user/projects/foo` → `/home/user`
fn extract_wsl_home(linux_path: &str) -> Option<String> {
    let parts: Vec<&str> = linux_path.splitn(4, '/').collect();
    if parts.len() >= 3 && parts[1] == "home" {
        Some(format!("/{}/{}", parts[1], parts[2]))
    } else {
        None
    }
}

/// Start daemon event listener for WSL projects.
///
/// Opens a dedicated TCP connection to the daemon, sends `watch` commands for
/// each WSL project, then runs the event loop. Events are forwarded to the
/// shared watcher channel, where `process_watch_events` handles them identically
/// to local watcher events.
///
/// On macOS/Linux (native daemon), this is a no-op — all project paths are local
/// and the local watcher handles them. The function still runs for consistency
/// but registers zero watches and exits immediately.
fn start_daemon_watches(
    daemon_addr: String,
    event_tx: std::sync::mpsc::Sender<fs::watcher::WatchEvent>,
    wsl_distro: Option<String>,
    projects: Vec<models::Project>,
) {
    let mut listener =
        match daemon::event_listener::DaemonEventListener::connect(&daemon_addr, event_tx) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to connect daemon event listener");
                return;
            }
        };

    // Register watches for all WSL projects
    let mut count = 0;
    let mut wsl_home: Option<String> = None;
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

        // Extract WSL home from first successful conversion
        if wsl_home.is_none() {
            wsl_home = extract_wsl_home(&linux_path);
        }

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

    // Watch Claude task directories for event-driven task sync.
    // Uses a special "__claude_tasks__" project ID that process_watch_events
    // intercepts to trigger background task scanning instead of normal file handling.
    if let Some(ref home) = wsl_home {
        let claude_tasks_dir = format!("{home}/.claude/tasks");
        if let Err(e) = listener.watch("__claude_tasks__", &claude_tasks_dir) {
            tracing::debug!(
                error = %e,
                path = %claude_tasks_dir,
                "Could not watch Claude tasks directory (may not exist yet)"
            );
        } else {
            tracing::info!(path = %claude_tasks_dir, "Watching Claude tasks directory (daemon)");
        }
    }

    if count > 0 || wsl_home.is_some() {
        tracing::info!(
            count,
            distro = ?wsl_distro,
            "Daemon watching WSL projects"
        );
        // Run blocks until daemon disconnects
        listener.run();
    }
}

/// Re-register all daemon watches after a reconnection.
///
/// Spawns a new `start_daemon_watches` thread using current project list from DB.
/// The old event listener thread has already exited (daemon connection was lost),
/// so this creates a fresh TCP connection for the event stream.
fn respawn_daemon_watches(app: &tauri::AppHandle) {
    let provider_state = app.state::<ProviderState>();
    let Some(ref daemon) = provider_state.daemon else {
        return;
    };
    let daemon_addr = daemon.addr().to_string();
    let distro = provider_state.wsl_distro.clone();

    let watcher_state = app.state::<WatcherState>();
    let event_tx = match watcher_state.0.lock() {
        Ok(w) => w.event_sender(),
        Err(_) => return,
    };

    let db_state = app.state::<commands::projects::DbState>();
    let projects = match db_state.0.lock() {
        Ok(conn) => db::queries::list_projects(&conn).unwrap_or_default(),
        Err(_) => return,
    };

    tracing::info!(
        project_count = projects.len(),
        "Re-registering daemon watches after reconnection"
    );

    std::thread::spawn(move || {
        start_daemon_watches(daemon_addr, event_tx, distro, projects);
    });

    // Also re-scan sessions that may have been missed while disconnected
    {
        let db_state = app.state::<commands::projects::DbState>();
        let conn = match db_state.0.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let all_projects = db::queries::list_projects(&conn).unwrap_or_default();
        for project in &all_projects {
            let root = if provider::path::is_wsl_path(&project.path) {
                provider::path::wsl_unc_to_linux(&project.path).map(std::path::PathBuf::from)
            } else {
                Some(std::path::PathBuf::from(&project.path))
            };
            if let Some(root) = root {
                match services::session_import::scan_and_import_sessions(&conn, &project.id, &root)
                {
                    Ok(imported) if !imported.is_empty() => {
                        tracing::info!(
                            project = project.name,
                            count = imported.len(),
                            "Imported missed sessions after reconnection"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Background thread that monitors daemon health via periodic pings.
///
/// On disconnect: attempts restart and reconnection (max 3 attempts per session).
/// Emits `daemon-status` events to the frontend for UI indicators.
/// Works for both initially-connected and initially-disconnected providers.
fn daemon_health_check(app: tauri::AppHandle, connected_at_startup: bool) {
    use std::time::Duration;

    const CHECK_INTERVAL: Duration = Duration::from_secs(30);
    /// Shorter interval when daemon hasn't connected yet (first-time connect).
    const FAST_CHECK_INTERVAL: Duration = Duration::from_secs(2);
    const MAX_RESTART_ATTEMPTS: u32 = 3;

    let mut consecutive_failures: u32 = 0;
    let mut restart_attempts: u32 = 0;
    let mut ever_connected = connected_at_startup;

    // Initial delay — let the app finish starting.
    // Short delay when daemon wasn't connected: it was already spawned in setup(),
    // just needs a moment to bind the port. Long delay when already connected:
    // no urgency, just periodic health monitoring.
    let initial_delay = if connected_at_startup {
        Duration::from_secs(5)
    } else {
        Duration::from_secs(1)
    };
    std::thread::sleep(initial_delay);

    loop {
        // Use shorter interval while waiting for first connection
        let interval = if ever_connected {
            CHECK_INTERVAL
        } else {
            FAST_CHECK_INTERVAL
        };
        std::thread::sleep(interval);

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
                let _ = app.emit("daemon-status", serde_json::json!({ "status": "failed" }));
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
                restart_attempts = 0;
                ever_connected = true;
                respawn_daemon_watches(&app);
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
                        restart_attempts = 0;
                        ever_connected = true;
                        respawn_daemon_watches(&app);
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
