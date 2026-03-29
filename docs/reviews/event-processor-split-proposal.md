# Event Processor Queue Split Proposal

Date: 2026-03-29
Author: architect-1
Task: #10

## Objective

Split `src-tauri/src/event_processor.rs` from a single inline worker into:

- one intake/classification stage that preserves the current batch-and-flush semantics
- bounded workload queues for heavy follow-on work
- isolated workers so slow search or session work does not delay unrelated git or task work

This proposal is deliberately incremental. It does not replace the watcher model. It reorganizes the work that happens after a batch flush.

## Current State

Today `process_watch_events()` does all of the following inline after each batch:

- activity timestamp writes
- git status refresh
- git retry scheduling
- commit reindexing on git change
- session import
- session search indexing
- file-change search updates
- gitignore-triggered project rebuild
- task-trigger fan-out for internal events
- frontend event emission

Strengths of the current design:

- the watcher-side batch-and-flush behavior is good
- the quiet-window and max-wait model is practical
- work is easy to reason about because it all happens in one place

Problems:

- one slow workload stretches the latency of all other workloads
- there is no clear per-workload backpressure model
- retries and cooldowns are scattered and workload-specific
- the event processor now mixes intake, scheduling, execution, and observability

## Recommendation

Use bounded queue-per-workload workers, not a shared work-stealing pool.

Recommended workers:

- `git_refresh`
- `search_update`
- `session_import`
- `task_scan`
- `project_activity` as a small extra queue even though it is not called out in the assignment, because keeping DB writes in intake defeats the point of the split

Why queue-per-workload is the better fit:

- search work is CPU and index-writer heavy
- git refresh is provider and process I/O heavy
- session import has correctness sensitivity around unique files
- task scans already have a natural coalescing model
- these workloads need different overflow and retry behavior

A shared generic pool would reduce code at first, but it would reintroduce contention and make overload behavior harder to reason about.

## Proposed Architecture

### 1. Intake stage

Keep the existing watcher thread and batching logic in `process_watch_events()`.

After each batch flush, the intake stage should do only three things:

- classify the batch into workload-specific work items
- emit lightweight immediate frontend events that reflect observed file changes
- enqueue work items into domain queues without performing the heavy work inline

The intake stage should no longer:

- lock the search index
- import sessions
- perform git status refresh
- rebuild project search indexes
- touch project activity directly

### 2. Worker state owned by the app

Introduce a managed state object, for example:

```rust
pub struct EventWorkState {
    pub git: QueueHandle<GitRefreshWork>,
    pub search: QueueHandle<SearchWork>,
    pub sessions: QueueHandle<SessionImportWork>,
    pub tasks: QueueHandle<TaskScanTrigger>,
    pub activity: QueueHandle<ActivityTouchWork>,
}
```

Each `QueueHandle` owns:

- bounded sender
- worker thread handle metadata
- queue metrics
- coalescing stats
- health status

### 3. Work item shapes

Suggested work items:

```rust
enum SearchWork {
    FileDelta { project_id: String, project_path: String, paths: Vec<PathBuf> },
    GitDelta { project_id: String, project_path: String },
    RebuildProject { project_id: String, project_path: String, reason: SearchRebuildReason },
    IndexSession { project_id: String, session_id: String },
}

struct GitRefreshWork {
    project_id: String,
    project_path: String,
    emit_when_unchanged: bool,
}

struct SessionImportWork {
    project_id: String,
    path: PathBuf,
}

struct ActivityTouchWork {
    project_ids: Vec<String>,
}
```

`TaskScanTrigger` already exists and should become a first-class managed queue instead of a side-thread hidden inside the event processor.

## Queue Design

### Preferred model

Use one bounded channel per workload with dedicated worker ownership.

Recommended initial capacities:

- `git_refresh`: 128
- `search_update`: 512
- `session_import`: 256
- `task_scan`: 64
- `project_activity`: 128

These are starting values only. Final sizing should be driven by observed queue depth and age metrics.

### Coalescing policy

Do not treat every event as unique queue work.

Coalescing rules:

- `git_refresh`
  - only one outstanding refresh per project
  - merge duplicate refresh requests
- `search_update`
  - merge file deltas by project
  - collapse repeated `RebuildProject` requests into one highest-priority rebuild marker
  - if a project already has a queued rebuild, drop lower-level file-delta work for that project
- `session_import`
  - dedupe exact file paths
  - do not collapse distinct session files into one generic item
- `task_scan`
  - preserve the existing debounce semantics
  - if multiple triggers arrive while a full scan is pending, collapse to `Full`
- `project_activity`
  - collapse to unique project IDs per short window

This keeps queues bounded without sacrificing correctness.

## Backpressure Model

Backpressure should be workload-specific. The intake stage must never block on a full heavy-work queue.

### Search queue

On full:

- coalesce by project first
- if still full, replace pending per-file deltas with a single `RebuildProject` marker for that project
- emit `event_queue.search.degraded`

Rationale:

- search is derived state
- it is acceptable to lose some incremental fidelity if the system schedules a later rebuild

### Git queue

On full:

- keep at most one pending refresh per project
- if enqueue still fails, record a per-project dirty marker and let the next periodic reconcile or later event retry it
- emit `event_queue.git.skipped`

Rationale:

- duplicate git refreshes are low value
- correctness is maintained by future refresh opportunities

### Session-import queue

On full:

- do not silently drop unique session files
- persist a lightweight “project needs session rescan” marker in memory and emit `event_queue.sessions.overflow`
- a periodic or on-idle reconciliation pass should rescan that project’s handoff directory

Rationale:

- session import is less safely lossy than search deltas

### Task-scan queue

On full:

- collapse to one pending `Full` trigger
- emit `event_queue.tasks.coalesced`

Rationale:

- task scans are already set up for eventual consistency through rescan

### Activity queue

On full:

- merge to a per-project set
- if overflow persists, drop duplicate touches but retain one pending touch per project

Rationale:

- activity timestamps do not require per-event exactness

## Failure Isolation

The main design goal is isolation.

Isolation rules:

- search worker failures must not block git refresh or session import
- session import failures must not block file-change UI emission
- task scan lag must not slow search writes
- worker-local retries must stay in the same queue domain

Recommended worker model:

- one worker thread per queue to start
- search remains single-threaded because the tantivy writer is already serialized
- git can remain single-threaded initially; increase only if metrics justify it
- session import stays single-threaded until the SQLite and indexing model is improved

This is intentionally conservative. The first improvement is isolation, not maximizing throughput.

## Observability

Add structured metrics per queue.

Required signals:

- current depth
- enqueue failures
- coalesced item count
- oldest item age
- processing duration
- success/failure count
- retry count
- worker heartbeat timestamp

Suggested events:

- `event_queue.enqueue`
- `event_queue.dequeue`
- `event_queue.overflow`
- `event_queue.coalesced`
- `event_queue.worker.failed`
- `event_queue.worker.recovered`
- `event_queue.item.completed`

Also add one snapshot-style health report that can be queried or logged periodically:

- queue depth by workload
- oldest queued age by workload
- worker alive flag by workload

## Migration Path

Use incremental extraction, not a big-bang rewrite.

### Phase 1: Introduce queue state without behavioral change

- add `EventWorkState`
- move the current task trigger side-thread into managed startup-owned worker state
- add queue metrics and health reporting

Outcome:

- no behavior change yet
- infrastructure exists for later extraction

### Phase 2: Extract search work first

- move file-delta indexing, git commit reindexing, and gitignore rebuilds into the search queue
- keep current intake batching and immediate `project-files-changed` emission

Why first:

- search is the heaviest and easiest workload to isolate
- it is derived state, so rollback risk is lower

### Phase 3: Extract git refresh

- move git status refresh and retry scheduling into the git queue
- keep dedupe-per-project semantics

### Phase 4: Extract session import

- move session import and session indexing into the session queue
- add overflow-to-rescan fallback

### Phase 5: Extract activity touches

- move `touch_activity` into the activity queue
- this removes the remaining DB write from the intake stage

### Phase 6: Delete the old inline execution path

- remove now-unused inline heavy-work branches
- keep the same batch classification surface so the watcher contract stays stable

## Rollback Story

Rollback should be cheap.

Recommended approach:

- gate worker routing behind one internal feature flag or runtime toggle
- preserve the old inline execution functions for the first migration stages
- if queue workers show regressions, switch routing back to inline while keeping the new metrics

The goal is reversible extraction, not one-way surgery.

## Risks

### Risk: queue system adds complexity without enough payoff

Mitigation:

- only introduce five small workload queues
- keep routing and work-item types simple
- avoid a generic scheduler framework

### Risk: overflow logic loses data

Mitigation:

- only allow lossy degradation for derived search work
- use rescan markers for session-import overflow
- preserve task-scan eventual consistency via `Full`

### Risk: immediate UI signals become inconsistent with delayed backend state

Mitigation:

- document which frontend events mean “observed file change” vs “backend projection updated”
- keep `search-index-updated` worker-emitted and `project-files-changed` intake-emitted

### Risk: queue metrics are added but not used

Mitigation:

- add explicit thresholds for warning logs on queue age and repeated overflow
- include queue health in startup/runtime diagnostics

## Recommended Final Direction

Adopt bounded queue-per-workload workers with explicit coalescing and overflow semantics.

Do not use a shared work-stealing pool for this refactor.

Why:

- failure isolation matters more than generic scheduler elegance here
- workloads have materially different overflow rules
- the current code already hints at domain-specific behavior such as task-trigger debounce and git retry caps
- this approach can be rolled out incrementally with low operational risk

## Short Version

Keep the watcher batcher. Remove heavy inline work from `process_watch_events()`. Route follow-on work into bounded workload queues with per-queue coalescing, metrics, and rollback capability. Start with search, then git, then session import, then activity writes.
