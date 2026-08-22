use std::time::Instant;

use tauri::Manager;

use super::telemetry;
use super::SetupContext;

pub(super) fn run_startup_orchestration(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap_complete = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    crate::startup::daemon::spawn_background_bootstrap(
        app.handle().clone(),
        context,
        bootstrap_complete.clone(),
    );
    crate::startup::daemon::start_runtime_monitors(
        app.handle().clone(),
        context,
        bootstrap_complete,
    );
    #[cfg(feature = "mesh-bridged-backend")]
    spawn_coordination_self_heal_monitor(app.handle().clone());

    let watchers_started_at = Instant::now();
    if let Err(error) = crate::startup::watchers::initialize(app, context) {
        telemetry::emit_startup_init_failed(
            "startup.watchers.failed",
            "Startup watchers initialization failed",
            "STARTUP_WATCHERS_INIT_FAILED",
            "watchers",
            watchers_started_at.elapsed().as_millis() as u64,
            error.as_ref(),
        );
        return Err(error);
    }
    telemetry::emit_startup_watchers_initialized(
        watchers_started_at.elapsed().as_millis() as u64,
        true,
        daemon_watch_bootstrap_enabled(context),
    );

    #[cfg(feature = "mesh-bridged-backend")]
    if let Err(error) = crate::startup::compaction::initialize(
        app,
        context.daemon_addr.is_some(),
        context.daemon_connected_at_startup,
    ) {
        tracing::warn!(
            error = %error,
            "app-owned compaction initialization failed; startup continues"
        );
    }

    let search_started_at = Instant::now();
    let search_doc_count = match crate::startup::search::initialize(app, context) {
        Ok(doc_count) => doc_count,
        Err(error) => {
            telemetry::emit_startup_init_failed(
                "startup.search.failed",
                "Startup search initialization failed",
                "STARTUP_SEARCH_INIT_FAILED",
                "search",
                search_started_at.elapsed().as_millis() as u64,
                error.as_ref(),
            );
            return Err(error);
        }
    };
    let index_path = context.data_dir.join("search_index");
    telemetry::emit_startup_search_initialized(
        index_path,
        search_doc_count,
        search_started_at.elapsed().as_millis() as u64,
    );

    crate::startup::bootstrap::spawn_background_startup_tasks(app.handle().clone());
    Ok(())
}

pub(super) fn daemon_watch_bootstrap_enabled(context: &SetupContext) -> bool {
    context.daemon_connected_at_startup && context.daemon_addr.is_some()
}

#[cfg(feature = "mesh-bridged-backend")]
fn spawn_coordination_self_heal_monitor(app: tauri::AppHandle) {
    use std::time::Duration;

    const INITIAL_DELAY: Duration = Duration::from_secs(5);
    const CHECK_INTERVAL: Duration = Duration::from_secs(30);

    std::thread::spawn(move || {
        telemetry::emit_startup_self_heal_started(
            INITIAL_DELAY.as_millis() as u64,
            CHECK_INTERVAL.as_millis() as u64,
        );
        std::thread::sleep(INITIAL_DELAY);
        loop {
            let pass_started_at = Instant::now();
            let state = app.state::<crate::coordination::state::CoordinationState>();
            match state.run_background_self_heal_pass() {
                Ok(summary) => {
                    if summary.teams_reconciled > 0
                        || summary.team_daemons_ensured > 0
                        || summary.team_errors > 0
                    {
                        telemetry::emit_startup_self_heal_completed(
                            pass_started_at.elapsed().as_millis() as u64,
                            summary.teams_scanned as u64,
                            summary.teams_skipped as u64,
                            summary.teams_reconciled as u64,
                            summary.team_daemons_ensured as u64,
                            summary.team_errors as u64,
                        );
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
                    telemetry::emit_startup_self_heal_failed(
                        pass_started_at.elapsed().as_millis() as u64,
                        &err.to_string(),
                    );
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
