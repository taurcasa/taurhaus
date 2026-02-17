use serde::Serialize;
use tauri::State;

use crate::search::indexer;
use crate::search::query::SearchResult;
use crate::SearchState;
use super::projects::DbState;

/// Index status returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    pub doc_count: u64,
    pub is_empty: bool,
}

#[tauri::command]
pub fn search(
    search_state: State<'_, SearchState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(20).min(50);
    index.search(&query, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_index_status(
    search_state: State<'_, SearchState>,
) -> Result<IndexStatus, String> {
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    let doc_count = index.doc_count().map_err(|e| e.to_string())?;
    Ok(IndexStatus {
        doc_count,
        is_empty: doc_count == 0,
    })
}

#[tauri::command]
pub fn rebuild_index(
    search_state: State<'_, SearchState>,
    db: State<'_, DbState>,
) -> Result<usize, String> {
    let mut index = search_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    indexer::rebuild_all(&mut index, &conn).map_err(|e| e.to_string())
}
