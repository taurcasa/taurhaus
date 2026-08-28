use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::settings_queries;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::models::Settings;

static SETTINGS_RECONCILE_QUEUED: AtomicBool = AtomicBool::new(false);

fn enqueue_activity_watch_reconcile(app: tauri::AppHandle, reason: &'static str) {
    if SETTINGS_RECONCILE_QUEUED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    #[cfg(test)]
    {
        crate::startup::watchers::reconcile_activity_watches(&app, reason);
        SETTINGS_RECONCILE_QUEUED.store(false, Ordering::Release);
    }

    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            struct ResetQueuedFlag;
            impl Drop for ResetQueuedFlag {
                fn drop(&mut self) {
                    SETTINGS_RECONCILE_QUEUED.store(false, Ordering::Release);
                }
            }

            let _reset_queued_flag = ResetQueuedFlag;
            crate::startup::watchers::reconcile_activity_watches(&app, reason);
        });
    }
}

#[tauri::command]
pub fn get_settings(db: State<'_, DbState>) -> IpcResult<Settings> {
    get_settings_with_span(db.inner())
}

fn get_settings_with_span(db: &DbState) -> IpcResult<Settings> {
    let span = IpcCommandSpan::start("get_settings");
    let result = get_settings_impl(db).ipc_cmd("get_settings");
    span.finish_result(&result);
    result
}

fn get_settings_impl(db: &DbState) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::get_all_settings(&conn)
        .map(Settings::with_runtime_terminal_contract)
        .sanitize_err()
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    settings: Settings,
) -> IpcResult<Settings> {
    update_settings_with_span(&app, db.inner(), settings)
}

fn update_settings_with_span(
    app: &tauri::AppHandle,
    db: &DbState,
    settings: Settings,
) -> IpcResult<Settings> {
    let span = IpcCommandSpan::start("update_settings");
    let result = (|| -> Result<Settings, String> {
        let updated = update_settings_impl(db, settings)?;
        #[cfg(feature = "mesh-bridged-backend")]
        reconcile_codex_compaction_setting(app, &updated);
        #[cfg(feature = "mesh-bridged-backend")]
        reconcile_agy_hooks_setting(&updated);
        enqueue_activity_watch_reconcile(app.clone(), "settings_updated");
        Ok(updated)
    })()
    .ipc_cmd("update_settings");
    span.finish_result(&result);
    result
}

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_agy_hooks_setting(settings: &Settings) {
    if let Err(error) =
        crate::commands::terminal_settings::reconcile_agy_hooks(settings.terminal.harness.agy_hooks)
    {
        tracing::warn!(error = %error, "Antigravity hook reconciliation failed");
        let mut fields = serde_json::Map::new();
        fields.insert(
            "error.message".to_string(),
            serde_json::Value::String(error.to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "agy.hooks.reconcile_failed",
            Some("Antigravity native activity hooks remain disabled".to_string()),
            fields,
        );
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn reconcile_codex_compaction_setting(app: &tauri::AppHandle, settings: &Settings) {
    use tauri::Manager;

    let has_managed_codex = match app.try_state::<crate::coordination::state::CoordinationState>() {
        Some(state) => {
            match crate::coordination::compact_hook::any_managed_codex_member(state.teams_dir()) {
                Ok(has_managed_codex) => has_managed_codex,
                Err(error) => {
                    emit_codex_compaction_degraded("discover_managed_members", &error.to_string());
                    false
                }
            }
        }
        None => false,
    };
    if let Err(error) = crate::commands::terminal_settings::reconcile_codex_compaction(
        settings.terminal.harness.codex_compaction,
        has_managed_codex,
    ) {
        emit_codex_compaction_degraded("reconcile_hook_files", &error.to_string());
    }
    if let Err(error) = crate::startup::compaction::reconcile_compaction_runtime(
        app,
        settings.terminal.harness.codex_compaction,
        "settings_updated",
    ) {
        emit_codex_compaction_degraded("reconcile_runtime_owner", &error.to_string());
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn emit_codex_compaction_degraded(stage: &str, error_message: &str) {
    tracing::warn!(
        stage,
        error = error_message,
        "Codex compaction reconciliation degraded"
    );
    let mut fields = serde_json::Map::new();
    fields.insert(
        "stage".to_string(),
        serde_json::Value::String(stage.to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        serde_json::Value::String(error_message.to_string()),
    );
    crate::commands::logging::emit_global(
        "warn",
        "coordination",
        "compaction.codex_hook.degraded",
        Some("Codex compaction reconciliation fell back safely".to_string()),
        fields,
    );
}

fn update_settings_impl(db: &DbState, settings: Settings) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let settings = settings.with_runtime_terminal_contract();
    settings_queries::save_settings(&conn, &settings).sanitize_err()?;
    settings_queries::get_all_settings(&conn)
        .map(Settings::with_runtime_terminal_contract)
        .sanitize_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::logging::{install_global_sink, LogFileState};
    use serde_json::Value;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    fn test_db_state() -> (DbState, NamedTempFile) {
        let tmp = NamedTempFile::new().expect("temp db");
        let conn = crate::db::init_db(tmp.path()).expect("init db");
        (DbState(Mutex::new(conn)), tmp)
    }

    #[test]
    fn settings_commands_get_and_update_round_trip() {
        let (db, _tmp) = test_db_state();
        let defaults = get_settings_impl(&db).expect("get defaults");

        let mut updated = defaults.clone();
        updated.dark_mode = !defaults.dark_mode;
        updated.scan_directories.push("/tmp/project-a".to_string());
        updated.ignore_patterns.push("node_modules".to_string());
        updated.daemon.port = 19001;
        updated.project_dialog_last_path = "/projects/taurhaus".to_string();

        let saved = update_settings_impl(&db, updated.clone()).expect("update settings");
        assert_eq!(saved.dark_mode, updated.dark_mode);
        assert_eq!(saved.daemon.port, 19001);
        assert_eq!(saved.project_dialog_last_path, "/projects/taurhaus");
        assert!(saved
            .scan_directories
            .contains(&"/tmp/project-a".to_string()));

        let fetched = get_settings_impl(&db).expect("get updated settings");
        assert_eq!(fetched, saved);
    }

    #[test]
    fn settings_commands_attach_runtime_terminal_contract_and_migrate_legacy_emulator() {
        let (db, _tmp) = test_db_state();
        let mut settings = get_settings_impl(&db).expect("get defaults");
        settings.terminal.emulator = "default".to_string();

        let saved = update_settings_impl(&db, settings).expect("update settings");

        assert_eq!(
            saved.terminal.emulator,
            saved.terminal_contract.default_emulator
        );
        assert!(saved
            .terminal_contract
            .supported_emulators
            .contains(&saved.terminal.emulator));
    }

    #[test]
    fn settings_commands_report_db_lock_failure() {
        let db = DbState(Mutex::new(
            rusqlite::Connection::open_in_memory().expect("open memory db"),
        ));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = db.0.lock().expect("lock");
            panic!("poison lock");
        }));

        let err = get_settings_impl(&db).expect_err("poisoned lock should fail");
        assert!(err.to_lowercase().contains("poison"));
    }

    /// Polls the global sink until both lifecycle records for `command` have
    /// landed. The sink is process-global, so counting lines would let
    /// unrelated traffic satisfy the wait before this command finishes
    /// writing; matching on the records themselves is what makes it stable.
    fn wait_for_command_events(path: &std::path::Path, command: &str) -> Vec<Value> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let events: Vec<Value> = std::fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|event| event["command"] == command)
                .collect();
            let has_received = events
                .iter()
                .any(|event| event["event"] == "ipc.command.received");
            let has_completed = events
                .iter()
                .any(|event| event["event"] == "ipc.command.completed");
            if has_received && has_completed {
                return events;
            }
            if std::time::Instant::now() >= deadline {
                panic!(
                    "timed out waiting for {command} lifecycle events in {}",
                    path.display()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn get_settings_emits_lifecycle_events() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let (db, _tmp) = test_db_state();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("settings-lifecycle.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let _ = get_settings_with_span(&db).expect("get settings");

        // The sink is process-global: anything else running in this test
        // binary writes to it too. This test is about the pair of events one
        // command emits, so it reads those and ignores the traffic around them.
        let events = wait_for_command_events(&log_path, "get_settings");

        let received = events
            .iter()
            .find(|event| event["event"] == "ipc.command.received")
            .expect("received event");
        let completed = events
            .iter()
            .find(|event| event["event"] == "ipc.command.completed")
            .expect("completed event");

        assert_eq!(received["command"], "get_settings");
        assert_eq!(completed["command"], "get_settings");
        assert_eq!(completed["status"], "ok");
    }
}
