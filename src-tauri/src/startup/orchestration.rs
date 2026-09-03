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
    if context.daemon_connected_at_startup {
        if let Err(error) = crate::commands::settings::push_launch_settings_to_daemon(app.handle())
        {
            tracing::warn!(error = %error, "Failed to seed daemon launch settings at startup");
        }
    }
    #[cfg(feature = "mesh-bridged-backend")]
    if let Err(error) = reconcile_startup_codex_compaction(app.handle()) {
        tracing::warn!(error = %error, "startup Codex compaction reconciliation failed");
    }
    #[cfg(feature = "mesh-bridged-backend")]
    if let Err(error) = reconcile_startup_claude_compaction(app.handle()) {
        tracing::warn!(error = %error, "startup Claude compaction reconciliation failed");
    }
    #[cfg(feature = "mesh-bridged-backend")]
    if let Err(error) = reconcile_startup_agy_hooks(app.handle()) {
        tracing::warn!(error = %error, "startup Antigravity hook reconciliation failed");
    }
    #[cfg(feature = "mesh-bridged-backend")]
    if let Err(error) = reconcile_startup_grok_hooks(app.handle()) {
        tracing::warn!(error = %error, "startup Grok compaction hook reconciliation failed");
    }
    #[cfg(feature = "mesh-bridged-backend")]
    reconcile_startup_account_homes(app.handle());
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

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_startup_codex_compaction(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<crate::coordination::state::CoordinationState>();
    let mut has_managed_codex = false;
    for teams_dir in state.teams_roots().map_err(|error| error.to_string())? {
        has_managed_codex |=
            crate::coordination::compact_hook::any_managed_codex_member(&teams_dir)
                .map_err(|error| error.to_string())?;
    }
    crate::commands::terminal_settings::reconcile_codex_hook(has_managed_codex)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_startup_claude_compaction(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<crate::coordination::state::CoordinationState>();
    for teams_dir in state.teams_roots().map_err(|error| error.to_string())? {
        crate::coordination::state::ensure_startup_claude_compact_hook(&teams_dir)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_startup_agy_hooks(app: &tauri::AppHandle) -> Result<(), String> {
    let db = app.state::<crate::commands::projects::DbState>();
    let terminal = crate::commands::terminal_settings::load_terminal_settings(&db);
    crate::commands::terminal_settings::reconcile_agy_hooks(terminal.harness.agy_hooks)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_startup_grok_hooks(app: &tauri::AppHandle) -> Result<(), String> {
    let db = app.state::<crate::commands::projects::DbState>();
    let terminal = crate::commands::terminal_settings::load_terminal_settings(&db);
    let state = app.state::<crate::coordination::state::CoordinationState>();
    let roots = state.teams_roots().map_err(|error| error.to_string())?;
    crate::commands::terminal_settings::reconcile_grok_hooks_for_roots(
        &roots,
        terminal.harness.grok_hooks,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// Sweep the hook out of every account home no roster member still launches
/// from. The per-tool startup reconcilers above visit only each tool's default
/// home, so a home an earlier build left a hook in — or one the last team on it
/// was disbanded from while taurhaus was closed — is only reached here. It logs
/// its own degradation and never fails startup.
#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_startup_account_homes(app: &tauri::AppHandle) {
    let db = app.state::<crate::commands::projects::DbState>();
    let terminal = crate::commands::terminal_settings::load_terminal_settings(&db);
    let state = app.state::<crate::coordination::state::CoordinationState>();
    let Ok(roots) = state.teams_roots() else {
        return;
    };
    crate::commands::terminal_settings::reconcile_managed_account_hooks_for_roots(
        &roots,
        terminal.harness.grok_hooks,
    );
}

pub(super) fn daemon_watch_bootstrap_enabled(context: &SetupContext) -> bool {
    context.daemon_connected_at_startup && context.daemon_addr.is_some()
}

#[cfg(test)]
mod tests {
    #[test]
    fn initial_connected_daemon_receives_launch_settings() {
        let source = include_str!("orchestration.rs");
        let body = source
            .split("pub(super) fn run_startup_orchestration(")
            .nth(1)
            .expect("startup orchestration")
            .split("fn reconcile_startup_codex_compaction")
            .next()
            .expect("startup orchestration body");
        assert!(body.contains("push_launch_settings_to_daemon"));
    }
}
