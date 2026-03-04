use serde::Serialize;
use tauri::State;

use super::projects::DbState;
use crate::errors::SanitizeErr;
use crate::search::indexer;
use crate::search::query::SearchResult;
use crate::SearchState;

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
    search_impl(search_state.inner(), query, limit)
}

fn search_impl(
    search_state: &SearchState,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(20).min(50);
    index.search(&query, limit).sanitize_err()
}

#[tauri::command]
pub fn get_index_status(search_state: State<'_, SearchState>) -> Result<IndexStatus, String> {
    get_index_status_impl(search_state.inner())
}

fn get_index_status_impl(search_state: &SearchState) -> Result<IndexStatus, String> {
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    let doc_count = index.doc_count().sanitize_err()?;
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
    rebuild_index_impl(search_state.inner(), db.inner())
}

fn rebuild_index_impl(search_state: &SearchState, db: &DbState) -> Result<usize, String> {
    let mut index = search_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    indexer::rebuild_all(&mut index, &conn).sanitize_err()
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

    fn test_search_state() -> SearchState {
        let index =
            crate::search::indexer::SearchIndex::open_in_memory().expect("open in-memory index");
        SearchState(Mutex::new(index))
    }

    #[test]
    fn search_commands_cover_search_status_and_rebuild() {
        let search_state = test_search_state();
        {
            let mut index = search_state.0.lock().expect("lock index");
            index
                .add_document("p1", "file", "src/main.rs", "main", "hello world")
                .expect("add document");
            index.commit().expect("commit index");
        }

        let results =
            search_impl(&search_state, "hello".to_string(), Some(10)).expect("search results");
        assert_eq!(results.len(), 1);

        let status = get_index_status_impl(&search_state).expect("index status");
        assert!(!status.is_empty);
        assert!(status.doc_count >= 1);

        let (db, _tmp) = test_db_state();
        let rebuilt = rebuild_index_impl(&search_state, &db).expect("rebuild index");
        assert_eq!(rebuilt, 0, "empty project db should rebuild zero docs");
    }

    #[test]
    fn search_commands_report_lock_failures() {
        let poisoned_search = test_search_state();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned_search.0.lock().expect("lock search");
            panic!("poison search state");
        }));
        let err = search_impl(&poisoned_search, "query".to_string(), Some(5))
            .expect_err("poisoned search");
        assert!(err.to_lowercase().contains("poison"));

        let (poisoned_db, _tmp) = test_db_state();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned_db.0.lock().expect("lock db");
            panic!("poison db");
        }));
        let search_state = test_search_state();
        let err = rebuild_index_impl(&search_state, &poisoned_db).expect_err("poisoned db");
        assert!(err.to_lowercase().contains("poison"));
    }
}
