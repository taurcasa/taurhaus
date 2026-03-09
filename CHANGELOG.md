# Changelog

All notable changes to taurhaus are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.5.8] - 2026-03-09

Cross-project mesh delivery fix and UI cleanup.

### Fixed

- **Mesh cross-project agent delivery** — outbound messages (`mesh send`, `mesh task assign`) to agents registered in a different project now work correctly. Previously failed with "agent not found" even when the inbox file existed on disk. Mesh 0.2.7.

### Changed

- **Compaction diagnostics removed from UI** — the debug-level compaction reinjection audit surface has been removed from the mesh runtime view. Compaction health data remains accessible through backend logs and `just analyze-compaction`.
- **Mesh version pin** — bumped from 0.2.6 to 0.2.7

## [0.5.7] - 2026-03-09

Event-driven compaction pipeline, daemon CPU optimization, multi-CLI lead support, and role import/export. The compaction detection chain is now fully notify-based — no more polling in the middle of an event-driven architecture. Daemon steady-state CPU dropped from ~49% to ~31% of one core.

### Added

- **Event-driven compaction detection** — compaction signal extraction now uses inotify/notify on Codex JSONL files instead of 500ms polling, with offset persistence and paired-record normalization
- **CompactionSignalWatcher** — file-system watcher on the signal log with reconciliation fallback, replacing the old poll-based consumption loop
- **CompactionSignalProcessor** — extracted downstream delivery logic into a clean single-responsibility processor
- **Config-dir topology watching** — team watcher reconciliation driven by inotify on `~/.claude/teams/` instead of periodic directory scanning
- **Shared runtime-session cache** — single scanner path feeds both display and compaction consumers, eliminating duplicate scanning
- **Stale daemon binary detection** — app startup validates running daemon via `/proc/<pid>/exe` against installed binary, auto-restarts on mismatch
- **Claude compact hook observability** — pipeline health reports and structured audit events for compaction lifecycle
- **PlatformPaths authority** — centralized cross-platform path resolution for Windows, WSL UNC, and Linux path forms
- **Compaction analysis tool** — `just analyze-compaction` recipe for live pipeline debugging
- **Role import/export** — adapter schema for Claude Code and Copilot custom agent formats with round-trip provenance tracking
- **Multi-CLI lead roles** — non-Claude agents (Codex, Gemini) can now serve as team lead with tool-appropriate presets and lifecycle
- **Unified team roster query** — single join point for member runtime state across all coordination consumers
- **Imperative resume card** — post-compaction reinjection card explicitly instructs agents to continue working rather than summarizing metadata
- **Compaction reinjection audit surface** — mesh runtime view shows compaction detection and delivery events

### Fixed

- **Inbox corruption handling** — corrupt inbox files are now quarantined instead of silently treated as empty, preventing delivered messages from being hidden
- **Paired Codex compaction boundaries** — extractor collapses `compacted` + `context_compacted` records within 2s into a single delivery
- **Liveness reconcile session_id overwrite** — reconciliation no longer clobbers existing session_id when backfilling missing jsonl_path
- **Daemon offline indicator** — recovers correctly when daemon comes back online
- **Cross-platform path normalization** — Codex normalizer and config aliases handle Windows ↔ WSL ↔ Linux path translation

### Performance

- **Daemon CPU ~31% steady-state** (down from ~61% pre-optimization, ~49% after first pass) — removed redundant 500ms compaction scan loop and switched to diff-based downstream fanout
- **Diff-based daemon fanout** — session activity exports and extractor updates only pushed when data actually changes, not every 500ms tick

### Changed

- **Mesh version pin** — bumped from 0.2.5 to 0.2.6 (selective mark-read: only marks displayed messages, not entire inbox)
- **Session type split** — `DisplaySession` and `RuntimeSession` are now separate types with distinct responsibilities
- **Legacy compaction module removed** — deleted `session_scanner/compaction.rs` (superseded by event-driven pipeline)
- **Dead defensive branches removed** — `EmptyAdditionalContext` skip, pane foreground guard, JSONL boundary guard all removed as irrelevant for inbox-file delivery

### Reliability

- **Flaky integration tests hardened** — TCP server tests (daemon_client, event_listener, session_listener) now use port-readiness waits instead of fixed sleeps

## [0.5.6] - 2026-03-08

Tmux foreground detection, non-blocking team initialization, and backend-owned role hydration. Mesh 0.2.5 with serde flatten to preserve extension fields.

### Added

- **Sidebar foreground indicator** — two horizontal brand-400 lines (top + bottom) highlight the project whose tmux window is currently focused, with 150ms fade-in animation
- **Tmux focus detection backend** — after-select-window hooks write a focus file; backend watches it and emits `foreground-project-changed` events to the frontend
- **Optimistic foreground clicks** — clicking a project immediately sets the foreground indicator while the backend event catches up

### Fixed

- **Tmux hooks on Windows** — hook commands now route through `wsl.exe` so they can write the focus file from the WSL tmux server to the correct Windows app data directory
- **Stale tmux hooks** — hooks are force-reinstalled on every app startup, clearing leftovers from previous versions
- **Focus file path drift** — canonicalized to `app_data_dir()` only, removing the `dirs::data_local_dir` fallback that caused path mismatch on Windows
- **App freeze during team init** — `coordination_initialize_team` converted to async with `spawn_blocking`, keeping the UI responsive
- **Preset role metadata missing on hover** — backend now hydrates role metadata (focus_area, context_summary, behavior_summary, instructions, behavioral_contract, capabilities) from template storage when the frontend sends minimal payloads
- **Mesh stripping extension fields** — mesh 0.2.5 uses serde flatten on TeamConfig/Member types, preserving taurhaus-specific role metadata through heartbeat config rewrites

### Changed

- **Backend-owned preset resolution** — frontend sends minimal preset init payload (preset ID + agent names + project bindings); backend resolves full role definitions from template storage via the composition engine
- **Mesh version pin** — bumped from 0.2.4 to 0.2.5

## [0.5.5] - 2026-03-07

Security, code quality, and performance hardening release. Full security and quality audits drove targeted fixes across both taurhaus and mesh. Mesh tab navigation is now fully non-blocking on all platforms.

### Security

- **Mesh PID file validation** — `timer-cancel` now verifies process identity before kill, preventing forged PID files from terminating unrelated processes (mesh 0.2.4)
- **Daemon singleton locking** — exclusive lock files (`create_new` + lifetime-held) replace the racy check-then-create PID file pattern, preventing duplicate daemon instances (mesh 0.2.4)
- **Session activity stats preserved** — `stopPolling()` now flushes tracker data before clearing, preventing data loss during daemon bridge handoff

### Refactored

- **Shell.svelte decomposition** — extracted navigation helpers and event wiring into `src/lib/shell/navigation.svelte.js` and `src/lib/shell/events.svelte.js` (1302 → ~1200 LOC)
- **command_center.rs split** — domain-based submodules (`session_listing`, `launching`, `navigation`, `activity_tracking`) with thin `#[tauri::command]` wrappers
- **Doc/metadata drift fixed** — corrected IPC command count, removed stale db placeholder recipes, cleaned up duplicate lockfile

### Performance

- **Non-blocking mesh live status** — `coordination_get_live_team_status` converted from synchronous to async Tauri command with `spawn_blocking`, eliminating tab-switch blocking entirely
- **Mesh runtime refresh coalescing** — deferred refresh and periodic polling share an in-flight gate, preventing duplicate ~2.5s backend calls from stacking
- **Stale refresh cleanup** — in-flight promises are severed on tab deactivation, preventing request accumulation across rapid tab cycling
- **Project switch debounce** — 25ms batch window coalesces rapid project switches so only the final IPC fan-out fires

### Changed

- Mesh binary bumped to 0.2.4 (PID file security hardening, daemon singleton locking)
- `just check` output now tees to `.check-logs/` with 5-file auto-rotation

## [0.5.4] - 2026-03-07

Daemon reliability and team lifecycle release. Automatic hot-swap eliminates manual runbooks for mesh upgrades, background self-heal no longer freezes the UI, and cold restart recovery lets you pick up running teams after an app restart.

### Added

**Team Resume & Cold Restart Recovery**
- Resume Team banner appears when the app detects a previously running team — one click to reconnect to existing agent panes
- Snapshot classification (active panes / stale daemons / cold start) drives the recovery flow
- IPC commands for team resume with progress reporting
- Lifecycle header replaces the old runtime warning banner with richer state context

**Daemon Hot-Swap**
- Automatic version drift detection — background self-heal compares running daemon binary against bundled version
- Atomic binary install: temp-stage + `mv -f` prevents "text file busy" and partial-copy corruption
- Full daemon cycling after upgrade: team-daemon self-restart + member daemon restart
- Works on both Linux and macOS (no `/proc` dependency — uses `ps`/`kill` universally)

**Mesh Canvas Polish**
- Cross-project member distinction — agents working on other projects get a visual treatment
- Runtime role hover card with context-steering metadata
- Sidebar session grouping with team connector rail and stacked tool logos

**Role System Overhaul**
- Context-steering model replaces capability-centric role definitions
- Role summary fields propagated through the full coordination pipeline
- Frontend role editor and catalog updated to show context-steering metadata

### Fixed

- **30-second UI freeze** — background self-heal held the shared IPC mutex, causing brief grey-out. Now uses an isolated orchestrator instance.
- **Windows process spawning storm** — Mesh view triggered rapid `mesh` process launches on every poll. Added in-flight guard + console window suppression.
- **Windows switch-away stall** (~5.1s → ~1.3s) — eliminated blocking runtime probes when switching away from Mesh tab.
- **Liveness reconciliation** — stale `SessionDead` records repaired during reconcile pass; dead member daemons restarted for active panes.
- **Daemon pidfile race** — resume daemon start now verifies pidfile before persisting `daemon_pid`, preventing ghost PID entries.
- **Mesh discovery on Windows** — path normalization for snapshot discovery, skipped liveness probes on Windows snapshot path.
- **Agent detail popup** — opens immediately instead of waiting for data, auto-closes after actions.
- **Mesh view remount** — eliminated unnecessary component remount on project switch.
- **Stale team folder warnings** — silenced noisy discovery warnings for team folders without configs.
- **serde_yml deprecation** — replaced unmaintained `serde_yml` with `serde_norway`.

### Changed

- Mesh binary bumped to 0.2.3 (canonical HOME resolution for sandboxed agents, cross-platform daemon process identity, `ps`-based process checks on macOS)
- Unified project mesh snapshot path across all platforms — single code path replaces platform-gated runtime probes
- Sidebar team indicators refined: standalone icon style, CSS grouping, connector rail

### Performance

- Instant Mesh tab render via cache-first snapshot with background refresh
- Instant project switch away from Mesh view (no blocking teardown)
- Agent detail popup latency halved (212ms → 102ms)

## [0.5.3] - 2026-03-06

Bug fix release targeting team creation and agent communication reliability.

### Fixed

- **Codex model flag rejected** — `gpt-5.4-high` (hyphenated) was rejected by ChatGPT accounts. Changed to `gpt-5.4 high` (space-separated) with backward-compat normalization for legacy values.
- **Claude hot-add "no inbox"** — Adding a Claude Code agent to a running team failed with "agent not found (no inbox)" because the add-agent pipeline launched the agent before registering it in team config. Fixed by pre-registering the member before pane creation.

### Changed

- **Shell depth treatment** — Subtle sealed-panel effect on main content area (faint top highlight, inner border, deeper shadow) and material gradient on dark teal frame. Both dark and light modes. No blur or translucency.

## [0.5.2] - 2026-03-06

Mesh canvas reliability release. Structurally resolves the recurring connection routing bug class by extracting a pure layout engine, adds a visual testing lane, and redesigns the project HoverCard.

### Added

**Mesh Layout Engine**
- Pure `meshLayout.js` module that computes node placement and connection routing in one coordinated pass
- Replaces scalar `bend` with explicit cubic control points (`start`, `end`, `control1`, `control2`)
- 34 layout invariant tests covering 1–8 agents, row collapse, non-crossing ordering, center-agent degeneracy, and viewBox bounds
- Architecture concept doc: `docs/architecture/mesh-canvas-layout-engine-concept.md`

**Visual Testing Lane**
- Vitest Browser Mode with Playwright provider — 34 screenshot tests across 5 component specs (MeshCanvas, HoverCard, MeshNodeDetail, Sidebar, smoke) in 7.6s
- Fixture modules with named scenarios and shared builders for each component
- Standalone Vite fixture host (`visual-host.html`) for manual component browsing with mock data
- `just test-visual` recipe and `bun run dev:visual` script
- Testing guide: `docs/testing-guide.md`

**HoverCard Redesign**
- Verdict-first layout: header → attention verdict → evidence stack → optional relationship
- Prefers session summary over commit list, surfaces unresolved handoff items
- Conversational copy replacing technical/formal phrasing
- Dark/light theming via `$derived` tokens, 100ms/70ms enter/exit hover timing

### Fixed

- **Mesh connection routing** — 4 rounds of connection bugs (#395, #400, #401, #412) structurally resolved by the layout engine extraction. No more ad-hoc bend patching.
- **Center agent invisible line** — straight line fallback when bezier would collapse to near-zero horizontal bend
- **Lead anchor fan-out** — connections now use distinct anchor points spread across the lead card instead of originating from a single center point
- **Connection curve overlap** — outer agents route outward, center agents stay straight
- **Focus button wiring** — mesh Focus button now navigates to the correct tmux pane
- **Onboarding test assertion** — aligned e2e test model expectation with gpt-5.3 fixture

### Changed

- `MeshConnection.svelte` is now a dumb renderer — receives pre-computed control points, no longer computes bezier curves internally
- `MeshCanvas.svelte` delegates all layout to `meshLayout.js` — inline row-packing, anchor fan-out, and bend logic removed
- Default Codex model updated to `gpt-5.4-high`
- Mesh binary bumped to 0.2.1 (rejoin reactivation fix for `mesh send` after daemon restart)

### Documentation

- Synced `ARCHITECTURE.md`, `CLAUDE.md`, and `AGENTS.md` with current implementation details (80 registered IPC commands, updated module/build references)
- Refreshed `docs/architecture/ipc-reference.md` to match the active command surface
- Updated `docs/coordination-architecture.md` to point to the practical orchestration direction and explicitly mark the v0.2 protocol design as archived
- Layout engine pipeline retro and visual testing pipeline lessons in `docs/retros/`
- HoverCard vision and UI concept design documents
- Mesh canvas library assessment (dagre, ELK — neither adopted; custom engine chosen)

## [0.5.1] - 2026-03-06

The largest release since the project started. 81 commits spanning a complete observability overhaul, architecture refactoring on both sides of the stack, a new coordination subsystem, Windows stability fixes, toolchain migration, and the first bundled mesh CLI with team-daemon support.

### Added

**Structured Logging Pipeline**
- JSONL structured log sink with per-event context, replacing unstructured stderr logging
- Complete IPC command lifecycle instrumentation — all 80 registered commands emit start/finish/error spans
- Startup and daemon bootstrap events with phase-level timing
- Watcher and event processor structured instrumentation with batch metrics
- Frontend log bridge rewritten with interaction IDs and structured payloads (`console.*` → IPC → JSONL file)

**Stall Detection & Coordination**
- New `StallDetectorService` — detects agents that stop making progress on assigned tasks
- Signal fusion scaffolding: combines session scanner signals, pane status checks, and mesh task state
- Escalation delivery with suppression rules and rate limiting to prevent alert fatigue
- Per-member activity snapshot export for mesh IdleMonitor integration

**Mesh 0.2.0 Integration**
- Bundled mesh 0.2.0: IdleMonitor (30s poll cycle), `mesh task assign`, `mesh nudge`, actionable message lint, centralized team-daemon
- Mesh version lock manifest (`mesh.version` + `mesh.lock.json`) tracked in git with build-time verification
- New build recipes: `mesh-verify-lock`, `update-mesh-lock`, `bundle-mesh`

**E2E Test Infrastructure**
- Failure artifact bundles collected automatically in `afterTest` hook (screenshots, logs, DOM state)
- Template CRUD UI E2E coverage with slide-over interaction helpers
- Annotated regression tests for sessionStore and bridge-missing fallback cases

**Developer Tooling**
- `just check-quick` fast feedback recipe: `cargo fmt` + `cargo check --tests` + frontend typecheck + frontend unit tests
- Practical orchestration design doc (auto-idle detection + communication quality patterns)

### Changed

**Architecture Refactoring — Backend**
- Split `coordination/pipelines.rs` (2541 LOC) into domain-specific stage modules (`initialize`, `members`, `lifecycle`, `helpers`)
- Split `templates/storage.rs` into focused modules (`roles`, `presets`, `git`, `state`)
- Extracted `sentinels.rs` — shared watch-target planner module for watcher reconciliation
- Startup refactored into phased bootstrap pipeline (`bootstrap`, `daemon`, `search`, `watchers`)
- IPC error envelope standardized with `SanitizeErr` trait for user-safe error surfaces
- Project identity normalization centralized across command handlers
- Coordination command overloads collapsed to canonical internal implementations
- Template mutations moved behind shared `mutate_and_commit` scaffold with store API

**Architecture Refactoring — Frontend**
- `MeshTab` decomposed: extracted `MeshRuntimeView`, `meshTabController.svelte.js`
- IPC layer split from monolithic `ipc.js` into domain modules (`client`, `projects`, `sessions`, `tasks`, `templates`, `coordination`, `system`)
- Context providers extracted to `src/lib/context/` (`ProjectContext.js`, `SessionContext.js`)
- Shell theme and mesh gate/notification modules extracted into focused files
- IPC payload normalizers consolidated into shared module

**Toolchain & Infrastructure**
- Migrated from npm to bun for all JS tooling (`bun install`, `bun run`, `bunx`)
- Replaced `notify-debouncer-full` with direct `notify`, migrated `serde_yaml` → `serde_yml`
- Replaced bash resource monitor with Python implementation

**Coordination Protocol**
- Removed abandoned v0.2.0 orchestration protocol assumptions from `CLAUDE.md` and `AGENTS.md`
- Documented practical orchestration direction grounded in available signals (file-based mesh + real-time taurhaus)

### Fixed

**Windows Stability**
- App crash on project selection with large workspaces — watcher reconciliation moved off IPC thread
- Daemon connection stall — removed blocking reconnect, added IPC timeout with regression tests
- IPC camelCase/snake_case normalization — root cause of mesh setup wizard hang and E2E failures
- P1 regressions: retry thread cap, stall detector timeout, removed panicking `expect` calls

**Frontend**
- Atomic project view reveal restored (parallel loading with `Promise.all`, no waterfall)
- Unified content-enter transitions across all tab views
- Search overlay layout fixed — CSS conflicts were breaking fixed positioning
- Session scanner camelCase payload normalization + polling fallback for missing bridge events
- User-facing error messages normalized, settings save feedback added
- Accessibility improvements: theme tokens, focus management, error surfacing

**Backend**
- Release builds now log at INFO level (was ERROR-only without `RUST_LOG`)
- Silent error swallowing eliminated in event processor, daemon lifecycle, logging pipeline
- Daemon watch gitignore filtering, template concurrency, watcher classification fixes
- Logger recursion guard + daemon error variant reclassification
- Post-compaction idle prevention in onboarding templates

**E2E**
- Config group stall eliminated — settings fast-paths, resilient clicks, driver cleanup
- Fresh selector in `ensureMainApp` to avoid stale element handles

### Security

- Bumped DOMPurify to 3.3.2+ to fix XSS vulnerability (GHSA-v2wj-7wpq-c8vv)

## [0.5.0] - 2026-03-05

### Added

**Mesh View Redesign (M1-M3)**
- M1 foundation: node-canvas primitives, `SlideOver`, and mesh design-token groundwork
- M2 integration: `MeshTab` orchestration flow, slide-over panel integration, and card-style agent presentation
- M3 runtime: initialization/runtime animations, `MeshRuntimeBar`, and runtime-mode visual continuity
- Designer-approved finish pass: shadows, glow, gradients, and a full-bleed canvas/surface overhaul
- Expanded light-mode variants for mesh connections/surfaces to preserve contrast in non-dark themes

**Team Composition & Presets**
- New built-in role template: `codex-architect` for structural decision ownership
- New built-in team preset: `standard-team` (orchestrator + architect + two developers + UI specialist)
- Preset setup now resolves member names from slot `name_pattern` overrides and role `default_name_pattern` fallbacks, producing role-appropriate names (for example `architect`, `developer1`, `developer2`, `ui-specialist`) instead of generic `agent-N`

**Project Bootstrap**
- Create New Project flow in `AddProjectModal` for creating and registering projects directly from the app

### Changed

**Performance Sprint (Backend + Frontend)**
- Daemon IPC latency reduced from **44ms to 0.114ms**
- Git timeline/range queries moved to single-pass scans with TTL memoization
- Session-scanner cycle cost reduced with batched search-commit queries
- Frontend rendering optimized with virtualization for heavy lists, bounded caches, and lazy-loaded markdown/Shiki paths
- Template IPC calls deduplicated with stricter stale async result guards

**Backend Error Handling & Core Hygiene**
- Error handling overhauled around `SanitizeErr` for user-safe error surfaces
- Mutex poison recovery and silent-drop logging added to improve degraded-path resilience
- IPC casing normalization and targeted deduplication landed across shared paths

**Frontend Reliability & Template UX**
- Async guard hardening applied across file/markdown/search surfaces (including `CodeViewer`, `MarkdownRenderer`, and `SearchOverlay`) to prevent stale UI states
- Template CRUD surfaces refined with role-aware agent forms, improved IPC wrappers, and Gemini session detection correctness
- Built-in role behavioral contracts updated for clearer specialization:
  - Orchestrator: stronger delegation-first execution contract
  - Codex developer: explicit architect escalation for structural decisions
  - Gemini UI specialist: frontend-only scope boundary

**Quality & Test Infrastructure**
- Added `just metrics` KPI reporting lane
- Clarified `just test` vs `just check` semantics and added faster test workflow
- Frontend branch coverage improved from **54% to 65%**

### Fixed

- Files tab loading regressions after project switches and metadata-only updates (stuck/blank first-load cases)
- Window-state restore behavior for undecorated windows (height/restore correctness on reopen)
- Mesh visual regressions introduced during redesign (overlay behavior, runtime-surface continuity, and light-mode connection contrast)
- Platform review hardening findings, including macOS `/proc` guard handling
- Onboarding E2E flakiness via real temp project directories and deterministic harness improvements

### Documentation

- Added design-first workflow guide: `docs/design-workflow.md`
- Refreshed release docs for v0.5.0 scope across architecture/contributing/coordination surfaces (`ARCHITECTURE.md`, `CONTRIBUTING.md`, and coordination documentation updates)

## [0.4.5] - 2026-03-04

### Added

**Team Template System**
- Git-backed template command surface: role/preset CRUD, composition/validation, storage status, history, diff, revert, import, and pending flush endpoints (`templates_*`)
- Template catalog and composition UI: role/preset browsing, quick compose preview, editable roster composition, and mesh-setup integration
- Template history UX: global/selected-template commit history, commit detail metadata, diff hunk view, dirty-state indicator, and revert action
- Template E2E workflow coverage (`e2e/specs/templates.js`) for catalog flow, composer validation, and role/preset CRUD paths

**Role Context Delivery**
- Template-launched agents now receive role-specific instructions, behavioral contract, and capabilities in their onboarding
- Dual delivery path: Codex/Gemini agents receive role context in tmux onboarding message; Claude agents receive it as first team message after session detection
- Role metadata persisted in member config for restart resilience

**E2E Isolation**
- Session-level E2E sandboxing in WebdriverIO: per-session temp roots for app data + Claude data, plus an isolated fixture git project for deterministic onboarding

### Changed

- Mesh setup now supports template-first onboarding paths (preset quick-select, catalog browse, custom composition) while preserving manual blank-slate fallback
- Frontend IPC layer migrated template calls from temporary mock command names to backend `templates_*` commands
- Runtime path resolution now supports `TAURHAUS_DATA_DIR` (app data) and `TAURHAUS_CLAUDE_DIR` (Claude tasks/teams roots) overrides for isolated runs
- E2E recipes (`just test-e2e*`) are now safe-by-default and do not auto-run `install-daemon`; opt in with `E2E_INSTALL_DAEMON=1`
- Template backend writes now use an atomic mutation pipeline (`mutate_and_commit`), shared agent-slot validation, and direct ID-path lookups for role/preset reads
- Compose IPC request handling now accepts camelCase/snake_case DTO aliases for agent slot fields
- Template UI polish: accessibility labels on agent controls, duplicate-name submit enforcement, sequence guards for async preset/diff races, save-as-preset slug validation, 12px label sizing, and shared derived surface tokens

### Fixed

- Template import failures now report detailed parse/validation context for role and preset attempts instead of a generic invalid-file error
- Template catalog CLI tool filter now correctly reads nested `defaults.cli_tool` field; previously all templates appeared as Claude regardless of actual tool

### Documentation

- Added `docs/team-templates.md` user guide for role templates, team presets, composition, history, and revert workflows
- Updated architecture docs (`ARCHITECTURE.md`, `docs/coordination-architecture.md`) for template storage, composition, and coordination integration points

## [0.4.4] - 2026-03-04

### Added

**Agent Resume Lifecycle**
- Resume offline members: `coordination_resume_member` pipeline with `Continue` and `Fresh` modes
- Resume contracts: `ResumeContextMode`, `ResumeMemberRequest`, and `ResumeAgentReport` IPC types
- MeshTeamRoster resume UX: Resume action on offline rows with mode-aware relaunch

**Liveness Reconciliation**
- Write-on-drift liveness reconciliation in live status queries (`reconcile_team_liveness`)
- Shell-return drift detection: `pane_is_shell` checks `#{pane_current_command}` for shell fallthrough
- Offline drift daemon cleanup: non-Claude `daemon_pid` check/terminate/clear behavior

**Documentation & Infographics**
- Regenerated 7 infographics for accuracy (mesh-view-lifecycle, coordination-architecture, system-architecture, data-model, task-aggregation, file-rendering-pipeline, build-release-pipeline)
- New mesh-resume-liveness-sequence infographic: end-to-end sequence diagram for resume and write-on-drift flows
- Updated ARCHITECTURE.md, CONTRIBUTING.md, mesh-view-design.md, coordination-architecture.md for resume and liveness features
- Added feature-matrix.md and phase-4-architecture.md documentation
- Security audit report for v0.4.3 release

## [0.4.3] - 2026-03-04

### Added

**Task Identity & Session Attribution**
- Task identity model: `source_key` column (migration 009) disambiguates tasks from different Claude source directories (session-id vs team-name)
- Codex/Gemini session identity: persist session ID from JSONL metadata with filename-stem fallback
- Transcript-derived commit time windows: use JSONL session timestamps instead of DB persistence timestamps for accurate commit association
- Structured enrichment warnings: surface commit-enrichment failures in API response instead of silently returning zero counts

**Scan Robustness & Performance**
- Tri-state scan outcomes: `Data` / `DefinitivelyEmpty` / `Unavailable` prevent false task pruning on degraded I/O
- Targeted project invalidation: task file changes rescan only affected project, not all registered projects
- Per-cycle index caching: `ClaudeSourceIndex` and session list built once per scan cycle and reused across projects
- Diff-based event emission: `project-tasks-changed` only emits on meaningful task count/status changes

**Mesh Agent Management**
- Agent removal from existing teams: Remove action on non-lead agents in mesh roster with confirmation dialog
- `RemoveAgentReport` with per-step outcomes (daemon terminate, mesh leave, pane kill, config/runtime cleanup)
- Lead-removal guard: backend hard-blocks removing the team lead
- Pane ownership pre-check: verify tmux pane belongs to expected session before killing
- Team-lead removal notification: lead-only mesh notification when an agent is removed (who, by whom, cleanup status)

**UI Task Board Polish**
- Archive metadata display: `archived_reason`, `state_changed_at`, `last_status` surfaced in SessionHistory and TaskDetailPanel
- Live session history refresh: subscribe to `project-tasks-changed` while history tab is active
- Deterministic task column sorting: in_progress by recency, pending by dependency count, completed by update time
- `active_form` secondary text on in-progress task cards
- Enrichment warning badge on sessions with suspect commit counts

### Fixed

- Always run task reconciliation on startup even for empty scans
- Tri-state enforcement on degraded I/O: read_dir/parse failures map to `Unavailable` instead of `DefinitivelyEmpty`
- Async event listener cleanup race in TaskBoard and SessionHistory (unmount before listen resolves)
- Sort tiebreaker: stable secondary key prevents ordering jitter when primary sort keys tie
- Archived task detail: targeted DB query replaces O(n) linear scan
- Generation map bounded with retention-window eviction (prevents unbounded memory growth)
- Inline dark/light ternaries in SessionHistory extracted to `$derived` tokens
- Add-agent project path: pass explicit cwd to `join_mesh` instead of falling back to app data directory
- Roster update idempotent: if member already exists from join step, update entry instead of failing on duplicate
- Skip transcript lookup for team-scoped Claude sessions: team names have no JSONL transcript, use task timestamps directly without warning
- Rust implementation gate documented and enforced via `just agent-quality` (`cargo fmt` + `clippy -D warnings` + `cargo check --tests`)

## [0.4.2] - 2026-03-04

### Added

**Unified Task Management**
- Unified task scanner: scan all `~/.claude/tasks/` subdirectories with index-based classification (session-ID and team-name dirs)
- Claude source index: maps session IDs and team names to project paths via live sessions, JSONL fallback, and team configs
- Snapshot-based task archiving: reconcile DB against disk on every scan cycle, including empty scans
- Archive metadata: `state_changed_at`, `last_status`, `archived_reason` fields with migration 008

### Fixed

- Handle empty git repos gracefully in recent commits (return empty list instead of error for unborn HEAD)

## [0.4.1] - 2026-03-04

### Fixed

**Markdown Link Navigation**
- Fix broken relative link clicks in rendered markdown (undefined `resolveImagePath` function)
- Tab-aware path resolution: Overview tab resolves links against README, Files tab resolves against selected file
- Cross-file anchor navigation: clicking `docs/foo.md#section` now opens the file and scrolls to the heading
- Directory links (`docs/`): expand in file tree and open README.md if present
- Platform route links (`../../releases`, `../../issues`): detect above-root paths, resolve via git remote URL, open in system browser
- Add `check_path_type` IPC command for file vs directory classification
- Add `get_remote_url` IPC command with SSH-to-HTTPS remote conversion
- Fix daemon test assertion for empty session store version

## [0.4.0] - 2026-03-03

### Added

**Mesh View — Multi-Agent Team Coordination**
- Complete Mesh tab: setup form, initialization progress tracker, live team roster, and team cleanup panel
- Coordination backend: orchestrator with lifecycle management, delivery routing, and audit events
- Coordination stores with advisory file locking and domain types
- Coordination IPC commands for team CRUD, agent management, and live status
- Mesh CLI bundling: `install-mesh` recipe builds and bundles the mesh binary into app resources
- MeshAvailabilityGate: prerequisite checker (mesh CLI, tmux) before team setup
- MeshSetupForm: agent roster builder with per-agent tool/model/project selection and custom chevron selects
- MeshInitProgress: 7-step initialization tracker with real-time IPC progress events
- MeshTeamRoster: live member status (active/idle/offline) with 5s auto-refresh and tool brand icons
- Team cleanup panel: discover and disband existing teams before starting new ones
- Team-conflict recovery: "Open Existing Team" and "Disband & Retry" actions when init hits a name collision
- ConfirmDialog component: themed `<dialog>` replacement for native `window.confirm()` — backdrop, Escape key, danger/default variants
- Per-agent CLI warnings surfaced in mesh preflight
- Coordination event pipeline with drift reconciliation
- Coordination runtime boundary refactoring and onboarding delivery stabilization

**Session Management**
- Daemon streaming: session updates via versioned long-poll API replacing Tauri polling
- Activity attribution model: distinguish tool-originated vs unattributed project activity
- Session indicator hydration on Tauri startup
- Codex activity disambiguation per process via session file mtime
- Unattributed project activity detection in session indicators

**Markdown & Rendering**
- Mermaid diagram rendering in markdown pipeline with fallback on parse errors

**Documentation**
- Architecture reference docs and updated ARCHITECTURE.md
- Feature documentation, UI documentation, operations documentation
- Security documentation with risk register and audit history
- Documentation guidelines and index
- Teal-themed infographics for architecture, file rendering pipeline, session management, and workflow
- Data model ERD and image optimization script
- Session activity docs aligned with daemon event stream

**Infrastructure**
- Persist dark/light theme selection across app restarts
- Remember window position and size across restarts (tauri-plugin-window-state)
- Unified coordination pane creation on native tmux layouts
- E2E: install daemon before all e2e runs

### Changed
- Coordination modules decomposed: types, pipelines, validation extracted from monolithic files
- Backend module decomposition: lib.rs split into bootstrap + event_processor + daemon_lifecycle
- Server.rs decomposed into handlers + watch submodules
- Idle.rs decomposed into per-resolver submodules
- Commands/tasks.rs extracted from command_center.rs
- Launch command resolution shared across command center and coordination
- Default Codex model updated to gpt-5.3-codex in mesh flows
- Mermaid session-management diagrams replaced with infographics

### Fixed
- Windows daemon session path normalization before UI events
- Metadata-only session update churn in daemon avoided
- Markdown relative image/link path resolution in file viewer
- WSL UNC path handling in coordination config writes and team discovery
- Mesh WSL home parsing hardened against shell banner noise
- DirectoryBrowser init race and gitTab midnight test flakes
- Cargo fmt formatting drift normalized across codebase
- Overflow menu hover hardcoded to dark mode — now theme-aware with click-outside dismiss
- Add-agent select styling unified to custom-chevron pattern matching setup form
- Init disband button height mismatch and visual hierarchy (Retry demoted when conflict recovery visible)
- Inline dark/light ternaries in cleanup panel extracted to $derived tokens per CLAUDE.md convention
- Cleanup toggle label "Manage (0)" edge case when only warnings exist

### Security
- Daemon authentication: shared token validates every request
- Command override validation: allowlist + shell metacharacter rejection
- Scoped tmux environment variables to session
- Scoped opener capability to http/https URLs only
- Bounded read before allocation in daemon server
- Error path sanitization: home directory paths replaced with ~
- `#![forbid(unsafe_code)]` at crate root
- Supply chain policy: cargo-deny configuration
- DOMPurify: forbid `<style>` elements in markdown output
- Coordination: reject `.` and `..` team/member names
- Search: block symlink escapes in incremental indexing
- Search: de-index unreadable files on incremental updates
- Provider: cap README asset reads at 5MB
- Daemon fail-open auth fixed: abort on token failure

### Performance
- Frontend log bridge pressure reduced
- Hidden-tab background refresh churn eliminated
- Shell: surface degraded project loads with retry

### Removed
- Windows E2E test infrastructure (recipes, platform detection, cross-filesystem tests)
- Native `window.confirm()` dialogs — replaced with themed ConfirmDialog component

## [0.3.8] - 2026-02-28

### Fixed
- Search→file navigation: normalize backslash paths at read time so stale search indexes work without manual reindex

### Changed
- E2E search tests: dynamic cross-filesystem discovery — tests WSL and Windows FS projects with subdirectory files instead of root-level README

## [0.3.7] - 2026-02-28

### Fixed
- Search→file navigation broken on Windows for WSL projects — search index stored backslash paths (`src\main.rs`) that the Linux daemon couldn't resolve

## [0.3.6] - 2026-02-28

### Added
- Search button in titlebar — magnifying glass icon left of the theme toggle makes search discoverable without knowing Ctrl+K
- Comprehensive E2E test suite — 138+ functional workflow tests replacing render-only checks, verified on both Linux and Windows

### Fixed
- Windows E2E: projects registered with WSL UNC paths for correct daemon provider routing
- Tantivy search index lock crash when multiple app instances run concurrently — graceful fallback to in-memory index
- Windows E2E: file tree first cold load through UNC bridge handled with skeleton wait + retry
- Windows E2E: cross-tab Git navigation pre-warms commit list to avoid cold-load timeout

## [0.3.5] - 2026-02-28

### Fixed
- Startup white screen: heavy bootstrap work (daemon spawn, tmux, protocol check) moved to background thread — synchronous setup reduced from ~10s to ~100ms

### Changed
- Release workflow: `just bump` now also updates package.json and Cargo.lock; `just release` pushes to remote before creating GitHub release

## [0.3.4] - 2026-02-27

### Fixed
- Daemon startup on macOS: `SO_REUSEADDR` prevents TIME_WAIT port conflict on app restart
- Health check timing: faster recovery when daemon disconnected at startup (10s → 3s)
- Daemon auth on reconnect: re-read token when daemon restarts with new token
- Sidebar filter: replaced static div with functional input for project name filtering

## [0.3.3] - 2026-02-27

### Security
- Daemon authentication: shared token validates every request (F-01)
- Command override validation: allowlist + shell metachar rejection (F-02)
- Scoped tmux environment variables to session (F-03)
- Scoped opener capability to http/https URLs only (F-04)
- Bounded read before allocation in daemon server (F-05)
- Error path sanitization: home directory paths replaced with ~ (F-06)
- `#![forbid(unsafe_code)]` at crate root (F-07)
- Supply chain policy: `deny.toml` for cargo-deny (F-08)
- DOMPurify: forbid `<style>` elements in markdown output (F-09)

### Fixed
- Tab-switch performance regression: removed CSS animation from tab internals that caused GPU compositor thrashing with large Shiki-highlighted content
- Window controls: replaced Preview button with minimize, maximize, close

## [0.3.2] - 2026-02-26

### Fixed
- Taskbar icon: use transparent background with padding so logo silhouette is visible

## [0.3.1] - 2026-02-25

### Added
- Splash screen with state-driven boot animation (clip-path reveal)
- Taurhaus logo (Horned Keystone) replacing placeholder icons
- Windows app icons (ICO bundle, all PNG sizes)
- Comprehensive post-split Shell.svelte tests (56 tests)
- README.md with screenshots, architecture diagram, setup guide
- End-user getting started guide
- Navigation history (back/forward) store

### Changed
- Extract OverviewTab, FilesTab, Sidebar, DirectoryBrowser from Shell.svelte
- Extract shared theme tokens, mock data, IPC modules
- Refactor large Rust functions in command_center and daemon server
- Titlebar logo: real logo image replaces "t" placeholder

### Fixed
- README markdown rendering on Overview tab with Shiki fallback
- Duplicate Windows Terminal tab on session launch
- Flaky Codex idle detection tests

## [0.3.0] - 2026-02-24

### Added
- Bootstrap chain: auto-start daemon and tmux on app launch
- Daemon status indicator in sidebar footer
- Setup guide documenting prerequisites and bootstrap chain
- Per-project position memory with `$bindable` pattern

### Changed
- Refactored R01-R14: dynamic paths, a11y fixes, WCAG contrast, cache eviction, layout rebalancing, branch pill contrast, WSL distro validation, shared tool logos
- Improved sidebar visual hierarchy: brighter text, branch pills, spacing

### Fixed
- Daemon spawn: use long-lived `wsl.exe` child instead of detaching
- File tree collapse and layout overflow on Git-to-Files navigation
- Branch/dirty status not showing on first launch
- Multi-tool session activity detection reliability

## [0.2.1] - 2026-02-23

### Added
- Code theme selector in Settings (light and dark Shiki themes)

## [0.2.0] - 2026-02-23

### Added
- Multi-CLI session management: Claude Code, Codex, Gemini CLI
- Live activity detection per tool (IO hysteresis, TCP sockets, file mtime)
- Tool indicator logos in sidebar (Anthropic, OpenAI, Gemini)
- Context menu with per-tool launch, stop, restart
- HoverCard showing all running sessions per project
- Git tab with commit history, inline diffs, infinite scroll, cross-tab navigation
- Session history enrichment with commit and file change context
- Kanban-style task board aggregating tasks from Claude Code, Codex, Gemini

### Changed
- Session store: groups sessions as `Map<path, session[]>` for multi-tool support
- Sidebar indicators: monochrome SVG logos with activity-state colors

## [0.1.0] - 2026-02-21

### Added
- Initial release: Tauri 2 + Svelte 5 + Rust scaffold
- SQLite database with project CRUD and migrations
- Git module: commit history, status, diffs via libgit2
- File browser with syntax-highlighted preview (Shiki)
- Session handoff parser (YAML frontmatter + JSON sidecar)
- File watcher with notify + ignore crates
- Full-text search with tantivy and Cmd+K overlay
- Relationship auto-detection from Cargo.toml, CLAUDE.md, sessions
- Settings persistence (KV store)
- First-run wizard with project scanning
- Floating Panel layout with dark teal frame
- Light/dark theme toggle
