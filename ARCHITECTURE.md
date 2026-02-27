# Architecture

A condensed overview for contributors. For the full 22 Architecture Decision Records, see [docs/phase-4-architecture.md](docs/phase-4-architecture.md).

## System Overview

taurhaus is a cross-platform dual-process desktop application built with Tauri 2. The native GUI (Rust + Svelte 5) handles storage, git, and search. A lightweight companion daemon handles process scanning, file watching, and tmux session management, communicating with the app over TCP (JSON-line protocol on localhost:9000).

The daemon runs on both platforms — the only difference is where:

- **Windows**: The daemon runs inside WSL2 (launched via `wsl.exe`), where it has access to `/proc` for process inspection and the Linux filesystem where AI tools run.
- **macOS**: The daemon runs natively as a subprocess (launched from `~/.local/bin/taurhaus-daemon`), using `libproc` and `lsof` for process inspection instead of `/proc`.

![System Architecture](docs/system-architecture.jpg)

## Platform Abstraction

The `platform/` module provides compile-time dispatch (`#[cfg(target_os)]`) between Linux and macOS implementations. Both platforms implement the same function signatures — the compiler enforces the API contract. The daemon binary is compiled per-platform with the correct implementation.

| Function | Linux (daemon in WSL2) | macOS (native daemon) |
|----------|--------------------|--------------------|
| `process_cwd(pid)` | `/proc/PID/cwd` readlink | `proc_pidinfo` (libproc) |
| `process_tty(pid)` | `/proc/PID/fd/0` readlink | `lsof -p PID -a -d 0` |
| `process_rchar(pid)` | `/proc/PID/io` rchar field | `proc_pid_rusage` (libproc) |
| `collect_socket_inodes(pid)` | `/proc/PID/fd` → socket inode extraction | `lsof -p PID -i TCP` |
| `has_established_443(pid)` | `/proc/PID/net/tcp` socket state parsing | `lsof` ESTABLISHED filter |

The session scanner (`session_scanner/`) and activity detector (`proc_io.rs`) are fully platform-agnostic — they call into the platform module and don't know which OS they're running on.

## Frontend (Svelte 5 + Tailwind v4)

The frontend runs inside Tauri's embedded WebView — not a browser. All data comes through the Rust backend via IPC.

| Component | Purpose |
|-----------|---------|
| `App.svelte` | Entry point, splash screen gate |
| `Shell.svelte` | Main layout: titlebar, tab routing, theme, position memory |
| `Sidebar.svelte` | Project list, session indicators, context menu, hover cards |
| `OverviewTab.svelte` | Project summary, README, recent commits, sessions |
| `FilesTab.svelte` | File tree with syntax-highlighted code preview |
| `GitTab.svelte` | Commit history, diffs, blame, cross-tab navigation |
| `TaskBoard.svelte` | Kanban board aggregating tasks from Claude Code, Codex, Gemini |
| `TaskDetailPanel.svelte` | Task detail view with metadata and description |
| `SearchOverlay.svelte` | Full-text search across all projects (Ctrl+K) |
| `Settings.svelte` | App preferences and configuration |
| `FirstRunWizard.svelte` | Onboarding flow: project discovery and registration |
| `SplashScreen.svelte` | Startup splash with bootstrap chain progress |
| `SessionHistory.svelte` | Session timeline with handoff summaries |

**Key patterns:**
- **Svelte 5 runes** (`$state`, `$derived`, `$effect`, `$props`) — no legacy stores
- **Derived theme tokens** — all color switching via `$derived` variables, never inline ternaries
- **`$bindable` position memory** — each tab exposes view state, Shell saves/restores per project
- **IPC layer** (`src/lib/ipc.js`) — Tauri `invoke()` wrappers with dev-mode mock fallbacks

## Backend (Rust)

### Modules

| Module | Purpose |
|--------|---------|
| `commands/` | Tauri IPC handlers — thin wrappers over domain modules |
| `platform/` | Compile-time OS dispatch (linux.rs / darwin.rs) |
| `db/` | SQLite connection, migrations, typed query functions |
| `git/` | libgit2 wrappers for commits, diffs, blame, status |
| `fs/` | File tree, content reading, asset serving, file watching |
| `search/` | tantivy full-text search index (build, update, query) |
| `session/` | Session import, parsing, archival |
| `session_scanner/` | CLI tool detection (process scanning, idle detection) |
| `task_scanner/` | Task aggregation from Claude Code, Codex, Gemini |
| `daemon/` | TCP client + server, daemon lifecycle (Windows/WSL only) |
| `terminal/` | Terminal emulator management (Windows Terminal, iTerm2, etc.) |
| `claude_code/` | Claude Code project resolution, memory, teams |
| `provider/` | CLI tool definitions and launch configuration |
| `services/` | Cross-cutting application services |
| `models/` | Shared data structures |
| `config/` | Application configuration |

### Storage

- **SQLite** (`rusqlite`): 6 tables — `projects`, `sessions`, `session_activity`, `relationships`, `tasks`, `settings`. Source of truth for structured data.
- **tantivy**: Full-text search index over files, commits, sessions. Rebuilt from filesystem on startup.
- **Filesystem**: Source of truth for content. SQLite stores metadata; files are always read fresh.

### IPC Commands (~46)

Fine-grained, one command per operation. Frontend calls in parallel for speed.

Grouped by domain:
- **Projects** (11): list, get, register, batch register, update, remove, activity, first-run check, readme, scan directory, system roots
- **Git** (7): all commits, recent commits, range commits, diff, commit files, status, blame
- **Files** (4): read file, list directory, read asset, file tree
- **Search** (3): search, rebuild index, index status
- **Sessions** (5): list, get latest, get archived, get detail, record activity
- **Relationships** (4): list, create, dismiss, remove
- **Command Center** (5): launch session, stop session, navigate to session, list sessions, project tasks
- **Daemon** (3): status, start, stop
- **Settings** (2): get, update
- **Logging** (1): frontend log forwarding — `console.log` in the frontend is monkey-patched (`logger.js`) to also call `frontend_log` IPC, writing to a unified `taurhaus.log` in `app_data_dir()`. Backend uses `tracing` crate. Single log file, truncated per launch.

### Session Scanner

Detects running CLI tool sessions (Claude Code, Codex, Gemini CLI). The detection logic is platform-agnostic — it calls into the `platform/` module for OS-specific process inspection.

| Tool | Detection | Activity Signal |
|------|-----------|-----------------|
| Claude Code | Process name + cwd | IO read bytes — hysteresis (2 consecutive above-threshold polls) |
| Codex | Process name + session file cwd | Session file mtime (10s threshold) |
| Gemini CLI | Process name + SHA-256 path hash | TCP socket state to :443 (ESTABLISHED = active) |

All detection uses 2-poll bidirectional hysteresis to prevent flickering.

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

### Daemon Protocol

JSON-line protocol over TCP (localhost:9000). Same protocol on both platforms — only the daemon launch mechanism differs (WSL on Windows, native subprocess on macOS).

**Events (daemon → app):**
- `file_changed` — watched file modified (triggers search re-index)
- `git_changed` — .git directory modified (triggers commit list refresh)
- `session_file_created` — new session handoff file detected

**Commands (app → daemon, 24 methods):**
- `ping`, `shutdown`, `watch`, `unwatch`, `scan_sessions`
- `git_status`, `git_log`, `git_latest_commit_time`, `git_commits_in_range`, `git_commit_files`, `git_commit_diff`
- `file_tree`, `read_file`, `read_readme`, `read_asset`, `list_directory`
- `list_claude_sessions`, `launch_session`, `stop_session`, `navigate_to_session`
- `get_project_tasks`

## Data Flow

```
User clicks project
  → Shell calls get_project, get_commits, get_file_tree (parallel IPC)
  → Rust reads SQLite (metadata) + libgit2 (commits) + filesystem (tree)
  → Frontend renders immediately

File changes detected
  → Daemon's file watcher detects change
  → Sends file_changed / git_changed event over TCP to app
  → App updates tantivy index + refreshes affected views

CLI session detected
  → Daemon's session scanner polls for tool processes
  → Platform module inspects /proc (Linux) or libproc (macOS)
  → App receives session_update, sidebar shows tool indicator (active/idle)
  → HoverCard shows full session details on hover
```

## Build System

All builds use `just` recipes. Both Windows and macOS builds happen natively on their target platforms — no cross-compilation.

```bash
just dev              # Tauri dev mode (hot-reload)
just build-windows    # Sync to D:\, npm install, cargo build natively via cmd.exe
just build-macos      # Sync to Mac Mini, build ARM DMG via SSH
just build-macos-intel # Build Intel (x86_64) DMG via SSH
just check            # clippy + svelte-check + all tests
just test             # All tests (Rust + frontend)
just test-macos       # Run Rust tests on Mac Mini via SSH
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
