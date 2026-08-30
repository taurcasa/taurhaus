use std::time::Instant;

use serde::Serialize;
use tauri::State;

use super::lifecycle::IpcCommandSpan;
use super::projects::DbState;
use crate::errors::{CommandResultExt, IpcResult, SanitizeErr};
use crate::search::indexer;
use crate::search::query::SearchResult;
use crate::SearchState;

/// Index status returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    pub doc_count: u64,
    pub is_empty: bool,
}

#[tauri::command(async)]
pub fn search(
    search_state: State<'_, SearchState>,
    query: String,
    limit: Option<usize>,
) -> IpcResult<Vec<SearchResult>> {
    let span = IpcCommandSpan::start("search");
    let result = search_impl(search_state.inner(), query, limit, Some(&span)).ipc_cmd("search");
    span.finish_result(&result);
    result
}

fn search_impl(
    search_state: &SearchState,
    query: String,
    limit: Option<usize>,
    span: Option<&IpcCommandSpan>,
) -> Result<Vec<SearchResult>, String> {
    let lock_started = Instant::now();
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(span) = span {
        span.emit_lock_wait("search_index", lock_started.elapsed().as_millis() as u64);
    }
    let limit = limit.unwrap_or(20).min(50);
    index.search(&query, limit).sanitize_err()
}

#[tauri::command(async)]
pub fn get_index_status(search_state: State<'_, SearchState>) -> IpcResult<IndexStatus> {
    let span = IpcCommandSpan::start("get_index_status");
    let result =
        get_index_status_impl(search_state.inner(), Some(&span)).ipc_cmd("get_index_status");
    span.finish_result(&result);
    result
}

fn get_index_status_impl(
    search_state: &SearchState,
    span: Option<&IpcCommandSpan>,
) -> Result<IndexStatus, String> {
    let lock_started = Instant::now();
    let index = search_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(span) = span {
        span.emit_lock_wait("search_index", lock_started.elapsed().as_millis() as u64);
    }
    let doc_count = index.doc_count().sanitize_err()?;
    Ok(IndexStatus {
        doc_count,
        is_empty: doc_count == 0,
    })
}

#[tauri::command(async)]
pub fn rebuild_index(
    search_state: State<'_, SearchState>,
    db: State<'_, DbState>,
) -> IpcResult<usize> {
    let span = IpcCommandSpan::start("rebuild_index");
    let result =
        rebuild_index_impl(search_state.inner(), db.inner(), Some(&span)).ipc_cmd("rebuild_index");
    span.finish_result(&result);
    result
}

fn rebuild_index_impl(
    search_state: &SearchState,
    db: &DbState,
    span: Option<&IpcCommandSpan>,
) -> Result<usize, String> {
    let search_lock_started = Instant::now();
    let mut index = search_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(span) = span {
        span.emit_lock_wait(
            "search_index",
            search_lock_started.elapsed().as_millis() as u64,
        );
    }
    let db_lock_started = Instant::now();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if let Some(span) = span {
        span.emit_lock_wait("db", db_lock_started.elapsed().as_millis() as u64);
    }
    indexer::rebuild_all(&mut index, &conn).sanitize_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::settings_queries;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;
    use tempfile::TempDir;

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

        let results = search_impl(&search_state, "hello".to_string(), Some(10), None)
            .expect("search results");
        assert_eq!(results.len(), 1);

        let status = get_index_status_impl(&search_state, None).expect("index status");
        assert!(!status.is_empty);
        assert!(status.doc_count >= 1);

        let (db, _tmp) = test_db_state();
        let rebuilt = rebuild_index_impl(&search_state, &db, None).expect("rebuild index");
        assert_eq!(rebuilt, 0, "empty project db should rebuild zero docs");
    }

    #[test]
    fn search_commands_report_lock_failures() {
        let poisoned_search = test_search_state();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned_search.0.lock().expect("lock search");
            panic!("poison search state");
        }));
        let err = search_impl(&poisoned_search, "query".to_string(), Some(5), None)
            .expect_err("poisoned search");
        assert!(err.to_lowercase().contains("poison"));

        let (poisoned_db, _tmp) = test_db_state();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned_db.0.lock().expect("lock db");
            panic!("poison db");
        }));
        let search_state = test_search_state();
        let err = rebuild_index_impl(&search_state, &poisoned_db, None).expect_err("poisoned db");
        assert!(err.to_lowercase().contains("poison"));
    }

    #[test]
    fn rebuild_index_honors_saved_ignore_patterns() {
        let (db, _tmp) = test_db_state();
        let project_dir = TempDir::new().expect("temp project dir");
        std::fs::write(project_dir.path().join("README.md"), "keep indexed").expect("write readme");
        std::fs::create_dir_all(project_dir.path().join("generated")).expect("mkdir generated");
        std::fs::write(
            project_dir.path().join("generated/skip.md"),
            "skip indexed content",
        )
        .expect("write ignored file");

        {
            let conn = db.0.lock().expect("lock db");
            conn.execute(
                "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "p1",
                    "Policy Project",
                    project_dir.path().to_string_lossy().to_string(),
                    "2026-03-19T00:00:00Z",
                    "2026-03-19T00:00:00Z",
                ],
            )
            .expect("insert project");

            let mut settings = settings_queries::get_all_settings(&conn).expect("get settings");
            settings.ignore_patterns = vec!["generated".into()];
            settings.scan_directories = vec![project_dir.path().to_string_lossy().to_string()];
            settings_queries::save_settings(&conn, &settings).expect("save settings");
        }

        let search_state = test_search_state();
        let rebuilt = rebuild_index_impl(&search_state, &db, None).expect("rebuild index");
        assert_eq!(rebuilt, 1);

        let kept = search_impl(&search_state, "keep".to_string(), Some(10), None)
            .expect("search kept content");
        assert_eq!(kept.len(), 1);

        let ignored = search_impl(&search_state, "skip".to_string(), Some(10), None)
            .expect("search ignored content");
        assert!(ignored.is_empty());
    }
}
