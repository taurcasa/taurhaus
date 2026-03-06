use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::db::settings_queries;
use crate::errors::SanitizeErr;
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
pub fn get_settings(db: State<'_, DbState>) -> Result<Settings, String> {
    get_settings_with_span(db.inner())
}

fn get_settings_with_span(db: &DbState) -> Result<Settings, String> {
    let span = IpcCommandSpan::start("get_settings");
    let result = get_settings_impl(db);
    span.finish_result(&result);
    result
}

fn get_settings_impl(db: &DbState) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::get_all_settings(&conn).sanitize_err()
}

#[tauri::command]
pub fn update_settings(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    settings: Settings,
) -> Result<Settings, String> {
    update_settings_with_span(&app, db.inner(), settings)
}

fn update_settings_with_span(
    app: &tauri::AppHandle,
    db: &DbState,
    settings: Settings,
) -> Result<Settings, String> {
    let span = IpcCommandSpan::start("update_settings");
    let result = {
        let updated = update_settings_impl(db, settings)?;
        enqueue_activity_watch_reconcile(app.clone(), "settings_updated");
        Ok(updated)
    };
    span.finish_result(&result);
    result
}

fn update_settings_impl(db: &DbState, settings: Settings) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::save_settings(&conn, &settings).sanitize_err()?;
    settings_queries::get_all_settings(&conn).sanitize_err()
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

        let saved = update_settings_impl(&db, updated.clone()).expect("update settings");
        assert_eq!(saved.dark_mode, updated.dark_mode);
        assert_eq!(saved.daemon.port, 19001);
        assert!(saved
            .scan_directories
            .contains(&"/tmp/project-a".to_string()));

        let fetched = get_settings_impl(&db).expect("get updated settings");
        assert_eq!(fetched, saved);
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

    fn wait_for_lines(path: &std::path::Path, expected: usize) -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| line.to_string())
                    .collect();
                if lines.len() >= expected {
                    return lines;
                }
            }
            if std::time::Instant::now() >= deadline {
                panic!("timed out waiting for log lines in {}", path.display());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn get_settings_emits_lifecycle_events() {
        let (db, _tmp) = test_db_state();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let log_path = dir.path().join("settings-lifecycle.log.jsonl");
        let state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&state);

        let _ = get_settings_with_span(&db).expect("get settings");

        let lines = wait_for_lines(&log_path, 2);
        let received: Value = serde_json::from_str(&lines[0]).expect("received json");
        let completed: Value = serde_json::from_str(&lines[1]).expect("completed json");

        assert_eq!(received["event"], "ipc.command.received");
        assert_eq!(received["command"], "get_settings");
        assert_eq!(completed["event"], "ipc.command.completed");
        assert_eq!(completed["command"], "get_settings");
        assert_eq!(completed["status"], "ok");
    }
}
