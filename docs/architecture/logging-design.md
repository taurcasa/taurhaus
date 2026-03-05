# AI-Optimized Logging Architecture Design

Date: 2026-03-05  
Owner: architect  
Scope: Read-only architecture/design proposal (no code changes)

## 1. Current State Assessment

### What exists today

- Frontend logs are bridged by monkey-patching `console.*` in [`src/lib/logger.js`](/home/mstie/projects/taurhaus/src/lib/logger.js), then forwarding plain strings over IPC (`frontend_log`).
- The IPC handler [`src-tauri/src/commands/logging.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/logging.rs) writes frontend lines to `taurhaus.log` as plain text:
  - format: `[HH:MM:SS.mmm] [INF|WRN|ERR|DBG] [frontend] message`
- Backend tracing is initialized in [`src-tauri/src/lib.rs`](/home/mstie/projects/taurhaus/src-tauri/src/lib.rs:268) via:
  - `tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();`
  - this writes to stderr (not a file layer).
- Startup creates/truncates `taurhaus.log` on each launch in [`src-tauri/src/startup/mod.rs`](/home/mstie/projects/taurhaus/src-tauri/src/startup/mod.rs:62).
- Some subsystems manually append ad-hoc lines to `taurhaus.log`:
  - bootstrap daemon launcher: [`src-tauri/src/daemon/launcher.rs`](/home/mstie/projects/taurhaus/src-tauri/src/daemon/launcher.rs:17)
  - command center launch/navigation paths: [`src-tauri/src/commands/command_center.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center.rs:210)
- Daemon process has its own stderr tracing config in [`src-tauri/src/bin/taurhaus-daemon.rs`](/home/mstie/projects/taurhaus/src-tauri/src/bin/taurhaus-daemon.rs:25), but no unified structured sink shared with app logs.

### Observed output quality (`taurhaus.log`)

Current sampled log (`/home/mstie/.local/share/com.taurhaus.dev/taurhaus.log`) had only 3 lines:

1. `[INF] [frontend] [logger] frontend log bridge initialized`
2. `(native)`
3. `[INF] [bootstrap] tmux session 'taurhaus' already exists`

This confirms very low diagnostic signal and non-uniform line quality.

### Gaps

1. Backend tracing is not persisted in the main log file.
2. Log format is mixed and unstructured (frontend tags, bootstrap text, command-center text, daemon stderr elsewhere).
3. Multiple writers/handles target the same file with different semantics, creating ordering/corruption risk.
4. `taurhaus.log` is truncated each launch, losing historical context.
5. No correlation IDs across frontend -> IPC -> backend -> daemon.
6. No standard lifecycle events (`started/completed/failed`, duration, lock wait, retries).
7. Frontend bridge drops many info/debug logs (prefix dropping + rate limiting) without emitted drop counters.
8. No standardized E2E failure artifact bundle from app/daemon logs.

## 2. Proposed Logging Architecture (AI-First)

### 2.1 Canonical format: JSON Lines

Adopt newline-delimited JSON (`.jsonl`) as the canonical machine log format.

Example event:

```json
{
  "ts":"2026-03-05T22:41:12.481Z",
  "level":"INFO",
  "component":"backend",
  "subsystem":"ipc",
  "event":"ipc.command.completed",
  "message":"IPC command completed",
  "run_id":"run_01JNR...",
  "trace_id":"tr_8b9...",
  "request_id":"req_4ab...",
  "command":"list_projects",
  "project_id":"proj_123",
  "status":"ok",
  "duration_ms":18
}
```

Required top-level keys for every event:

- `ts`, `level`, `component`, `event`, `run_id`
- `trace_id` or `request_id` when event is request-scoped
- `status` for lifecycle end events (`ok|error|timeout|cancelled`)
- `error.code` / `error.message` when failing

### 2.2 Single-writer pipeline

Use one in-process async writer for app logs:

1. Frontend emits structured IPC events (`frontend.log_event`), not free-form strings.
2. Backend tracing emits structured events (`tracing` fields) into the same sink.
3. Ad-hoc direct file writes (`writeln!`) are removed in favor of structured events.
4. File handle is append-only and exclusively owned by the writer task.

Result: no interleaved writers, no torn lines, deterministic ordering at sink level.

### 2.3 Correlation model

- `run_id`: generated once at app startup; attached to every event.
- `interaction_id`: generated in frontend per user interaction (click/action chain).
- `request_id`: per IPC command invocation.
- `daemon_request_id`: unique ID per daemon RPC (must not be static strings).
- `session_id`/`project_id`/`pane_id`/`spec_id` where applicable.

This enables causal reconstruction across frontend, backend, daemon, and E2E.

### 2.4 Performance model

- Default production level: `INFO` with focused event vocabulary.
- `DEBUG` opt-in via env/config at runtime.
- Bounded async channel + non-blocking writer.
- Sampling/throttling only for known-noisy events, with explicit `dropped_count` events.
- Avoid heavy string interpolation unless level enabled.

### 2.5 Human-readable output

Keep human pretty stderr output for local dev, but treat JSONL as source-of-truth.

- stderr: concise pretty logs
- file: structured JSONL

## 3. Priority-Ordered Data Flows To Instrument First

### P0 (do first)

1. App startup -> daemon connect -> session bridge -> frontend hydration
   - events: `startup.phase.started/completed/failed`
   - fields: `phase`, `duration_ms`, `daemon_addr`, `connected_at_startup`, `error.*`
2. IPC command lifecycle
   - events: `ipc.command.received`, `ipc.lock.wait`, `ipc.command.completed/failed`
   - fields: `command`, `request_id`, `lock_name`, `wait_ms`, `duration_ms`
3. Daemon RPC lifecycle
   - events: `daemon.rpc.sent`, `daemon.rpc.response`, `daemon.rpc.timeout`, `daemon.reconnect.*`
   - fields: `daemon_request_id`, `method`, `status`, `duration_ms`, `retry_count`

### P1 (next)

1. File watcher pipeline
   - events: `watch.event.received`, `watch.event.filtered`, `watch.batch.emitted`
   - fields: `project_id`, `path_hash`, `event_kind`, `batch_size`, `latency_ms`
2. Reconcile/background tasks
   - events: `task.started`, `task.progress`, `task.completed/failed`
   - fields: `task_name`, `trigger`, `duration_ms`, `error.*`

### P2 (after stabilization)

1. Frontend hydration/render milestones
   - events: `ui.hydration.started/completed`, `ui.route.ready`
2. E2E observability markers
   - events: `e2e.app.ready`, `e2e.webdriver.session.created`, `e2e.webdriver.session.lost`

## 4. Infrastructure Recommendations

1. File strategy
   - Replace `taurhaus.log` plain text with `taurhaus.log.jsonl`.
   - Stop truncating on startup; rotate instead.
2. Rotation/retention
   - Size-based + daily rolling (example: 20 MB segments, keep 7 days, gzip old files).
3. Unified schema governance
   - Define a versioned event schema (`schema_version`) and event naming conventions.
   - Add a short reference doc for allowed events/fields.
4. Daemon log integration
   - Option A: daemon writes its own JSONL file with same schema + `component=daemon`.
   - Option B: app ingests daemon events and re-emits into unified sink.
   - Prefer A first for lower coupling, then optional unification.
5. E2E failure artifact bundle (mandatory)
   - On WDIO test failure, automatically collect:
     - last N lines from app JSONL log
     - daemon JSONL tail
     - webdriver/tauri-driver logs
     - screenshot and spec metadata (`spec`, `run_id`, `request_id` if available)
   - Save as per-spec artifact directory under WDIO output.
6. Tooling for AI agents
   - Provide a small log-query helper (`just logs-query` or script) that filters by `run_id`, `request_id`, `event`, and `level`.
   - Provide canned queries for startup failures, IPC timeouts, and daemon reconnect loops.

## Recommended First Implementation Slice

1. Introduce JSONL sink and schema in backend (no behavior changes yet).
2. Instrument startup + IPC lifecycle with request/duration fields.
3. Migrate frontend bridge to structured payload and emit dropped-log counters.
4. Add daemon request correlation IDs.
5. Add WDIO failure artifact collector that snapshots log tails.

This slice directly addresses the deadlock-debugging failure mode: when the app appears hung, logs will show exactly which phase/command is waiting, for how long, and with which correlation IDs.
