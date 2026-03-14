# Restore And Project Selection Instrumentation - 2026-03-14

## Scope

Task `#1295`: land structured timing/logging instrumentation for:

- minimize / hidden -> visible recovery
- project-selection batch timing
- per-section project-selection timing with provider-route classification

The goal is not to change behavior yet. The goal is to make the next stall
diagnosable from logs without repeating a full static code trace.

## Code Landed

Frontend instrumentation landed in:

- `src/lib/shell/events.svelte.js`
- `src/Shell.svelte`
- `src/lib/projectSelection.js`

Regression coverage landed in:

- `src/lib/shell/events.test.js`
- `src/lib/projectSelection.test.js`

## New Log Events

## 1. Visibility / restore events

Emitted from `setupSessionPollingLifecycle(...)` in
`src/lib/shell/events.svelte.js`.

### `shell.visibility.hidden`

Fields:

- `session_bridge_live`
- `recovery_mode`

Current `recovery_mode` values:

- `bridge_live_no_polling_change`
- `pause_fallback_polling`

### `shell.visibility.visible`

Fields:

- `session_bridge_live`
- `hidden_duration_ms`
- `recovery_mode`

Current `recovery_mode` values:

- `bridge_live_no_polling_change`
- `resume_fallback_polling`

How to use:

- confirm whether the app really saw a hidden -> visible cycle
- measure how long the app stayed hidden before the restore-path switch
- distinguish bridge-live restores from fallback-polling restores

## 2. Shell selection lifecycle events

Emitted from `selectProject(...)` in `src/Shell.svelte`.

### `shell.project_selection.started`

Fields:

- `project_id`
- `project_path`
- `daemon_status`
- `session_bridge_live`
- `visibility_state`
- `selection_generation`
- `blocking`
- `deferred`

### `shell.project_selection.discarded`

Fields:

- `project_id`
- `elapsed_ms`
- `daemon_status`
- `selection_generation`
- `reason`
- `blocking`
- `deferred`

Current `reason`:

- `stale_generation`

### `shell.project_selection.applied`

Fields:

- `project_id`
- `elapsed_ms`
- `daemon_status`
- `issue_count`
- `pending_retry`
- `selection_generation`
- `blocking`
- `deferred`

How to use:

- measure total shell-visible selection latency
- tell the difference between a slow selection that eventually applied and a
  selection that was overtaken by a newer one
- correlate applied/degraded selection with daemon recovery state

## 3. Project-selection batch events

Emitted from `src/lib/projectSelection.js`.

### `project.selection.batch.started`

Fields:

- `project_id`
- `section_count`
- `project_path`
- `daemon_status`
- `batch_kind`
- `blocking`
- `deferred`

### `project.selection.batch.completed`

Fields:

- `project_id`
- `duration_ms`
- `section_count`
- `failed_section_count`
- `failed_sections`
- `retryable_section_count`
- `retryable_sections`
- `batch_kind`
- `blocking`
- `deferred`

How to use:

- distinguish a real blocking user selection from a speculative deferred prefetch
- see whether the whole batch was slow or whether only one or two sections were
  problematic
- see whether failures were daemon-retry-shaped versus permanent

## 4. Per-section timing events

Emitted once per selection section from `withFallback(...)` in
`src/lib/projectSelection.js`.

### `project.selection.section.completed`

Fields:

- `project_id`
- `section`
- `section_key`
- `provider_route`
- `duration_ms`
- `timeout_ms`
- `ok`
- `retryable_on_daemon_reconnect`
- `error_message` when `ok=false`
- `batch_kind`
- `blocking`
- `deferred`

Current `provider_route` values:

- `db`
- `local_provider`
- `daemon_provider`
- `local_provider_fallback`
- `provider_route_unknown`

Interpretation:

- `db`: SQLite-backed section (`getProject`, sessions, relationships)
- `local_provider`: provider-backed section expected to stay local
- `daemon_provider`: provider-backed section expected to go through the daemon
- `local_provider_fallback`: WSL-path section expected to fall back to local
  access because the daemon is not currently usable
- `provider_route_unknown`: path implies a provider decision, but the shell did
  not yet have enough daemon state to classify it cleanly

How to use:

- if only `commits` or `readme` are slow, the stall is likely in provider work,
  not in the whole shell update
- if `db` sections are fast but `daemon_provider` sections are slow, the route
  diagnosis is already done without another code review
- if a post-restore switch shows `local_provider_fallback` on WSL paths, the
  stall is likely happening during daemon recovery or fallback selection rather
  than inside the normal daemon route

## Recommended Diagnostic Flow

For the next reported restore -> project-switch stall, check the logs in this
order:

1. `shell.visibility.hidden` and `shell.visibility.visible`
   - confirm a real hidden -> visible cycle happened
   - capture `hidden_duration_ms`
2. `shell.project_selection.started`
   - capture the target project and daemon/session state at click time
3. `project.selection.section.completed`
   - identify which section was slow
   - check `provider_route`
   - check whether it was `blocking` or `deferred`
4. `project.selection.batch.completed`
   - confirm whether the delay was a single slow section or a whole-batch stall
5. `shell.project_selection.applied` or `shell.project_selection.discarded`
   - determine whether the user saw the selection complete, or whether it was
     superseded before completion
