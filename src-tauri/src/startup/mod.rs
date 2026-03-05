use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;

use crate::commands;
use crate::commands::projects::DbState;
use crate::{db, provider, services, ProviderState};

pub(crate) mod bootstrap;
pub(crate) mod daemon;
pub(crate) mod search;
pub(crate) mod watchers;

const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

#[derive(Clone)]
pub(crate) struct SetupContext {
    pub(crate) data_dir: PathBuf,
    pub(crate) log_path: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) wsl_distro: Option<String>,
    pub(crate) daemon_addr: Option<String>,
    pub(crate) daemon_connected_at_startup: bool,
}

struct SetupPaths {
    data_dir: PathBuf,
    log_path: PathBuf,
    db_path: PathBuf,
}

struct DaemonPhase {
    wsl_distro: Option<String>,
    daemon_provider: Option<provider::daemon_client::DaemonProvider>,
    daemon_connected_at_startup: bool,
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("taurhaus starting");

    let setup_paths = initialize_paths_and_logging(app)?;
    let conn = initialize_database(&setup_paths)?;
    let daemon_phase = determine_daemon_phase(&conn);
    let context = build_setup_context(&setup_paths, &daemon_phase);

    register_managed_state(app, conn, &setup_paths, daemon_phase);
    run_startup_orchestration(app, &context)?;

    tracing::info!(db_path = %context.db_path.display(), "database initialized");
    Ok(())
}

fn initialize_paths_and_logging(
    app: &mut tauri::App,
) -> Result<SetupPaths, Box<dyn std::error::Error>> {
    let data_dir = resolve_app_data_dir(app.handle().clone())?;
    std::fs::create_dir_all(&data_dir)?;

    let log_path = data_dir.join("taurhaus.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;
    tracing::info!(?log_path, "Log file ready");
    app.manage(commands::logging::LogFileState(Mutex::new(log_file)));

    Ok(SetupPaths {
        db_path: data_dir.join("taurhaus.db"),
        data_dir,
        log_path,
    })
}

fn initialize_database(
    setup_paths: &SetupPaths,
) -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    Ok(db::init_db(&setup_paths.db_path)?)
}

fn determine_daemon_phase(conn: &rusqlite::Connection) -> DaemonPhase {
    let wsl_distro = detect_wsl_distro(conn);
    let (daemon_provider, daemon_connected_at_startup) = connect_daemon_provider(&wsl_distro);
    DaemonPhase {
        wsl_distro,
        daemon_provider,
        daemon_connected_at_startup,
    }
}

fn detect_wsl_distro(conn: &rusqlite::Connection) -> Option<String> {
    let projects = db::queries::list_projects(conn).unwrap_or_default();
    if crate::daemon::launcher::is_native_daemon() {
        Some("native".to_string())
    } else {
        projects
            .iter()
            .find_map(|project| provider::path::wsl_distro_from_path(&project.path))
    }
}

fn connect_daemon_provider(
    wsl_distro: &Option<String>,
) -> (Option<provider::daemon_client::DaemonProvider>, bool) {
    if wsl_distro.is_none() {
        return (None, false);
    }

    let port = crate::daemon::server::DEFAULT_PORT;
    let addr = format!("127.0.0.1:{port}");
    match provider::daemon_client::DaemonProvider::connect(&addr) {
        Ok(provider) => {
            tracing::info!("Connected to existing daemon (fast path)");
            (Some(provider), true)
        }
        Err(_) => {
            tracing::info!(addr, "Daemon not running — will start in background");
            (
                Some(provider::daemon_client::DaemonProvider::new_disconnected(
                    &addr,
                )),
                false,
            )
        }
    }
}

fn build_setup_context(setup_paths: &SetupPaths, daemon_phase: &DaemonPhase) -> SetupContext {
    let daemon_addr = daemon_phase
        .daemon_provider
        .as_ref()
        .map(|daemon| daemon.addr().to_string());

    SetupContext {
        data_dir: setup_paths.data_dir.clone(),
        log_path: setup_paths.log_path.clone(),
        db_path: setup_paths.db_path.clone(),
        wsl_distro: daemon_phase.wsl_distro.clone(),
        daemon_addr,
        daemon_connected_at_startup: daemon_phase.daemon_connected_at_startup,
    }
}

fn register_managed_state(
    app: &mut tauri::App,
    conn: rusqlite::Connection,
    setup_paths: &SetupPaths,
    daemon_phase: DaemonPhase,
) {
    app.manage(DbState(Mutex::new(conn)));
    app.manage(services::task_sync::TaskScanGenerationState::default());
    app.manage(commands::templates::TemplateStoreState::new(
        setup_paths.data_dir.clone(),
    ));

    app.manage(ProviderState {
        local: provider::local::LocalProvider,
        daemon: daemon_phase.daemon_provider,
        wsl_distro: daemon_phase.wsl_distro,
    });

    #[cfg(feature = "mesh-bridged-backend")]
    app.manage(crate::coordination::state::CoordinationState::for_app_startup());
}

fn run_startup_orchestration(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    daemon::spawn_background_bootstrap(app.handle().clone(), context);
    daemon::start_runtime_monitors(app.handle().clone(), context);
    watchers::initialize(app, context)?;
    search::initialize(app, context)?;
    bootstrap::spawn_background_startup_tasks(app.handle().clone());
    Ok(())
}

fn env_path_override(var: &str) -> Option<PathBuf> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn resolve_app_data_dir(app: tauri::AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = env_path_override(DATA_DIR_OVERRIDE_ENV) {
        tracing::info!(
            env = DATA_DIR_OVERRIDE_ENV,
            path = %path.display(),
            "Using app data dir override"
        );
        return Ok(path);
    }

    app.path().app_data_dir().map_err(|error| {
        io::Error::other(format!("failed to resolve app_data_dir: {error}")).into()
    })
}

pub(crate) fn resolve_claude_tasks_dir() -> Option<PathBuf> {
    if let Some(path) = env_path_override(CLAUDE_DIR_OVERRIDE_ENV) {
        return Some(path.join("tasks"));
    }
    dirs::home_dir().map(|home| home.join(".claude").join("tasks"))
}
