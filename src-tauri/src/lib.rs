mod commands;
mod config;
pub mod db;
pub mod errors;
pub mod models;
pub mod services;

pub mod git;

pub mod fs;

pub mod session;

// Modules added incrementally as implemented:
// mod claude_code;
// mod scanner;
// mod search;

use std::sync::Mutex;

use commands::projects::DbState;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running taurhaus");
}
