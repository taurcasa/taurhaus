# Changelog

All notable changes to taurhaus are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
