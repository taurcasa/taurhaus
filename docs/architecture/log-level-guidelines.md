# Log Level Guidelines (Structured, AI-Optimized)

Date: 2026-03-06  
Owner: architect  
Scope: Backend, frontend, daemon, and E2E instrumentation policy.

Reference docs:
- [`logging-design.md`](logging-design.md)
- [`logging-integration-point-inventory.md`](../archive/architecture/logging-integration-point-inventory.md) (historical inventory)

## 1. Core Rules

1. Always emit structured events (JSONL), not prose-only messages.
2. Event names must follow `subsystem.entity.verb`.
3. Every lifecycle operation should produce: `started`, `completed`, and `failed` events.
4. Log for machine diagnosis first, human readability second.

## 2. Event Naming Convention

Pattern:

- `subsystem.entity.verb`

Examples:

- `startup.app.started`
- `ipc.command.completed`
- `daemon.rpc.failed`
- `watch.batch.flushed`
- `ui.projects_hydrate.degraded`
- `e2e.webdriver_session.ready`

Naming constraints:

- Use lowercase snake_case segments.
- Prefer the lifecycle verbs: `started`, `completed`, `failed`, `received`, `sent`, `timeout`, `skipped`, `reconciled`, `dropped`.
- State verbs are also in use where the event reports a condition rather than a step: `changed`, `degraded`, `recovered`, `resolved`, `delivered`, `selected`, `replayed`, `established`, `lost`, `reconnecting`, `rendered`, `ignored`, `invalid`, `deprecated`, `mismatch`, `foreign`, `corrupt`, `unresolved`, `opaque`, `appended`, `executable_missing`, `heartbeat`. Reach for a lifecycle verb first; add to this list rather than inventing a synonym for one already here.
- Do not encode dynamic IDs into the event name; put them in fields.

## 3. Level Selection Policy

### INFO

Use for state transitions and successful milestones that matter for timeline reconstruction.

Use INFO for:

- startup phase boundaries
- command completion summaries
- daemon connect/reconnect success
- watch reconcile completion
- E2E session start/ready/finish

Good examples from current code:

- `tracing::info!("taurhaus starting")` in [`startup/mod.rs`](../../src-tauri/src/startup/mod.rs#L25)
- `tracing::info!("Background bootstrap: daemon connected")` in [`startup/daemon.rs`](../../src-tauri/src/startup/daemon.rs#L146)
- `tracing::info!(watched, unwatched, reason, ...)` in [`startup/watchers.rs`](../../src-tauri/src/startup/watchers.rs#L310-L315)

Do not use INFO for:

- high-frequency per-file/per-path spam
- low-level polling loops on every iteration
- stack traces or internal debug internals

### WARN

Use for recoverable failures, degraded behavior, retries, fallback paths, or data loss risks.

Use WARN for:

- daemon reconnect failures where app can continue
- dropped events
- lock poison recovery
- partial hydrate failures
- fallback to less precise behavior

Good examples from current code:

- `tracing::warn!(..., "Daemon health check failed")` in [`daemon_lifecycle.rs`](../../src-tauri/src/daemon_lifecycle.rs#L697)
- `tracing::warn!(..., "dropping daemon event ...")` in [`daemon/event_listener.rs`](../../src-tauri/src/daemon/event_listener.rs#L361)
- `tracing::warn!(..., "git status refresh failed ... scheduling one retry")` in [`event_processor.rs`](../../src-tauri/src/event_processor.rs#L604)

Do not use WARN for:

- expected control flow (for example, "not found" in optional queries)
- transient debug-only details
- every retry attempt in tight loops without throttling

### ERROR

Use for non-recoverable failures of a critical path, or when user-visible functionality is broken.

Use ERROR for:

- startup failure to initialize core subsystems
- protocol incompatibility that blocks expected behavior
- unrecoverable command failures surfaced to UI
- daemon server fatal errors

Good examples from current code:

- `tracing::error!("Failed to lock DB for activity reseed: ...")` in [`bootstrap.rs`](../../src-tauri/src/bootstrap.rs#L26)
- `tracing::error!(..., "DAEMON IS OUTDATED ...")` in [`startup/daemon.rs`](../../src-tauri/src/startup/daemon.rs#L229)
- `tracing::error!(..., "Daemon server error")` in [`bin/taurhaus-daemon.rs`](../../src-tauri/src/bin/taurhaus-daemon.rs#L106)

Do not use ERROR for:

- issues that auto-recover without impact
- one-off retries that still have fallback
- user cancellations/timeouts that are expected in UX flow

### DEBUG

Use for deep diagnostic context needed during investigations, with high cardinality allowed.

Use DEBUG for:

- per-batch/per-scan metrics
- cache hit/miss details
- retries, polling internals, and classification internals
- request/response payload sizes and timings

Good examples from current code:

- `tracing::debug!(..., "session_scanner metrics")` in [`session_scanner/scans.rs`](../../src-tauri/src/session_scanner/scans.rs#L149)
- `tracing::debug!(..., "flushing watch event batch")` in [`event_processor.rs`](../../src-tauri/src/event_processor.rs#L551)
- `tracing::debug!(..., "Received request")` in [`daemon/handlers.rs`](../../src-tauri/src/daemon/handlers.rs#L33)

Do not use DEBUG for:

- large payload dumps by default
- plaintext secrets/tokens
- expensive string interpolation without level gating

## 4. Canonical Field Naming

### Required fields (all events)

- `ts` (RFC3339 UTC timestamp)
- `level`
- `event`
- `component` (`frontend`, `backend`, `daemon`, `coordination`, `e2e_runner`) — the coordination/compaction/launch event families emit `coordination`
- `run_id`

### Correlation fields

- `trace_id` (optional)
- `interaction_id` (frontend user action chain)
- `request_id` (frontend->backend IPC)
- `daemon_request_id` (backend->daemon RPC)
- `session_id`
- `project_id`

### Lifecycle/status fields

- `status` (`ok`, `error`, `timeout`, `cancelled`, `degraded`, `skipped`)
- `duration_ms`
- `retry_count`
- `attempt`
- `max_attempts`

### Error fields

- `error.code`
- `error.message`
- `error.kind`
- `error.stage`

### Capacity/backpressure fields

- `dropped_count`
- `queue_depth`
- `batch_size`
- `result_count`
- `docs_updated`

### Naming rules

- Use `snake_case` for all keys.
- Booleans start with `is_`, `has_`, or read as predicates (`connected_at_startup`).
- Durations always use `_ms` suffix.
- Counts always use `_count` suffix unless domain-standard (`batch_size`).

## 5. Anti-Patterns To Avoid

1. Message-only logging:
   - Bad: `"Failed to load"`
   - Good: event + structured error fields.
2. Mixed naming styles:
   - Bad: `projectId`, `project-id`, `ProjectID`.
   - Good: `project_id`.
3. Static request IDs:
   - Bad: constant ids like `"status-ping"` for multiple RPC calls.
   - Good: unique `daemon_request_id` per request.
4. Missing completion events:
   - Bad: log `started` only.
   - Good: always emit `completed`/`failed` with `duration_ms`.
5. Silent throttling:
   - Bad: dropping logs/events without telemetry.
   - Good: emit `*.dropped` events with `dropped_count` and reason.

## 6. Performance Rules

1. No heavy interpolation unless level is enabled.
2. Keep INFO volume bounded; use DEBUG for high-frequency internals.
3. Sample repetitive DEBUG events and include sampling metadata.
4. Use bounded queues/channels for async logging; emit overflow counters.
5. Avoid blocking filesystem writes on hot paths.
6. Redact secrets/tokens before serialization.

## 7. Instrumentation Checklist (Developer Quick Reference)

When instrumenting a new flow:

1. Pick an event name using `subsystem.entity.verb`.
2. Emit `started` with correlation fields.
3. Measure elapsed time and emit `completed` (`status=ok`, `duration_ms`).
4. On failure, emit `failed` (`status=error`, `error.*`, `duration_ms`).
5. Choose level by impact:
   - success milestone -> INFO
   - recoverable degradation -> WARN
   - unrecoverable critical failure -> ERROR
   - deep diagnostics -> DEBUG
6. Ensure field names match canonical list and `snake_case`.

This checklist is the minimum bar before opening a PR that adds instrumentation.
