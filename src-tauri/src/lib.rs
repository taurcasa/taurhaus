mod commands;
mod config;
mod db;
mod models;

// Modules added incrementally as implemented:
// mod claude_code;
// mod fs;
// mod git;
// mod scanner;
// mod search;
// mod session;

use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .setup(|_app| {
            tracing::info!("taurhaus starting");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running taurhaus");
}
