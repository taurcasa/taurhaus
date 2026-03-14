use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tauri::Manager;

use crate::commands;
use crate::commands::projects::DbState;
use crate::{db, provider, services, ProviderState};

pub(crate) mod bootstrap;
#[cfg(feature = "mesh-bridged-backend")]
pub(crate) mod compaction;
pub(crate) mod daemon;
pub(crate) mod search;
pub(crate) mod watchers;

const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const STARTUP_DAEMON_FAST_PATH_TIMEOUT: Duration = Duration::from_millis(350);

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

#[cfg(test)]
#[derive(Debug)]
struct StartupOrchestrationReport {
    daemon_watch_bootstrap: bool,
    search_doc_count: u64,
}

#[cfg(test)]
struct StartupOrchestrationHooks<
    SpawnBootstrap,
    StartRuntimeMonitors,
    InitializeWatchers,
    InitializeCompaction,
    InitializeSearch,
    SpawnBackgroundTasks,
> {
    spawn_background_bootstrap: SpawnBootstrap,
    start_runtime_monitors: StartRuntimeMonitors,
    initialize_watchers: InitializeWatchers,
    initialize_compaction: InitializeCompaction,
    initialize_search: InitializeSearch,
    spawn_background_tasks: SpawnBackgroundTasks,
}

#[cfg(test)]
#[derive(Debug, thiserror::Error)]
enum StartupOrchestrationError {
    #[error("watchers init failed: {source}")]
    Watchers {
        #[source]
        source: Box<dyn std::error::Error>,
    },
    #[error("compaction init failed: {source}")]
    Compaction {
        #[source]
        source: Box<dyn std::error::Error>,
    },
    #[error("search init failed: {source}")]
    Search {
        #[source]
        source: Box<dyn std::error::Error>,
    },
}

fn emit_startup_event(level: &str, event: &str, message: &'static str, fields: Map<String, Value>) {
    commands::logging::emit_global(level, "backend", event, Some(message.to_string()), fields);
}

fn startup_base_fields() -> Map<String, Value> {
    Map::new()
}

fn emit_startup_app_started() {
    let mut fields = startup_base_fields();
    fields.insert(
        "app_version".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    fields.insert("platform".to_string(), Value::String(platform.to_string()));
    fields.insert(
        "pid".to_string(),
        Value::Number(serde_json::Number::from(std::process::id())),
    );
    emit_startup_event(
        "info",
        "startup.app.started",
        "Startup bootstrap entered",
        fields,
    );
}

fn emit_startup_paths_resolved(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "data_dir".to_string(),
        Value::String(setup_paths.data_dir.display().to_string()),
    );
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "log_path".to_string(),
        Value::String(setup_paths.log_path.display().to_string()),
    );
    fields.insert(
        "used_data_dir_override".to_string(),
        Value::Bool(env_path_override(DATA_DIR_OVERRIDE_ENV).is_some()),
    );
    fields.insert(
        "used_claude_dir_override".to_string(),
        Value::Bool(env_path_override(CLAUDE_DIR_OVERRIDE_ENV).is_some()),
    );
    emit_startup_event(
        "info",
        "startup.paths.resolved",
        "Startup paths resolved",
        fields,
    );
}

fn emit_startup_logging_initialized(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "log_path".to_string(),
        Value::String(setup_paths.log_path.display().to_string()),
    );
    fields.insert("format".to_string(), Value::String("jsonl".to_string()));
    fields.insert("rotation_enabled".to_string(), Value::Bool(true));
    emit_startup_event(
        "info",
        "startup.logging.initialized",
        "Startup logging sink initialized",
        fields,
    );
}

fn emit_startup_database_started(setup_paths: &SetupPaths) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    emit_startup_event(
        "info",
        "startup.database.started",
        "Startup database initialization started",
        fields,
    );
}

fn emit_startup_database_completed(setup_paths: &SetupPaths, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    fields.insert(
        "migration_count".to_string(),
        Value::Number(serde_json::Number::from(0_u64)),
    );
    emit_startup_event(
        "info",
        "startup.database.completed",
        "Startup database initialization completed",
        fields,
    );
}

fn emit_startup_database_failed(setup_paths: &SetupPaths, error: &str) {
    let mut fields = startup_base_fields();
    fields.insert(
        "db_path".to_string(),
        Value::String(setup_paths.db_path.display().to_string()),
    );
    fields.insert(
        "error.code".to_string(),
        Value::String("STARTUP_DATABASE_INIT_FAILED".to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error.to_string()),
    );
    emit_startup_event(
        "error",
        "startup.database.failed",
        "Startup database initialization failed",
        fields,
    );
}

fn emit_startup_daemon_phase_started() {
    let fields = startup_base_fields();
    emit_startup_event(
        "info",
        "startup.daemon_phase.started",
        "Startup daemon phase determination started",
        fields,
    );
}

fn emit_startup_daemon_phase_completed(
    context: &SetupContext,
    wsl_distro: Option<&str>,
    duration_ms: u64,
) {
    let mut fields = startup_base_fields();
    if let Some(wsl_distro) = wsl_distro {
        fields.insert(
            "wsl_distro".to_string(),
            Value::String(wsl_distro.to_string()),
        );
    }
    if let Some(daemon_addr) = context.daemon_addr.as_ref() {
        fields.insert(
            "daemon_addr".to_string(),
            Value::String(daemon_addr.clone()),
        );
    }
    fields.insert(
        "daemon_connected_at_startup".to_string(),
        Value::Bool(context.daemon_connected_at_startup),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.daemon_phase.completed",
        "Startup daemon phase determination completed",
        fields,
    );
}

fn emit_startup_daemon_connect_succeeded(daemon_addr: &str, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "daemon_addr".to_string(),
        Value::String(daemon_addr.to_string()),
    );
    fields.insert("mode".to_string(), Value::String("fast_path".to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.daemon_connect.succeeded",
        "Startup daemon fast-path connect succeeded",
        fields,
    );
}

fn emit_startup_daemon_connect_deferred(
    daemon_addr: &str,
    wsl_distro: Option<&str>,
    reason: &str,
    duration_ms: u64,
) {
    let mut fields = startup_base_fields();
    fields.insert(
        "daemon_addr".to_string(),
        Value::String(daemon_addr.to_string()),
    );
    if let Some(wsl_distro) = wsl_distro {
        fields.insert(
            "wsl_distro".to_string(),
            Value::String(wsl_distro.to_string()),
        );
    }
    fields.insert("reason".to_string(), Value::String(reason.to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "warn",
        "startup.daemon_connect.deferred",
        "Startup daemon fast-path connect deferred",
        fields,
    );
}

fn emit_startup_orchestration_started() {
    let mut fields = startup_base_fields();
    let steps = vec![
        Value::String("daemon".to_string()),
        Value::String("watchers".to_string()),
        Value::String("compaction".to_string()),
        Value::String("search".to_string()),
        Value::String("background_tasks".to_string()),
    ];
    fields.insert("steps".to_string(), Value::Array(steps));
    emit_startup_event(
        "info",
        "startup.orchestration.started",
        "Startup orchestration started",
        fields,
    );
}

fn emit_startup_orchestration_completed(duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.orchestration.completed",
        "Startup orchestration completed",
        fields,
    );
}

fn emit_startup_watchers_initialized(
    duration_ms: u64,
    local_watcher_enabled: bool,
    daemon_watch_bootstrap: bool,
) {
    let mut fields = startup_base_fields();
    fields.insert(
        "local_watcher_enabled".to_string(),
        Value::Bool(local_watcher_enabled),
    );
    fields.insert(
        "daemon_watch_bootstrap".to_string(),
        Value::Bool(daemon_watch_bootstrap),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.watchers.initialized",
        "Startup watchers initialized",
        fields,
    );
}

fn emit_startup_search_initialized(index_path: PathBuf, doc_count: u64, duration_ms: u64) {
    let mut fields = startup_base_fields();
    fields.insert(
        "index_path".to_string(),
        Value::String(index_path.display().to_string()),
    );
    fields.insert(
        "doc_count".to_string(),
        Value::Number(serde_json::Number::from(doc_count)),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    emit_startup_event(
        "info",
        "startup.search.initialized",
        "Startup search initialized",
        fields,
    );
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("taurhaus starting");

    let setup_paths = initialize_paths_and_logging(app)?;
    emit_startup_app_started();
    emit_startup_paths_resolved(&setup_paths);
    emit_startup_logging_initialized(&setup_paths);
    emit_startup_database_started(&setup_paths);
    let database_started_at = Instant::now();
    let conn = match initialize_database(&setup_paths) {
        Ok(conn) => conn,
        Err(error) => {
            emit_startup_database_failed(&setup_paths, &error.to_string());
            return Err(error);
        }
    };
    emit_startup_database_completed(
        &setup_paths,
        database_started_at.elapsed().as_millis() as u64,
    );
    emit_startup_daemon_phase_started();
    let daemon_phase_started_at = Instant::now();
    let daemon_phase = determine_daemon_phase(&conn, &setup_paths.log_path);
    let context = build_setup_context(&setup_paths, &daemon_phase);
    emit_startup_daemon_phase_completed(
        &context,
        daemon_phase.wsl_distro.as_deref(),
        daemon_phase_started_at.elapsed().as_millis() as u64,
    );

    register_managed_state(app, conn, &setup_paths, daemon_phase);
    emit_startup_orchestration_started();
    let orchestration_started_at = Instant::now();
    run_startup_orchestration(app, &context)?;
    emit_startup_orchestration_completed(orchestration_started_at.elapsed().as_millis() as u64);

    tracing::info!(db_path = %context.db_path.display(), "database initialized");
    Ok(())
}

fn initialize_paths_and_logging(
    app: &mut tauri::App,
) -> Result<SetupPaths, Box<dyn std::error::Error>> {
    let data_dir = resolve_app_data_dir(app.handle().clone())?;
    std::fs::create_dir_all(&data_dir)?;
    std::env::set_var(DATA_DIR_OVERRIDE_ENV, &data_dir);

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

fn determine_daemon_phase(conn: &rusqlite::Connection, log_path: &std::path::Path) -> DaemonPhase {
    let wsl_distro = detect_wsl_distro(conn);
    let (daemon_provider, daemon_connected_at_startup) =
        connect_daemon_provider(&wsl_distro, log_path);
    DaemonPhase {
        wsl_distro,
        daemon_provider,
        daemon_connected_at_startup,
    }
}

fn detect_wsl_distro(conn: &rusqlite::Connection) -> Option<String> {
    let projects = match db::queries::list_projects(conn) {
        Ok(projects) => projects,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to list projects while detecting startup daemon distro"
            );
            Vec::new()
        }
    };
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
    log_path: &std::path::Path,
) -> (Option<provider::daemon_client::DaemonProvider>, bool) {
    let port = crate::daemon::server::DEFAULT_PORT;
    let addr = format!("127.0.0.1:{port}");
    connect_daemon_provider_with(
        wsl_distro.as_deref(),
        log_path,
        &addr,
        port,
        provider::daemon_client::DaemonProvider::connect,
        validate_startup_daemon_fast_path,
    )
}

fn validate_startup_daemon_fast_path(
    provider: &provider::daemon_client::DaemonProvider,
    wsl_distro: Option<&str>,
    port: u16,
    log_path: &std::path::Path,
) -> Result<crate::daemon::launcher::StartupDaemonValidation, std::io::Error> {
    let validation = crate::daemon::launcher::validate_startup_daemon_binary(
        provider, wsl_distro, port, log_path,
    )?;
    let ping = provider
        .ping_info_with_timeout(STARTUP_DAEMON_FAST_PATH_TIMEOUT)
        .map_err(|error| std::io::Error::other(format!("startup daemon ping failed: {error}")))?;

    if ping.protocol_version != crate::daemon::protocol::PROTOCOL_VERSION {
        return Err(std::io::Error::other(format!(
            "startup daemon protocol mismatch: running={}, expected={}",
            ping.protocol_version,
            crate::daemon::protocol::PROTOCOL_VERSION
        )));
    }

    if ping.version.trim() != env!("CARGO_PKG_VERSION") {
        return Err(std::io::Error::other(format!(
            "startup daemon version mismatch: running={}, expected={}",
            ping.version.trim(),
            env!("CARGO_PKG_VERSION")
        )));
    }

    Ok(validation)
}

fn connect_daemon_provider_with<Connect, Validate>(
    wsl_distro: Option<&str>,
    log_path: &std::path::Path,
    addr: &str,
    port: u16,
    connect: Connect,
    validate: Validate,
) -> (Option<provider::daemon_client::DaemonProvider>, bool)
where
    Connect:
        FnOnce(&str) -> Result<provider::daemon_client::DaemonProvider, crate::errors::AppError>,
    Validate: FnOnce(
        &provider::daemon_client::DaemonProvider,
        Option<&str>,
        u16,
        &std::path::Path,
    )
        -> Result<crate::daemon::launcher::StartupDaemonValidation, std::io::Error>,
{
    if wsl_distro.is_none() {
        return (None, false);
    }

    let connect_started_at = Instant::now();
    match connect(addr) {
        Ok(provider) => {
            match validate(&provider, wsl_distro, port, log_path) {
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "Startup daemon binary validation failed; deferring daemon readiness"
                    );
                    provider.disconnect("startup_binary_validation_failed");
                    emit_startup_daemon_connect_deferred(
                        addr,
                        wsl_distro,
                        "daemon_binary_validation_failed",
                        connect_started_at.elapsed().as_millis() as u64,
                    );
                    return (Some(provider), false);
                }
            }
            tracing::info!("Connected to existing daemon (fast path)");
            emit_startup_daemon_connect_succeeded(
                addr,
                connect_started_at.elapsed().as_millis() as u64,
            );
            (Some(provider), true)
        }
        Err(_) => {
            tracing::info!(addr, "Daemon not running — will start in background");
            emit_startup_daemon_connect_deferred(
                addr,
                wsl_distro,
                "daemon_unavailable_at_startup",
                connect_started_at.elapsed().as_millis() as u64,
            );
            (
                Some(provider::daemon_client::DaemonProvider::new_disconnected(
                    addr,
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
    app.manage(crate::commands::tasks::TaskQueryRefreshState::default());
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
    #[cfg(feature = "mesh-bridged-backend")]
    spawn_coordination_self_heal_monitor(app.handle().clone());
    let watchers_started_at = Instant::now();
    if let Err(error) = watchers::initialize(app, context) {
        let mut fields = startup_base_fields();
        fields.insert(
            "error.code".to_string(),
            Value::String("STARTUP_WATCHERS_INIT_FAILED".to_string()),
        );
        fields.insert(
            "error.message".to_string(),
            Value::String(error.to_string()),
        );
        emit_startup_event(
            "error",
            "startup.watchers.failed",
            "Startup watchers initialization failed",
            fields,
        );
        return Err(error);
    }
    emit_startup_watchers_initialized(
        watchers_started_at.elapsed().as_millis() as u64,
        true,
        context.daemon_connected_at_startup && context.daemon_addr.is_some(),
    );

    #[cfg(feature = "mesh-bridged-backend")]
    compaction::initialize(app)?;

    let search_started_at = Instant::now();
    let search_doc_count = match search::initialize(app, context) {
        Ok(doc_count) => doc_count,
        Err(error) => {
            let mut fields = startup_base_fields();
            fields.insert(
                "error.code".to_string(),
                Value::String("STARTUP_SEARCH_INIT_FAILED".to_string()),
            );
            fields.insert(
                "error.message".to_string(),
                Value::String(error.to_string()),
            );
            emit_startup_event(
                "error",
                "startup.search.failed",
                "Startup search initialization failed",
                fields,
            );
            return Err(error);
        }
    };
    let index_path = context.data_dir.join("search_index");
    emit_startup_search_initialized(
        index_path,
        search_doc_count,
        search_started_at.elapsed().as_millis() as u64,
    );

    bootstrap::spawn_background_startup_tasks(app.handle().clone());
    Ok(())
}

#[cfg(test)]
fn run_startup_orchestration_with<
    SpawnBootstrap,
    StartRuntimeMonitors,
    InitializeWatchers,
    InitializeCompaction,
    InitializeSearch,
    SpawnBackgroundTasks,
>(
    context: &SetupContext,
    hooks: StartupOrchestrationHooks<
        SpawnBootstrap,
        StartRuntimeMonitors,
        InitializeWatchers,
        InitializeCompaction,
        InitializeSearch,
        SpawnBackgroundTasks,
    >,
) -> Result<StartupOrchestrationReport, StartupOrchestrationError>
where
    SpawnBootstrap: FnOnce(),
    StartRuntimeMonitors: FnOnce(),
    InitializeWatchers: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
    InitializeCompaction: FnOnce() -> Result<(), Box<dyn std::error::Error>>,
    InitializeSearch: FnOnce() -> Result<u64, Box<dyn std::error::Error>>,
    SpawnBackgroundTasks: FnOnce(),
{
    let StartupOrchestrationHooks {
        spawn_background_bootstrap,
        start_runtime_monitors,
        initialize_watchers,
        initialize_compaction,
        initialize_search,
        spawn_background_tasks,
    } = hooks;

    spawn_background_bootstrap();
    start_runtime_monitors();

    let watchers_started_at = Instant::now();
    if let Err(source) = initialize_watchers() {
        let mut fields = startup_base_fields();
        fields.insert(
            "error.code".to_string(),
            Value::String("STARTUP_WATCHERS_INIT_FAILED".to_string()),
        );
        fields.insert(
            "error.message".to_string(),
            Value::String(source.to_string()),
        );
        emit_startup_event(
            "error",
            "startup.watchers.failed",
            "Startup watchers initialization failed",
            fields,
        );
        return Err(StartupOrchestrationError::Watchers { source });
    }
    emit_startup_watchers_initialized(
        watchers_started_at.elapsed().as_millis() as u64,
        true,
        context.daemon_connected_at_startup && context.daemon_addr.is_some(),
    );

    initialize_compaction().map_err(|source| StartupOrchestrationError::Compaction { source })?;

    let search_started_at = Instant::now();
    let search_doc_count = match initialize_search() {
        Ok(doc_count) => doc_count,
        Err(source) => {
            let mut fields = startup_base_fields();
            fields.insert(
                "error.code".to_string(),
                Value::String("STARTUP_SEARCH_INIT_FAILED".to_string()),
            );
            fields.insert(
                "error.message".to_string(),
                Value::String(source.to_string()),
            );
            emit_startup_event(
                "error",
                "startup.search.failed",
                "Startup search initialization failed",
                fields,
            );
            return Err(StartupOrchestrationError::Search { source });
        }
    };
    emit_startup_search_initialized(
        context.data_dir.join("search_index"),
        search_doc_count,
        search_started_at.elapsed().as_millis() as u64,
    );

    spawn_background_tasks();

    Ok(StartupOrchestrationReport {
        daemon_watch_bootstrap: context.daemon_connected_at_startup
            && context.daemon_addr.is_some(),
        search_doc_count,
    })
}

#[cfg(feature = "mesh-bridged-backend")]
fn spawn_coordination_self_heal_monitor(app: tauri::AppHandle) {
    use std::time::Duration;

    const INITIAL_DELAY: Duration = Duration::from_secs(5);
    const CHECK_INTERVAL: Duration = Duration::from_secs(30);

    std::thread::spawn(move || {
        std::thread::sleep(INITIAL_DELAY);
        loop {
            let state = app.state::<crate::coordination::state::CoordinationState>();
            match state.run_background_self_heal_pass() {
                Ok(summary) => {
                    if summary.teams_reconciled > 0
                        || summary.team_daemons_ensured > 0
                        || summary.team_errors > 0
                    {
                        tracing::info!(
                            teams_scanned = summary.teams_scanned,
                            teams_skipped = summary.teams_skipped,
                            teams_reconciled = summary.teams_reconciled,
                            team_daemons_ensured = summary.team_daemons_ensured,
                            team_errors = summary.team_errors,
                            "background coordination self-heal pass completed"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "background coordination self-heal pass failed"
                    );
                }
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
}

fn env_path_override(var: &str) -> Option<PathBuf> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

pub(crate) fn resolve_app_data_dir(
    app: tauri::AppHandle,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
    let _ = env_path_override(CLAUDE_DIR_OVERRIDE_ENV);
    Some(crate::provider::platform_paths::PlatformPaths::claude_dir().join("tasks"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use crate::daemon::launcher::StartupDaemonValidation;
    use std::cell::RefCell;
    use std::net::TcpListener;
    use std::path::Path;
    use std::time::Duration;

    fn read_lines(path: &Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    fn wait_for_lines(path: &Path, expected_minimum: usize) -> Vec<String> {
        for _ in 0..100 {
            let lines = read_lines(path);
            if lines.len() >= expected_minimum {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        read_lines(path)
    }

    fn spawn_stub_daemon_listener() -> (String, u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
        let addr = listener.local_addr().expect("local addr");
        let addr_string = addr.to_string();
        let handle = std::thread::spawn(move || {
            let _stream = listener.accept().expect("accept daemon connection");
            std::thread::sleep(Duration::from_millis(50));
        });
        (addr_string, addr.port(), handle)
    }

    #[test]
    fn startup_emitters_use_inventory_specific_event_names() {
        let _heavy_guard = crate::test_support::acquire_heavy_test_guard();
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("startup-events.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let setup_paths = SetupPaths {
            data_dir: dir.path().join("data"),
            log_path: log_path.clone(),
            db_path: dir.path().join("taurhaus.db"),
        };
        let context = SetupContext {
            data_dir: setup_paths.data_dir.clone(),
            log_path: setup_paths.log_path.clone(),
            db_path: setup_paths.db_path.clone(),
            wsl_distro: Some("Ubuntu".to_string()),
            daemon_addr: Some("127.0.0.1:17233".to_string()),
            daemon_connected_at_startup: false,
        };

        emit_startup_app_started();
        emit_startup_paths_resolved(&setup_paths);
        emit_startup_logging_initialized(&setup_paths);
        emit_startup_database_started(&setup_paths);
        emit_startup_database_completed(&setup_paths, 1);
        emit_startup_daemon_phase_started();
        emit_startup_daemon_phase_completed(&context, Some("Ubuntu"), 2);
        emit_startup_daemon_connect_succeeded("127.0.0.1:17233", 1);
        emit_startup_daemon_connect_deferred(
            "127.0.0.1:17233",
            Some("Ubuntu"),
            "daemon_unavailable_at_startup",
            1,
        );
        emit_startup_orchestration_started();
        emit_startup_orchestration_completed(3);
        emit_startup_watchers_initialized(4, true, false);
        emit_startup_search_initialized(context.data_dir.join("search_index"), 0, 5);

        let lines = wait_for_lines(&log_path, 13);
        let events: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("valid json"))
            .collect();

        let expected = [
            "startup.app.started",
            "startup.paths.resolved",
            "startup.logging.initialized",
            "startup.database.started",
            "startup.database.completed",
            "startup.daemon_phase.started",
            "startup.daemon_phase.completed",
            "startup.daemon_connect.succeeded",
            "startup.daemon_connect.deferred",
            "startup.orchestration.started",
            "startup.orchestration.completed",
            "startup.watchers.initialized",
            "startup.search.initialized",
        ];
        for event_name in expected {
            assert!(
                events.iter().any(|value| value["event"] == event_name),
                "missing expected startup event: {event_name}"
            );
        }
        assert!(
            events
                .iter()
                .all(|value| value["event"] != "startup.phase.started"
                    && value["event"] != "startup.phase.completed"
                    && value["event"] != "startup.phase.failed"),
            "legacy generic startup phase events must not be emitted"
        );
    }

    #[test]
    fn resolve_claude_tasks_dir_uses_platform_paths() {
        let expected = crate::provider::platform_paths::PlatformPaths::claude_dir().join("tasks");
        assert_eq!(resolve_claude_tasks_dir(), Some(expected));
    }

    #[test]
    fn initialize_database_fails_when_db_path_is_a_directory() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("taurhaus.db");
        std::fs::create_dir_all(&db_path).expect("create directory at db path");
        let setup_paths = SetupPaths {
            data_dir: temp_dir.path().join("data"),
            log_path: temp_dir.path().join("taurhaus.log.jsonl"),
            db_path,
        };

        let error = initialize_database(&setup_paths).expect_err("directory db path should fail");

        assert!(
            !error.to_string().trim().is_empty(),
            "database init failure should surface an error message"
        );
    }

    #[test]
    fn connect_daemon_provider_with_marks_fast_path_connected_when_connect_and_validation_succeed()
    {
        let (addr, port, listener_handle) = spawn_stub_daemon_listener();
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let (provider, connected_at_startup) = connect_daemon_provider_with(
            Some("native"),
            temp_dir.path(),
            &addr,
            port,
            provider::daemon_client::DaemonProvider::connect,
            |_, _, _, _| Ok(StartupDaemonValidation::Healthy),
        );

        let provider = provider.expect("provider");
        assert!(connected_at_startup);
        assert!(provider.is_connected());

        listener_handle.join().expect("listener thread");
    }

    #[test]
    fn connect_daemon_provider_with_defers_when_validation_fails_after_connect() {
        let (addr, port, listener_handle) = spawn_stub_daemon_listener();
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let (provider, connected_at_startup) = connect_daemon_provider_with(
            Some("native"),
            temp_dir.path(),
            &addr,
            port,
            provider::daemon_client::DaemonProvider::connect,
            |_, _, _, _| Err(io::Error::other("stale daemon binary")),
        );

        let provider = provider.expect("provider");
        assert!(!connected_at_startup);
        assert!(!provider.is_connected());

        listener_handle.join().expect("listener thread");
    }

    #[test]
    fn connect_daemon_provider_with_returns_disconnected_provider_when_daemon_is_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port");
        let addr = listener.local_addr().expect("local addr");
        let addr_string = addr.to_string();
        let port = addr.port();
        drop(listener);

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let (provider, connected_at_startup) = connect_daemon_provider_with(
            Some("native"),
            temp_dir.path(),
            &addr_string,
            port,
            provider::daemon_client::DaemonProvider::connect,
            |_, _, _, _| Ok(StartupDaemonValidation::Healthy),
        );

        let provider = provider.expect("disconnected fallback provider");
        assert!(!connected_at_startup);
        assert!(!provider.is_connected());
        assert_eq!(provider.addr(), addr_string);
    }

    #[test]
    fn run_startup_orchestration_with_reports_successful_branch_order_and_flags() {
        let calls = RefCell::new(Vec::new());
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let context = SetupContext {
            data_dir: temp_dir.path().to_path_buf(),
            log_path: temp_dir.path().join("taurhaus.log.jsonl"),
            db_path: temp_dir.path().join("taurhaus.db"),
            wsl_distro: Some("native".to_string()),
            daemon_addr: Some("127.0.0.1:17233".to_string()),
            daemon_connected_at_startup: true,
        };

        let report = run_startup_orchestration_with(
            &context,
            StartupOrchestrationHooks {
                spawn_background_bootstrap: || calls.borrow_mut().push("bootstrap"),
                start_runtime_monitors: || calls.borrow_mut().push("monitors"),
                initialize_watchers: || {
                    calls.borrow_mut().push("watchers");
                    Ok(())
                },
                initialize_compaction: || {
                    calls.borrow_mut().push("compaction");
                    Ok(())
                },
                initialize_search: || {
                    calls.borrow_mut().push("search");
                    Ok(7)
                },
                spawn_background_tasks: || calls.borrow_mut().push("tasks"),
            },
        )
        .expect("orchestration succeeds");

        assert_eq!(
            calls.into_inner(),
            vec![
                "bootstrap",
                "monitors",
                "watchers",
                "compaction",
                "search",
                "tasks"
            ]
        );
        assert!(report.daemon_watch_bootstrap);
        assert_eq!(report.search_doc_count, 7);
    }

    #[test]
    fn run_startup_orchestration_with_short_circuits_after_watcher_failure() {
        let calls = RefCell::new(Vec::new());
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let context = SetupContext {
            data_dir: temp_dir.path().to_path_buf(),
            log_path: temp_dir.path().join("taurhaus.log.jsonl"),
            db_path: temp_dir.path().join("taurhaus.db"),
            wsl_distro: None,
            daemon_addr: None,
            daemon_connected_at_startup: false,
        };

        let error = run_startup_orchestration_with(
            &context,
            StartupOrchestrationHooks {
                spawn_background_bootstrap: || calls.borrow_mut().push("bootstrap"),
                start_runtime_monitors: || calls.borrow_mut().push("monitors"),
                initialize_watchers: || {
                    calls.borrow_mut().push("watchers");
                    Err(io::Error::other("watchers boom").into())
                },
                initialize_compaction: || {
                    calls.borrow_mut().push("compaction");
                    Ok(())
                },
                initialize_search: || {
                    calls.borrow_mut().push("search");
                    Ok(7)
                },
                spawn_background_tasks: || calls.borrow_mut().push("tasks"),
            },
        )
        .expect_err("watcher init should fail");

        assert!(matches!(error, StartupOrchestrationError::Watchers { .. }));
        assert_eq!(
            calls.into_inner(),
            vec!["bootstrap", "monitors", "watchers"]
        );
    }

    #[test]
    fn run_startup_orchestration_with_short_circuits_after_search_failure() {
        let calls = RefCell::new(Vec::new());
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let context = SetupContext {
            data_dir: temp_dir.path().to_path_buf(),
            log_path: temp_dir.path().join("taurhaus.log.jsonl"),
            db_path: temp_dir.path().join("taurhaus.db"),
            wsl_distro: Some("native".to_string()),
            daemon_addr: Some("127.0.0.1:17233".to_string()),
            daemon_connected_at_startup: true,
        };

        let error = run_startup_orchestration_with(
            &context,
            StartupOrchestrationHooks {
                spawn_background_bootstrap: || calls.borrow_mut().push("bootstrap"),
                start_runtime_monitors: || calls.borrow_mut().push("monitors"),
                initialize_watchers: || {
                    calls.borrow_mut().push("watchers");
                    Ok(())
                },
                initialize_compaction: || {
                    calls.borrow_mut().push("compaction");
                    Ok(())
                },
                initialize_search: || {
                    calls.borrow_mut().push("search");
                    Err(io::Error::other("search boom").into())
                },
                spawn_background_tasks: || calls.borrow_mut().push("tasks"),
            },
        )
        .expect_err("search init should fail");

        assert!(matches!(error, StartupOrchestrationError::Search { .. }));
        assert_eq!(
            calls.into_inner(),
            vec!["bootstrap", "monitors", "watchers", "compaction", "search"]
        );
    }
}
