# Architecture

A condensed overview for contributors. For detailed references, see the [docs/ index](docs/README.md).

This document is the system-level overview.

- Use [`docs/architecture/data-architecture.md`](docs/architecture/data-architecture.md) for the authoritative data-store inventory, ownership boundaries, and authority rules.
- Use [`docs/coordination-architecture.md`](docs/coordination-architecture.md) for coordination subsystem decisions, invariants, recovery semantics, and runtime behavior.

## System Overview

taurhaus is a cross-platform dual-process desktop application built with Tauri 2. The native GUI (Rust + Svelte 5) handles storage, git, search, native/local file watching, and UI-facing orchestration. A lightweight companion daemon handles process scanning, tmux session management, activity detection, interactive team lifecycle pipelines, and WSL file watching only when the app is bridging into Linux workspaces. It also owns the managed-task deadline pass, foreground tmux focus (a `tmux list-clients` probe carried inside the versioned session snapshot), Codex native idle notify ingestion (`taurhaus-daemon codex-notify`), the Antigravity activity-hook sink (`agy_hooks.rs`), provider account detection, and the usage poller on Windows/WSL. The app and daemon communicate using an authenticated JSON-line protocol over TCP; [`daemon::server::DEFAULT_PORT`](src-tauri/src/daemon/server.rs) defines the default endpoint as `127.0.0.1:17233`. The app refuses a daemon whose protocol version differs from its own.

The daemon can run on all supported platforms, but it is only responsible for watch/process work that the app cannot do directly:

- **Windows**: The daemon runs inside WSL2 (launched via `wsl.exe`), where it has access to `/proc` for process inspection and the Linux filesystem where AI tools run.
- **macOS / Linux**: The daemon runs natively as a subprocess (launched from `~/.local/bin/taurhaus-daemon`) for session scanning / terminal control, while the app keeps ownership of native project file watchers.

![System Architecture](docs/images/system-architecture.jpg)

## Harness Model

taurhaus does not host models itself: Claude Code is the Claude harness (subscription models are only reachable through it), Codex CLI, Antigravity CLI (`agy`) and Grok CLI (`grok`) are theirs, and taurhaus coordinates all four from outside through tmux panes and the mesh bridge. Harness-native capabilities (a sessions registry, hooks, turn-complete notifications) are used where they exist; tmux + mesh is the floor that reaches any CLI. Per-tool behaviour is confined to capability slices behind one registry (`src-tauri/src/session_scanner/cli_tool.rs`), so adding a CLI touches only the slices where it differs. Model and reasoning effort are separate fields end to end, an account is chosen per project and tool wherever the harness has a selector, and the app and daemon refuse to run on mismatched protocol versions. See [Harness Model](docs/architecture/harness-model.md).

## Platform Abstraction

The platform boundary is intentionally split:

- `platform/` provides compile-time dispatch (`#[cfg(target_os)]`) for process-inspection primitives such as cwd, tty, IO, and socket state.
- `provider/path.rs` owns cross-platform project-path normalization and Windows/WSL/Linux translation.
- `provider/platform_paths.rs` is the authority for app data, Claude/team roots, tool session roots, daemon binary location, log path, and Claude hook paths.

Linux and macOS implement the real process-inspection surface. Windows still uses explicit stubs for scanner-only APIs while sharing hidden-window process launch helpers and UNC-path resolution for WSL-backed tool state.

| Function | Linux (daemon in WSL2) | macOS (native daemon) |
|----------|--------------------|--------------------|
| `process_cwd(pid)` | `/proc/PID/cwd` readlink | `proc_pidinfo` (libproc) |
| `process_tty(pid)` | `/proc/PID/fd/0` readlink | `lsof -p PID -a -d 0` |
| `process_rchar(pid)` | `/proc/PID/io` rchar field | `proc_pid_rusage` (libproc) |
| `collect_socket_inodes(pid)` | `/proc/PID/fd` → socket inode extraction | `lsof -p PID -i TCP` |
| `has_established_443(pid)` | `/proc/PID/net/tcp` socket state parsing | `lsof` ESTABLISHED filter |

The session scanner (`session_scanner/`) and activity detector are platform-agnostic at the call site — they rely on the `platform/` module for the OS-specific work while keeping the higher-level detection logic shared.

## Frontend (Svelte 5 + Tailwind v4)

The frontend runs inside Tauri's embedded WebView — not a browser. All data comes through the Rust backend via IPC.

| Component | Purpose |
|-----------|---------|
| `App.svelte` | Entry point, splash screen gate |
| `Shell.svelte` | Shell coordinator: project loading, navigation state, daemon/session wiring, and tab composition |
| `src/lib/components/shell/ShellTitlebar.svelte` | Titlebar tabs, search, theme toggle, and window controls |
| `src/lib/components/shell/ShellMainPanel.svelte` | Main panel shell, banners, and tab-panel host |
| `src/lib/shell/shortcuts.svelte.js` | Keyboard shortcut wiring for search and history navigation |
| `src/lib/shell/sessionLifecycle.svelte.js` | Session/foreground lifecycle wiring; handles the `tmux-focus-changed` event (already resolved to `project_id` by the backend) |
| `src/lib/shell/window.js` | Tauri window controls and startup viewport sync |
| `src/lib/activitySignal.js` | Single activity derivation (working/active/idle/uncertain/offline + confidence) for sidebar, hover card, and mesh |
| `src/lib/modelCatalog.js` | Helpers over the backend-owned `ModelCatalog` from `settings.terminal_contract` |
| `src/lib/context/ModelCatalogContext.js` | Model catalog context provider |
| `src/lib/components/ModelSelect.svelte` | Effort-aware model picker fed by the backend catalog |
| `src/lib/accounts.svelte.js` | Per-tool account state (accounts, pins, usage, pending chooser) |
| `src/lib/accountMenu.js` | Registry-driven account rows for a context menu; the one place compact usage is rendered as menu text |
| `src/lib/usageWindows.js` | Normalisation helpers over a provider's usage windows |
| `src/lib/components/AccountChooser.svelte` | Per-launch account decision (Shell), shown only when a launch is unplaced and 2+ accounts of that tool are signed in |
| `src/lib/components/AccountChip.svelte` | Per-project account display/change control (OverviewTab) |
| `src/lib/components/UsageMeter.svelte` | One bar per usage window, in the tool's own titles (full and compact forms) |
| `Sidebar.svelte` | Project list, session indicators, context menu, hover cards |
| `src/lib/OverviewTab.svelte` | Project summary, README, recent commits, sessions |
| `src/lib/FilesTab.svelte` | File tree with syntax-highlighted code preview |
| `src/lib/GitTab.svelte` | Commit history, diffs, cross-tab navigation |
| `src/lib/TaskBoard.svelte` | Kanban board aggregating tasks from Claude Code and Codex |
| `src/lib/TaskDetailPanel.svelte` | Task detail view with metadata and description |
| `src/lib/SearchOverlay.svelte` | Full-text search across all projects (Ctrl+K) |
| `src/lib/Settings.svelte` | App preferences and configuration |
| `src/lib/FirstRunWizard.svelte` | Onboarding flow: project discovery and registration |
| `src/lib/SplashScreen.svelte` | Startup splash with bootstrap chain progress |
| `src/lib/SessionHistory.svelte` | Session timeline with handoff summaries |
| `src/lib/HoverCard.svelte` | Decision-oriented project hover preview with live status, latest change, and relationship cue |
| `src/lib/components/MeshTab.svelte` | Mesh View orchestration surface (gate/setup/init/runtime states) |
| `src/lib/components/MeshSetupView.svelte` | Setup shell that hosts the primary team-builder and init-progress states |
| `src/lib/components/MeshTeamBuilder.svelte` | Primary setup surface with quick presets, role filters, and drag/drop roster composition |
| `src/lib/components/MeshCanvas.svelte` | Mesh runtime canvas that renders node/detail UI from layout-engine output |
| `src/lib/components/meshLayout.js` | Pure mesh layout engine for node boxes and explicit connection routes |
| `src/lib/components/MeshConnection.svelte` | SVG cubic-route renderer for explicit control-point geometry |
| `src/lib/components/MeshRuntimeBar.svelte` | Runtime status controls (add-agent/disband/summary pills) |
| `src/lib/a11y.js` | Accessibility primitives: modal isolation stack, focus traps, keyboard helpers |
| `src/lib/errorCopy.js` | User-facing error and empty-state copy helpers for all surfaces |
| `src/lib/shell/events.svelte.js` | Shell-level Tauri event subscriptions and lifecycle wiring |

**Key patterns:**
- **Svelte 5 runes** (`$state`, `$derived`, `$effect`, `$props`) — no legacy stores
- **Derived theme tokens** — all color switching via `$derived` variables, never inline ternaries
- **`$bindable` position memory** — each tab exposes view state, Shell saves/restores per project
- **IPC layer** — `src/lib/ipc.js` is a thin compatibility re-export; the real Tauri `invoke()` wrappers and mock fallbacks live under `src/lib/ipc/`
- **Accessibility primitives** — modal isolation, focus traps, keyboard navigation, and ARIA semantics are centralized instead of per-component one-offs
- **Shared error copy** — error and empty-state wording is funneled through `errorCopy.js` helpers instead of ad hoc string assembly
- **Visual testing lane** — Browser Mode screenshots live under `src/test/visual/`; a plain Vite fixture host lives at `visual-host.html`

## Backend (Rust)

### Modules

| Module | Purpose |
|--------|---------|
| `commands/` | Tauri IPC handlers — thin wrappers over domain modules |
| `platform/` | Compile-time OS dispatch (linux.rs / darwin.rs) |
| `db/` | SQLite connection, migrations, typed query functions |
| `git/` | libgit2 wrappers for commits, diffs, status |
| `fs/` | File tree, content reading, asset serving, file watching |
| `search/` | tantivy full-text search index (build, update, query) |
| `session/` | Session import, parsing, archival |
| `session_scanner/` | CLI tool detection (process scanning, idle detection), plus `cli_tool.rs` (the harness registry), `launch.rs` (ModelSpec/LaunchSpec command renderer), `accounts/` (per-tool account + usage providers), `idle/` (`claude_registry.rs`, `codex.rs`, `agy.rs`, `grok.rs`), `tmux.rs` (`list_clients`/`focus_from_clients`), `classification.rs`/`scans.rs` (authoritative vs heuristic state, degraded-scan last-good snapshot) |
| `task_scanner/` | Task aggregation from Claude Code and Codex — the two harnesses with a verified task source (`claude_index.rs` maps source_key -> project for robust scans) |
| `daemon/` | TCP protocol/server/event-listener/launcher code for the companion daemon, plus the session-activity hub (`session_activity.rs`: versioned snapshot, tmux focus, degradation cursor), `handlers.rs`, `codex_notify.rs` (`codex-notify` subcommand), `agy_hooks.rs` (Antigravity activity-hook sink), `usage_poller.rs` (per-account usage polling), `auth.rs`, `watch.rs`, `session_listener.rs` |
| `daemon_api.rs` | App-facing daemon request wrapper used by commands and startup flows |
| `terminal/` | Terminal emulator management (Windows Terminal, iTerm2, etc.) |
| `claude_code/` | Claude Code project resolution, memory, teams |
| `provider/` | Concrete provider implementations plus path translation and `PlatformPaths` root authority |
| `project_provider.rs` | Shared `ProjectProvider` trait and provider selection boundary |
| `services/` | Cross-cutting services: relationships, scanner, project utilities, session import |
| `services/scan_policy.rs` | Shared settings-backed scan/index policy for discovery and indexing |
| `models/` | Shared data structures (Project, Session, ActivityState, etc.) |
| `config/` | Application configuration |
| `coordination/` | Multi-CLI team orchestration (behind `mesh-bridged-backend` feature flag) |
| `coordination/runtime/` | Split runtime surface for system, tmux, process, and recording concerns |
| `coordination/compact_hook.rs` | One compaction hook bridge for Claude, Codex and Grok (tool inferred from grok's reserved `GROK_*` hook env, else from the transcript path), plus the managed Codex `hooks.json` and Grok `~/.grok/hooks` installers |
| `coordination/agy_hooks_installer.rs` | Managed installer for Antigravity's activity hooks — merged into the shared `~/.gemini/config/hooks.json` (`agy.hooks.degraded`) |
| `startup/` | Startup sequence orchestration (DB init, daemon connect, watcher/index bootstrap, task/session hydration) |
| `startup/setup.rs`, `startup/telemetry.rs`, `startup/orchestration.rs` | Split startup path resolution, startup logging, and orchestration phases |
| `templates/adapters.rs` | Role import/export adapters, provenance, and field-mapping rules for external agent formats |
| `sentinels.rs` | Shared sentinel/fallback utilities used by startup and command flows |
| `event_processor.rs` | File/git event batching (300ms quiet window, 2s ceiling) |
| `daemon_lifecycle.rs` | Daemon auto-launch, reconnection, shutdown |
| `watch_targets.rs` | Activity-based planning for local and daemon watch reconciliation |
| `tmux_layout.rs` | Shared tmux pane/window layout policy used by command-center and coordination runtime code |

The crate enforces `#![deny(unsafe_code)]` — the single exception (libgit2 init) uses a scoped `#[allow]`.

### Provider Routing

The `ProviderState` routes each IPC operation to the right backend based on project path:

- **LocalProvider** — direct filesystem/git/search access. Used for native projects (macOS, Linux).
- **DaemonProvider** — proxies operations over TCP to the daemon. Used for WSL projects on Windows.

Both implement the `ProjectProvider` trait. The routing is transparent to command handlers — they call `provider_state.resolve(path)` and get the right implementation.

### Storage

- **SQLite** (`rusqlite`): 8 domain tables — `projects`, `sessions`, `session_activity`, `relationships`, `tasks`, `settings`, `archived_task_session_summaries` (migration 011), `project_tool_accounts` (migration 013) — plus the internal `_migrations` bookkeeping table created by `db/migrations.rs`. `project_tool_accounts` holds one `pinned` or `last_used` account per (project, tool) and carries the old `projects.claude_account_id` pins (migration 012) forward; that column survives in the table but is read only as `_legacy_claude_account_id`. `tasks.effort` (migration 014) carries the level a lead asked an assignment to run at and `tasks.effort_why` the lead's reason for it; both stay NULL for a task no lead assigned. Source of truth for structured data.
- **tantivy**: Full-text search index over files, commits, sessions. Rebuilt from filesystem on startup.
- **Filesystem**: Source of truth for content. SQLite stores metadata; files are always read fresh.
- **Path overrides**: `TAURHAUS_DATA_DIR` overrides Tauri `app_data_dir()` resolution; `TAURHAUS_CLAUDE_DIR` overrides Claude-derived roots used by task/coordination watchers.

See [data model reference](docs/architecture/data-model.md) for schema details.

### IPC Commands

Fine-grained, one command per operation. The default build registers 95 commands in the authoritative [`generate_handler!` list](src-tauri/src/lib.rs#L176) — 80 without the default `mesh-bridged-backend` feature, which gates the 15 coordination commands. Frontend calls in parallel for speed. See [IPC reference](docs/architecture/ipc-reference.md) for the command catalog.

Grouped by command module:
- **Projects** (12): includes `create_project`, registration flows, path/directory helpers, and first-run checks
- **Git** (4): commit lists + status + remote URL
- **Files** (5): file tree/read/readme/asset/path-type
- **Search** (3): search, rebuild, index status
- **Sessions** (3): list/latest/detail
- **Workflow runs** (3): `list_workflow_runs`, `get_workflow_run`, `workflow_ledger_row`
- **Relationships** (4): list/create/dismiss/remove
- **Command Center** (9): launch/stop/navigate/list/list snapshot/resolve launch account/record activity/get project activity/get foreground project
- **Accounts and usage** (4): `list_accounts`, `resolve_launch_bases`, `refresh_accounts_usage`, `set_project_account`
- **Tasks** (6): board data + detail + archive + commit context helpers
- **Daemon** (5): platform/status/start/install checks
- **Mesh install** (2): check/install mesh binary
- **Settings** (2): get/update
- **Coordination** (15): team lifecycle + member lifecycle + live status + snapshot + preflight/availability
- **Templates** (17): role/preset CRUD, import/export, composition, storage status, history/diff/revert/flush, agent-definition export
- **Logging** (1): frontend `console.*` is bridged to `frontend_log` IPC with structured payloads. Backend emits structured events into a JSONL sink at `taurhaus.log.jsonl`.

### Logging and Observability

Logging is structured and machine-first:

- **Canonical file**: `app_data_dir()/taurhaus.log.jsonl` (or `<TAURHAUS_DATA_DIR>/taurhaus.log.jsonl`).
- **Schema**: JSONL records with required keys (`ts`, `level`, `component`, `event`, `run_id`) plus event fields.
- **Sink architecture**: single-writer async pipeline (`src-tauri/src/commands/logging.rs`) with a bounded channel and one writer thread to prevent torn/interleaved lines.
- **Rotation policy**: size-based rotation (20 MB segment threshold) with retention pruning (7 days).
- **Frontend bridge**: `src/lib/logger.js` forwards structured payloads (`component`, `subsystem`, `event`, `message`, `context`) and emits drop telemetry (`frontend.logs.dropped`) under throttling.
- **Lifecycle instrumentation**:
  - startup phases (one event family per phase — there is no generic `startup.phase.*`; a test in `startup/telemetry.rs` asserts those legacy names are never emitted): `startup.app.started`, `startup.paths.resolved`, `startup.logging.initialized`, `startup.database.started/completed/failed`, `startup.daemon_phase.started/completed`, `startup.daemon_connect.succeeded/deferred`, `startup.orchestration.started/completed`, `startup.watchers.initialized`, `startup.search.initialized`, `startup.background_tasks.started/completed` from `startup/telemetry.rs`; `startup.watchers.failed`/`startup.search.failed` from `startup/orchestration.rs` and `startup/harness.rs`; `startup.watchers.bootstrap.started/completed` from `startup/watchers.rs`; `startup.bootstrap_thread.spawned`, `startup.daemon_bootstrap.*`, `startup.mesh_install.*` and `startup.daemon_protocol.checked` from `startup/daemon.rs`
  - IPC lifecycle: `ipc.command.received/completed/failed`, `ipc.lock.wait`
  - daemon RPC lifecycle: `daemon.rpc.sent/response/timeout`
  - coordination step lifecycle: `coordination.step.started/completed/failed`
  - member runtime writes: `coordination.runtime.commit_skipped` (a compare-and-commit whose dependencies moved, with `changed_fields`), `coordination.runtime.record_skipped` (an unreadable record, deduplicated per member and reason), `coordination.runtime_store.io_failed`, `coordination.store.lock_unsupported`
  - assignment effort: `effort.resume.started/completed/failed` (`coordination/task_effort.rs`), including the single `failed` record carrying `reason: budget_exhausted` once the three attempts for a task and level are spent
  - daemon background coordination: `self_heal.pass.completed/failed` and the bounded once-per-process `effort.sweep.awaiting_settings`
  - coordination audit stream: `coordination.audit.*`
  - project mutation and reseed outcomes: `projects.*`, `projects.reseed.degraded`
  - watch/index activity: `watch.batch.flushed`, `watch.git_status.*`, `search.file_index.*`
  - session activity transitions: `activity.state.changed` (`pid`, `tool`, `from`, `to`, `source`)
  - process inventory health: `session_scanner.process_scan.degraded/recovered` — one `degraded` on entry, a bounded 60s reminder while the outage lasts, one `recovered` on exit
  - launch rendering: `launch.command.rendered`, `launch.account.*`, `launch.model.*`, `launch.effort.*`, `launch.flag.deprecated`, `launch.capability_missing`, `launch.notify.ignored`, `launch.selector.ignored/rewritten`, and the base-command outcomes `launch.base.opaque` / `launch.base.unresolved`
  - compaction: `compaction.injected/skipped/failed`, `compaction.<tool>_hook.received/resolved/delivered/skipped/failed` (`claude`/`codex`/`grok`, built from the inferred tool in `compact_hook.rs`), `compaction.codex_hook.unsupported/version_unknown/reconciled/degraded`, `compaction.hook.compat_import`, `compaction.compact_hook.failed`
  - accounts and usage: `usage.fetched`, `usage.failed` (`daemon/usage_poller.rs`), `account.provider.floor` (`session_scanner/accounts/mod.rs`), `claude.usage.legacy_bridge.removed` (the one-shot status-line-bridge uninstall)
  - Antigravity activity hooks: `agy.hooks.degraded` (`coordination/agy_hooks_installer.rs`)
  - Codex native idle notify: `codex.notify.appended`
  - daemon pairing: `startup.daemon_protocol.checked`

Correlation model used across events:

- `run_id`: per app run, attached to all records.
- `interaction_id`: frontend user interaction chain.
- `request_id`: frontend->backend IPC request lifecycle.
- `daemon_request_id`: backend->daemon RPC lifecycle.

Logging policy and level selection reference:
- [`docs/architecture/log-level-guidelines.md`](docs/architecture/log-level-guidelines.md)

### Coordination (Mesh View)

The `coordination/` subsystem powers multi-agent team orchestration and is gated by the `mesh-bridged-backend` Cargo feature (enabled by default).

- **State bootstrap**: interactive mutation runs share one daemon-process `CoordinationState` and lazily build its orchestrator on first use (no startup hard dependency on mesh availability).
- **Persistence**: by default, team definitions are stored in `~/.claude/teams/<team>/config.json` (`TeamConfigStore`), while runtime attachment state lives in `~/.claude/teams/<team>/runtime/*.json` (`MemberRuntimeStore`). If `TAURHAUS_CLAUDE_DIR` is set, coordination uses `<TAURHAUS_CLAUDE_DIR>/teams/...` instead.
- **Writer boundary**: protocol 22 completes daemon routing. The Windows app may read team state but never mutates it directly; daemon modules and the named WSL-native hook processes are the only writers. `module_boundary_assertions` enumerates every allowed store-write caller so a new app-side writer fails CI.
- **Pipelines**: `coordination/pipelines/` drives initialize, hot-add, and resume flows (validate -> create/resolve panes -> launch sessions -> mesh join -> daemon start -> onboarding delivery). `pipelines/effort.rs` is a separate pipeline for assignment effort: it owns held-task target selection, the launch-base rewrite, the stop-before-resume sequencing, and the three-attempt budget per task and level.
- **Runtime write exclusion**: a runtime decision performs every external probe *outside* the locks, then commits through `MemberRuntimeStore::commit_if_unchanged`, which holds the team lock across target-file lock -> re-read -> compare -> mutate -> atomic save. If any dependency moved the commit is skipped and reports the `changed_fields` that moved — `pane_id`, `pane_pid`, `pane_start_time`, `session_id`, `daemon_pid`, `health`, `appliedEffort`, or the sentinel `record` when the file itself appeared, vanished, or could not be parsed. Daemon-owned interactive and background orchestrators therefore cannot interleave a runtime-record write for the same team.
- **Delivery outcomes are separate facts**: `DeliveryResult` carries `delivered` (the backend completed its operation), `method`, `durable` (the inbox append persisted), `wake` (a typed `WakeDisposition`: `AlreadyLive`, `Spawned`, `Adopted`, `NotAttempted { reason }`, `Failed { reason }`), and `post_write_warnings` — failures that happen *after* a successful delivery, such as the operational-snapshot or runtime-record update. The orchestrator replaces `wake` and extends `post_write_warnings`; the member pipeline lifts both into the report's warnings, promoting only a `Failed` wake or a `NotAttempted` whose reason is one of the two pane-dead constants into an operator-visible warning.
- **Resume lifecycle**: recovery supports both per-member resume (`coordination_resume_member`) and daemon-owned team-level cold-restart resume (`coordination_resume_team`). Team resume reuses the existing member-resume pipeline, resumes the lead first, then the remaining members, and returns structured per-member progress/failure results.
- **Snapshot classification**: project mesh snapshots and live team status use fast persisted config/runtime reads and classify team state as `none`, `active`, `degraded`, or `cold_resume`, which the frontend maps into recovery affordances like `Resume Team` (cold restart) and `Resume Stopped (n)` (degraded).
- **Windows runtime safety**: Windows coordination and command-center background calls use hidden-window spawning for `wsl`/mesh/tmux invocations, and runtime ownership comparisons normalize Windows, WSL UNC, and Linux project-path forms before matching sessions, panes, and team members.
- **Liveness repair**: pane/process/daemon reconciliation runs in explicit recovery flows and the daemon-owned background self-heal scheduler, not on the UI-critical snapshot IPC path. Background self-heal uses a dedicated orchestrator instance so it does not block foreground coordination RPCs on the shared interactive orchestrator mutex.
- **Mesh daemon hot-swap**: mesh installs are version-aware. Member daemon reconciliation checks executable identity and automatically replaces drifted daemons; bounded background self-heal does the same for drifted team-daemons, so normal upgrades do not require a manual `team-daemon stop/start/restart-all` cycle.
- **Runtime responsiveness**: Mesh steady-state polling stays on the fast snapshot path, and the frontend suspends hidden-tab refresh work, which avoids switch-away stalls and reduces Windows popup latency during runtime navigation.
- **Runtime/disband behavior**: disband removes persisted team state and performs best-effort teardown of managed agent resources (mesh membership, daemon processes, panes). Attach-existing leads are preserved only for Claude — the validation is capability-driven (`should_use_mesh_sidecar`, i.e. any harness without the native inbox poller), so Codex, Antigravity and Grok leads validate as `launch_new` only, and mesh-backed or app-owned leads are torn down like other managed members.
- **Compaction reinjection**: `coordination/compact_hook.rs` is the only compaction owner for Claude, Codex and Grok. It resolves the affected managed member and restores working context only when the operational snapshot still has a resumable task. The tool is inferred from the reserved `GROK_*` env names grok injects into every hook process, and otherwise from the transcript path. It accepts `SessionStart` with `source=compact`, plus `PostCompact` for a harness whose registry delivery is the mesh inbox — grok, whose session-start source never reports `compact`. A `PostCompact` payload for a stdout-answered harness is skipped as `post_compact_signal_only`. Where the registry declares `HookStdout` delivery (Claude, Codex) the card goes straight back to the CLI as `hookSpecificOutput.additionalContext`; where it declares `MeshInbox` (grok, whose passive-hook stdout is documented as ignored) the card is queued in the member's inbox. It installs runtime-appropriate `.sh` / `.cmd` wrappers, normalizes current hook payload field variants, logs standalone hook execution into the canonical JSONL sink, and manages idempotent, removable, exe-path self-repairing Codex `hooks.json` and Grok `~/.grok/hooks` installers. Because grok also loads `~/.claude/settings.json` hooks, the registry declares `compaction_hook_compat_import` and the bridge deduplicates, so one compaction is one reinjection.

  Managed Codex uses this hook path by default when `CliVersions.codex_compaction_hooks_supported`; Codex 0.147 is the floor. Older Codex versions log `compaction.codex_hook.unsupported` once and receive no reinjection. There is no compaction mode setting, transcript tailer, or daemon/app owner election.
- **Runtime UI architecture**: Mesh View uses a deterministic node canvas (`MeshCanvas`) backed by a pure layout engine (`meshLayout.js`) instead of force-sim layouts. Lead/agent boxes and cubic connection routes are computed together from container size and roster cardinality (single-row up to medium teams, split rows for larger teams), with explicit state mapping for setup/initializing/runtime.
- **Runtime interactions**: node detail actions (`MeshNodeDetail`) and runtime controls (`MeshRuntimeBar`) operate on the same live-status pipeline (`coordination_get_live_team_status`, add/remove/resume/disband IPCs), so canvas state and control-bar state stay consistent without a separate client-side data model. `MeshRuntimeBar` is also the shipped cold-restart/degraded recovery surface for team resume.
- **Recovery status at the final active-development snapshot**: shipped resume/recovery flows are covered by dedicated E2E specs. Known degraded-path edge cases remain recorded in the task and commit history rather than presented as an active roadmap.

See [coordination architecture](docs/coordination-architecture.md) for deeper design details and decision history.

### Team Templates

The template system provides reusable role templates and team presets, with composition and history integrated into mesh setup.

- **Storage model**: built-in templates ship read-only from resources; user templates are YAML files in the app-managed templates directory (`roles/`, `presets/`, `_meta/`).
- **Storage roots**: template files live under the resolved app-data directory (`app_data_dir()/templates` by default, or `<TAURHAUS_DATA_DIR>/templates` when overridden).
- **Git-backed state**: template writes are committed through `TemplateStore`, enabling history (`templates_get_history`), diff (`templates_get_diff`), and forward revert (`templates_revert`).
- **Composition engine**: `templates::composition::compose_team` resolves lead and agent slots into a concrete roster, returning `warnings` and `validation_errors`.
- **Frontend pipeline**: `MeshSetupView` hosts `MeshTeamBuilder` as the primary setup surface (quick presets, role filters, drag/drop roster editing), while `TemplateBrowserPanel` and `TeamCustomizerPanel` remain the advanced catalog/history/edit flows. All of them still resolve into the same `InitializeTeamRequest` shape consumed by `coordination_initialize_team`.
- **Operational visibility**: storage mode, dirty state, and pending actions are exposed via `templates_get_storage_status`; manual flush is available via `templates_flush_pending`.
- **Role model**: roles are context-steering lanes, not capability labels. The persisted schema now combines lane identity (`focus_area`, `context_summary`, `behavior_summary`), operating style (`communication_style`), workflow expectations (`quality_gates`, `definition_of_done`, `phase_scope`, `mode`), composition metadata (`inherits_from`), and deliverable expectations (`required_artifacts`). Behavioral contract, defaults, capabilities, provenance, and constraints still complete the role definition.
- **Lead tool support**: built-in and user templates can define lead roles for Claude, Codex, or Antigravity; the canonical built-in leads are `v3-lead-claude`, `codex-orchestrator`, and `antigravity-orchestrator`. The shipped Grok role is the agent role `v4-developer-grok`, staffed by the `Grok Pair` preset, so a Grok lead needs a user template. Frontend preset/customizer flows preserve the selected lead tool/model all the way into `coordination_initialize_team`; they do not silently backfill Claude defaults.
- **Role adapters**: Taurhaus can export/import Claude agent files, Copilot agent files, and instruction-only formats. Taurhaus-authored Claude/Copilot exports compile the extended role fields into explicit Markdown sections and now re-import those sections back into structured fields, while instruction-only formats remain intentionally lossy. Imported roles persist `provenance` and explicit `non_roundtrippable_fields` so downgraded conversions stay visible to operators.

See [team templates guide](docs/team-templates.md) for user-facing workflows.

### Session Scanner

Detects running CLI tool sessions (Claude Code, Codex, Antigravity CLI, Grok CLI). The detection logic is platform-agnostic — it calls into the `platform/` module for OS-specific process inspection.

Two session views exist on purpose:

- `DisplaySession`: UI-safe session view for sidebar/runtime display
- `RuntimeSession`: transcript-aware session view for coordination, task sync, and compaction, preserving `session_id` and `jsonl_path`

| Tool | Detection | Activity Signal |
|------|-----------|-----------------|
| Claude Code | Sessions registry `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` (authoritative), read under the process's own `CLAUDE_CONFIG_DIR` with a `procStart` PID-reuse guard | Registry status (`busy`/`idle`/`waiting`); rchar rate is the fallback heuristic |
| Codex | Rollout transcript bound with fd proof | `codex-notify.jsonl` idle edge when `codex_notify_supported`; transcript mtime as the heuristic |
| Antigravity CLI (`agy`) | Conversation id from `~/.gemini/antigravity-cli/cache/last_conversations.json`, confirmed by the presence lock in `~/.gemini/antigravity-cli/presence/`; while that index has no live conversation for the cwd, the newest `agy-hooks.jsonl` record naming that workspace with a held presence lock (`idle/agy.rs`) | `agy-hooks.jsonl` sink (`daemon/agy_hooks.rs`), on by default above agy 1.1.10 and bounded to a 5-minute record age; process-IO hysteresis otherwise (`authoritative_idle: false`) |
| Grok CLI (`grok`) | `<GROK_HOME>/active_sessions.json` row bound by pid and cwd — written at the first prompt, removed on `/quit` (`idle/grok.rs`) | `<GROK_HOME>/sessions/<encoded-cwd>/<session-id>/events.jsonl` turn lifecycle: busy unless the newest lifecycle line is `turn_ended` (authoritative) |

Authoritative states skip the rchar heuristic and 2-poll bidirectional hysteresis; heuristic states still use hysteresis to prevent flickering. Tool processes without a controlling terminal (e.g. detached `codex exec`) are dropped before classification and are never sessions.

The process inventory is fail-soft. A scan whose inventory cannot be read is reported `degraded`: it short-circuits classification, returns the last fully classified display/runtime snapshot (shared between both entry points), prunes no trackers, and leaves the daemon hub's snapshot version and export untouched. The degraded flag crosses the daemon boundary in `get_runtime_session_snapshot`, and the frontend treats it as no observation rather than as an empty result.

Managed compaction is hook-driven. The surviving implementation surface is:

- `coordination/compact_hook.rs` — native hook parsing, member resolution, delivery and managed installers
- `coordination/compaction_events.rs` — terminal delivery-result events
- `session_scanner/transcript_boundary.rs` — bounded transcript-tail parsing used to timestamp Codex hook delivery

**Platform details:**
- **Linux**: reads `/proc/PID/io` for IO bytes, `/proc/PID/fd` + `/proc/PID/net/tcp` for socket state
- **macOS**: uses `proc_pid_rusage` (libproc) for IO bytes, `lsof` for socket state

### Terminal Management

The terminal module manages launching and focusing terminal emulators with the correct tmux session. Same decision tree on all platforms — only the emulator options differ.

| Platform | Emulators | Default |
|----------|-----------|---------|
| Windows | Windows Terminal | `wt.exe -w taurhaus` |
| macOS | iTerm2, Ghostty, Terminal.app | iTerm2 (auto-detect fallback) |
| Linux | manual attach / custom CLI contract | manual |

Terminal defaults and supported emulators are now carried through a shared runtime terminal contract in settings, so the frontend, backend, and tests resolve from the same platform authority. `TerminalPlatformContract` also carries `cli_command_defaults`, the `ModelCatalog` (per-model efforts and deprecation hints), and `CliVersions` — probed `codex`/`claude` versions plus the `codex_compaction_hooks_supported`, `codex_notify_supported`, and `codex_queue_wake_supported` gates.

macOS uses event-driven AppleScript to handle click-to-activate focus transitions reliably.

### Event Processing

File system events from `notify` arrive in rapid bursts (5–8 events per file edit). The event processor (`event_processor.rs`) uses **batch-and-flush** to coalesce them:

- **Quiet window** (300ms): batch flushes after no new events for 300ms
- **Max-wait ceiling** (2s): batch flushes regardless after 2s, preventing starvation

Result: one `project-files-changed` Tauri event per edit instead of 5–8. The frontend listener in Shell.svelte dispatches to the active tab via reactive props.

The watch ownership model is now:

- **Native/local projects**: app-owned `notify` watcher with pre-pruned directory registration and `.gitignore` rebuild support
- **WSL projects on Windows**: daemon watch bridge for file/git/session-file events
- **Auxiliary watches**: app-owned task-directory watch bootstrapped from `startup/watchers.rs`. There is no tmux-focus watch — the hook → focus-file → inotify chain was removed and focus is probed by the daemon hub instead.

### Recent Performance Improvements

- **Git range queries**: range traversal uses a single-pass algorithm in `git/commits.rs` (covered by `single_pass_range_matches_dual_pass_output`), reducing duplicated revwalk work.
- **Search indexing**: file updates are batch-committed in `search/indexer.rs` (`update_file_batch_commits_once_for_multiple_files`) to avoid per-file commit overhead.
- **Session scanner CPU**: activity detection uses hysteresis and cadence widening (`session_scanner/proc_io.rs`, `daemon/session_activity.rs`) to reduce false active spikes and idle-loop churn.
- **Watcher registration**: ignored directories are pre-pruned before inotify registration (`fs/watcher.rs`, `startup/watchers.rs`), which cuts watch count and avoids wasted startup/watch work for directories we would immediately ignore anyway.

### Daemon Protocol

The app uses the same authenticated JSON-line protocol on both platforms; only the daemon launch mechanism differs (WSL on Windows, native subprocess on macOS). The default endpoint comes from [`daemon::server::DEFAULT_PORT`](src-tauri/src/daemon/server.rs), and the CLI can override it with `--port`. See the [daemon protocol reference](docs/architecture/daemon-protocol.md) for the full method catalog.

**Events (daemon → app):**
- `file_changed` — watched file modified (triggers search re-index)
- `git_changed` — .git directory modified (triggers commit list refresh)
- `session_file_created` — new session handoff file detected

**Pairing rule:** `PROTOCOL_VERSION = 22`. App and daemon must match **exactly** — startup (`startup/setup.rs`, `ensure_expected_daemon_runtime` in `startup/daemon.rs`) and every reconnect path reject a mismatch.

**Bump rule:** bump the constant when a wire change requires the app to be rebuilt against the new daemon; a change to the `CliTool` wire vocabulary counts, because either side decodes the other's tool value as `Unknown`. Purely additive methods are the documented exception — they ship without a bump and degrade to `UNKNOWN_METHOD` on older daemons. `protocol.rs` pins the current value in one place (`protocol_version_is_pinned`), and the `protocol_version_excludes_daemons_*` family asserts only that it is above the last incompatible one: 7 for hub-owned focus, 9 for the degradation cursor, 10 for the Claude-only account methods, 11 for the retired Google tool value, 12 for the missing `grok` value, 13 for the retired Codex compaction mode, 14 for daemons without the deadline scheduler, 15 for daemons without team initialization, 16 for daemons without member operations, 17 for daemons without team-resume/reonboard operations, 18 for daemons without standalone team and roster operations, 19 for daemons with the retired stop-member wire pair, 20 for daemons without background self-heal/effort ownership, and 21 for daemons without the final writer intents. After changing the contract, run `just install-daemon`.

**Version history:** v11 replaced the Claude-only account methods with generic `list_accounts` / `project_transcript` and added `refresh_usage`; v12 replaced the retired Google tool value with `agy`; v13 added `grok`; v14 retired the Codex compaction mode method; v15 moved the managed-task deadline pass into the daemon; v16 moved team initialization into the daemon; v17 moved add/resume/stop into the daemon; v18 moved resume-team/reonboard into the daemon; v19 moved standalone team create/disband and roster edits into the daemon; v20 retired the redundant stop-member wire pair; v21 moved self-heal and effort passes into the daemon; v22 moved task-snapshot publication, live-presence reconciliation, and active-project mapping writes into the daemon.

**Commands (app → daemon, 53 callable methods — 54 constants, one of them unhandled):**
- `ping`, `shutdown`, `watch`, `unwatch`, `scan_sessions`
- `git_status`, `git_log`, `git_latest_commit_time`, `git_commits_in_range`, `git_commit_files`, `git_commit_diff`
- `file_tree`, `read_file`, `read_readme`, `read_asset`, `list_directory` (a method constant with no handler — not callable)
- `list_display_sessions`, `list_runtime_sessions`, `get_runtime_session_snapshot` (carries tmux focus + the degraded flag), `wait_session_updates`, `launch_session`, `stop_session`, `navigate_to_session`
- `get_project_tasks` (supports optional `scan_cycle_id` in protocol v6)
- `list_accounts`, `project_transcript`, `refresh_usage`, `resolve_launch_base` (generic account methods since v11; launch-base resolution is host-local)
- `list_workflow_runs`, `get_workflow_run`
- `coordination.initialize_team`, `coordination.initialize_status` (daemon-owned initialization since v16)
- `coordination.add_agent`, `coordination.add_agent_status`, `coordination.resume_member`, `coordination.resume_member_status` (daemon-owned member operations since v17)
- `coordination.resume_team`, `coordination.resume_team_status`, `coordination.reonboard`, `coordination.reonboard_status` (daemon-owned team operations since v18)
- `coordination.create_team`, `coordination.create_team_status`, `coordination.disband_team`, `coordination.disband_team_status`, `coordination.add_member`, `coordination.add_member_status`, `coordination.remove_member`, `coordination.remove_member_status` (daemon-owned standalone team and roster mutations since v19)
- `coordination.put_launch_settings`, `coordination.apply_task_effort`, `coordination.apply_task_effort_status` (daemon-owned background and task-arrival effort routing since v21)
- `coordination.publish_operational_snapshots`, `coordination.reconcile_live_presence`, `coordination.set_active_project_team` (final daemon-owned team-state writers since v22)

## Startup Sequence

The bootstrap chain runs on app launch (progress shown in `SplashScreen.svelte`). It is not a single serial chain: a synchronous setup lane runs to completion while daemon bootstrap and the heavier scans run concurrently.

**Synchronous lane** (`startup/mod.rs` → `startup/orchestration.rs`, blocks `setup()`):

1. **Paths and logging** — resolve data/Claude roots, open the JSONL sink
2. **Database** — open/create SQLite, run migrations
3. **Daemon fast path** — attempt a connect to an already-running daemon and validate its ping (`startup/setup.rs`); a failure here defers rather than blocks
4. **Watch bootstrap** — create the local watcher/event processor, reconcile activity-based local watches, and reconcile WSL daemon watches when applicable
5. **Harness hooks** — reconcile the managed Codex and Grok hook registrations; compaction itself runs through the native hook bridge, not a startup-owned worker
6. **Search open** — open the tantivy index

**Concurrent daemon bootstrap** (spawned first, runs on its own thread): ensure the bundled daemon is installed/updated (`ensure_bundled_daemon_installed`), auto-launch it when the fast path did not connect, and log `startup.daemon_protocol.checked`. Daemon readiness is **not** a prerequisite for the UI — the app comes up on the local provider and picks the daemon up when it lands.

**Background tasks** (spawned last, `startup/bootstrap.rs`): activity reseed (`last_activity_at` from the latest git commit per project), session scan/import, search index build, task scan from live CLI tool sources.

The watch bootstrap also ensures the dedicated Claude task-directory watch. In Tauri runtime, session updates are event-driven (`sessions-updated`) with a one-time startup hydrate; frontend-only mock mode uses polling fallback.

Claude task-directory watching follows the same override rules: default `~/.claude/tasks`, or `<TAURHAUS_CLAUDE_DIR>/tasks` when `TAURHAUS_CLAUDE_DIR` is set.

## Data Flow

```
User clicks project
  → Shell calls get_project, get_commits, get_file_tree (parallel IPC)
  → Rust reads SQLite (metadata) + libgit2 (commits) + filesystem (tree)
  → Frontend renders immediately

File changes detected
  → Native/local projects: app-owned `notify` watcher detects change
  → WSL projects: daemon watch bridge emits `file_changed` / `git_changed`
  → Shared scan/index policy applies saved ignore patterns during discovery and reindex
  → App updates tantivy index + refreshes affected views

CLI session state changes
  → Daemon bridge emits sessions-updated event to frontend
  → Frontend session store applies delta and refreshes indicators
  → Startup hydration and fallback polling use list_cli_session_snapshot, which says whether the list is an observation (fresh) or continuity data (degraded/cached/unavailable)
  → Backend scanner inspects /proc (Linux) or libproc (macOS)
  → Every surface derives a five-level signal from activitySignal.js:
    working / active / idle / uncertain / offline (+ confidence)
    attribution decides working vs uncertain; reused or dead panes read as offline
  → HoverCard shows full session details on hover
```

## Build System

All builds use `just` recipes. Both Windows and macOS builds happen natively on their target platforms — no cross-compilation.

```bash
just dev              # Tauri dev mode (hot-reload)
just dev-frontend     # Frontend-only dev server
just build-windows    # Sync to C:\taurhaus_build (override: TAURHAUS_WINDOWS_BUILD_DIR), then native NSIS build via PowerShell
just build-windows-sccache # Same as build-windows, but with Windows-side sccache auto-detection
just install-windows  # Silent-install the latest Windows NSIS build and verify installed exe hash
just build-macos      # Sync to Mac Mini, build ARM DMG via SSH
just build-macos-universal # Build universal macOS app via SSH
just check-quick      # Fast iteration gate: fmt + cargo check --tests + frontend typecheck/tests
just check            # Full gate: fmt + lint + typecheck + just test
just test             # All non-E2E tests (Rust + frontend)
just test-fast        # Fast lane: cargo check --tests + frontend tests
just test-visual      # Browser-mode visual screenshot lane for mocked component states
just test-e2e         # Linux Tier 1 E2E suite
just test-e2e-full    # Linux Tier 1 + Tier 2 E2E suite
just metrics          # Quality KPI report snapshot
just test-macos       # Run Rust tests on Mac Mini via SSH
just build-daemon     # Build the daemon binary
just install-daemon   # Build + install the daemon, preserving its env/args and restarting it
just build-mesh       # Resolve a mesh binary candidate (rebuilds the workspace when the commit drifts)
just mesh-verify-lock # The lock gate: verify the resolved binary against mesh.lock.json
just update-mesh-lock # Bump the mesh lock manifest (intentional entry point)
just bundle-mesh      # Bundle mesh into src-tauri/resources (lock-verified)
just install-mesh     # Lock-verified mesh install to ~/.local/bin
```

Manual visual review uses the Vite fixture host:

```bash
bun run dev:visual
```

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Git | libgit2 (in-process) | No CLI dependency, fast, full control |
| Search | tantivy | Rust-native, fast indexing, BM25 ranking |
| DB | SQLite | Single-file, zero config, rusqlite is solid |
| File watch | notify + ignore | .gitignore-aware, cross-platform |
| Frontend | Svelte 5 | Runes are excellent, minimal boilerplate |
| Styling | Tailwind v4 | @theme tokens, no CSS-in-JS overhead |
| IPC | Tauri commands | Type-safe, async, built-in |
| Task aggregation | Per-tool adapters | Each CLI tool stores tasks differently |
| Daemon comms | JSON-line over TCP | Simple, debuggable, mirrored networking in WSL2 |
| Platform dispatch | Compile-time `#[cfg]` | Zero runtime cost, compiler-enforced API contract |
| Terminal mgmt | Per-platform emulator enum | Same decision tree, platform-specific activation |
| Provider routing | Trait-based dispatch | Transparent local vs daemon routing |
| Unsafe code | `#![deny(unsafe_code)]` | One scoped exception for libgit2 init |
| Harness | Harness-native where available, tmux + mesh as the floor | Subscription Claude is CLI-only; the floor reaches any CLI |
| Tool extensibility | Capability slices behind one registry | Adding a CLI touches only where it differs |
| Model + effort | Separate fields end to end, validated per tool | Effort used to be dropped silently; now logged, never silent |
| App ↔ daemon | Exact protocol match on every connect; additive by default | A half-working pair is worse than a refused one; releases ship both |
| Accounts | Per-tool `AccountProvider`; project memory follows the account used last (pin → last used → global default → base-command selector → default dir) | Several subscriptions per host are normal; the account a project used last is the right default, and history lives in the account's dir |
| Usage | Per-tool `UsageProvider` on the tool's own endpoint or command; credentials read at request time, never logged, persisted or refreshed | Shows what the CLI's own `/usage`/`/status` shows; the credential stays the tool's |
| Google harness | Antigravity CLI (`agy`) replaces Gemini CLI | Google refuses the old client for individuals; `agy` has print/stream modes, hooks and a free `/usage` command |
| Fourth harness | Grok CLI (`grok`) | A live session registry, an `events.jsonl` turn lifecycle and a hook system make it the most observable CLI we run |
| Process inventory | argv element boundaries preserved on Linux | A prompt word must never be mistaken for a subcommand (`grok "help me"` vs `grok help`) |
| Degraded scans | Inert — last good snapshot stands | Treating a failed read as "no sessions" caused the blackouts |

## Further Reading

- [Harness model](docs/architecture/harness-model.md) — what taurhaus owns versus what the CLIs own; capability slices; pairing and stability rules
- [Data model reference](docs/architecture/data-model.md) — SQLite schema, tantivy index, filesystem layout
- [IPC reference](docs/architecture/ipc-reference.md) — all Tauri IPC commands with parameters and types
- [Daemon protocol](docs/architecture/daemon-protocol.md) — TCP JSON-line protocol specification
- [Coordination architecture](docs/coordination-architecture.md) — mesh orchestration subsystem details
- [Team templates guide](docs/team-templates.md) — role templates, presets, composition, history, and revert workflow
- [Platform abstraction](docs/platform-abstraction.md) — Linux/macOS dispatch implementation details
- [File rendering pipeline](docs/file-rendering-pipeline.md) — classification, caching, and rendering
- [Feature documentation](docs/README.md#features) — per-feature guides
- [CLAUDE.md](CLAUDE.md) — code standards, build recipes, development workflow
