use std::time::Instant;

use super::compaction::{configured_compaction_owner, CompactionOwner};
use super::orchestration::daemon_watch_bootstrap_enabled;
use super::telemetry;
use super::SetupContext;

#[derive(Debug)]
pub(super) struct StartupOrchestrationReport {
    pub(super) daemon_watch_bootstrap: bool,
    pub(super) search_doc_count: u64,
}

pub(super) struct StartupOrchestrationHooks<
    SpawnBootstrap,
    StartRuntimeMonitors,
    InitializeWatchers,
    InitializeCompaction,
    InitializeSearch,
    SpawnBackgroundTasks,
> {
    pub(super) spawn_background_bootstrap: SpawnBootstrap,
    pub(super) start_runtime_monitors: StartRuntimeMonitors,
    pub(super) initialize_watchers: InitializeWatchers,
    pub(super) initialize_compaction: InitializeCompaction,
    pub(super) initialize_search: InitializeSearch,
    pub(super) spawn_background_tasks: SpawnBackgroundTasks,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum StartupOrchestrationError {
    #[error("watchers init failed: {source}")]
    Watchers {
        #[source]
        source: Box<dyn std::error::Error>,
    },
    #[error("search init failed: {source}")]
    Search {
        #[source]
        source: Box<dyn std::error::Error>,
    },
}

pub(super) fn run_startup_orchestration_with<
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
        telemetry::emit_startup_init_failed(
            "startup.watchers.failed",
            "Startup watchers initialization failed",
            "STARTUP_WATCHERS_INIT_FAILED",
            "watchers",
            watchers_started_at.elapsed().as_millis() as u64,
            source.as_ref(),
        );
        return Err(StartupOrchestrationError::Watchers { source });
    }
    telemetry::emit_startup_watchers_initialized(
        watchers_started_at.elapsed().as_millis() as u64,
        true,
        daemon_watch_bootstrap_enabled(context),
    );

    if configured_compaction_owner(
        context.daemon_addr.is_some(),
        context.daemon_connected_at_startup,
    ) == CompactionOwner::App
    {
        if let Err(error) = initialize_compaction() {
            tracing::warn!(
                error = %error,
                "app-owned compaction initialization failed; startup continues"
            );
        }
    }

    let search_started_at = Instant::now();
    let search_doc_count = match initialize_search() {
        Ok(doc_count) => doc_count,
        Err(source) => {
            telemetry::emit_startup_init_failed(
                "startup.search.failed",
                "Startup search initialization failed",
                "STARTUP_SEARCH_INIT_FAILED",
                "search",
                search_started_at.elapsed().as_millis() as u64,
                source.as_ref(),
            );
            return Err(StartupOrchestrationError::Search { source });
        }
    };
    telemetry::emit_startup_search_initialized(
        context.data_dir.join("search_index"),
        search_doc_count,
        search_started_at.elapsed().as_millis() as u64,
    );

    spawn_background_tasks();

    Ok(StartupOrchestrationReport {
        daemon_watch_bootstrap: daemon_watch_bootstrap_enabled(context),
        search_doc_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io;

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
            vec!["bootstrap", "monitors", "watchers", "search", "tasks"]
        );
        assert!(report.daemon_watch_bootstrap);
        assert_eq!(report.search_doc_count, 7);
    }

    #[test]
    fn run_startup_orchestration_with_treats_app_compaction_failure_as_best_effort() {
        // Regression: 27770fbd put compaction initialization on the app startup
        // critical path even when the app was the configured owner.
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
                    Err(io::Error::other("compaction boom").into())
                },
                initialize_search: || {
                    calls.borrow_mut().push("search");
                    Ok(7)
                },
                spawn_background_tasks: || calls.borrow_mut().push("tasks"),
            },
        )
        .expect("compaction failure should not abort app startup");

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
            vec!["bootstrap", "monitors", "watchers", "search"]
        );
    }
}
