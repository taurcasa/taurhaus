# ADR-023: Dual-Path Filesystem Architecture (Windows Local + WSL Daemon)

> Addendum to Phase 4 Architecture. Addresses the cross-filesystem performance problem discovered during Phase 5 implementation.

---

## Problem

taurhaus runs as a Windows-native Tauri app. Projects may live on:

1. **Windows filesystems** (`C:\`, `D:\`) — native, fast
2. **WSL2 Linux filesystems** accessed via UNC paths (`\\wsl$\Ubuntu\...`) — crosses the 9P virtual filesystem protocol, extremely slow

The 9P bridge makes metadata-heavy operations unusable:

| Operation | Native | Over 9P (WSL UNC) | Multiplied by N projects |
|-----------|--------|--------------------|--------------------------|
| `git status` (working tree scan) | ~50ms | 3–15s | 14 projects = 42–210s |
| `git log` (read commits) | ~20ms | 1–5s | 14–70s |
| `read_dir` (list directory) | <1ms | 100–500ms | — |
| Single file read | <1ms | 50–200ms | — |
| `inotify`/`ReadDirectoryChangesW` | Native | Unreliable over UNC | — |

This isn't fixable with caching alone — every refresh, file read, and watcher event still crosses 9P. The fundamental issue is: the Windows process should not do Linux filesystem I/O.

## Decision

**Dual-path architecture** with automatic routing based on project path:

- **Windows-local projects** → Tauri app handles directly (native I/O, fast)
- **WSL projects** → routed through a lightweight daemon running inside WSL (native Linux I/O, fast)

Communication between the Windows app and WSL daemon uses **TCP on localhost** (~0.1ms latency).

```
┌────────────────────────────────────┐
│         Tauri App (Windows)        │
│                                    │
│  ┌──────────────────────────────┐  │
│  │       Provider Router        │  │
│  │  ┌────────────┬───────────┐  │  │
│  │  │ Is \\wsl$? │   else    │  │  │
│  │  └─────┬──────┴─────┬─────┘  │  │
│  │        │            │        │  │
│  │   DaemonProvider  LocalProvider │
│  │   (TCP client)    (direct I/O)  │
│  └────────┼────────────┼────────┘  │
│           │            │           │
│  ┌────────┴────┐  Native Windows   │
│  │ TCP localhost│  filesystem       │
│  └────────┬────┘                   │
└───────────┼────────────────────────┘
            │
   WSL2 network boundary (fast)
            │
┌───────────┼────────────────────────┐
│  ┌────────┴────┐                   │
│  │ TCP server  │  taurhaus-daemon  │
│  └────────┬────┘  (runs in WSL)    │
│           │                        │
│  ┌────────┴──────────────────────┐ │
│  │  • Git ops (libgit2, native)  │ │
│  │  • File I/O (native)          │ │
│  │  • inotify (native)           │ │
│  │  • tmux queries               │ │
│  │  • Claude Code session detect │ │
│  │  • Search indexing             │ │
│  └───────────────────────────────┘ │
└────────────────────────────────────┘
```

## Path Routing

Detection is a simple prefix check on the project path:

```rust
fn is_wsl_path(path: &str) -> bool {
    path.starts_with("\\\\wsl$\\")
        || path.starts_with("\\\\wsl.localhost\\")
}
```

All filesystem operations for a project go through the appropriate provider based on `is_wsl_path(project.path)`. The routing happens in a **provider abstraction layer** that sits between the IPC command handlers and the filesystem/git modules.

## Provider Abstraction

A trait defines all filesystem operations. Two implementations exist:

```rust
trait ProjectProvider {
    // Git
    fn git_status(&self, project_path: &str) -> Result<GitStatus>;
    fn recent_commits(&self, project_path: &str, limit: usize) -> Result<Vec<Commit>>;
    fn all_commits(&self, project_path: &str, limit: usize, offset: usize) -> Result<Vec<Commit>>;
    fn latest_commit_time(&self, project_path: &str) -> Result<Option<DateTime<Utc>>>;

    // Files
    fn file_tree(&self, project_path: &str) -> Result<Vec<FileTreeNode>>;
    fn read_file(&self, project_path: &str, relative: &str) -> Result<String>;
    fn read_readme(&self, project_path: &str) -> Result<Option<ReadmeContent>>;
    fn list_directory(&self, path: &str) -> Result<Vec<DirectoryEntry>>;

    // Sessions
    fn scan_sessions(&self, project_path: &str) -> Result<Vec<PathBuf>>;

    // Watch
    fn watch(&self, project_path: &str) -> Result<WatchHandle>;
}

struct LocalProvider;       // Direct filesystem calls (existing code)
struct DaemonProvider {     // TCP client to WSL daemon
    client: TcpClient,
}
```

The IPC command layer resolves the provider per-project:

```rust
fn provider_for(path: &str, daemon: Option<&DaemonClient>) -> Box<dyn ProjectProvider> {
    if is_wsl_path(path) {
        if let Some(client) = daemon {
            return Box::new(DaemonProvider { client: client.clone() });
        }
        // Fallback: direct I/O (slow but works)
        return Box::new(LocalProvider);
    }
    Box::new(LocalProvider)
}
```

## Daemon Protocol

**Transport**: TCP on localhost, fixed port (default `17233`, configurable).

**Format**: Newline-delimited JSON (NDJSON). Each request is a JSON object on one line, each response is a JSON object on one line. Simple, debuggable, no framing complexity.

**Request format**:
```json
{"id": "req-1", "method": "git_status", "params": {"path": "/home/user/projects/foo"}}
```

**Response format**:
```json
{"id": "req-1", "result": {"branch": "main", "is_dirty": false, "ahead": 0, "behind": 0}}
```

**Error format**:
```json
{"id": "req-1", "error": {"code": "NOT_FOUND", "message": "Path does not exist"}}
```

**Event streaming** (for file watcher):
```json
{"event": "file_changed", "data": {"path": "/home/user/projects/foo", "paths": ["src/main.rs"]}}
{"event": "git_changed", "data": {"path": "/home/user/projects/foo"}}
```

Events are pushed from daemon to client without a request. The client distinguishes events from responses by the presence of `event` vs `id` keys.

**Methods**:

| Method | Params | Returns | Notes |
|--------|--------|---------|-------|
| `ping` | — | `{version, uptime}` | Health check |
| `git_status` | `{path}` | `GitStatus` | |
| `git_log` | `{path, limit, offset}` | `[Commit]` | |
| `git_latest_commit_time` | `{path}` | `DateTime?` | |
| `file_tree` | `{path}` | `[FileTreeNode]` | |
| `read_file` | `{path, relative}` | `{content, binary}` | Base64 for binary |
| `read_readme` | `{path}` | `ReadmeContent?` | |
| `list_directory` | `{path}` | `[DirectoryEntry]` | |
| `scan_sessions` | `{path}` | `[session_path]` | |
| `watch` | `{path}` | `{ok}` | Start watching; events stream back |
| `unwatch` | `{path}` | `{ok}` | Stop watching |
| `tmux_list` | — | `[{session, windows}]` | Future: session registry |
| `tmux_pane_info` | `{target}` | `{pid, command, cwd}` | Future: Claude Code detection |

Note: paths in daemon requests use **Linux paths** (`/home/user/...`), not Windows UNC paths. The Windows app translates: `\\wsl$\Ubuntu\home\user\projects\foo` → `/home/user/projects/foo`.

## Path Translation

```rust
/// Convert a Windows UNC WSL path to a Linux-native path for the daemon.
/// \\wsl$\Ubuntu\home\user\projects → /home/user/projects
/// \\wsl.localhost\Ubuntu\home\user → /home/user
fn wsl_unc_to_linux(unc_path: &str) -> Option<String> {
    let stripped = unc_path
        .strip_prefix("\\\\wsl$\\")
        .or_else(|| unc_path.strip_prefix("\\\\wsl.localhost\\"))?;
    // Skip distro name (first segment), rest is the Linux path
    let after_distro = stripped.find('\\').map(|i| &stripped[i..])?;
    Some(after_distro.replace('\\', "/"))
}

/// Extract the WSL distro name from a UNC path.
fn wsl_distro_from_path(unc_path: &str) -> Option<String> {
    let stripped = unc_path
        .strip_prefix("\\\\wsl$\\")
        .or_else(|| unc_path.strip_prefix("\\\\wsl.localhost\\"))?;
    Some(stripped.split('\\').next()?.to_string())
}
```

## Daemon Lifecycle

### Auto-Start

When the Tauri app starts and finds WSL projects in the database, it attempts to connect to the daemon. If the connection fails:

1. Start the daemon via WSL interop:
   ```
   wsl.exe -d <distro> -- /path/to/taurhaus-daemon --port 17233
   ```
2. Wait up to 3 seconds for the daemon to accept connections
3. If it doesn't start, log a warning and fall back to direct I/O

The daemon binary path is determined by:
- Convention: `~/.local/bin/taurhaus-daemon` inside WSL
- Or: configurable in settings

### Health Check

The Windows app sends `ping` every 30 seconds. If 3 consecutive pings fail:
- Mark daemon as disconnected
- Attempt restart (max 3 attempts per app session)
- Fall back to direct I/O for WSL projects
- Show a subtle status indicator in the UI ("WSL daemon disconnected")

### Shutdown

When the Tauri app closes:
- Send a graceful shutdown message to the daemon
- The daemon has 5 seconds to finish pending work and exit
- If it doesn't exit, the Windows app does NOT kill it (it may serve future sessions)

The daemon also shuts down on its own after 10 minutes of no client connections (idle timeout). This prevents orphaned daemons.

### Multi-User / Multi-Instance

The daemon binds to `127.0.0.1` only (not exposed to network). Multiple Tauri app instances can connect to the same daemon. The daemon is stateless per-client — watch registrations are reference-counted and cleaned up on client disconnect.

## Code Organization

The daemon shares the same Rust crate but is a separate binary target:

```
src-tauri/
  Cargo.toml          # [lib] + [[bin]] for both tauri app and daemon
  src/
    lib.rs            # Tauri app entry (existing)
    main.rs           # Tauri binary entry (existing)
    daemon/
      main.rs         # Daemon binary entry (new)
      server.rs       # TCP server, request dispatch (new)
      protocol.rs     # JSON request/response types (new)
    provider/
      mod.rs          # Provider trait + router (new)
      local.rs        # LocalProvider — wraps existing modules (new)
      daemon_client.rs # DaemonProvider — TCP client (new)
    commands/         # Existing IPC handlers (refactored to use providers)
    git/              # Existing git modules (shared)
    fs/               # Existing fs modules (shared)
    search/           # Existing search modules (shared)
    session/          # Existing session modules (shared)
    db/               # Existing DB modules (Windows app only)
    models/           # Existing model types (shared)
```

The daemon does NOT use SQLite — it's a pure filesystem/git proxy. All metadata persistence stays in the Windows app's SQLite database. This keeps the daemon stateless and simple.

## What Changes for Existing Code

| Current | After |
|---------|-------|
| IPC commands call git/fs modules directly | IPC commands call provider, which routes to local or daemon |
| File watcher runs in Windows process | Local watcher for Windows projects; daemon watcher for WSL projects |
| `list_projects` in Rust reads git status | `list_projects` reads cached git data from SQLite; daemon/watcher keep cache fresh |
| Startup reseed calls `get_latest_commit_time` per project | Background thread calls provider per project (fast for both paths) |

## Cached Git Data in SQLite

To avoid git operations on the IPC hot path for either provider:

Add columns to `projects` table:
```sql
ALTER TABLE projects ADD COLUMN cached_branch TEXT;
ALTER TABLE projects ADD COLUMN cached_is_dirty INTEGER; -- 0/1/NULL (unknown)
```

- `list_projects` reads these columns (instant)
- File watcher events (from either local watcher or daemon events) update the cache
- Background startup refresh updates all caches
- Frontend shows cached data on load, updates reactively via events

## Fallback Behavior

The daemon is an **accelerator**, not a hard dependency:

| Daemon state | WSL project behavior |
|---|---|
| Running, connected | All operations routed through daemon (fast) |
| Not running | Direct filesystem access over 9P (slow, but works) |
| Started but crashed | Auto-restart attempt, then fallback |
| WSL not installed | No WSL projects possible anyway |

Windows-local projects are **never affected** by daemon state.

## Future Extensions (enabled by this architecture)

The daemon running inside WSL is perfectly positioned for future command-center features:

| Feature | Daemon capability |
|---------|-------------------|
| **Session registry** | Detect running Claude Code sessions via process inspection |
| **tmux integration** | Query tmux server for session/window/pane status |
| **Launch sessions** | Create tmux panes, start Claude Code on command from Windows app |
| **Real-time session monitoring** | Watch Claude Code output files, stream status to Windows app |
| **Native search indexing** | Build/update tantivy index inside WSL (fast file reads) |
| **Project auto-discovery** | Watch scan directories with native inotify |

## Consequences

**Positive**:
- WSL project operations become as fast as native (~50ms vs ~5s for git status)
- File watching uses native inotify (reliable, low-overhead)
- Clean separation: Windows app is UI + metadata, daemon is filesystem + execution
- Enables future command-center features (tmux, session registry)
- Graceful degradation — works without daemon, just slower

**Negative**:
- Two binaries to build and distribute
- Daemon lifecycle management (auto-start, health check, restart)
- Path translation layer adds complexity
- First-time setup: daemon binary must be placed in WSL (install step)
- Testing requires both Windows and WSL environments

**Neutral**:
- Windows-local project code path is unchanged (no regression)
- Protocol is simple enough to debug with `nc` or `telnet`
- Daemon is stateless — no migration, no data loss if it crashes

## Implementation Priority

1. **Provider trait + LocalProvider** — refactor existing code behind the abstraction (no behavior change)
2. **Cached git data in SQLite** — add columns, update watcher to write cache
3. **Daemon binary + TCP server** — minimal: `ping`, `git_status`, `git_log`, `read_file`, `file_tree`, `read_readme`
4. **DaemonProvider** — TCP client in the Windows app
5. **Auto-start + health check** — daemon lifecycle management
6. **Watch forwarding** — daemon watches via inotify, streams events to Windows app
7. **tmux/session detection** — future command-center features
