# Observability Review: Critical Failure Paths

Date: 2026-03-19
Owner: architect-1
Task: `#1330`

## Scope

Reviewed structured logging coverage for:

- startup
- daemon RPC / daemon connection lifecycle
- project mutation flows
- coordination failure paths

Primary files inspected:

- `src-tauri/src/commands/logging.rs`
- `src-tauri/src/commands/lifecycle.rs`
- `src-tauri/src/startup/telemetry.rs`
- `src-tauri/src/startup/orchestration.rs`
- `src-tauri/src/startup/daemon.rs`
- `src-tauri/src/daemon_api.rs`
- `src-tauri/src/provider/daemon_client.rs`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/services/project.rs`
- `src-tauri/src/commands/coordination.rs`
- `src-tauri/src/commands/coordination/progress.rs`
- `src-tauri/src/coordination/orchestrator.rs`

## Current strengths

The foundation is solid in three places:

1. IPC lifecycle coverage is good.
   - `IpcCommandSpan` emits `ipc.command.received/completed/failed` plus `ipc.lock.wait`.

2. Daemon RPC transport coverage is good at the generic transport layer.
   - `DaemonRpcSpan` emits `daemon.rpc.sent/response/timeout/failed`.
   - Daemon connection lifecycle emits `daemon.connection.*`.

3. Startup already has a meaningful structured event surface.
   - `startup.app.started`
   - `startup.paths.resolved`
   - `startup.database.*`
   - `startup.daemon_*`
   - `startup.watchers.initialized`
   - `startup.search.initialized`

The main observability debt is above those layers: semantic failure context is still thin in the places where operators actually need to answer “what workflow failed, at what step, for which project/team/member, and what degraded behavior followed?”

## Recommended logging additions

Ordered by value for diagnosis.

### 1. Add canonical structured events for coordination pipeline step outcomes

**Gap**

Coordination commands emit top-level IPC lifecycle events, and the UI gets `coordination-step-progress` events, but the backend does not emit canonical JSONL step lifecycle events for initialize/add-agent/resume/reonboard/disband flows.

**Evidence**

- `src-tauri/src/commands/coordination.rs`
- `src-tauri/src/commands/coordination/progress.rs`
- `src-tauri/src/coordination/orchestrator.rs:1352-1357`

Today, `coordination-step-progress` is UI-only, and buffered audit events are flushed through `tracing::info!(target: "coordination_audit", ...)` rather than the canonical `emit_global(...)` path.

**Why this is a blind spot**

When `coordination_initialize_team` or `coordination_resume_member` fails, JSONL reliably shows the IPC failure, but not:

- which pipeline step failed
- whether the team/config was partially created
- which member was being resumed/onboarded/removed
- whether cleanup ran and what it did

That makes degraded Mesh incidents much harder to reconstruct from the canonical sink.

**Recommended events**

- `coordination.pipeline.started`
- `coordination.step.started`
- `coordination.step.completed`
- `coordination.step.failed`
- `coordination.pipeline.completed`
- `coordination.pipeline.failed`

**Required fields**

- `team_name`
- `member_name` when relevant
- `operation` (`initialize_team`, `add_agent`, `resume_member`, `resume_team`, `reonboard`, `remove_member`, `disband_team`)
- `step`
- `status`
- `duration_ms`
- `error.code`
- `error.message`
- `cleanup_ran`
- `partial_state_written`

### 2. Add semantic structured events for project mutations

**Gap**

Project mutations currently rely almost entirely on generic IPC lifecycle events. The actual domain actions do not emit canonical structured mutation events.

**Evidence**

- `src-tauri/src/commands/projects.rs:285-425`
- `src-tauri/src/services/project.rs:13-123`

`create_project`, `update_project`, `remove_project`, and `register_projects_batch` use `IpcCommandSpan`, but there is no `emit_global(...)` or equivalent semantic event describing the mutation itself.

**Why this is a blind spot**

When a user says “my project disappeared,” “batch registration only partly worked,” or “remove succeeded but cleanup looks incomplete,” the canonical sink cannot answer:

- which project ids/paths were mutated
- how many paths succeeded vs failed in batch registration
- whether post-create reseed or post-remove search cleanup degraded
- whether a mutation changed only metadata or the project set itself

Warnings exist for cleanup failures, but there is no structured success/failure story for the mutation workflow.

**Recommended events**

- `projects.create.started/completed/failed`
- `projects.update.completed/failed`
- `projects.remove.completed/failed`
- `projects.batch_register.completed`
- `projects.batch_register.item_failed`
- `projects.reseed.degraded`

**Required fields**

- `project_id`
- `project_path`
- `project_name`
- `batch_size`
- `success_count`
- `failure_count`
- `error.code`
- `error.message`
- `search_cleanup_status`
- `git_reseed_status`

### 3. Fill startup failure-shape gaps and background-task visibility gaps

**Gap**

Startup has a good top-level event inventory, but several failure events are too thin and some declared phases have no matching completion/failure events.

**Evidence**

- `src-tauri/src/startup/telemetry.rs`
- `src-tauri/src/startup/orchestration.rs`
- `src-tauri/src/startup/daemon.rs`

Examples:

- `emit_startup_init_failed(...)` emits `error.code` and `error.message`, but not `duration_ms`, `phase`, or degraded fallback details.
- `startup.orchestration.started` declares `background_tasks` in its step list, but there is no structured `startup.background_tasks.*` event family.
- `spawn_coordination_self_heal_monitor(...)` in `startup/orchestration.rs` uses tracing-only info/warn lines for success/failure, not canonical structured events.

**Why this is a blind spot**

Startup issues are often racey or environment-specific. Without `duration_ms`, `phase`, and fallback-state fields, the canonical sink tells us that startup failed but not whether Taurhaus:

- fell back to local-only watching
- lost search initialization but kept the shell alive
- started runtime monitors but never finished bootstrap
- hit repeated self-heal failures after startup looked healthy

**Recommended events**

- `startup.watchers.failed` with `duration_ms` and fallback fields
- `startup.search.failed` with `duration_ms`
- `startup.background_tasks.started/completed/failed`
- `startup.self_heal.started/completed/failed`

**Required fields**

- `phase`
- `duration_ms`
- `degraded_mode`
- `local_watcher_enabled`
- `daemon_watch_bootstrap`
- `search_available`
- `error.code`
- `error.message`

### 4. Add caller-context fields above daemon RPC spans

**Gap**

Daemon transport events are well instrumented, but they are too generic to fully explain user-visible failures in higher-level workflows.

**Evidence**

- `src-tauri/src/daemon_api.rs`
- `src-tauri/src/provider/daemon_client.rs`
- `src-tauri/src/commands/command_center/launching.rs`

`daemon.rpc.*` events currently capture:

- `daemon_request_id`
- `method`
- `status`
- `duration_ms`
- `retry_count`

But they do not capture higher-level workflow context like `project_id`, `team_name`, `member_name`, or caller operation.

**Why this is a blind spot**

If multiple daemon calls are failing at once, the canonical sink cannot easily answer:

- which project’s launch failed
- whether the failing RPC came from startup, command center, search, or coordination
- which Mesh team/member was affected

The higher-level command-center logs help for launch, but the correlation is inconsistent and not generalized.

**Recommended additions**

Either:

- enrich `daemon.rpc.*` with optional caller context fields when known

or:

- emit paired semantic events such as `command_center.launch.daemon_request`, `startup.daemon_probe.failed`, `coordination.runtime_probe.failed`

**Required fields**

- `project_id`
- `project_path`
- `team_name`
- `member_name`
- `caller`
- `request_id` when an IPC request initiated the daemon call

### 5. Promote coordination audit output into the canonical JSONL sink

**Gap**

Coordination audit data exists, but it is not emitted through the same canonical structured event sink used elsewhere.

**Evidence**

- `src-tauri/src/coordination/orchestrator.rs:1347-1357`

`flush_audit_to_log()` serializes audit events to JSON and writes them through `tracing::info!(target: "coordination_audit", ...)`.

**Why this is a blind spot**

That makes coordination audit visibility more fragile than the rest of the logging system:

- different sink path
- weaker consistency with JSONL event naming
- harder to join with `run_id`, `request_id`, and daemon correlation

This is especially costly for postmortems involving partial initialization, cleanup after failed add/remove, or degraded resume.

**Recommended change**

Emit audit events through `emit_global(...)` as canonical `coordination.audit.*` events, or mirror them into that sink even if tracing output is retained.

**Required fields**

- `team_name`
- `member_name`
- `event_type`
- `operation`
- `status`
- `request_id` when present

## Suggested implementation order

1. Coordination pipeline lifecycle events
2. Project mutation semantic events
3. Startup failure-shape and background-task events
4. Daemon caller-context enrichment
5. Coordination audit mirroring into canonical JSONL

## Bottom line

The repo does not have a “no logging” problem. It has a “semantic context stops one layer too early” problem.

The fastest win is to add structured events at the workflow layer above IPC and daemon transport:

- coordination step outcomes
- project mutation outcomes
- startup degraded/fallback states

Those additions will make the canonical JSONL sink far more useful for real field debugging without requiring a logging-system redesign.
