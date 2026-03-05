# Changelog

All notable changes to taurhaus are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.5.0] - 2026-03-05

### Added

- **Mesh View redesign**: full visual overhaul from foundational node canvas to runtime experience, including card-style nodes, slide-over panels, runtime bar, init/runtime animations, improved light mode variants, and toolbar/surface polish.
- **Create New Project flow** in `AddProjectModal` for creating and registering projects directly from the app.
- **New built-in role template**: `codex-architect` for architectural review and structural decision ownership.
- **New built-in team preset**: `standard-team` (orchestrator + architect + two developers + UI specialist).

### Changed

- **Performance sprint across backend and frontend**:
  - Daemon IPC latency reduced from **44ms to 0.114ms**.
  - Git timeline/range queries optimized with single-pass range scans and TTL memoization.
  - Session scanner cycle cost reduced via batched search-commit queries.
  - Frontend rendering optimized with virtualization for heavy lists, bounded caches (LRU-style), and lazy loading for markdown/Shiki paths.
  - Template IPC calls deduplicated and stale async result guards tightened.
- **Test and quality infrastructure**:
  - Added `just metrics` KPI reporting.
  - Clarified and restructured `just test` / `just check` recipe semantics with a faster test lane.
  - Frontend branch coverage improved from **54% to 65%**.
- **Accessibility and UX polish** on template CRUD surfaces (labels, focus/interaction flows, async state handling, and visual consistency) across six components.

### Fixed

- Files tab blank/stuck states after project switches and metadata-only updates.
- Window state restore behavior for undecorated windows (height/restore correctness).
- Platform hardening issues from review findings, including macOS `/proc` guard handling.
- Flaky onboarding E2E behavior by switching to real temp project directories and more deterministic harness setup.
- Additional backend/frontend reliability fixes: async guard hardening, IPC casing normalization cleanup, and backend error-path resilience improvements.

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
