use tauri::State;

use crate::commands::projects::DbState;
use crate::db::settings_queries;
use crate::models::Settings;

#[tauri::command]
pub fn get_settings(db: State<'_, DbState>) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_settings(db: State<'_, DbState>, settings: Settings) -> Result<Settings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    settings_queries::save_settings(&conn, &settings).map_err(|e| e.to_string())?;
    settings_queries::get_all_settings(&conn).map_err(|e| e.to_string())
}
