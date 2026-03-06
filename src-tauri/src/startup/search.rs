use std::io;
use std::path::Path;
use std::sync::Mutex;

use tauri::Manager;

use super::SetupContext;
use crate::{search, SearchState};

pub(crate) fn initialize(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<u64, Box<dyn std::error::Error>> {
    let index_dir = context.data_dir.join("search_index");
    let search_index = open_with_fallback(&index_dir)?;
    let doc_count = search_index.doc_count().unwrap_or(0);

    app.manage(SearchState(Mutex::new(search_index)));
    Ok(doc_count)
}

fn open_with_fallback(index_dir: &Path) -> Result<search::indexer::SearchIndex, io::Error> {
    match search::indexer::SearchIndex::open(index_dir) {
        Ok(index) => Ok(index),
        Err(error) => {
            tracing::warn!(
                "Search index unavailable (another instance running?): {error}. \
                 Falling back to in-memory index."
            );
            let fallback =
                search::indexer::SearchIndex::open_in_memory().map_err(|open_error| {
                    io::Error::other(format!(
                        "failed to create in-memory search index: {open_error}"
                    ))
                })?;
            Ok(fallback)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_in_memory_index_when_persistent_index_path_is_unusable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let index_dir = tmp.path().join("search_index");
        std::fs::write(&index_dir, b"not a directory").expect("create blocking file");
        assert!(
            index_dir.is_file(),
            "test precondition should create a file"
        );

        let mut index = open_with_fallback(&index_dir).expect("fallback index should open");
        index
            .add_document(
                "project-1",
                "document",
                "README.md",
                "README",
                "startup fallback keeps app functional",
            )
            .expect("add doc");
        index.commit().expect("commit");

        let results = index.search("fallback", 5).expect("search should work");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "README.md");
        assert!(
            index_dir.is_file(),
            "fallback should not require rewriting index path"
        );
    }
}
