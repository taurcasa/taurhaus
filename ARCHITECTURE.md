# Architecture

A condensed overview for contributors. For detailed references, see the [docs/ index](docs/README.md).

## System Overview

taurhaus is a cross-platform dual-process desktop application built with Tauri 2. The native GUI (Rust + Svelte 5) handles storage, git, search, native/local file watching, and UI-facing orchestration. A lightweight companion daemon handles WSL-side process scanning, tmux session management, and WSL file watching when the app is bridging into Linux workspaces, communicating with the app over TCP (JSON-line protocol on localhost:17233).

The daemon can run on all supported platforms, but it is only responsible for watch/process work that the app cannot do directly:

- **Windows**: The daemon runs inside WSL2 (launched via `wsl.exe`), where it has access to `/proc` for process inspection and the Linux filesystem where AI tools run.
- **macOS / Linux**: The daemon runs natively as a subprocess (launched from `~/.local/bin/taurhaus-daemon`) for session scanning / terminal control, while the app keeps ownership of native project file watchers.

![System Architecture](docs/images/system-architecture.jpg)

_Note: this infographic is stale and needs regeneration. The current codebase uses 89 IPC commands, and native/local file watching is app-owned rather than daemon-owned on all platforms._

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
| `Shell.svelte` | Main layout: titlebar, tab routing, theme, position memory |
| `Sidebar.svelte` | Project list, session indicators, context menu, hover cards |
| `src/lib/OverviewTab.svelte` | Project summary, README, recent commits, sessions |
| `src/lib/FilesTab.svelte` | File tree with syntax-highlighted code preview |
| `src/lib/GitTab.svelte` | Commit history, diffs, cross-tab navigation |
| `src/lib/TaskBoard.svelte` | Kanban board aggregating tasks from Claude Code, Codex, Gemini |
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

**Key patterns:**
- **Svelte 5 runes** (`$state`, `$derived`, `$effect`, `$props`) — no legacy stores
- **Derived theme tokens** — all color switching via `$derived` variables, never inline ternaries
- **`$bindable` position memory** — each tab exposes view state, Shell saves/restores per project
- **IPC layer** — `src/lib/ipc.js` is a thin compatibility re-export; the real Tauri `invoke()` wrappers and mock fallbacks live under `src/lib/ipc/`
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
| `session_scanner/` | CLI tool detection (process scanning, idle detection) |
| `task_scanner/` | Task aggregation from Claude Code, Codex, Gemini (`claude_index.rs` maps source_key -> project for robust scans) |
| `daemon/` | TCP protocol/server/event-listener/launcher code for the companion daemon |
| `daemon_api.rs` | App-facing daemon request wrapper used by commands and startup flows |
| `terminal/` | Terminal emulator management (Windows Terminal, iTerm2, etc.) |
| `claude_code/` | Claude Code project resolution, memory, teams |
| `provider/` | Concrete provider implementations plus path translation and `PlatformPaths` root authority |
| `project_provider.rs` | Shared `ProjectProvider` trait and provider selection boundary |
| `services/` | Cross-cutting services: relationships, scanner, project utilities, session import |
| `models/` | Shared data structures (Project, Session, ActivityState, etc.) |
| `config/` | Application configuration |
| `coordination/` | Multi-CLI team orchestration (behind `mesh-bridged-backend` feature flag) |
| `startup/` | Startup sequence orchestration (DB init, daemon connect, watcher/index bootstrap, task/session hydration) |
| `templates/adapters.rs` | Role import/export adapters, provenance, and field-mapping rules for external agent formats |
| `sentinels.rs` | Shared sentinel/fallback utilities used by startup and command flows |
| `event_processor.rs` | File/git event batching (300ms quiet window, 2s ceiling) |
| `daemon_lifecycle.rs` | Daemon auto-launch, reconnection, shutdown |
| `watch_targets.rs` | Activity-based planning for local and daemon watch reconciliation |

The crate enforces `#![deny(unsafe_code)]` — the single exception (libgit2 init) uses a scoped `#[allow]`.

### Provider Routing

The `ProviderState` routes each IPC operation to the right backend based on project path:

- **LocalProvider** — direct filesystem/git/search access. Used for native projects (macOS, Linux).
- **DaemonProvider** — proxies operations over TCP to the daemon. Used for WSL projects on Windows.

Both implement the `ProjectProvider` trait. The routing is transparent to command handlers — they call `provider_state.resolve(path)` and get the right implementation.

### Storage

- **SQLite** (`rusqlite`): 6 tables — `projects`, `sessions`, `session_activity`, `relationships`, `tasks`, `settings`. Source of truth for structured data.
- **tantivy**: Full-text search index over files, commits, sessions. Rebuilt from filesystem on startup.
- **Filesystem**: Source of truth for content. SQLite stores metadata; files are always read fresh.
- **Path overrides**: `TAURHAUS_DATA_DIR` overrides Tauri `app_data_dir()` resolution; `TAURHAUS_CLAUDE_DIR` overrides Claude-derived roots used by task/coordination watchers.

See [data model reference](docs/architecture/data-model.md) for schema details.

### IPC Commands (89)

Fine-grained, one command per operation. Frontend calls in parallel for speed. See [IPC reference](docs/architecture/ipc-reference.md) for the full command catalog.

Grouped by command module:
- **Projects** (12): includes `create_project`, registration flows, path/directory helpers, and first-run checks
- **Git** (4): commit lists + status + remote URL
- **Files** (5): file tree/read/readme/asset/path-type
- **Search** (3): search, rebuild, index status
- **Sessions** (3): list/latest/detail
- **Relationships** (4): list/create/dismiss/remove
- **Command Center** (6): launch/stop/navigate/list/record activity
- **Tasks** (6): board data + detail + archive + commit context helpers
- **Daemon** (5): platform/status/start/install checks
- **Mesh install** (2): check/install mesh binary
- **Settings** (2): get/update
- **Coordination** (15): team lifecycle + member lifecycle + live status + snapshot + preflight/availability
- **Templates** (14): role/preset CRUD, composition, storage status, history/diff/revert/flush
- **Logging** (1): frontend `console.*` is bridged to `frontend_log` IPC with structured payloads. Backend emits structured events into a JSONL sink at `taurhaus.log.jsonl`.

### Logging and Observability

Logging is structured and machine-first:

- **Canonical file**: `app_data_dir()/taurhaus.log.jsonl` (or `<TAURHAUS_DATA_DIR>/taurhaus.log.jsonl`).
- **Schema**: JSONL records with required keys (`ts`, `level`, `component`, `event`, `run_id`) plus event fields.
- **Sink architecture**: single-writer async pipeline (`src-tauri/src/commands/logging.rs`) with a bounded channel and one writer thread to prevent torn/interleaved lines.
- **Rotation policy**: size-based rotation (20 MB segment threshold) with retention pruning (7 days).
- **Frontend bridge**: `src/lib/logger.js` forwards structured payloads (`component`, `subsystem`, `event`, `message`, `context`) and emits drop telemetry (`frontend.logs.dropped`) under throttling.
- **Lifecycle instrumentation**:
  - startup phases: `startup.phase.started/completed/failed`
  - IPC lifecycle: `ipc.command.received/completed/failed`, `ipc.lock.wait`
  - daemon RPC lifecycle: `daemon.rpc.sent/response/timeout`

Correlation model used across events:

- `run_id`: per app run, attached to all records.
- `interaction_id`: frontend user interaction chain.
- `request_id`: frontend->backend IPC request lifecycle.
- `daemon_request_id`: backend->daemon RPC lifecycle.

Logging policy and level selection reference:
- [`docs/architecture/log-level-guidelines.md`](docs/architecture/log-level-guidelines.md)

### Coordination (Mesh View)

The `coordination/` subsystem powers multi-agent team orchestration and is gated by the `mesh-bridged-backend` Cargo feature (enabled by default).

- **State bootstrap**: `CoordinationState` is app-managed and lazily builds the orchestrator on first coordination IPC use (no startup hard dependency on mesh availability).
- **Persistence**: by default, team definitions are stored in `~/.claude/teams/<team>/config.json` (`TeamConfigStore`), while runtime attachment state lives in `~/.claude/teams/<team>/runtime/*.json` (`MemberRuntimeStore`). If `TAURHAUS_CLAUDE_DIR` is set, coordination uses `<TAURHAUS_CLAUDE_DIR>/teams/...` instead.
- **Pipelines**: `coordination/pipelines/` drives initialize, hot-add, and resume flows (validate -> create/resolve panes -> launch sessions -> mesh join -> daemon start -> onboarding delivery).
- **Resume lifecycle**: recovery supports both per-member resume (`coordination_resume_member`) and team-level cold-restart resume (`coordination_resume_team`). Team resume reuses the existing member-resume pipeline, resumes the lead first, then the remaining members, and returns structured per-member progress/failure results.
- **Snapshot classification**: project mesh snapshots and live team status use fast persisted config/runtime reads and classify team state as `none`, `active`, `degraded`, or `cold_resume`, which the frontend maps into recovery affordances like `Resume Team` and `Resume Offline (n)`.
- **Windows runtime safety**: Windows coordination and command-center background calls use hidden-window spawning for `wsl`/mesh/tmux invocations, and runtime ownership comparisons normalize Windows, WSL UNC, and Linux project-path forms before matching sessions, panes, and team members.
- **Liveness repair**: pane/process/daemon reconciliation runs in explicit recovery flows and the background self-heal loop, not on the UI-critical snapshot IPC path. Background self-heal uses an isolated orchestrator instance so it does not block foreground coordination IPC on the shared cached orchestrator mutex.
- **Mesh daemon hot-swap**: mesh installs are version-aware. Member daemon reconciliation checks executable identity and automatically replaces drifted daemons; bounded background self-heal does the same for drifted team-daemons, so normal upgrades do not require a manual `team-daemon stop/start/restart-all` cycle.
- **Runtime responsiveness**: Mesh steady-state polling stays on the fast snapshot path, and the frontend suspends hidden-tab refresh work, which avoids switch-away stalls and reduces Windows popup latency during runtime navigation.
- **Runtime/disband behavior**: disband removes persisted team state and performs best-effort teardown of managed agent resources (mesh membership, daemon processes, panes). Attach-existing leads are preserved only for Claude. Codex/Gemini leads currently validate as `launch_new` only, and mesh-backed or app-owned leads are torn down like other managed members.
- **Compaction reinjection**: Codex compaction now runs through an event-driven path: `CompactionSignalExtractor` tails active managed transcripts, `CompactionSignalWatcher` consumes the low-traffic signal log, and `CompactionSignalProcessor` resolves the attached member and appends a bounded reinjection card to the mesh inbox only when the operational snapshot still has resumable task context. Claude uses a `SessionStart(source=compact)` hook bridge that installs runtime-appropriate `.sh` / `.cmd` wrappers, normalizes current hook payload field variants, returns `hookSpecificOutput.additionalContext`, and logs standalone hook execution into the canonical JSONL sink.
- **Runtime UI architecture**: Mesh View uses a deterministic node canvas (`MeshCanvas`) backed by a pure layout engine (`meshLayout.js`) instead of force-sim layouts. Lead/agent boxes and cubic connection routes are computed together from container size and roster cardinality (single-row up to medium teams, split rows for larger teams), with explicit state mapping for setup/initializing/runtime.
- **Runtime interactions**: node detail actions (`MeshNodeDetail`) and runtime controls (`MeshRuntimeBar`) operate on the same live-status pipeline (`coordination_get_live_team_status`, add/remove/resume/disband IPCs), so canvas state and control-bar state stay consistent without a separate client-side data model. `MeshRuntimeBar` is also the shipped cold-restart/degraded recovery surface for team resume.

See [coordination architecture](docs/coordination-architecture.md) for deeper design details and decision history.

### Team Templates

The template system provides reusable role templates and team presets, with composition and history integrated into mesh setup.

- **Storage model**: built-in templates ship read-only from resources; user templates are YAML files in the app-managed templates directory (`roles/`, `presets/`, `_meta/`).
- **Storage roots**: template files live under the resolved app-data directory (`app_data_dir()/templates` by default, or `<TAURHAUS_DATA_DIR>/templates` when overridden).
- **Git-backed state**: template writes are committed through `TemplateStore`, enabling history (`templates_get_history`), diff (`templates_get_diff`), and forward revert (`templates_revert`).
- **Composition engine**: `templates::composition::compose_team` resolves lead and agent slots into a concrete roster, returning `warnings` and `validation_errors`.
- **Frontend pipeline**: `MeshSetupView` hosts `MeshTeamBuilder` as the primary setup surface (quick presets, role filters, drag/drop roster editing), while `TemplateBrowserPanel` and `TeamCustomizerPanel` remain the advanced catalog/history/edit flows. All of them still resolve into the same `InitializeTeamRequest` shape consumed by `coordination_initialize_team`.
- **Operational visibility**: storage mode, dirty state, and pending actions are exposed via `templates_get_storage_status`; manual flush is available via `templates_flush_pending`.
- **Role model**: roles are context-steering lanes, not capability labels. The key persisted fields are `focus_area`, `context_summary`, and `behavior_summary`, which steer what context a role keeps accumulating and where it should act independently versus escalate.
- **Lead tool support**: built-in and user templates can define lead roles for Claude, Codex, or Gemini. Frontend preset/customizer flows preserve the selected lead tool/model all the way into `coordination_initialize_team`; they do not silently backfill Claude defaults.
- **Role adapters**: Taurhaus can export/import Claude agent files, Copilot agent files, and instruction-only formats. Imported roles persist `provenance` and explicit `non_roundtrippable_fields` so lossy conversions stay visible to operators.

See [team templates guide](docs/team-templates.md) for user-facing workflows.

### Session Scanner

Detects running CLI tool sessions (Claude Code, Codex, Gemini CLI). The detection logic is platform-agnostic — it calls into the `platform/` module for OS-specific process inspection.

Two session views exist on purpose:

- `DisplaySession`: UI-safe session view for sidebar/runtime display
- `RuntimeSession`: transcript-aware session view for coordination, task sync, and compaction, preserving `session_id` and `jsonl_path`

| Tool | Detection | Activity Signal |
|------|-----------|-----------------|
| Claude Code | Process name + cwd | IO read bytes — hysteresis (2 consecutive above-threshold polls) |
| Codex | Process name + session file cwd | Session file mtime (10s threshold) |
| Gemini CLI | Process name + SHA-256 path hash | TCP socket state to :443 (ESTABLISHED = active) |

All detection uses 2-poll bidirectional hysteresis to prevent flickering.

Managed Codex compaction is no longer a legacy poll-and-inject loop. It flows through:

- `session_scanner/compaction_extractor.rs`
- `session_scanner/compaction_watcher.rs`
- `coordination/compaction_processor.rs`

**Platform details:**
- **Linux**: reads `/proc/PID/io` for IO bytes, `/proc/PID/fd` + `/proc/PID/net/tcp` for socket state
- **macOS**: uses `proc_pid_rusage` (libproc) for IO bytes, `lsof` for socket state

### Terminal Management

The terminal module manages launching and focusing terminal emulators with the correct tmux session. Same decision tree on all platforms — only the emulator options differ.

| Platform | Emulators | Default |
|----------|-----------|---------|
| Windows | Windows Terminal | `wt.exe -w taurhaus` |
| macOS | iTerm2, Ghostty, Terminal.app | iTerm2 (auto-detect fallback) |

macOS uses event-driven AppleScript to handle click-to-activate focus transitions reliably.

### Event Processing

File system events from `notify` arrive in rapid bursts (5–8 events per file edit). The event processor (`event_processor.rs`) uses **batch-and-flush** to coalesce them:

- **Quiet window** (300ms): batch flushes after no new events for 300ms
- **Max-wait ceiling** (2s): batch flushes regardless after 2s, preventing starvation

Result: one `project-files-changed` Tauri event per edit instead of 5–8. The frontend listener in Shell.svelte dispatches to the active tab via reactive props.

### Recent Performance Improvements

- **Git range queries**: range traversal uses a single-pass algorithm in `git/commits.rs` (covered by `single_pass_range_matches_dual_pass_output`), reducing duplicated revwalk work.
- **Search indexing**: file updates are batch-committed in `search/indexer.rs` (`update_file_batch_commits_once_for_multiple_files`) to avoid per-file commit overhead.
- **Session scanner CPU**: activity detection uses hysteresis and cadence widening (`session_scanner/proc_io.rs`, `daemon/session_activity.rs`) to reduce false active spikes and idle-loop churn.
- **Watcher registration**: ignored directories are pre-pruned before inotify registration (`fs/watcher.rs`, `startup/watchers.rs`), which cuts watch count and avoids wasted startup/watch work for directories we would immediately ignore anyway.

### Daemon Protocol

JSON-line protocol over TCP (localhost:17233). Same protocol on both platforms — only the daemon launch mechanism differs (WSL on Windows, native subprocess on macOS). See [daemon protocol reference](docs/architecture/daemon-protocol.md) for the full command catalog.

**Events (daemon → app):**
- `file_changed` — watched file modified (triggers search re-index)
- `git_changed` — .git directory modified (triggers commit list refresh)
- `session_file_created` — new session handoff file detected

**Commands (app → daemon, 23 methods):**
- `ping`, `shutdown`, `watch`, `unwatch`, `scan_sessions`
- `git_status`, `git_log`, `git_latest_commit_time`, `git_commits_in_range`, `git_commit_files`, `git_commit_diff`
- `file_tree`, `read_file`, `read_readme`, `read_asset`
- `list_display_sessions`, `list_runtime_sessions`, `wait_session_updates`, `launch_session`, `stop_session`, `navigate_to_session`
- `get_project_tasks` (supports optional `scan_cycle_id` in protocol v6)

## Startup Sequence

![Startup Sequence](docs/images/startup-sequence.jpg)

_Note: this infographic predates the current activity-based watch reconciliation and should be regenerated._

The bootstrap chain runs on app launch (progress shown in `SplashScreen.svelte`):

1. **Database** — open/create SQLite, run migrations
2. **Daemon** — connect to existing daemon or auto-launch (platform-specific)
3. **Watch bootstrap** — create the local watcher/event processor, reconcile activity-based local watches, and reconcile WSL daemon watches when applicable
4. **Activity reseed** — update `last_activity_at` from latest git commit per project
5. **Session import** — import any unimported session handoff files
6. **Search index** — build tantivy index from filesystem if empty
7. **Task scan** — seed task database from live CLI tool sources

Steps 3–7 run in background threads — the UI is interactive as soon as the database and daemon are ready. The watch bootstrap also ensures the dedicated Claude task-directory watch. In Tauri runtime, session updates are event-driven (`sessions-updated`) with a one-time startup hydrate; frontend-only mock mode uses polling fallback.

Claude task-directory watching follows the same override rules: default `~/.claude/tasks`, or `<TAURHAUS_CLAUDE_DIR>/tasks` when `TAURHAUS_CLAUDE_DIR` is set.

## Data Flow

![Data Flow](docs/images/data-flow.jpg)

_Note: this infographic predates the current local-vs-daemon watcher split and should be regenerated._

```
User clicks project
  → Shell calls get_project, get_commits, get_file_tree (parallel IPC)
  → Rust reads SQLite (metadata) + libgit2 (commits) + filesystem (tree)
  → Frontend renders immediately

File changes detected
  → Native/local projects: app-owned `notify` watcher detects change
  → WSL projects: daemon watch bridge emits `file_changed` / `git_changed`
  → App updates tantivy index + refreshes affected views

CLI session state changes
  → Daemon bridge emits sessions-updated event to frontend
  → Frontend session store applies delta and refreshes indicators
  → Startup hydration uses list_cli_sessions once (mock mode uses polling fallback)
  → Backend scanner inspects /proc (Linux) or libproc (macOS)
  → Sidebar shows tool indicator (active/idle)
  → HoverCard shows full session details on hover
```

## Build System

All builds use `just` recipes. Both Windows and macOS builds happen natively on their target platforms — no cross-compilation.

```bash
just dev              # Tauri dev mode (hot-reload)
just build-windows    # Sync to D:\, then run the native Windows NSIS build via PowerShell
just build-windows-sccache # Same as build-windows, but with Windows-side sccache auto-detection
just install-windows  # Silent-install the latest Windows NSIS build and verify installed exe hash
just build-macos      # Sync to Mac Mini, build ARM DMG via SSH
just build-macos-universal # Build universal macOS app via SSH
just check-quick      # Fast iteration gate: fmt + cargo check --tests + frontend typecheck/tests
just check            # Full gate: fmt + lint + typecheck + just test
just test             # All non-E2E tests (Rust + frontend)
just test-fast        # Fast lane: cargo check --tests + frontend tests
just test-visual      # Browser-mode visual screenshot lane for mocked component states
just metrics          # Quality KPI report snapshot
just test-macos       # Run Rust tests on Mac Mini via SSH
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

## Further Reading

- [Data model reference](docs/architecture/data-model.md) — SQLite schema, tantivy index, filesystem layout
- [IPC reference](docs/architecture/ipc-reference.md) — all Tauri IPC commands with parameters and types
- [Daemon protocol](docs/architecture/daemon-protocol.md) — TCP JSON-line protocol specification
- [Coordination architecture](docs/coordination-architecture.md) — mesh orchestration subsystem details
- [Team templates guide](docs/team-templates.md) — role templates, presets, composition, history, and revert workflow
- [Platform abstraction](docs/platform-abstraction.md) — Linux/macOS dispatch implementation details
- [File rendering pipeline](docs/file-rendering-pipeline.md) — classification, caching, and rendering
- [Feature documentation](docs/README.md#features) — per-feature guides
- [CLAUDE.md](CLAUDE.md) — code standards, build recipes, development workflow
