# Compaction + Task Management + Daemon Lifecycle Interaction Audit — 2026-03-11

## Scope

Task `#929` asked for a cross-subsystem audit of three recent change areas:

1. compaction detection and reinjection
2. task-management performance/background refresh
3. daemon lifecycle and reconnect recovery

The goal was to find **interaction risks and hidden coupling**, not to re-review each subsystem in isolation.

Primary sources reviewed:

- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/operational_context.rs`
- `src-tauri/src/commands/tasks.rs`
- `src-tauri/src/services/task_query.rs`
- `src-tauri/src/services/task_sync.rs`
- `src-tauri/src/daemon_lifecycle.rs`
- `src-tauri/src/commands/command_center/session_listing.rs`
- `src-tauri/src/commands/command_center/mod.rs`
- `src-tauri/src/commands/coordination.rs`
- `src-tauri/src/session_snapshot_cache.rs`
- `src/lib/TaskBoard.svelte`
- `src/lib/SessionHistory.svelte`
- `src/lib/sessionStore.svelte.js`
- [task-management-freeze-2026-03-11.md](./task-management-freeze-2026-03-11.md)
- [backend-freeze-audit-2026-03-11.md](./backend-freeze-audit-2026-03-11.md)
- [compaction-delivery-audit-2026-03-10.md](./compaction-delivery-audit-2026-03-10.md)

## Current Interaction Model

The current architecture is split into three control planes:

### 1. Task plane

- `get_project_tasks` and `get_archived_sessions` are now DB-first request paths.
- background refresh is triggered from `commands/tasks.rs`.
- every successful task scan still persists task rows and then calls `sync_project_task_snapshots(...)` in `services/task_sync.rs`.

### 2. Compaction plane

- Codex compaction resolves a managed member and loads the current `OperationalContextSnapshot` in `coordination/compaction_processor.rs`.
- Claude compaction loads the same snapshot in `coordination/claude_hooks.rs`.
- both paths suppress reinjection when `snapshot_has_resumable_task(...)` is false.

### 3. Runtime/daemon plane

- command-center session queries now prefer daemon runtime snapshots and may fall back to a cached snapshot in `session_snapshot_cache.rs`.
- `daemon_lifecycle.rs` now reconnects more conservatively and emits a fresh `sessions-updated` snapshot immediately after reconnect.
- coordination live-team status still has its own direct daemon snapshot logic in `commands/coordination.rs`.

That means the systems are coupled through two shared state surfaces:

1. `OperationalContextSnapshotStore`
2. daemon runtime session snapshots / runtime member attachments

## What Is Already Better

These interaction problems were already reduced by recent fixes:

1. task requests no longer synchronously scan task files on the UI request path
2. task history refresh no longer reuses `project-tasks-changed`; it uses `project-task-history-changed`
3. daemon reconnect no longer restarts the daemon on the first socket failure
4. session UI request paths no longer fall back into expensive local WSL scans when a daemon provider exists

Those changes are real and should stay.

## Findings

### 1. High: task background refresh still rewrites compaction context

Code path:

- `commands/tasks.rs::schedule_project_task_refresh(...)`
- `services/task_sync.rs::persist_task_scan_with_generation(...)`
- `coordination/operational_context.rs::sync_project_task_snapshots(...)`
- `coordination/operational_context.rs::latest_owned_task(...)`
- `coordination/compaction_processor.rs::process_signal_at(...)`
- `coordination/claude_hooks.rs::handle_session_start_hook(...)`

What happens:

1. a background task refresh scans task files
2. persistence runs
3. `sync_project_task_snapshots(...)` rewrites member operational snapshots from DB task state
4. later compaction reinjection uses that rewritten snapshot as the source of truth

Why this is risky:

- reinjection eligibility is controlled by `snapshot_has_resumable_task(...)`
- `latest_owned_task(...)` only keeps tasks with status `pending` or `in_progress`
- if the scan temporarily sees no resumable owned task, the snapshot task becomes empty
- then compaction reinjection is skipped with `no_resumable_task_context`

This means task scanning is still allowed to clear the very task context that compaction delivery depends on.

That is not a theoretical coupling. It is an explicit control dependency.

Why it matters:

- task-management performance changes were supposed to make task refresh cheaper, not make compaction delivery more brittle
- right now a transient task scan gap can become a compaction-delivery decision

Recommended fix direction:

- separate **assignment-owned active task context** from **scanner-derived latest owned task**
- do not let scanner refreshes clear resumable operational task context immediately
- introduce hysteresis or explicit provenance before erasing the current task from the snapshot

### 2. High: daemon-backed runtime state still has inconsistent fallback semantics across subsystems

Code path A, command-center UI/session queries:

- `commands/command_center/session_listing.rs::daemon_runtime_session_snapshot(...)`
- `commands/command_center/mod.rs::get_foreground_project_impl(...)`
- uses live daemon snapshot, inline reconnect, then cached snapshot, then empty/none

Code path B, coordination live-team status:

- `commands/coordination.rs::daemon_runtime_session_snapshot(...)`
- `commands/coordination.rs::coordination_get_live_team_status_impl(...)`
- uses live daemon snapshot only, then falls back to orchestrator reconcile + attachment state

Why this is risky:

- two major surfaces now answer runtime-session questions with different freshness and fallback rules
- during daemon reconnect windows:
  - Task/session UI can show cached daemon state
  - coordination/mesh runtime views can show reconciled attachment state instead
- compaction resolution itself does not use the command-center cache path, so diagnostics across views can disagree during exactly the periods we care about most

This is a maintainability problem and an operational debugging problem.

If the same app asks “what sessions are live?” in two places, it should not use materially different recovery semantics unless that difference is intentional and documented.

Recommended fix direction:

- centralize daemon runtime snapshot access behind one shared policy layer
- make command-center and coordination explicitly choose from the same freshness contract
- if one surface needs stale-cache tolerance and another does not, encode that as a named policy instead of separate ad hoc implementations

### 3. Medium: the new session snapshot cache has no freshness boundary or invalidation policy

Code path:

- `session_snapshot_cache.rs`
- `commands/command_center/session_listing.rs`

Current behavior:

- successful daemon snapshots are cached globally in-process
- there is no timestamp, generation, TTL, or restart epoch attached to the cached value
- there is no explicit invalidation on daemon restart or long disconnect windows

Why this is risky:

- it was the right immediate freeze fix for request-path resilience
- but as a long-lived architectural surface it is underspecified
- after a daemon restart, the cache can temporarily represent a different runtime epoch than the live daemon or coordination attachment layer

This is not as severe as the old request-path fallback scans, but it is exactly the kind of “stability patch becomes implicit architecture” problem that causes later confusion.

Recommended fix direction:

- add freshness metadata to the cache (`captured_at`, daemon identity/version if available)
- bound reuse with a short TTL
- clear the cache on explicit daemon restart transitions

### 4. Medium: background task refresh is still bridge-unaware and can compete with daemon recovery

Code path:

- `commands/tasks.rs::schedule_project_task_refresh(...)`
- `services/task_sync.rs::scan_tasks_from_files(...)`
- `daemon_lifecycle.rs`

What improved:

- this work no longer blocks the request path

What still matters:

- the background refresh still runs while daemon reconnect/recovery is ongoing
- the scan can still touch provider-backed paths and then persist task state / operational snapshots
- if the daemon bridge is flapping, background refresh can keep producing state churn while the runtime plane is still stabilizing

This is not the same severity as the original freeze, because it no longer blocks the UI directly.
But it is still a coupling point where one recovering subsystem can be stressed by another subsystem’s “safe background” work.

Recommended fix direction:

- gate project task background refresh on daemon health when the project depends on daemon-backed runtime/session information
- or at minimum suppress operational snapshot rewrite on refreshes that occur while runtime health is degraded

### 5. Low: the recent docs already drift on one important interaction boundary

Example:

- [task-management-freeze-2026-03-11.md](./task-management-freeze-2026-03-11.md) still says both refresh paths emit `project-tasks-changed`
- current code emits `project-task-history-changed` for archived refreshes in `commands/tasks.rs`

Why it matters:

- this is low severity at runtime
- but it is a direct signal that these interactions are changing quickly enough that architecture docs are lagging almost immediately
- that makes future cross-subsystem debugging harder

## Overall Assessment

### What is actually stable now

1. UI request paths are much safer than before
2. daemon reconnect behavior is much safer than before
3. task history no longer causes the same duplicate reload path it used to
4. compaction delivery itself is not obviously broken by the task-management freeze fix

### What is still structurally weak

1. compaction resumability still depends on a snapshot that task scans can rewrite
2. daemon runtime truth is still exposed through multiple fallback contracts
3. snapshot caching is operationally useful but architecturally underdefined
4. “background” task refresh still mutates state that compaction delivery treats as authoritative

## Recommended Follow-up Tasks

1. **Decouple operational task context from scanner-derived latest-owned-task refresh**
   - keep a distinct persisted active-task channel for reinjection / delivery context
   - scanner refresh may update it, but should not immediately clear it without stronger evidence

2. **Unify daemon runtime snapshot access behind one shared service/policy**
   - one implementation
   - explicit freshness/fallback modes
   - remove duplicate direct snapshot request logic from command-center vs coordination

3. **Add freshness metadata and invalidation rules to `session_snapshot_cache`**
   - TTL
   - restart/epoch invalidation
   - diagnostics so stale-cache use is visible in logs

4. **Make background task refresh daemon-health-aware when mutating operational snapshots**
   - either defer snapshot sync during degraded runtime windows
   - or keep refresh results but suppress snapshot overwrite until runtime is healthy again

## Risk If We Do Nothing

If we leave the current architecture as-is:

- the app will be much less freeze-prone than before, which is good
- but the next confusing “compaction was skipped even though the agent had an active task” incident is still plausible
- and future daemon-reconnect diagnostics will still be harder than necessary because different views are not using the same runtime-session truth model

So this is not an emergency regression. It is a **hidden-coupling debt** cluster.

The most important item is the first one: task refresh should not be allowed to silently erase resumable compaction context.
