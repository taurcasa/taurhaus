# Logging Architecture (Structured JSONL)

Date: 2026-03-06  
Owner: architect  
Scope: app logging architecture, implementation snapshot, and known gaps.

## Current Architecture

taurhaus now uses structured JSON Lines as the canonical application log format.

- Canonical file: `app_data_dir()/taurhaus.log.jsonl`
- Override: `<TAURHAUS_DATA_DIR>/taurhaus.log.jsonl`
- Record model: one JSON object per line with stable top-level keys:
  - `ts`, `level`, `component`, `event`, `run_id`
- Correlation keys (as applicable):
  - `interaction_id`, `request_id`, `daemon_request_id`

The sink is implemented as a single-writer async pipeline in
[`src-tauri/src/commands/logging.rs`](../../src-tauri/src/commands/logging.rs):

- frontend/backend producers enqueue structured records
- one writer thread owns the file handle
- append-only writes (no launch-time truncation)
- size-based rotation + retention pruning

## Implementation Status

### P0 slice status

| P0 item | Status | Notes |
|---|---|---|
| JSONL sink and schema | Implemented | `LogFileState` + global emitter (`emit_global`) write structured records to `taurhaus.log.jsonl`. |
| Startup lifecycle events | Implemented | `startup.phase.started/completed/failed` with `phase`, `duration_ms`, startup context fields. |
| IPC lifecycle events | Implemented | `ipc.command.received/completed/failed` and `ipc.lock.wait` via `IpcCommandSpan`. |
| Daemon RPC lifecycle events | Implemented | `daemon.rpc.sent/response/timeout` with `daemon_request_id`, `method`, `status`, `duration_ms`, `retry_count`. |
| Frontend bridge migration to structured payloads | Implemented | `logger.js` forwards structured payloads (`component`, `subsystem`, `event`, `message`, optional context/correlation). |
| Frontend drop telemetry | Implemented | `frontend.logs.dropped` emitted with drop counts/reasons under throttle. |

### Related observability work

| Item | Status | Notes |
|---|---|---|
| Log rotation and retention | Implemented | 20 MB rotation threshold, 7-day retention policy for rotated segments. |
| E2E failure artifact capture | Implemented | Failure artifacts include app-log tails and test metadata in per-failure bundles. |

### Known gaps at the final active-development snapshot

- Extend event coverage for watcher and reconcile pipelines (`watch.*`, background task lifecycles).
- Add dedicated daemon reconnect lifecycle vocabulary (`daemon.reconnect.*`) where missing.
- Provide a lightweight log query helper for agent workflows (filter by `run_id`, `request_id`, `event`, `level`).

## Correlation Model

- `run_id`: generated once per app run; attached to all events.
- `interaction_id`: frontend interaction chain marker.
- `request_id`: per IPC command lifecycle.
- `daemon_request_id`: per daemon RPC lifecycle.

This model enables cross-layer reconstruction from UI action -> IPC -> backend -> daemon transport events.

## Source Files

- Frontend bridge:
  - [`src/lib/logger.js`](../../src/lib/logger.js)
- Sink and global structured emitter:
  - [`src-tauri/src/commands/logging.rs`](../../src-tauri/src/commands/logging.rs)
- IPC lifecycle:
  - [`src-tauri/src/commands/lifecycle.rs`](../../src-tauri/src/commands/lifecycle.rs)
- Startup lifecycle:
  - [`src-tauri/src/startup/mod.rs`](../../src-tauri/src/startup/mod.rs)
- Daemon RPC lifecycle:
  - [`src-tauri/src/daemon_api.rs`](../../src-tauri/src/daemon_api.rs)
  - [`src-tauri/src/provider/daemon_client.rs`](../../src-tauri/src/provider/daemon_client.rs)

## Level Policy

See the canonical level/event policy in
[`docs/architecture/log-level-guidelines.md`](log-level-guidelines.md).
