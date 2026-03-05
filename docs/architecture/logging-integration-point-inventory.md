# Logging Integration Point Inventory

Date: 2026-03-06  
Owner: architect  
Scope: Codebase-wide inventory of critical flows for structured, AI-optimized observability.

Reference architecture: [`logging-design.md`](/home/mstie/projects/taurhaus/docs/architecture/logging-design.md)

## Conventions Used In This Inventory

- Event naming pattern: `subsystem.entity.verb`
- Field names: `snake_case`
- Priority tiers:
  - `P0`: must instrument first (startup, IPC lifecycle, daemon RPC lifecycle)
  - `P1`: instrument next (watcher/event pipeline, reconcile/background loops)
  - `P2`: instrument after stabilization (frontend hydration milestones, E2E markers)

## P0: Startup Phases

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| App process bootstrap entered | `startup.app.started` | INFO | backend | `run_id`, `app_version`, `platform`, `pid` | `src-tauri/src/startup/mod.rs:41` |
| App data dir resolved | `startup.paths.resolved` | INFO | backend | `run_id`, `data_dir`, `db_path`, `log_path`, `used_data_dir_override`, `used_claude_dir_override` | `src-tauri/src/startup/mod.rs:56` |
| Log sink initialized | `startup.logging.initialized` | INFO | backend | `run_id`, `log_path`, `format`, `rotation_enabled` | `src-tauri/src/startup/mod.rs:62` |
| Database initialization started/completed | `startup.database.started` / `startup.database.completed` | INFO | backend | `run_id`, `db_path`, `duration_ms`, `migration_count` | `src-tauri/src/startup/mod.rs:78` |
| Database initialization failure | `startup.database.failed` | ERROR | backend | `run_id`, `db_path`, `error.code`, `error.message` | `src-tauri/src/startup/mod.rs:78` |
| Daemon phase determination started/completed | `startup.daemon_phase.started` / `startup.daemon_phase.completed` | INFO | backend | `run_id`, `wsl_distro`, `daemon_addr`, `daemon_connected_at_startup`, `duration_ms` | `src-tauri/src/startup/mod.rs:84` |
| Fast-path daemon connect success | `startup.daemon_connect.succeeded` | INFO | backend | `run_id`, `daemon_addr`, `mode`, `duration_ms` | `src-tauri/src/startup/mod.rs:114` |
| Daemon unavailable at startup | `startup.daemon_connect.deferred` | WARN | backend | `run_id`, `daemon_addr`, `wsl_distro`, `reason` | `src-tauri/src/startup/mod.rs:119` |
| Orchestration fan-out started/completed | `startup.orchestration.started` / `startup.orchestration.completed` | INFO | backend | `run_id`, `steps`, `duration_ms` | `src-tauri/src/startup/mod.rs:169` |
| Watcher subsystem initialized | `startup.watchers.initialized` | INFO | backend | `run_id`, `local_watcher_enabled`, `daemon_watch_bootstrap`, `duration_ms` | `src-tauri/src/startup/watchers.rs:28` |
| Search subsystem initialized | `startup.search.initialized` | INFO | backend | `run_id`, `index_path`, `doc_count`, `duration_ms` | `src-tauri/src/startup/search.rs` |
| Background bootstrap thread spawned | `startup.bootstrap_thread.spawned` | INFO | backend | `run_id`, `thread_name`, `connected_at_startup` | `src-tauri/src/startup/daemon.rs:7` |
| Daemon background bootstrap attempt | `startup.daemon_bootstrap.started` | INFO | backend | `run_id`, `daemon_addr`, `wsl_distro` | `src-tauri/src/startup/daemon.rs:12` |
| Daemon background bootstrap result | `startup.daemon_bootstrap.completed` / `startup.daemon_bootstrap.failed` | INFO / WARN | backend | `run_id`, `status`, `duration_ms`, `error.code`, `error.message` | `src-tauri/src/startup/daemon.rs:18` |
| Startup protocol compatibility check | `startup.daemon_protocol.checked` | INFO / WARN / ERROR | backend | `run_id`, `daemon_protocol_version`, `expected_protocol_version`, `status` | `src-tauri/src/startup/daemon.rs:40` |
| Startup background tasks started | `startup.background_tasks.started` | INFO | backend | `run_id`, `task_group` (`activity_reseed`,`session_scan`,`search_index`,`task_scan`) | `src-tauri/src/startup/bootstrap.rs:3` |

## P0: IPC Command Lifecycle

### Cross-cutting instrumentation (apply to every `#[tauri::command]`)

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| IPC request accepted | `ipc.command.received` | INFO | backend | `run_id`, `request_id`, `command`, `interaction_id`, `project_id` | all commands in `src-tauri/src/commands/*` |
| Command context resolution | `ipc.command.context_resolved` | DEBUG | backend | `request_id`, `command`, `provider_mode`, `is_wsl_path`, `daemon_connected` | e.g. `commands/files.rs:130`, `commands/command_center.rs:229` |
| Lock wait observed | `ipc.command.lock_wait` | DEBUG/WARN | backend | `request_id`, `command`, `lock_name`, `wait_ms`, `threshold_ms` | DB/Search lock sites across commands |
| Command completed successfully | `ipc.command.completed` | INFO | backend | `request_id`, `command`, `status`, `duration_ms`, `result_size` | all command returns |
| Command failed | `ipc.command.failed` | WARN/ERROR | backend | `request_id`, `command`, `duration_ms`, `error.code`, `error.message`, `error.kind` | all `Err(...)` command exits |
| Frontend log bridge invoke received | `ipc.log.received` | DEBUG | backend | `request_id`, `level`, `message_len`, `dropped_prefix_match` | `commands/logging.rs:13` |
| Frontend log bridge invoke failed | `ipc.log.failed` | WARN | backend | `request_id`, `error.code`, `error.message` | `commands/logging.rs:45` |

### High-value command families (first wave after cross-cutting)

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| Project list fetch | `projects.list.completed` | INFO | backend | `request_id`, `project_count`, `duration_ms` | `commands/projects.rs:list_projects` |
| Project selection detail hydrate | `projects.hydrate.completed` / `projects.hydrate.degraded` | INFO / WARN | backend + frontend | `request_id`, `project_id`, `sections_loaded`, `sections_failed`, `duration_ms` | `Shell.svelte:531` + command handlers |
| Command-center launch session path | `command_center.launch.completed` / `command_center.launch.failed` | INFO / WARN | backend | `request_id`, `project_id`, `cli_tool`, `mode`, `transport` (`daemon`/`local_tmux`), `duration_ms` | `commands/command_center.rs:200` |
| Daemon control command status ping | `daemon.status.checked` | INFO/WARN | backend | `request_id`, `daemon_status`, `protocol_version`, `expected_protocol_version`, `duration_ms` | `commands/daemon.rs:28` |
| Search request path | `search.query.completed` / `search.query.failed` | INFO / WARN | backend | `request_id`, `query_hash`, `limit`, `result_count`, `duration_ms` | `commands/search.rs` |
| Task query path | `tasks.query.completed` / `tasks.query.failed` | INFO / WARN | backend | `request_id`, `project_id`, `task_count`, `duration_ms` | `commands/tasks.rs` |

## P0: Daemon RPC Lifecycle

### App-side RPC client (`DaemonProvider`, listeners)

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| RPC send | `daemon.rpc.sent` | DEBUG | backend | `request_id`, `daemon_request_id`, `method`, `timeout_ms`, `daemon_addr` | `provider/daemon_client.rs:247` |
| RPC response received | `daemon.rpc.response` | INFO | backend | `request_id`, `daemon_request_id`, `method`, `status`, `duration_ms`, `response_size` | `provider/daemon_client.rs:305` |
| RPC timeout | `daemon.rpc.timeout` | WARN | backend | `request_id`, `daemon_request_id`, `method`, `timeout_ms`, `daemon_addr` | `provider/daemon_client.rs:306` |
| RPC protocol failure | `daemon.rpc.failed` | WARN/ERROR | backend | `request_id`, `daemon_request_id`, `method`, `error.code`, `error.message` | `provider/daemon_client.rs:310` |
| Provider marked disconnected | `daemon.connection.marked_disconnected` | WARN | backend | `daemon_addr`, `reason`, `request_id`, `daemon_request_id` | `provider/daemon_client.rs:255` |
| Reconnect attempt | `daemon.connection.reconnect_attempted` | INFO | backend | `daemon_addr`, `cooldown_ms`, `attempt` | `provider/daemon_client.rs:192` |
| Reconnect success/failure | `daemon.connection.reconnect_succeeded` / `daemon.connection.reconnect_failed` | INFO / WARN | backend | `daemon_addr`, `duration_ms`, `error.code`, `error.message` | `provider/daemon_client.rs:215` |
| Event-listener watch handshake started | `daemon.watch_handshake.started` | DEBUG | backend | `project_id`, `linux_path`, `timeout_ms`, `daemon_request_id` | `daemon/event_listener.rs:75` |
| Event-listener watch handshake completed/failed | `daemon.watch_handshake.completed` / `daemon.watch_handshake.failed` | INFO / WARN | backend | `project_id`, `linux_path`, `status`, `duration_ms`, `error.*` | `daemon/event_listener.rs:144` |
| Daemon push event dropped before app pipeline | `daemon.event.dropped` | WARN | backend | `event`, `stage`, `path`, `dropped_count`, `error.message` | `daemon/event_listener.rs:354`, `daemon/server.rs:364` |

### Daemon-side request lifecycle (`taurhaus-daemon`)

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| Daemon process started | `daemon.server.started` | INFO | daemon | `run_id`, `daemon_addr`, `port`, `idle_timeout_secs`, `auth_enabled` | `bin/taurhaus-daemon.rs:57`, `daemon/server.rs:86` |
| Client accepted | `daemon.connection.accepted` | INFO | daemon | `client_addr`, `connection_id` | `daemon/server.rs:90` |
| Request parsed/authenticated | `daemon.request.accepted` | DEBUG | daemon | `connection_id`, `daemon_request_id`, `method`, `auth_valid` | `daemon/server.rs:276`, `daemon/handlers.rs:53` |
| Request parse/auth failure | `daemon.request.rejected` | WARN | daemon | `connection_id`, `stage`, `error.code`, `error.message` | `daemon/server.rs:276`, `daemon/server.rs:285` |
| Request dispatched/completed | `daemon.request.completed` | INFO | daemon | `connection_id`, `daemon_request_id`, `method`, `status`, `duration_ms` | `daemon/server.rs:295` |
| Connection handler error | `daemon.connection.failed` | WARN | daemon | `connection_id`, `client_addr`, `error.code`, `error.message` | `daemon/server.rs:111` |
| Idle timeout shutdown | `daemon.server.idle_timeout` | INFO | daemon | `timeout_secs`, `uptime_secs`, `active_connections` | `daemon/server.rs:117` |

## P1: File Watcher -> Event Processor -> Frontend Update

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| Local watch registration | `watch.local.registered` | INFO | backend | `project_id`, `project_path`, `watch_mode` | `fs/watcher.rs:221` |
| Local watch unregistration | `watch.local.unregistered` | INFO | backend | `project_id`, `project_path` | `fs/watcher.rs:255` |
| Notify event classified | `watch.event.classified` | DEBUG | backend | `project_id`, `event_kind`, `paths_in`, `paths_out`, `gitignore_changed` | `fs/watcher.rs:127` |
| Notify event filtered by ignore rules | `watch.event.filtered` | DEBUG | backend | `project_id`, `reason`, `path_hash`, `rule_source` | `fs/watcher.rs:176` |
| Daemon watch registered | `watch.daemon.registered` | INFO | backend | `project_id`, `linux_path`, `source` (`daemon`) | `daemon/event_listener.rs:162` |
| Daemon watch reconciliation result | `watch.daemon.reconciled` | INFO | backend | `reason`, `watched`, `unwatched`, `distro`, `duration_ms` | `daemon_lifecycle.rs:261`, `startup/watchers.rs:244` |
| Event batch flush | `watch.batch.flushed` | DEBUG | backend | `batch_size`, `file_projects`, `git_projects`, `session_files`, `elapsed_ms` | `event_processor.rs:389` |
| Activity touch update | `watch.activity.updated` / `watch.activity.update_failed` | DEBUG / WARN | backend | `project_id`, `touched_count`, `error.*` | `event_processor.rs:401` |
| Git status refresh from watch | `watch.git_status.refreshed` / `watch.git_status.refresh_failed` | INFO / WARN | backend | `project_id`, `retry_scheduled`, `duration_ms`, `error.*` | `event_processor.rs:431` |
| Session import from watch | `watch.session_import.completed` / `watch.session_import.failed` | INFO / WARN | backend | `project_id`, `session_id`, `path`, `duration_ms`, `error.*` | `event_processor.rs:467` |
| File index incremental update | `search.file_index.updated` / `search.file_index.failed` | INFO / WARN | backend | `project_id`, `docs_updated`, `changed_path_count`, `duration_ms`, `error.*` | `event_processor.rs:535` |
| Gitignore-triggered rebuild | `search.gitignore_reindex.completed` / `search.gitignore_reindex.failed` | INFO / WARN | backend | `project_id`, `docs_updated`, `cooldown_applied`, `duration_ms`, `error.*` | `event_processor.rs:600` |
| Frontend change events emitted | `watch.emit.project_files_changed` / `watch.emit.search_index_updated` | DEBUG | backend | `project_id`, `reason`, `docs_updated`, `path_count` | `event_processor.rs:527`, `event_processor.rs:589` |

## P1: Reconcile and Background Task Lifecycles

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| Activity watch reconcile started | `watch_reconcile.run.started` | INFO | backend | `reason`, `has_daemon`, `project_count` | `startup/watchers.rs:102` |
| Reconcile skipped (already in progress) | `watch_reconcile.run.skipped` | DEBUG | backend | `reason`, `skip_reason` | `startup/watchers.rs:103` |
| Activity watch reconcile completed | `watch_reconcile.run.completed` | INFO | backend | `reason`, `watched`, `unwatched`, `watch_limit_hit`, `duration_ms` | `startup/watchers.rs:236` |
| Daemon health-check cycle | `daemon_health.check.completed` / `daemon_health.check.failed` | DEBUG / WARN | backend | `daemon_addr`, `failures`, `restart_attempts`, `duration_ms`, `error.*` | `daemon_lifecycle.rs:485` |
| Daemon reconnect workflow | `daemon_health.reconnect.started` / `daemon_health.reconnect.completed` / `daemon_health.reconnect.failed` | INFO / WARN | backend | `daemon_addr`, `attempt`, `max_attempts`, `status`, `duration_ms` | `daemon_lifecycle.rs:551` |
| Session updates bridge lifecycle | `session_bridge.thread.started` / `session_bridge.connected` / `session_bridge.poll_failed` | INFO / DEBUG | backend | `daemon_addr`, `since_version`, `wait_timeout_ms`, `error.*` | `daemon_lifecycle.rs:631` |
| Session updates emitted to frontend | `session_bridge.emit.sessions_updated` | DEBUG | backend | `version`, `session_count`, `duration_ms` | `daemon_lifecycle.rs:710` |
| Startup activity reseed cycle | `startup_reseed.activity.completed` / `startup_reseed.activity.failed` | INFO / WARN | backend | `updated`, `project_count`, `duration_ms`, `error.*` | `bootstrap.rs:20` |
| Startup session import scan cycle | `startup_reseed.sessions.completed` / `startup_reseed.sessions.failed` | INFO / WARN | backend | `project_count`, `imported_count`, `duration_ms`, `error.*` | `bootstrap.rs:171` |
| Startup search index build cycle | `startup_reseed.search_index.completed` / `startup_reseed.search_index.failed` | INFO / WARN | backend | `doc_count`, `duration_ms`, `error.*` | `bootstrap.rs:115` |
| Task scan cycle | `task_scan.cycle.started` / `task_scan.cycle.completed` / `task_scan.cycle.fallback` | INFO / DEBUG / WARN | backend | `cycle_id`, `trigger`, `project_count`, `task_count`, `duration_ms`, `fallback_reason` | `bootstrap.rs:277` |
| Session scanner pass metrics | `session_scanner.scan.completed` | DEBUG | backend | `session_count`, `process_scan_ms`, `tmux_ms`, `idle_ms`, `cache_hits`, `duration_ms` | `session_scanner/mod.rs:485` |

## P2: Frontend Hydration and UI Milestones

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| Splash flow started/completed | `ui.splash.started` / `ui.splash.completed` | INFO | frontend | `run_id`, `daemon_status`, `duration_ms`, `continue_anyway_used` | `src/App.svelte:41`, `src/lib/SplashScreen.svelte` |
| Shell ready state entered | `ui.shell.ready` | INFO | frontend | `run_id`, `wizard_checked`, `show_wizard` | `src/App.svelte:99`, `src/Shell.svelte:226` |
| Initial project list hydrate | `ui.projects_hydrate.started` / `ui.projects_hydrate.completed` / `ui.projects_hydrate.failed` | INFO / WARN | frontend | `interaction_id`, `project_count`, `duration_ms`, `error.*` | `src/Shell.svelte:484` |
| First project deferred detail hydrate | `ui.project_hydrate.deferred_started` / `ui.project_hydrate.completed` / `ui.project_hydrate.degraded` | INFO / WARN | frontend | `interaction_id`, `project_id`, `sections_loaded`, `sections_failed`, `duration_ms` | `src/Shell.svelte:521`, `src/Shell.svelte:531` |
| Daemon status banner transitions | `ui.daemon_status.changed` | INFO | frontend | `status`, `previous_status`, `source` (`splash`,`event`,`poll`) | `src/Shell.svelte:290`, `src/Shell.svelte:454` |
| Frontend event listener registration | `ui.events.listener_registered` / `ui.events.listener_failed` | DEBUG / WARN | frontend | `event_name`, `duration_ms`, `error.*` | `src/Shell.svelte:388` |
| Session store hydration on startup | `ui.sessions_hydrate.started` / `ui.sessions_hydrate.completed` / `ui.sessions_hydrate.failed` | INFO / WARN | frontend | `session_count`, `duration_ms`, `error.*` | `src/Shell.svelte:472`, `src/lib/sessionStore.svelte.js` |
| Frontend log bridge backpressure | `frontend_log.bridge_dropped` | DEBUG | frontend | `drop_reason`, `dropped_count`, `window_ms`, `prefix` | `src/lib/logger.js:37` |

## P2: E2E Observability Markers

| Integration point | Event name | Level | Emitter | Key fields/context | Source hook(s) |
|---|---|---|---|---|---|
| WDIO run preparation | `e2e.run.prepared` | INFO | e2e_runner | `run_id`, `wdio_port`, `native_driver_port`, `spec_group_count`, `skip_build` | `e2e/wdio.conf.js:424` |
| WebDriver session startup | `e2e.webdriver_session.started` / `e2e.webdriver_session.ready` / `e2e.webdriver_session.failed` | INFO / ERROR | e2e_runner | `session_id`, `wdio_port`, `native_driver_path`, `duration_ms`, `error.*` | `e2e/wdio.conf.js:453`, `e2e/wdio.conf.js:228` |
| App readiness gate | `e2e.app.ready_check.started` / `e2e.app.ready_check.completed` / `e2e.app.ready_check.failed` | INFO / WARN | e2e_runner | `session_id`, `readiness_path` (`splash`,`wizard`,`overview`), `duration_ms`, `error.*` | `e2e/helpers.js:19` |
| Test hook timing samples | `e2e.hook.timing` / `e2e.test.timing` | DEBUG | e2e_runner | `spec`, `test_name`, `status`, `duration_ms`, `threshold_ms` | `e2e/wdio.conf.js:391`, `e2e/wdio.conf.js:405` |
| WebDriver session teardown | `e2e.webdriver_session.completed` | INFO | e2e_runner | `session_id`, `status`, `duration_ms`, `cleanup_actions` | `e2e/wdio.conf.js:493`, `e2e/wdio.conf.js:500` |
| Failure artifact capture (required) | `e2e.artifacts.collected` / `e2e.artifacts.collect_failed` | INFO / WARN | e2e_runner | `session_id`, `spec`, `test_name`, `artifact_dir`, `files`, `error.*` | add in `afterTest`/`afterSession` hooks |

## Implementation Notes (to keep inventory actionable)

1. Introduce a shared helper for lifecycle triplets:
   - `<x>.started`
   - `<x>.completed` with `status=ok`, `duration_ms`
   - `<x>.failed` with `status=error`, `error.code`, `error.message`, `duration_ms`
2. Wrap all Tauri commands once (middleware-style) rather than instrumenting each command manually first.
3. Generate `request_id` in frontend IPC client and pass through invoke payload metadata.
4. Stop using static daemon request ids like `"status-ping"`; use unique ids and surface as `daemon_request_id`.
5. Add dropped-event counters for all throttling/buffering paths (`frontend_log`, daemon event forwarding, watcher backpressure).

