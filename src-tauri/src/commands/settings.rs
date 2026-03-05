use tauri::State;

use crate::commands::projects::DbState;
use crate::db::settings_queries;
use crate::errors::SanitizeErr;
use crate::models::Settings;

#[tauri::command]
pub fn get_settings(db: State<'_, DbState>) -> Result<Settings, String> {
    get_settings_impl(db.inner())
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
    let updated = update_settings_impl(db.inner(), settings)?;
    crate::startup::watchers::reconcile_activity_watches(&app, "settings_updated");
    Ok(updated)
}

fn update_settings_impl(db: &DbState, settings: Settings) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::save_settings(&conn, &settings).sanitize_err()?;
    settings_queries::get_all_settings(&conn).sanitize_err()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
