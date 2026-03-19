use std::path::PathBuf;
use std::time::Instant;

pub(crate) mod bootstrap;
#[cfg(feature = "mesh-bridged-backend")]
pub(crate) mod compaction;
pub(crate) mod daemon;
#[cfg(test)]
mod harness;
mod orchestration;
pub(crate) mod search;
mod setup;
mod telemetry;
pub(crate) mod watchers;

#[derive(Clone)]
pub(crate) struct SetupContext {
    pub(crate) data_dir: PathBuf,
    pub(crate) log_path: PathBuf,
    pub(crate) db_path: PathBuf,
    pub(crate) wsl_distro: Option<String>,
    pub(crate) daemon_addr: Option<String>,
    pub(crate) daemon_connected_at_startup: bool,
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("taurhaus starting");

    let setup_paths = setup::initialize_paths_and_logging(app)?;
    telemetry::emit_startup_app_started();
    telemetry::emit_startup_paths_resolved(&setup_paths);
    telemetry::emit_startup_logging_initialized(&setup_paths);
    telemetry::emit_startup_database_started(&setup_paths);
    let database_started_at = Instant::now();
    let conn = match setup::initialize_database(&setup_paths) {
        Ok(conn) => conn,
        Err(error) => {
            telemetry::emit_startup_database_failed(&setup_paths, &error.to_string());
            return Err(error);
        }
    };
    telemetry::emit_startup_database_completed(
        &setup_paths,
        database_started_at.elapsed().as_millis() as u64,
    );
    telemetry::emit_startup_daemon_phase_started();
    let daemon_phase_started_at = Instant::now();
    let daemon_phase = setup::determine_daemon_phase(&conn, &setup_paths.log_path);
    let context = setup::build_setup_context(&setup_paths, &daemon_phase);
    telemetry::emit_startup_daemon_phase_completed(
        &context,
        daemon_phase.wsl_distro.as_deref(),
        daemon_phase_started_at.elapsed().as_millis() as u64,
    );

    setup::register_managed_state(app, conn, &setup_paths, daemon_phase);
    telemetry::emit_startup_orchestration_started();
    let orchestration_started_at = Instant::now();
    orchestration::run_startup_orchestration(app, &context)?;
    telemetry::emit_startup_orchestration_completed(
        orchestration_started_at.elapsed().as_millis() as u64
    );

    tracing::info!(db_path = %context.db_path.display(), "database initialized");
    Ok(())
}
pub(crate) use setup::resolve_app_data_dir;
pub(crate) use setup::resolve_claude_tasks_dir;

#[cfg(test)]
mod tests {
    use super::resolve_claude_tasks_dir;

    #[test]
    fn startup_setup_helpers_still_reexport_paths() {
        let expected = crate::provider::platform_paths::PlatformPaths::claude_dir().join("tasks");
        assert_eq!(resolve_claude_tasks_dir(), Some(expected));
    }
}
