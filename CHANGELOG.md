# Changelog

All notable changes to taurhaus are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
