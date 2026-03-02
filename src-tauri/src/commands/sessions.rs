use tauri::State;

use crate::commands::projects::DbState;
use crate::db::session_queries;
use crate::models::{SessionDetail, SessionSummary};

#[tauri::command]
pub fn get_latest_session(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Option<SessionDetail>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::get_latest_session(&conn, &project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sessions(
    db: State<'_, DbState>,
    project_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SessionSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::list_sessions(
        &conn,
        &project_id,
        limit.unwrap_or(20).min(100),
        offset.unwrap_or(0),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session(db: State<'_, DbState>, session_id: String) -> Result<SessionDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    session_queries::get_session(&conn, &session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Session not found: {session_id}"))
}
