use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::commands;
use crate::commands::projects::DbState;
use crate::{db, provider, services, ProviderState};

use super::telemetry;
use super::SetupContext;

const DATA_DIR_OVERRIDE_ENV: &str = "TAURHAUS_DATA_DIR";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";
const STARTUP_DAEMON_FAST_PATH_TIMEOUT: Duration = Duration::from_millis(350);
const STARTUP_WSL_DISTRO_DETECTION_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) struct SetupPaths {
    pub(super) data_dir: PathBuf,
    pub(super) log_path: PathBuf,
    pub(super) db_path: PathBuf,
}

pub(super) struct DaemonPhase {
    pub(super) wsl_distro: Option<String>,
    pub(super) daemon_provider: Option<provider::daemon_client::DaemonProvider>,
    pub(super) daemon_connected_at_startup: bool,
}

pub(super) fn initialize_paths_and_logging(
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

pub(super) fn initialize_database(
    setup_paths: &SetupPaths,
) -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    Ok(db::init_db(&setup_paths.db_path)?)
}

pub(super) fn determine_daemon_phase(
    conn: &rusqlite::Connection,
    log_path: &std::path::Path,
) -> DaemonPhase {
    let wsl_distro = detect_wsl_distro(conn);
    crate::coordination::mesh_cli::set_preferred_wsl_distro_for_coordination(wsl_distro.as_deref());
    let (daemon_provider, daemon_connected_at_startup) =
        connect_daemon_provider(&wsl_distro, log_path);
    DaemonPhase {
        wsl_distro,
        daemon_provider,
        daemon_connected_at_startup,
    }
}

fn detect_wsl_distro(conn: &rusqlite::Connection) -> Option<String> {
    if crate::daemon::launcher::is_native_daemon() {
        return Some("native".to_string());
    }

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
    let project_distro =
        resolve_startup_wsl_distro(projects.iter().map(|project| project.path.as_str()), None);
    if project_distro.is_some() {
        return project_distro;
    }

    match detect_default_wsl_distro() {
        Ok(Some(distro)) => {
            tracing::info!(
                wsl_distro = %distro,
                "Using default WSL distro for startup daemon bootstrap"
            );
            Some(distro)
        }
        Ok(None) => None,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to detect default WSL distro during startup"
            );
            None
        }
    }
}

fn resolve_startup_wsl_distro<'a>(
    project_paths: impl IntoIterator<Item = &'a str>,
    detected_default: Option<String>,
) -> Option<String> {
    project_paths
        .into_iter()
        .find_map(provider::path::wsl_distro_from_path)
        .or(detected_default)
}

fn detect_default_wsl_distro() -> Result<Option<String>, String> {
    let output = crate::process_utils::run_command_with_timeout(
        crate::daemon::launcher::wsl_command().args(["--list", "--quiet"]),
        STARTUP_WSL_DISTRO_DETECTION_TIMEOUT,
        "wsl --list --quiet",
    )
    .map_err(|error| format!("Failed to run wsl.exe: {error}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(parse_distro_from_wsl_output(&output.stdout))
}

fn parse_distro_from_wsl_output(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .map(|line| line.replace('\0', "").trim().to_string())
        .find(|line| !line.is_empty())
}

fn connect_daemon_provider(
    wsl_distro: &Option<String>,
    log_path: &std::path::Path,
) -> (Option<provider::daemon_client::DaemonProvider>, bool) {
    let port = crate::daemon::server::DEFAULT_PORT;
    let addr = format!("127.0.0.1:{port}");
    let connect_distro = wsl_distro.clone();
    connect_daemon_provider_with(
        wsl_distro.as_deref(),
        log_path,
        &addr,
        port,
        move |addr| {
            provider::daemon_client::DaemonProvider::connect_with_distro(
                addr,
                connect_distro.as_deref(),
            )
        },
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

pub(super) fn connect_daemon_provider_with<Connect, Validate>(
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
                    telemetry::emit_startup_daemon_connect_deferred(
                        addr,
                        wsl_distro,
                        "daemon_binary_validation_failed",
                        connect_started_at.elapsed().as_millis() as u64,
                    );
                    return (Some(provider), false);
                }
            }
            tracing::info!("Connected to existing daemon (fast path)");
            telemetry::emit_startup_daemon_connect_succeeded(
                addr,
                connect_started_at.elapsed().as_millis() as u64,
            );
            (Some(provider), true)
        }
        Err(_) => {
            tracing::info!(addr, "Daemon not running — will start in background");
            telemetry::emit_startup_daemon_connect_deferred(
                addr,
                wsl_distro,
                "daemon_unavailable_at_startup",
                connect_started_at.elapsed().as_millis() as u64,
            );
            (
                Some(
                    provider::daemon_client::DaemonProvider::new_disconnected_with_distro(
                        addr, wsl_distro,
                    ),
                ),
                false,
            )
        }
    }
}

pub(super) fn build_setup_context(
    setup_paths: &SetupPaths,
    daemon_phase: &DaemonPhase,
) -> SetupContext {
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

pub(super) fn register_managed_state(
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

fn env_path_override(var: &str) -> Option<PathBuf> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

pub(super) fn data_dir_override_enabled() -> bool {
    env_path_override(DATA_DIR_OVERRIDE_ENV).is_some()
}

pub(super) fn claude_dir_override_enabled() -> bool {
    env_path_override(CLAUDE_DIR_OVERRIDE_ENV).is_some()
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
    use crate::daemon::launcher::StartupDaemonValidation;
    use std::net::TcpListener;

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
    fn resolve_startup_wsl_distro_prefers_project_path_distro() {
        let resolved = resolve_startup_wsl_distro(
            [r"\\wsl$\Ubuntu\home\user\projects\taurhaus"],
            Some("Debian".to_string()),
        );

        assert_eq!(resolved.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn resolve_startup_wsl_distro_falls_back_to_detected_default() {
        let resolved = resolve_startup_wsl_distro(
            [r"C:\Users\user\projects\taurhaus"],
            Some("Ubuntu".to_string()),
        );

        assert_eq!(resolved.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn parse_distro_from_wsl_output_handles_utf16le_null_bytes() {
        let raw = b"U\0b\0u\0n\0t\0u\0\n\0";

        assert_eq!(
            parse_distro_from_wsl_output(raw),
            Some("Ubuntu".to_string())
        );
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
}
