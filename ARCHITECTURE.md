# Architecture

A condensed overview for contributors. For the full 22 Architecture Decision Records, see [docs/phase-4-architecture.md](docs/phase-4-architecture.md).

## System Overview

taurhaus is a dual-process desktop application:

```
┌──────────────────────────────────────────────────┐
│  taurhaus.exe (Windows native)                   │
│  Tauri 2 shell + Svelte 5 frontend              │
│  ├── SQLite     (metadata, sessions, settings)   │
│  ├── tantivy    (full-text search index)         │
│  └── libgit2    (in-process git operations)      │
└─────────────────┬────────────────────────────────┘
                  │ TCP (localhost:9000, JSON-line protocol)
┌─────────────────▼────────────────────────────────┐
│  taurhaus-daemon (runs inside WSL2)              │
│  ├── Process scanner  (/proc filesystem)         │
│  ├── File watcher     (notify + ignore crates)   │
│  ├── tmux manager     (session creation/control) │
│  └── Activity detector (IO, TCP, file mtime)     │
└──────────────────────────────────────────────────┘
```

**Why two processes?** The Windows exe provides the native GUI. The WSL2 daemon handles Linux-specific operations (process scanning via /proc, tmux control, file watching inside WSL filesystems). They communicate over TCP on localhost.

## Frontend (Svelte 5 + Tailwind v4)

| Component | Purpose |
|-----------|---------|
| `App.svelte` | Entry point, splash screen gate |
| `Shell.svelte` | Main layout: titlebar, tab routing, theme |
| `Sidebar.svelte` | Project list, session indicators, context menu |
| `OverviewTab.svelte` | Project summary, README, recent commits |
| `FilesTab.svelte` | File tree with syntax-highlighted preview |
| `GitTab.svelte` | Commit history, diffs, cross-tab navigation |
| `TaskBoard.svelte` | Kanban board aggregating CLI tool tasks |
| `Settings.svelte` | App preferences and configuration |

**Key patterns:**
- **Svelte 5 runes** (`$state`, `$derived`, `$effect`, `$props`) — no legacy stores
- **Derived theme tokens** — all color switching via `$derived` variables, never inline ternaries
- **`$bindable` position memory** — each tab exposes view state, Shell saves/restores per project
- **IPC layer** (`src/lib/ipc.js`) — Tauri `invoke()` wrappers with dev-mode mock fallbacks

## Backend (Rust)

### Storage

- **SQLite** (`rusqlite`): Project metadata, sessions, relationships, settings. Source of truth for structured data.
- **tantivy**: Full-text search index over files, commits, sessions. Rebuilt from filesystem on startup.
- **Filesystem**: Source of truth for content. SQLite stores metadata; files are always read fresh.

### IPC Commands (~25)

Fine-grained, one command per operation. Frontend calls in parallel for speed.

```
get_projects, get_project, add_project, remove_project
get_commits, get_commit_files, get_diff, get_file_blame
get_file_tree, read_file_content, read_project_asset
get_sessions, get_latest_session
search_index, rebuild_search_index
get_relationships
get_settings, update_setting
get_daemon_status, start_daemon
launch_tool_session, stop_tool_session, focus_terminal
```

### Session Scanner

Detects running CLI tool sessions (Claude Code, Codex, Gemini CLI) via:

| Tool | Detection | Activity Signal |
|------|-----------|-----------------|
| Claude Code | Process name + cwd | `/proc/PID/io` read bytes (IO hysteresis) |
| Codex | Process name + session file cwd | Session file mtime (10s threshold) |
| Gemini CLI | Process name + SHA-256 path hash | TCP socket state to :443 |

All detection uses 2-poll bidirectional hysteresis to prevent flickering.

### Daemon Protocol

JSON-line protocol over TCP. The daemon sends events:

- `session_update` — session list changed (new/ended/activity state change)
- `file_change` — watched file modified (triggers search index update)
- `health` — periodic heartbeat

The app sends commands:

- `launch_session` — start a CLI tool in tmux
- `stop_session` — send `/exit` to a tmux pane
- `list_sessions` — request current session state

## Data Flow

```
User clicks project
  → Shell calls get_project, get_commits, get_file_tree (parallel IPC)
  → Rust reads SQLite (metadata) + libgit2 (commits) + filesystem (tree)
  → Frontend renders immediately

File changes in WSL
  → Daemon's file watcher detects change
  → Sends file_change event over TCP
  → App updates tantivy index incrementally
  → Search results reflect change within seconds
```

## Build System

All builds use `just` recipes. The Windows exe is built **natively on Windows** via WSL2 interop — no cross-compilation.

```bash
just dev              # Tauri dev mode (hot-reload)
just build-windows    # Sync to D:\, npm install, cargo build natively
just check            # clippy + svelte-check + all tests
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
