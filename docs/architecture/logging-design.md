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

The daemon binary and the compact-hook CLI install the *same* sink (`install_global_sink`) and emit through `emit_global`, so daemon-side events — the session hub, the compaction runtime, `codex-notify` — land in the same file as app events. Tests drain the writer through the `LogFileState::flush_for_test()` barrier rather than sleeping before they read the file.

## Implementation Status

### P0 slice status

| P0 item | Status | Notes |
|---|---|---|
| JSONL sink and schema | Implemented | `LogFileState` + global emitter (`emit_global`) write structured records to `taurhaus.log.jsonl`. |
| Startup lifecycle events | Implemented | One event family per phase from `startup/telemetry.rs` (`startup.app.started`, `startup.paths.resolved`, `startup.logging.initialized`, `startup.database.*`, `startup.daemon_phase.*`, `startup.daemon_connect.*`, `startup.orchestration.*`, `startup.watchers.*`, `startup.search.*`, `startup.background_tasks.*`), all carrying the startup context fields. The daemon-owned replacement is `self_heal.pass.completed/failed`, with `effort.sweep.awaiting_settings` while launch settings are absent and `effort.sweep.skipped_busy` when a cycle's effort sweep never ran because another operation owned the orchestrator (each bounded to one record per daemon run). `duration_ms` is on the completion and failure events where a phase is measured, not on every startup event — `startup.app.started`, `startup.paths.resolved`, `startup.logging.initialized`, `startup.database.started`, `startup.daemon_phase.started` and `startup.orchestration.started` carry none. There is no generic `startup.phase.*` family — a test asserts those legacy names are never emitted (`startup/telemetry.rs:544-547`). |
| IPC lifecycle events | Implemented | `ipc.command.received/completed/failed` and `ipc.lock.wait` via `IpcCommandSpan`. |
| Daemon RPC lifecycle events | Implemented | `daemon.rpc.sent/response/timeout` with `daemon_request_id`, `method`, `status`, `duration_ms`, `retry_count`. |
| Frontend bridge migration to structured payloads | Implemented | `logger.js` forwards structured payloads (`component`, `subsystem`, `event`, `message`, optional context/correlation). |
| Frontend drop telemetry | Implemented | `frontend.logs.dropped` emitted with drop counts/reasons under throttle. |

### Watcher and daemon-connection coverage

| Family | Level | Notes |
|---|---|---|
| `watch.batch.flushed` | debug | Per-batch watcher flush metrics. |
| `watch.local.registered` / `.unregistered` / `.reconciled` | info | Local watcher registration lifecycle. |
| `watch.git_status.refreshed` / `.refresh_failed` | info / warn | Git-status refresh from watcher events. |
| `watch.event.dropped` | warn | Backpressure drop with counts. |
| `inotify.capacity.warning` / `.error` | warn / error | Watch-descriptor capacity headroom. |
| `daemon.connection.established` / `.reconnecting` | info | Provider connection lifecycle. |
| `daemon.connection.lost` | warn | Connection dropped; app falls back to `LocalProvider`. |
| `daemon.session_updates_bridge.recovered` | info | Long-poll bridge resumed after failure. |

### 2026-08 event families

| Family | Level | Notes |
|---|---|---|
| `activity.state.changed` | info | `{pid, tool, from, to, source}`. Not emitted on first sight of an already-idle process. |
| `session_scanner.process_scan.degraded` / `.recovered` | warn / info | Blackout edge plus a 60 s reminder while degraded. |
| `launch.command.rendered` | info | The rendered launch command. Carries notes: `launch.flag.deprecated`, `launch.model.ignored`, `launch.model.deprecated`, `launch.effort.ignored`, `launch.effort.invalid`, `launch.selector.ignored` (the base command already sets the tool's account selector), `launch.notify.ignored` (warn when attached to a launch). |
| `launch.model.invalid` | warn | Configured model is not in the catalog. |
| `launch.account.fallback` | warn | Requested account unusable for that tool; fell back down the resolution order. |
| `launch.account.derived_from_session` | info | Account inferred from an existing session's transcript. |
| `launch.account.ignored_for_team` | warn | A per-launch account pick was dropped because a team launch runs on the default account home. |
| `account.provider.floor` | info (`tracing` only) | A harness declares an account selector but its `AccountProvider` has not landed, so detection returns an empty scan. Emitted once per run per tool through `tracing::info!` (`accounts/mod.rs:463-481`), **not** through `emit_global` — it reaches stderr and the `tracing` layer, not the canonical JSONL sink. |
| `usage.fetched` | debug | `{tool, account_id, status, windows}` — never tokens, never a URL with a query string. |
| `usage.failed` | warn | Once per state change, per (tool, account). |
| `claude.usage.legacy_bridge.removed` | info | One-shot uninstall of the retired 0.6.8 Claude status-line bridge. |
| `agy.hooks.degraded` | warn | Antigravity's activity hooks could not be installed, or the CLI version gate (agy 1.1.10) left them off. Logged once per run for the version gate. |
| `codex.notify.appended` / `.executable_missing` | info (daemon) / warn | Codex turn-complete sink. |
| `daemon.data_root.mismatch` | warn | App and daemon resolved different app-data roots. |
| `compaction.injected` / `.skipped` | info | Native-hook delivery bookkeeping. |
| `compaction.failed` | warn | |
| `compaction.<tool>_hook.received` / `.resolved` / `.delivered` / `.skipped` | info | `<tool>` is `claude`, `codex`, `grok`, or `compact` when the tool cannot be inferred. |
| `compaction.hook.compat_import` | info | Once per run, on the first resolved invocation of a tool whose registry entry sets `compaction_hook_compat_import` (grok): that tool imports the `~/.claude/settings.json` hook registration as well as its own, so one compaction can reach the bridge twice and duplicates are dropped. The payload does **not** say which registration produced the current invocation — it is emitted before the duplicate check (`compact_hook.rs:468-473`) and announces the capability, not the provenance. |
| `compaction.<tool>_hook.failed`, `compaction.compact_hook.failed` | warn | Plus `compaction.compact_hook.parse_payload_debug`. |
| `compaction.codex_hook.degraded` / `.unsupported` / `.version_unknown` | warn | Native-hook installation and version gating. |
| `compaction.codex_hook.reconciled` | info | Hook install/removal applied. |
| `coordination.pane.foreign` | warn | A reused tmux pane no longer belongs to the member. |
| `coordination.team_daemon.skipped` | info | Lead control-auth credential missing. |
| `mesh.inbox.corrupt` | warn | Inbox quarantined to `<member>.json.corrupt.<ts>`. |
| `startup.paths.resolved` | info | Resolved roots for this run. |

### Related observability work

| Item | Status | Notes |
|---|---|---|
| Log rotation and retention | Implemented | 20 MB rotation threshold, 7-day retention policy for rotated segments. |
| E2E failure artifact capture | Implemented | Failure artifacts include app-log tails and test metadata in per-failure bundles. |

### Known gaps

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
- Startup path/telemetry events:
  - [`src-tauri/src/startup/telemetry.rs`](../../src-tauri/src/startup/telemetry.rs)
- Daemon RPC lifecycle:
  - [`src-tauri/src/daemon_api.rs`](../../src-tauri/src/daemon_api.rs)
  - [`src-tauri/src/provider/daemon_client.rs`](../../src-tauri/src/provider/daemon_client.rs)
- Compaction events:
  - [`src-tauri/src/coordination/compaction_events.rs`](../../src-tauri/src/coordination/compaction_events.rs)
  - [`src-tauri/src/coordination/compact_hook.rs`](../../src-tauri/src/coordination/compact_hook.rs)
- Session activity events:
  - [`src-tauri/src/session_scanner/classification.rs`](../../src-tauri/src/session_scanner/classification.rs)
  - [`src-tauri/src/session_scanner/process.rs`](../../src-tauri/src/session_scanner/process.rs)

## Level Policy

See the canonical level/event policy in
[`docs/architecture/log-level-guidelines.md`](log-level-guidelines.md).
