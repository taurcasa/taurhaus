# Event-Driven Updates Audit

Date: 2026-03-28
Owner: `dev-1`

## Scope

Audited the realtime update path from backend watcher events to frontend state:

- `src-tauri/src/fs/watcher.rs`
- `src-tauri/src/event_processor.rs`
- `src-tauri/src/startup/watchers.rs`
- frontend listeners in `src/Shell.svelte`, `src/lib/GitTab.svelte`, `src/lib/TaskBoard.svelte`, and `src/lib/SearchOverlay.svelte`

## Findings

### 1. Git watcher events updated branch/dirty state but not commit-driven views

Backend `project-git-changed` events were emitted correctly, but the frontend only used them to patch sidebar/header git state. The Overview commit list and `GitTab` commit list stayed stale until a manual refresh or tab reload.

Fix:

- `src/Shell.svelte` now refreshes the selected project's Overview commit data on `project-git-changed`
- `src/lib/GitTab.svelte` now subscribes to `project-git-changed` and refreshes its commit list in place

### 2. Open task detail panels could drift out of sync after realtime task refreshes

`TaskBoard` already refreshed the task list on `project-tasks-changed`, but an open detail panel kept showing the old selected task object. If the selected task disappeared, the detail panel stayed open against stale data.

Fix:

- `src/lib/TaskBoard.svelte` now reconciles the selected task after every realtime task refresh
- if the selected task disappears, the panel closes
- if the selected task still exists, the panel state is updated and detail is re-fetched when task metadata changes

### 3. Search index updates had no frontend consumer

The backend emitted `search-index-updated`, but the frontend never listened for it. An open search overlay would continue showing stale results until the user changed the query manually.

Fix:

- `src/lib/SearchOverlay.svelte` now listens for `search-index-updated` in Tauri mode and re-runs the active query while the overlay is open

### 4. Watcher-driven task sync could suppress metadata-only task updates

The watcher-side task signature in `src-tauri/src/bootstrap.rs` compared only coarse task identity/state information. Metadata-only changes could compare equal and suppress `project-tasks-changed`, even though the UI should refresh.

Fix:

- expanded the watcher-side signature to include update/archive signals so metadata changes participate in equality checks
- added a regression test covering metadata-only changes

## Notes

The reported "task additions don't update in real time" symptom did not reproduce as a missing list-refresh path in the current `TaskBoard` component. The board already re-fetches on `project-tasks-changed`, and that path had existing coverage. The audit did uncover two adjacent task-update bugs instead:

- stale selected-task detail after realtime refresh
- backend suppression risk for metadata-only task changes

## Validation

Executed:

- `bun --bun ./node_modules/vitest/vitest.mjs run src/lib/gitTab.test.js src/lib/SearchOverlay.test.js src/lib/taskBoard.test.js`
- `cargo test -p taurhaus bootstrap::tests::task_status_signature_changes_when_task_metadata_changes`

Result:

- frontend regressions: 102 passing tests
- Rust regression: passing
