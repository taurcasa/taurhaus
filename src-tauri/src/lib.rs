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

// Modules added incrementally as implemented:
// mod claude_code;
// mod scanner;

use std::sync::Mutex;

use commands::projects::DbState;
use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

/// Managed state: holds the file watcher so it lives for the app lifetime.
pub struct WatcherState(pub Mutex<fs::watcher::ProjectWatcher>);

/// Managed state: holds the tantivy search index.
pub struct SearchState(pub Mutex<search::indexer::SearchIndex>);

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .setup(|app| {
            tracing::info!("taurhaus starting");

            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app_data_dir");
            std::fs::create_dir_all(&data_dir).expect("failed to create data directory");

            let db_path = data_dir.join("taurhaus.db");
            let conn = db::init_db(&db_path).expect("failed to initialize database");
            app.manage(DbState(Mutex::new(conn)));

            // Start file watcher
            let (watcher, rx) = fs::watcher::ProjectWatcher::new();
            app.manage(WatcherState(Mutex::new(watcher)));

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

            // Import any unimported sessions for registered projects
            startup_session_scan(app);

            // Build search index if empty
            startup_search_index(app);

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
            commands::git::get_recent_commits,
            commands::git::get_all_commits,
            commands::git::get_git_status,
            commands::files::get_file_tree,
            commands::files::read_file,
            commands::files::get_readme,
            commands::sessions::get_latest_session,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            commands::search::search,
            commands::search::get_index_status,
            commands::search::rebuild_index,
        ])
        .run(tauri::generate_context!())
        .expect("error while running taurhaus");
}

/// Process file watcher events on a background thread.
/// Emits Tauri events to the frontend and triggers session imports.
fn process_watch_events(
    rx: std::sync::mpsc::Receiver<fs::watcher::WatchEvent>,
    app: tauri::AppHandle,
) {
    use fs::watcher::WatchEvent;

    for event in rx {
        match event {
            WatchEvent::GitChanged { project_id } => {
                // Re-read git status and emit to frontend
                let db_state = app.state::<DbState>();
                let conn = match db_state.0.lock() {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let project = match db::queries::get_project(&conn, &project_id) {
                    Ok(Some(p)) => p,
                    _ => continue,
                };
                drop(conn);

                let status = git::status::get_status(std::path::Path::new(&project.path));
                if let Ok(status) = status {
                    let _ = app.emit(
                        "project-git-changed",
                        serde_json::json!({
                            "project_id": project_id,
                            "branch": status.branch,
                            "is_dirty": status.is_dirty,
                        }),
                    );
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
            }
            WatchEvent::GitignoreChanged { project_id } => {
                tracing::info!(project_id, "gitignore changed — watch rebuild not yet implemented");
            }
        }
    }
}

/// On startup, build the search index if it's empty.
fn startup_search_index(app: &tauri::App) {
    let search_state = app.state::<SearchState>();
    let mut index = match search_state.0.lock() {
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
fn startup_session_scan(app: &tauri::App) {
    let db_state = app.state::<DbState>();
    let conn = match db_state.0.lock() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to lock DB for startup session scan: {e}");
            return;
        }
    };

    let projects = match db::queries::list_projects(&conn) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to list projects for startup scan: {e}");
            return;
        }
    };

    for project in &projects {
        let project_root = std::path::Path::new(&project.path);
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
    }
}
