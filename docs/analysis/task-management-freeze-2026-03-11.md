# Task Management Performance Fix — 2026-03-11

## Scope

Task `#920` targeted Windows UI freezes in task management, especially on projects with large task sets. The requirement was to preserve task/history functionality while removing synchronous heavy work from the request path.

## Phase 1: Blocking Path Trace

### `get_project_tasks` before this fix

Frontend path:
- `TaskBoard.svelte` -> `getProjectTasks(projectId)` IPC
- `src-tauri/src/commands/tasks.rs::get_project_tasks`
- `src-tauri/src/services/task_query.rs::get_or_refresh_project_tasks`

Blocking behavior before this fix:
- request path could fall through to on-demand recovery scanning
- recovery invoked task-file scanning and persistence inline
- on Windows/WSL projects, this could mean:
  - live session scan
  - Claude source index rebuild
  - task directory traversal under `~/.claude/tasks/`
  - SQLite persistence
  - operational snapshot sync
- all of that happened before the IPC returned

That made the Task Board sensitive to both filesystem size and runtime/provider state.

### `get_archived_sessions` before this fix

Frontend path:
- `SessionHistory.svelte` -> `getArchivedSessions(projectId)` IPC
- `src-tauri/src/commands/tasks.rs::get_archived_sessions`
- `src-tauri/src/services/task_query.rs::get_archived_sessions`

Blocking behavior before this fix:
- loaded all archived task rows
- grouped them by session
- for each session, synchronously computed:
  - transcript-derived time range
  - git commit/file counts via `provider.commits_in_range(...)`
- this meant history loads could block on transcript resolution and git/provider work before the IPC returned

This was the worst part of the freeze profile because it mixed DB work with transcript scanning and git history queries on the UI request path.

## Phase 2: Architecture Decision

### Active tasks

New rule:
- `get_project_tasks` is now a pure DB read
- no synchronous recovery scanning is allowed on the request path
- a throttled background refresh is scheduled after the response

Why:
- task freshness already has an event-driven backbone (`task_scan_loop`, watch-triggered scans, startup scan)
- request-time recovery duplicated that work and turned view-open into a scan/persist operation
- background refresh preserves eventual correctness without blocking UI

### Archived sessions

New rule:
- `get_archived_sessions` now reads:
  - archived task rows
  - persisted archived session summaries from SQLite
- if summaries are missing or stale, the request still returns immediately with fallback values and schedules a background rebuild

Why:
- the expensive part of history was enrichment, not loading task rows
- transcript/git enrichment belongs in an async cache-build path, not the request path

### Cache table

New table:
- `archived_task_session_summaries`

Stored per `(project_path, session_key)`:
- `session_id`
- `started_at`
- `ended_at`
- `duration_ms`
- `commit_count`
- `file_count`
- `sources_json`
- `last_archived_at`
- `enrichment_warnings`
- `updated_at`

This keeps the UI request path DB-only while preserving the existing session-history surface.

## Phase 3: Implementation Plan

1. Add a persisted archived-summary table and tests.
2. Convert `get_project_tasks` to DB-only reads.
3. Add throttled background project-task refresh scheduling in the command layer.
4. Add cached archived-session reads plus missing/stale cache detection.
5. Add background archived-summary rebuild scheduling in the command layer.
6. Reuse existing `project-tasks-changed` event for both Task Board and Session History refresh.
7. Validate with targeted Rust tests plus the repo quick gate.
8. Verify against the real Windows task corpus.

## Phase 4: Implementation Landed

### Schema

Added migration 11:
- `src-tauri/src/db/migrations/011_archived_task_session_summaries.sql`

Wired in:
- `src-tauri/src/db/migrations.rs`
- `src-tauri/src/db/mod.rs`

### DB query layer

Added summary persistence/query helpers in:
- `src-tauri/src/db/task_queries.rs`

Notable additions:
- `PersistedArchivedSessionSummary`
- `archived_session_key(...)`
- `get_archived_session_summaries_for_project(...)`
- `replace_archived_session_summaries_for_project(...)`

### Service layer

Updated:
- `src-tauri/src/services/task_query.rs`

Changes:
- `get_or_refresh_project_tasks(...)` is now DB-only
- `get_archived_sessions(...)` now returns:
  - result payload
  - cache status (`Fresh`, `Missing`, `Stale`)
- added `rebuild_archived_session_summaries(...)`
- added cached/fallback archived-session assembly
- removed the old tests that encoded synchronous recovery expectations

### Command layer

Updated:
- `src-tauri/src/commands/tasks.rs`

Changes:
- added `TaskQueryRefreshState`
- `get_project_tasks` now schedules a throttled background refresh after returning the DB result
- `get_archived_sessions` now schedules a throttled archived-summary rebuild only when cache is missing/stale
- both refresh paths emit `project-tasks-changed` when data changes so the frontend refreshes without introducing a new event type

### Startup state

Updated:
- `src-tauri/src/startup/mod.rs`

Change:
- managed `TaskQueryRefreshState`

### Shared scan generation

Updated:
- `src-tauri/src/bootstrap.rs`

Change:
- exported `next_task_scan_cycle_id()` so background refreshes use the same generation sequencing as the existing task scan loop

## Phase 5: Verification

### Targeted Rust validation

Passed:
- `cargo test --manifest-path src-tauri/Cargo.toml commands::tasks -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml task_query -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml task_queries -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml --tests`
- `just check-quick`

`just check-quick` result:
- passed
- `1101` frontend tests green in the current tree

### Real Windows dataset verification

Using the live Windows app DB at:
- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.db`

Observed task volume:
- project `/home/user/projects/taurhaus`
  - total rows: `1175`
  - active rows: `897`
  - archived rows: `278`
- project `/home/user/projects/mesh`
  - total rows: `986`
  - active rows: `825`
  - archived rows: `161`

Measured remaining DB-only request-path costs on the live Windows DB (read-only):
- `taurhaus` active full-row fetch (`897` rows): about `104ms`
- `taurhaus` archived full-row fetch (`278` rows): about `14ms`
- `mesh` active full-row fetch (`825` rows): about `7ms`
- `mesh` archived full-row fetch (`161` rows): about `1ms`

Interpretation:
- the request path is now bounded by SQLite row fetch/serialization instead of transcript scanning, git enrichment, and task-file rescans
- the large synchronous freeze source has been removed

## Tradeoffs

### Kept
- same frontend IPC contract
- same `project-tasks-changed` invalidation event
- same archived-session task payload shape

### Changed
- history can temporarily show fallback `commit_count=0` / `file_count=0` with a warning while background enrichment catches up
- task/history freshness is now eventual on view-open safety refresh, not synchronous inline recovery

That tradeoff is intentional. It prefers responsiveness over request-time rebuilds.

## Remaining Risks

1. The first history open after migration may show fallback summary values until the background rebuild completes.
2. If `project-tasks-changed` delivery is delayed, the frontend may briefly display older task/history data until the next refresh.
3. This fix removes the dominant backend freeze source, but it does not address any separate frontend rendering cost from very large payloads.

## Recommendation

Ship this change.

It removes the architectural mistake that made task/history navigation block on scanning and enrichment work. Any further performance work should now focus on payload/render size or cache warmup behavior, not reintroducing synchronous recovery.
