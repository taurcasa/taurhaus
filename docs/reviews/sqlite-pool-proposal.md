# SQLite Pooling Proposal

Date: 2026-03-29
Author: architect-1
Task: #11

## Objective

Replace the current single shared SQLite connection:

- `DbState(pub Mutex<Connection>)`

with a pooled access model that:

- preserves SQLite and WAL mode
- improves read concurrency
- keeps write behavior deterministic
- makes lock and pool wait time observable
- can be migrated incrementally without rewriting the app

## Current State

The app currently manages one `rusqlite::Connection` for the full process and shares it through a mutex.

That connection is used by:

- frontend IPC handlers
- startup bootstrap work
- watcher/event processing
- task persistence
- settings writes
- coordination request normalization and related helper paths

Strengths of the current design:

- simple mental model
- transaction ownership is easy to understand
- no cross-connection surprises

Problems:

- unrelated reads and writes serialize behind one mutex
- background work competes directly with UI-triggered IPC
- lock-wait instrumentation only exists in a few places today
- the app has outgrown the “one connection for everything” stage

## Recommendation

Adopt a dual-pool design:

- one small read pool
- one single-writer pool

Recommended shape:

```rust
pub struct DbState {
    pub reads: Pool<SqliteConnectionManager>,
    pub writes: Pool<SqliteConnectionManager>,
}
```

Use:

- `r2d2`
- `r2d2_sqlite::SqliteConnectionManager`

This is the right compromise for taurhaus:

- SQLite remains the correct embedded database
- WAL mode already aligns with many-readers/one-writer usage
- the codebase wants more concurrency, not a database replacement

## Why Dual Pools Instead of One Big Pool

SQLite in WAL mode allows concurrent readers, but still serializes writes.

A single larger mixed pool would improve acquisition contention somewhat, but it would not express the real architecture:

- reads are abundant and parallel-friendly
- writes are fewer and must remain disciplined

A separate single-writer pool makes that contract explicit.

Benefits:

- read-heavy surfaces stop queueing behind unrelated writes
- write-path review becomes easier because all mutations go through one lane
- instrumentation can distinguish read-pool wait from write-pool wait

## Pool Size Strategy

### Write pool

- size: `1`

Reason:

- SQLite still has one writer at a time
- a larger write pool mostly increases contention and hides sequencing
- many taurhaus writes are short and should stay that way

### Read pool

Initial recommendation:

- size: `max(2, min(available_parallelism, 6))`

Reason:

- enough headroom for UI IPC plus background work
- small enough to avoid pointless connection proliferation
- desktop SQLite usually does not benefit from very large pools

If a simpler fixed default is preferred:

- use `4`

That is a reasonable first value and easier to reason about operationally.

## Connection Configuration

Every pooled connection must be initialized identically.

Required per-connection settings:

- `PRAGMA journal_mode = WAL`
- `PRAGMA foreign_keys = ON`
- `busy_timeout`

Recommended initial `busy_timeout`:

- `250ms` to `500ms`

Why:

- enough time for brief writer overlap
- short enough that lock pressure still shows up in telemetry instead of becoming hidden latency

WAL considerations:

- keep WAL enabled; pooling reads is much less useful without it
- do not introduce aggressive manual checkpointing in the first migration
- if WAL growth becomes an issue, solve it as a separate operational tuning step

## Access Model

### Read access

Read-only queries should acquire from the read pool.

Examples:

- `list_projects`
- `get_project`
- `get_settings`
- `list_sessions`
- task detail reads
- startup project snapshots

### Write access

All mutations should acquire from the write pool.

Examples:

- project registration/removal
- settings updates
- task persistence
- relationship mutations
- activity touches
- cached git status updates

### Mixed read/write flows

For flows that need both reads and writes:

- perform authoritative mutation on the write connection
- avoid holding a write connection across filesystem, provider, or daemon I/O
- if a write path needs pre-read state for a transaction, do the pre-read on the write connection inside the same transaction

This matters because pooling introduces a real difference between:

- “read something first”
- “read the exact state that the write transaction must act on”

## API Shape Recommendation

Do not expose raw pool access everywhere.

Introduce helpers such as:

```rust
impl DbState {
    pub fn with_read<T, F>(&self, command: &'static str, f: F) -> IpcResult<T>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>;

    pub fn with_write<T, F>(&self, command: &'static str, f: F) -> IpcResult<T>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>;
}
```

These helpers should:

- measure pool acquisition wait
- emit lifecycle/lock telemetry
- normalize errors
- centralize connection configuration assumptions

This prevents the new pool design from turning into scattered `pool.get()` calls everywhere.

## Instrumentation Extension

Today only some commands emit `ipc.lock.wait` for DB access.

After pooling, instrumentation should move from mutex wait to pool acquisition wait.

Recommended fields:

- `lock_name = "db_read_pool"` or `lock_name = "db_write_pool"`
- `wait_ms`
- `command`
- `request_id`

Add separate metrics for:

- connection acquisition wait
- transaction duration
- write queue time if a later write-serialization layer is added

Suggested additional events:

- `db.pool.acquire`
- `db.pool.acquire_failed`
- `db.tx.completed`
- `db.tx.failed`

This will make it clear whether latency is caused by:

- pool starvation
- write lock pressure
- slow query logic

## Migration Path

Use staged migration.

### Phase 1: Add pooled state next to the old contract

- introduce `DbState` backed by pools instead of a mutexed connection
- add `with_read` and `with_write` helpers
- keep call sites close to their current signatures

At this stage, avoid large refactors to services or commands.

### Phase 2: Migrate read-only commands first

Start with simple read surfaces:

- settings reads
- projects list/detail
- session reads
- relationship reads
- search rebuild project snapshots

Why first:

- low transaction complexity
- immediate concurrency gain
- easier validation

### Phase 3: Migrate clear write-only commands

Examples:

- `update_settings`
- relationship mutations
- cached git status updates
- activity touches

### Phase 4: Migrate multi-step mutation flows

Examples:

- project registration/removal
- task refresh/persistence
- startup reseed/update flows

These need the most care because they currently rely on the simplicity of one shared connection.

### Phase 5: Clean up tests and remove mutex assumptions

- replace helper constructors that build `DbState(Mutex::new(conn))`
- add `DbState::for_test(temp_db_path)` or equivalent
- remove legacy mutex-lock failure tests that are no longer meaningful

## Risks

### Risk: accidental cross-connection transaction bugs

Example:

- a command reads with the read pool, then writes based on stale assumptions with the write pool

Mitigation:

- any read that informs a mutation must move into the write transaction boundary
- document this rule clearly in `ARCHITECTURE.md` or a DB access note

### Risk: more connections increase lock contention instead of reducing it

Mitigation:

- keep write pool size at `1`
- keep read pool modest
- use acquisition metrics before tuning sizes upward

### Risk: background work still starves UI even with a pool

Mitigation:

- pair this migration with the event-processor queue split
- do not let long-running background jobs hold write connections across external I/O

### Risk: connection initialization drift

Mitigation:

- apply all PRAGMAs in one shared connection-factory function
- test that every pooled connection has WAL and foreign keys enabled

## Test Strategy

Add tests for:

- pooled read concurrency does not break existing query semantics
- every connection has the required PRAGMAs
- write helpers preserve transaction behavior
- read-after-write paths that require transactional correctness stay on the write lane
- instrumentation emits `db_read_pool` and `db_write_pool` wait events

Also update current unit tests that expect lock poisoning from `Mutex<Connection>`. Those tests should be replaced by:

- pool acquisition failure tests where relevant
- query/transaction error tests

## Rollback Story

Rollback is straightforward if the migration is staged.

Approach:

- keep old helper names but change their internals behind `DbState`
- migrate commands in slices
- if a migrated area regresses, move that area back to the old helper implementation temporarily

If extra safety is needed:

- add a compile-time or runtime switch between pooled access and legacy single-connection access during the migration window

## Recommended Final Direction

Use a dual-pool SQLite access layer:

- read pool for query concurrency
- single-writer pool for all mutations
- helper-based acquisition and instrumentation
- WAL preserved

Do not jump straight to a more elaborate database abstraction or to a different database engine. The main problem is the current access model, not SQLite itself.

## Short Version

Keep SQLite. Replace `DbState(Mutex<Connection>)` with a small read pool plus a single-writer pool. Configure every connection for WAL, foreign keys, and busy timeout. Migrate read paths first, then writes, and move instrumentation to pool acquisition so lock pressure becomes visible instead of implicit.
