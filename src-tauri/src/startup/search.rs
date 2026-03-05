use std::io;
use std::sync::Mutex;

use tauri::Manager;

use super::SetupContext;
use crate::{search, SearchState};

pub(crate) fn initialize(
    app: &mut tauri::App,
    context: &SetupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let index_dir = context.data_dir.join("search_index");
    let search_index = match search::indexer::SearchIndex::open(&index_dir) {
        Ok(index) => index,
        Err(error) => {
            tracing::warn!(
                "Search index unavailable (another instance running?): {error}. \
                 Falling back to in-memory index."
            );
            search::indexer::SearchIndex::open_in_memory().map_err(|open_error| {
                io::Error::other(format!(
                    "failed to create in-memory search index: {open_error}"
                ))
            })?
        }
    };

    app.manage(SearchState(Mutex::new(search_index)));
    Ok(())
}
