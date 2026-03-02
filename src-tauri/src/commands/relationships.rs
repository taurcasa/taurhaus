use tauri::State;

use crate::commands::projects::DbState;
use crate::db::relationship_queries;
use crate::models::Relationship;

#[tauri::command]
pub fn get_relationships(
    db: State<'_, DbState>,
    project_id: String,
) -> Result<Vec<Relationship>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::list_relationships(&conn, &project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn dismiss_relationship(db: State<'_, DbState>, relationship_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::dismiss_relationship(&conn, &relationship_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_relationship(
    db: State<'_, DbState>,
    source_id: String,
    target_id: String,
    relationship_type: String,
) -> Result<Relationship, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let rel = Relationship {
        id: uuid::Uuid::new_v4().to_string(),
        source_project_id: source_id,
        target_project_id: target_id,
        relationship_type,
        detection_source: "manual".to_string(),
        dismissed: false,
        first_detected_at: now.clone(),
        last_seen_at: now,
    };

    relationship_queries::insert_relationship(&conn, &rel).map_err(|e| e.to_string())?;
    Ok(rel)
}

#[tauri::command]
pub fn remove_relationship(db: State<'_, DbState>, relationship_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    relationship_queries::remove_relationship(&conn, &relationship_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}
