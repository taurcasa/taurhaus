# Taurhaus Architecture Review

Date: 2026-03-28
Reviewer: architect-1

## Executive Summary

Taurhaus has a strong architectural foundation for a desktop app in this class. The major strengths are:

- clear top-level subsystem separation
- unusually good architecture documentation for an active codebase
- a sound provider boundary for local vs daemon-backed project access
- strong path normalization and platform handling
- structured lifecycle logging that is materially useful for production debugging
- a well-factored coordination subsystem relative to its complexity

The main architectural risks are not conceptual mistakes. They are mostly scale and consistency problems:

- IPC command contracts are mixed between legacy `Result<T, String>` handlers and newer structured `IpcResult<T>` handlers
- backend state is concentrated behind a few global mutexes, especially one SQLite connection and one search writer
- the file-watch/event pipeline does too much synchronous work in one thread
- startup and runtime supervision rely on many detached threads with limited shared control
- a few central files and modules are large enough that future change velocity will degrade
- parts of the documentation are now ahead of or behind the real implementation

Overall assessment: good architecture with real production intent, but it now needs a consolidation phase. The next gains come from reducing contention, standardizing contracts, and shrinking the number of central choke points.

## Highest-Priority Improvements

### 1. Standardize IPC on one error/result contract

- Impact: High
- Effort: Medium

The backend currently mixes legacy string errors and newer structured IPC errors.

Evidence:

- `src-tauri/src/errors.rs` defines `IpcError`, `IpcErrorCode`, and `IpcResult<T>`
- newer commands such as `src-tauri/src/commands/tasks.rs` and `src-tauri/src/commands/coordination.rs` use `IpcResult<T>`
- many core commands still return `Result<..., String>`, including `src-tauri/src/commands/projects.rs`, `src-tauri/src/commands/files.rs`, `src-tauri/src/commands/git.rs`, `src-tauri/src/commands/search.rs`, and `src-tauri/src/commands/sessions.rs`
- the frontend transport in `src/lib/ipc/client.js` must normalize both plain strings and structured JSON-shaped errors

Why this matters:

- retryability is only explicit on some commands
- frontend handling stays defensive and branchy
- telemetry quality is uneven because some errors include structured codes and some do not
- cross-command UX consistency is harder than it should be

Recommendation:

- migrate all commands to `IpcResult<T>` plus `.ipc_cmd("command_name")`
- keep `normalizeInvokeError()` in the frontend, but treat it as compatibility code rather than the primary contract
- add a small audit test that asserts every registered command either returns `IpcResult<T>` or is intentionally grandfathered

### 2. Replace the single SQLite connection with a pooled access model

- Impact: High
- Effort: Medium to High

The backend uses a single `rusqlite::Connection` wrapped in `DbState(pub Mutex<Connection>)`.

Evidence:

- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/startup/setup.rs`
- broad call sites across commands, startup, and event processing lock the same connection

Why this matters:

- unrelated reads and writes serialize behind one mutex
- the watch pipeline, startup bootstrap, and user-triggered IPC all compete for the same lock
- lock-wait telemetry exists, but only a small subset of commands emits it

Recommendation:

- move to a small connection pool, or at minimum a dedicated write connection plus short-lived read connections
- keep SQLite as the store; the problem is the access model, not the database choice
- extend `ipc.lock.wait` instrumentation to all DB-backed commands during the migration so lock pressure becomes measurable

### 3. Split the event pipeline into lighter classification and heavier work stages

- Impact: High
- Effort: Medium

`src-tauri/src/event_processor.rs` is robust in batching, but it is also a choke point. One loop currently handles:

- event classification
- activity timestamp updates
- git-status refresh
- commit reindexing
- session import
- incremental file indexing
- gitignore-triggered rebuilds
- frontend event emission

Strength:

- the 300 ms quiet window and 2 s max-wait ceiling are pragmatic and well chosen
- the pipeline clearly avoids the usual watch-storm problem

Risk:

- slow indexing or a slow provider call delays unrelated session and UI events
- all work shares the same failure and backpressure domain

Recommendation:

- keep the current watcher batching stage
- move follow-on work into separate bounded queues for:
  - search updates
  - git refresh
  - session import
  - task scan triggers
- make the batcher publish work items and lightweight UI signals instead of performing all heavy work inline

### 4. Add supervision semantics for background threads

- Impact: Medium to High
- Effort: Medium

Startup and runtime maintenance are spread across many detached threads.

Evidence:

- `src-tauri/src/startup/orchestration.rs`
- `src-tauri/src/startup/bootstrap.rs`
- `src-tauri/src/startup/watchers.rs`
- `src-tauri/src/startup/daemon.rs`
- `src-tauri/src/event_processor.rs`

This is workable today, but the system now has enough always-on background behavior that it wants a more explicit supervision model.

Recommendation:

- introduce a small runtime supervisor that owns thread lifecycles and shutdown intent
- define restart policy per worker: never, bounded retry, or permanent fail-open
- surface worker health in one internal status view rather than relying only on logs

### 5. Break up the largest orchestration files before they become ownership traps

- Impact: Medium
- Effort: Medium

Several files are already in the range where architecture erodes through concentration rather than design choice.

Examples:

- `src-tauri/src/commands/projects.rs` at 1519 lines
- `src-tauri/src/event_processor.rs` at 1224 lines
- `src-tauri/src/coordination/state.rs` at 1186 lines
- `src-tauri/src/commands/coordination.rs` at 1012 lines
- `src/Shell.svelte` at 590 lines

Recommendation:

- split by operational responsibility, not by arbitrary file size
- keep `commands/` thin and move domain logic into service modules where that split is not already in place
- continue the pattern already used well in tasks and coordination pipelines

## Area-by-Area Review

### 1. IPC Command Design

Current state:

- The app registers 85 fine-grained commands in `src-tauri/src/lib.rs`.
- The granularity is generally right for a Tauri desktop app where the frontend benefits from parallel fan-out.
- Domain grouping is coherent and discoverable.

Strengths:

- command naming is consistent
- command modules map cleanly to product surfaces
- fine-grained commands help optimistic and parallel UI loading
- newer command families use `IpcCommandSpan` and structured errors well

Problems:

- the command surface is large enough that consistency must now be enforced, not assumed
- error contracts are mixed
- some command modules still hold a lot of business logic directly, especially projects
- documentation drift exists: `docs/architecture/ipc-reference.md` documents `coordination_get_compaction_audit`, but that command is not implemented or registered in `src-tauri/src/lib.rs`

Recommendations:

- keep fine-grained commands; do not collapse into coarse RPCs
- standardize handler shape, result type, and lifecycle logging across all modules
- generate the IPC reference from the Rust registry or from per-command annotations to stop drift

Impact/Effort:

- standardization: High impact / Medium effort
- IPC doc generation: Medium impact / Medium effort

### 2. Storage Layer

Current state:

- The authority split is conceptually sound:
  - SQLite for structured app metadata and history
  - tantivy for derived search projection
  - filesystem as content truth
  - coordination JSON under `~/.claude/teams` as live runtime truth for mesh state

Strengths:

- `docs/architecture/data-architecture.md` is clear about authority
- `src-tauri/src/provider/path.rs` and `src-tauri/src/provider/platform_paths.rs` centralize path truth sensibly
- coordination explicitly avoids making SQLite the live source of truth
- search is treated as rebuildable derived state

Problems:

- SQLite access is serialized through one mutexed connection
- search writes are serialized through one mutexed writer
- the operational model depends on multiple store types, but there is no single “projection freshness” model across them
- task persistence already has generation logic in `src-tauri/src/services/task_sync.rs`, but search and some other projections do not have equivalent generation or checkpoint semantics

Recommendations:

- keep the current storage split
- improve concurrency around SQLite access
- define projection freshness explicitly for search and other derived stores
- add a small architecture note that distinguishes:
  - authoritative state
  - rebuildable projection
  - cached convenience materialization

Impact/Effort:

- DB concurrency model: High impact / Medium to High effort
- projection freshness model: Medium impact / Medium effort

### 3. Event Pipeline

Current state:

- The watcher stack is thoughtful.
- `src-tauri/src/fs/watcher.rs` does good pre-pruning and `.gitignore`-aware watch selection.
- `src-tauri/src/event_processor.rs` correctly batches noisy watcher streams.

Strengths:

- good debounce design
- internal sentinel events are treated differently from project events
- gitignore changes trigger a full project re-evaluation rather than pretending incremental logic is enough
- the pipeline emits useful structured telemetry

Problems:

- the event processor is both classifier and worker
- indexing, git refresh, and session import run inline in the same pipeline
- backpressure is controlled informally rather than by explicit queue budgets per workload
- some retries exist, but retry strategy is workload-specific and scattered

Recommendations:

- separate event intake from work execution
- give each heavy workload its own bounded executor and metrics
- preserve the current batch-flush semantics as the front door

Impact/Effort:

- queue split: High impact / Medium effort
- unified retry policy: Medium impact / Medium effort

### 4. Service Boundaries

Current state:

- Top-level module organization is good.
- The provider abstraction is one of the strongest boundaries in the codebase.
- Coordination is significantly better factored than the rest of the backend.

Strengths:

- `ProjectProvider` creates a clean local-vs-daemon seam
- path handling is centralized rather than duplicated
- coordination has real subdomains: backend, runtime, stores, pipelines, orchestrator
- tasks already show the right pattern: thin command handlers over `services::task_query` and `services::task_sync`

Problems:

- large command files still carry too much orchestration and mutation logic
- the quality of boundaries is uneven across domains
- startup logic is split across several modules, but the operational boundary is still “start everything from everywhere”

Recommendations:

- use tasks and coordination as the pattern for the rest of the backend
- pull project mutation flows out of `commands/projects.rs` into a `services/project_mutation` style module
- create explicit background-worker modules for watch processing and startup sub-jobs

Impact/Effort:

- command-to-service migration: Medium impact / Medium effort
- startup worker extraction: Medium impact / Medium effort

### 5. State Management

Current state:

- Frontend state has improved from a monolithic component approach by introducing controller modules and a state bridge.
- Backend state is mostly app-managed singletons with mutex protection.

Strengths:

- Svelte 5 runes are used consistently
- frontend controller extraction is already moving in the right direction
- context providers are minimal and avoid store sprawl
- backend state ownership is easy to locate

Problems:

- `src/Shell.svelte` remains a large composition root and still owns a lot of mutable shell state directly
- `createStateBridge()` is pragmatic but not very explicit; it trades ceremony for refactor fragility
- backend state uses a few large serialized singletons:
  - DB
  - search index
  - watcher
  - coordination orchestrator cache

Recommendations:

- keep the controller approach on the frontend
- continue shrinking `Shell.svelte` by moving feature-local state closer to tab or controller boundaries
- on the backend, replace “one mutex per global subsystem” with narrower ownership where the subsystem is now hot

Impact/Effort:

- frontend shell slimming: Medium impact / Medium effort
- backend state narrowing: High impact / Medium to High effort

### 6. Error Handling

Current state:

- The architecture is in transition from plain string errors to a better structured IPC model.

Strengths:

- `src-tauri/src/errors.rs` is a solid target design
- frontend normalization in `src/lib/ipc/client.js` is sensible and user-focused
- command lifecycle logging is capable of capturing structured error codes when present

Problems:

- many commands still surface only strings
- lock poisoning often becomes raw string output
- some observability exists only where newer code opted into it
- the current codebase still relies on heuristic mapping from message text to error code in `map_command_error()`

Recommendations:

- standardize on `IpcError`
- reduce text-based error classification over time by constructing typed errors at the source
- make retryability explicit for daemon, coordination, and watch-related temporary failures

Impact/Effort:

- error-model consolidation: High impact / Medium effort
- typed source errors: Medium impact / Medium effort

### 7. Startup Pipeline

Current state:

- Startup is thoughtfully staged and instrumented.
- The split between setup, orchestration, daemon bootstrap, watcher init, search init, and background bootstrap is sensible.

Strengths:

- clear startup telemetry
- sensible fail-fast behavior for critical initialization
- pragmatic daemon fallback behavior
- coordination remains lazily bootstrapped rather than becoming a hard startup dependency

Problems:

- many detached background threads reduce explicit lifecycle control
- background bootstrap in `src-tauri/src/startup/bootstrap.rs` runs several heavy jobs in sequence inside one background thread, so one slow task delays the others
- there is no single supervisor that can answer “which startup-maintenance workers are healthy right now?”

Recommendations:

- keep the staged startup design
- replace detached worker spawning with a small supervised runtime service
- decide which startup background tasks should be parallel, serialized, or cancellable

Impact/Effort:

- worker supervision: Medium to High impact / Medium effort
- bootstrap scheduling cleanup: Medium impact / Low to Medium effort

### 8. Coordination Subsystem

Current state:

- This is one of the best-architected parts of the codebase.
- The filesystem-first truth model, config/runtime split, and pipeline decomposition are coherent.

Strengths:

- `coordination/` has real subsystem boundaries
- the authority model is explicit and mostly honored in code
- runtime attachment vs logical membership is clearly separated
- recovery, reinjection, and runtime stores are treated as first-class concerns

Problems:

- `CoordinationState` has become a large mixed-responsibility object
- the default startup selector still forces M0 mesh behavior via `BackendSelector::m0()`
- documentation correctly marks some decisions as partial, but the codebase is now mature enough that those partials need a closure plan

Recommendations:

- keep the filesystem-first coordination model
- split `CoordinationState` into:
  - orchestrator cache/bootstrap
  - background self-heal runner
  - backend/runtime factory assembly
- define an explicit graduation plan for “partial” decisions in `docs/coordination-architecture.md`

Impact/Effort:

- state-object split: Medium impact / Medium effort
- partial-decision closure plan: Medium impact / Low effort

### 9. Data Flow

Current state:

- End-to-end flows are understandable, but they cross many layers.
- The architecture is strongest where it uses explicit authority boundaries and weakest where flows remain implicit across background threads.

Key flow assessments:

- Project load:
  - good fit for fine-grained IPC fan-out
  - frontend controller model supports deferred hydration well
  - risk comes from command inconsistency rather than the flow shape itself
- Session scan:
  - functionally plausible, but operationally spread across scanners, daemon bridges, watchers, and startup/bootstrap entrypoints
- Search:
  - clean projection model, but the write/update path is centralized and lock-heavy
- Coordination:
  - best-defined flow in the system because the stores and runtime meaning are documented and encoded

Recommendation:

- write one short “critical flow” architecture note per major path:
  - project load
  - watch event to UI refresh
  - session scan to persistence
  - search update
  - coordination initialize/resume

Impact/Effort:

- flow docs: Medium impact / Low effort
- flow-specific metrics: Medium impact / Medium effort

## What Is Working Especially Well

- Provider routing is clean and justified.
- Path normalization is centralized and treated as architecture, not utility trivia.
- The logging pipeline is unusually disciplined for an app at this stage.
- Coordination uses explicit stores and typed concepts instead of hidden shared state.
- Watcher pruning and debounce policy show practical production experience.
- Frontend state has already started the right extraction away from a single mega-component.

## Architectural Drift and Documentation Gaps

The documentation quality is high overall, but drift is now visible.

Examples:

- `ARCHITECTURE.md` explicitly notes a stale infographic
- `docs/architecture/ipc-reference.md` lists `coordination_get_compaction_audit`, but there is no matching command implementation or registration in the current code

Recommendation:

- treat architecture docs as generated-or-verified artifacts for inventories
- keep narrative docs hand-written
- auto-derive command inventories and maybe module maps where possible

## Recommended Execution Order

### Phase 1: Consistency and observability

- unify IPC error contracts
- extend lock-wait instrumentation to all DB/search-backed commands
- repair IPC documentation drift

### Phase 2: Contention reduction

- replace single-connection SQLite access
- split the event processor into intake plus workload queues
- make search update freshness measurable

### Phase 3: Structural cleanup

- extract project mutation logic from command handlers
- split `CoordinationState`
- continue shrinking `Shell.svelte`

## Final Assessment

Taurhaus does not need an architectural rewrite. It needs architectural consolidation.

The system already has the right major ideas:

- filesystem-first truth where it makes sense
- provider boundaries for platform differences
- derived projections instead of duplicated truth
- explicit coordination state rather than hidden global behavior
- strong structured observability

The next step is to make those ideas mechanically consistent across the entire codebase. The highest-value work is not adding new abstraction. It is finishing the abstraction work that the code has already started.
