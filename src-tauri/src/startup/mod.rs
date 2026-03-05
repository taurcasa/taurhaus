use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Map, Value};
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

struct StartupPhaseSpan {
    phase: &'static str,
    started_at: Instant,
    daemon_addr: Option<String>,
    connected_at_startup: Option<bool>,
}

impl StartupPhaseSpan {
    fn start(
        phase: &'static str,
        daemon_addr: Option<String>,
        connected_at_startup: Option<bool>,
    ) -> Self {
        emit_startup_phase_event(
            "info",
            "startup.phase.started",
            phase,
            None,
            daemon_addr.clone(),
            connected_at_startup,
            None,
            None,
        );
        Self {
            phase,
            started_at: Instant::now(),
            daemon_addr,
            connected_at_startup,
        }
    }

    fn complete(&self) {
        emit_startup_phase_event(
            "info",
            "startup.phase.completed",
            self.phase,
            Some(self.started_at.elapsed().as_millis() as u64),
            self.daemon_addr.clone(),
            self.connected_at_startup,
            None,
            None,
        );
    }

    fn fail(&self, error_code: &str, error_message: &str) {
        emit_startup_phase_event(
            "error",
            "startup.phase.failed",
            self.phase,
            Some(self.started_at.elapsed().as_millis() as u64),
            self.daemon_addr.clone(),
            self.connected_at_startup,
            Some(error_code),
            Some(error_message),
        );
    }
}

fn emit_startup_phase_event(
    level: &str,
    event: &str,
    phase: &str,
    duration_ms: Option<u64>,
    daemon_addr: Option<String>,
    connected_at_startup: Option<bool>,
    error_code: Option<&str>,
    error_message: Option<&str>,
) {
    let mut fields = Map::new();
    fields.insert("phase".to_string(), Value::String(phase.to_string()));
    if let Some(duration_ms) = duration_ms {
        fields.insert(
            "duration_ms".to_string(),
            Value::Number(serde_json::Number::from(duration_ms)),
        );
    }
    if let Some(daemon_addr) = daemon_addr {
        fields.insert("daemon_addr".to_string(), Value::String(daemon_addr));
    }
    if let Some(connected_at_startup) = connected_at_startup {
        fields.insert(
            "connected_at_startup".to_string(),
            Value::Bool(connected_at_startup),
        );
    }
    if let Some(error_code) = error_code {
        fields.insert(
            "error.code".to_string(),
            Value::String(error_code.to_string()),
        );
    }
    if let Some(error_message) = error_message {
        fields.insert(
            "error.message".to_string(),
            Value::String(error_message.to_string()),
        );
    }
    commands::logging::emit_global(
        level,
        "backend",
        event,
        Some("Startup phase lifecycle event".to_string()),
        fields,
    );
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("taurhaus starting");

    let setup_paths = initialize_paths_and_logging(app)?;
    let bootstrap_phase = StartupPhaseSpan::start("bootstrap", None, None);
    let conn = match initialize_database(&setup_paths) {
        Ok(conn) => conn,
        Err(error) => {
            bootstrap_phase.fail("STARTUP_DATABASE_INIT_FAILED", &error.to_string());
            return Err(error);
        }
    };
    let daemon_phase = determine_daemon_phase(&conn);
    let context = build_setup_context(&setup_paths, &daemon_phase);

    register_managed_state(app, conn, &setup_paths, daemon_phase);
    bootstrap_phase.complete();
    run_startup_orchestration(app, &context)?;

    tracing::info!(db_path = %context.db_path.display(), "database initialized");
    Ok(())
}

fn initialize_paths_and_logging(
    app: &mut tauri::App,
) -> Result<SetupPaths, Box<dyn std::error::Error>> {
    let data_dir = resolve_app_data_dir(app.handle().clone())?;
    std::fs::create_dir_all(&data_dir)?;

    let log_path = commands::logging::jsonl_log_path(&data_dir);
    let log_state = commands::logging::LogFileState::new(log_path.clone())?;
    commands::logging::install_global_sink(&log_state);
    tracing::info!(?log_path, "Log file ready");
    app.manage(log_state);

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
    let daemon_phase = StartupPhaseSpan::start(
        "daemon",
        context.daemon_addr.clone(),
        Some(context.daemon_connected_at_startup),
    );
    daemon::spawn_background_bootstrap(app.handle().clone(), context);
    daemon::start_runtime_monitors(app.handle().clone(), context);
    daemon_phase.complete();

    let watchers_phase = StartupPhaseSpan::start(
        "watchers",
        context.daemon_addr.clone(),
        Some(context.daemon_connected_at_startup),
    );
    if let Err(error) = watchers::initialize(app, context) {
        watchers_phase.fail("STARTUP_WATCHERS_INIT_FAILED", &error.to_string());
        return Err(error);
    }
    watchers_phase.complete();

    let search_phase = StartupPhaseSpan::start(
        "search",
        context.daemon_addr.clone(),
        Some(context.daemon_connected_at_startup),
    );
    if let Err(error) = search::initialize(app, context) {
        search_phase.fail("STARTUP_SEARCH_INIT_FAILED", &error.to_string());
        return Err(error);
    }
    search_phase.complete();

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
