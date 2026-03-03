# Search

Search provides full-text lookup across all registered projects from a single overlay. It indexes project files, imported session summaries, and recent commit metadata, then returns ranked matches for fast cross-project navigation.

## Overview

Users open search from the titlebar button or keyboard shortcut:

- macOS: `Cmd+K`
- Windows/Linux: `Ctrl+K`

The overlay (`SearchOverlay.svelte`) runs debounced queries and groups results by entity type (`document`, `session`, `commit`). Selecting a result navigates to the appropriate tab and project context.

## Search overlay behavior

Frontend flow (`src/lib/SearchOverlay.svelte`):

1. User types into the search input.
2. Input is debounced by 150 ms.
3. Calls `search(query, 20)` via IPC.
4. Results are grouped and rendered in this order: documents, sessions, commits.
5. Keyboard navigation supports `ArrowUp/ArrowDown`, `Enter`, and `Escape`.

Navigation mapping:

- `document` -> Files tab (`filePath` navigation)
- `session` -> Overview tab (session section)
- `commit` -> Overview tab (commits section)

If a result belongs to another project, `Shell.svelte` first switches selected project, then applies tab/file navigation.

## Search index model

Backend search uses Tantivy (`src-tauri/src/search/`) with a shared schema:

| Field | Type | Purpose |
|---|---|---|
| `project_id` | `STRING + STORED` | Project scoping for cross-project results/navigation |
| `entity_type` | `STRING + STORED` | Result grouping (`document`, `session`, `commit`) |
| `file_path` | `STRING + STORED` | Path/identifier (`src/main.rs`, `session:<id>`, `commit:<hash>`) |
| `title` | `TEXT + STORED` | Display title + ranked text field |
| `content` | `TEXT + STORED` | Full-text searchable body |

Query execution (`query.rs`):

- Parses query against `title` + `content` fields.
- Uses Tantivy `TopDocs` relevance ordering (BM25 ranking).
- Generates snippets from matched content.
- Returns `SearchResult` with `relevance_score`.

## What gets indexed

### Documents (filesystem)

`index_project_files` indexes text-like files under each project root:

- Respects `.gitignore` and `.git/info/exclude` via `ignore::WalkBuilder`.
- Uses an allowlist of text extensions (`md`, `rs`, `ts`, `svelte`, `json`, `toml`, etc.).
- Skips files larger than 1 MB for indexing.
- Normalizes stored paths to forward slashes.

### Sessions (database)

`index_project_sessions` indexes imported session records from SQLite:

- Session summary
- Next steps
- Open questions

Stored as `entity_type = session` with synthetic path `session:<id>`.

### Commits (git)

`index_project_commits` indexes recent commits (default 100 per project for full build):

- Commit message (title)
- Combined commit text (`message`, `author`, `date`) as searchable content

Stored as `entity_type = commit` with synthetic path `commit:<hash>`.

## Index lifecycle and management

### Startup build

On app startup, `bootstrap::startup_search_index` builds the initial index only if doc count is zero.

### Incremental updates

Filesystem/git/session watcher events feed `event_processor.rs`, which updates index incrementally:

- File changes -> `update_file(...)`
- Session import events -> `index_session(...)`
- Git changes -> `reindex_commits(...)`

Batched watcher processing reduces lock churn:

- Quiet window: 300 ms
- Max wait: 2 s

`search-index-updated` events are emitted after successful incremental updates.

### Manual rebuild

Settings exposes `Rebuild index`, backed by IPC command `rebuild_index`, which clears and rebuilds all project documents/sessions/commits.

### Status

Settings also reads `get_index_status` and displays:

- `doc_count`
- `is_empty`

## Symlink safety in incremental indexing

`update_file` includes a project-boundary guard for changed paths:

- Canonicalizes project root and changed file path.
- Rejects/cleans entries whose canonical path resolves outside project root.
- Removes stale docs for deleted/unreadable/non-indexable files.

This prevents symlink escape paths from injecting out-of-project content into search results.

## Error handling behavior

- Empty/whitespace query returns empty results.
- Backend limit defaults to 20 and is capped to 50 per request.
- Frontend handles IPC errors by showing empty results state.
- Missing snippets fall back to title text on backend.

## Key files

| File | Purpose |
|---|---|
| `src/lib/SearchOverlay.svelte` | Search UI overlay, debounce, grouping, keyboard handling, result navigation mapping. |
| `src/Shell.svelte` | Global `Cmd/Ctrl+K` shortcut and cross-project search-result navigation handling. |
| `src-tauri/src/commands/search.rs` | IPC commands: `search`, `get_index_status`, `rebuild_index`. |
| `src-tauri/src/search/indexer.rs` | Tantivy schema, full rebuild, incremental updates, document/session/commit indexing. |
| `src-tauri/src/search/query.rs` | Query parsing, BM25-ranked retrieval, snippet generation. |
| `src-tauri/src/bootstrap.rs` | Startup index build (`startup_search_index`). |
| `src-tauri/src/event_processor.rs` | Watch-event batching and incremental index update triggers. |

## Related documents

- [IPC command reference](../architecture/ipc-reference.md) — search command signatures
- [Data model](../architecture/data-model.md) — tantivy index schema and lifecycle
