# Daemon protocol

The daemon is a companion process that handles filesystem access, process scanning, and tmux session management. It communicates with the Tauri app over a TCP connection using a JSON-line (NDJSON) protocol.

![Daemon Protocol](../images/daemon-protocol.jpg)

> Stale render: the diagram says 22 methods and uses superseded method names. The catalog is 27 callable methods (28 constants — `list_directory` has no handler) plus 3 push events at protocol 13; the tables below are authoritative.

## Why a daemon

The app and the daemon exist as separate processes because of platform boundaries:

- **Windows**: The GUI runs as a native Windows app, but AI CLI tools (Claude Code, Codex, Antigravity CLI, Grok CLI) run inside WSL2. The daemon runs inside WSL2 where it has access to `/proc` and the Linux filesystem.
- **macOS / Linux**: The daemon runs natively as a subprocess (`is_native_daemon()` is true for both). No platform boundary — but the same protocol keeps the architecture consistent.

On every platform the daemon process hosts the single session hub: the app reads sessions from the daemon snapshot and only falls back to a local scan when no daemon is configured or reachable.

## Connection

| Property | Value |
|----------|-------|
| Transport | TCP |
| Default address | `127.0.0.1:17233` ([authoritative source](../../src-tauri/src/daemon/server.rs)) |
| Format | NDJSON — one JSON object per line |
| Protocol version | 13 (current) |
| Authentication | Shared token (32-byte hex, file-based) |

### Authentication

On startup, the daemon generates a random 32-byte token, writes it to a well-known path with `0600` permissions, and validates it on every request:

| Platform | Token path |
|----------|-----------|
| Linux | `~/.local/share/taurhaus/daemon.token` |
| macOS | `~/Library/Application Support/taurhaus/daemon.token` |
| Windows | `{FOLDERID_LocalAppData}/taurhaus/daemon.token` |

The app reads this token on connect and includes it in the `auth` field of every request. A normal daemon run rejects missing or incorrect tokens. Authentication can be disabled only with a debug-build flag or in an explicitly unauthenticated test configuration.

On Windows the token file lives inside the WSL distro the daemon runs in, so every app-side connection reads it for that distro (`read_auth_token_for_distro`), never for whichever distro happens to be default. That includes both connections the focus bridge opens — the long-poll session listener and its direct seed fetch — because since v8 they carry tmux focus and nothing else does.

### Connection lifecycle

1. **Startup**: App tries to connect to an already-running daemon.
2. **Current-binary validation**: after connect, startup validates that the serving daemon matches the currently installed binary; stale/deleted inodes are evicted and restarted before the app keeps using the connection.
3. **Auto-launch**: if connection fails, the app starts the daemon and retries.
4. **Health check**: periodic `ping` requests verify the connection is alive and protocol-compatible.
5. **Reconnect**: on connection loss, the app retries connection/restart and re-registers daemon watches.
6. **Disconnect detection**: failed sends mark the provider as disconnected; IPC commands fall back to `LocalProvider`.

### Timeouts

| Operation | Timeout |
|-----------|---------|
| Ping | 5s |
| File operations | 10s |
| Git operations | 30s |

## Wire format

### Request (app → daemon)

```json
{
  "id": "r1",
  "method": "git_status",
  "params": { "path": "/home/user/project" },
  "auth": "a1b2c3..."
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique request ID for response matching |
| `method` | string | Yes | Method name (see command catalog below) |
| `params` | object | Yes | Method-specific parameters (may be `null`) |
| `auth` | string | Yes in normal runs | Shared auth token; the wire type remains optional for debug/test configurations |

### Response (daemon → app)

**Success:**
```json
{
  "id": "r1",
  "result": { "branch": "main", "is_dirty": false, "ahead": 0, "behind": 0 }
}
```

**Error:**
```json
{
  "id": "r1",
  "error": { "code": "NOT_FOUND", "message": "Path does not exist" }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Matches the request ID |
| `result` | any | Method-specific result (present on success) |
| `error` | object | Error with `code` and `message` (present on failure) |

Only one of `result` or `error` is present in each response.

### Event (daemon → app, push-only)

```json
{
  "event": "git_changed",
  "data": { "path": "/home/user/project" }
}
```

Events have no `id` field — this is how the client distinguishes them from responses. Events are fire-and-forget; no acknowledgment is expected.

### Message disambiguation

The client deserializes each line as a `DaemonMessage` enum using serde's `#[serde(untagged)]`:
- Has `id` field → `Response`
- Has `event` field → `Event`

## Events

| Event | Data | Trigger |
|-------|------|---------|
| `file_changed` | `{ path, files[] }` | Watched file modified (triggers search re-index + UI refresh) |
| `git_changed` | `{ path }` | `.git` directory modified (triggers commit list refresh, debounced 2s) |
| `session_file_created` | `{ path, file }` | New session handoff file detected |

## Command catalog

### Infrastructure

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `ping` | — | `{ version, protocol_version, uptime_secs, data_root }` | Health check. App checks `protocol_version` compatibility; startup separately validates serving-binary currentity. `data_root` is the daemon's canonical app-data root (additive) — startup compares it with the app's root and logs `daemon.data_root.mismatch` (warn) when they differ. |
| `shutdown` | — | — | Graceful daemon shutdown |
| `watch` | `{ path }` | `{ ok }` | Start watching a project directory for file/git changes |
| `unwatch` | `{ path }` | `{ ok }` | Stop watching a project directory |
| `set_codex_compaction_mode` | `{ mode: "hooks" \| "transcript" }` | `{ ok }` | App selects the daemon's Codex compaction mode; the daemon flips its runtime and waits for the switch to apply (v9). |

`LIST_DIRECTORY` exists as a method constant but has no handler — it is not a callable method.

### Git

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `git_status` | `{ path }` | `{ branch, is_dirty, ahead, behind }` | Branch name, dirty flag, ahead/behind remote |
| `git_log` | `{ path, limit?, offset? }` | `Commit[]` | Paginated commit history (default limit=50, offset=0) |
| `git_latest_commit_time` | `{ path }` | `{ timestamp }` | RFC 3339 timestamp of most recent commit (or null) |
| `git_commits_in_range` | `{ path, after, before }` | `{ commits[], files[] }` | Commits between two RFC 3339 timestamps |
| `git_commit_files` | `{ path, hash }` | `{ files[] }` | Files changed in a specific commit (with status) |
| `git_commit_diff` | `{ path, hash, file_path }` | `{ hunks[] }` | Diff hunks for a specific file in a commit |

### Files

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `file_tree` | `{ path }` | `FileTreeNode[]` | Recursive directory tree (.gitignore-filtered) |
| `read_file` | `{ path, relative }` | `{ content, language }` | File content with detected language |
| `read_readme` | `{ path }` | `{ content }` or null | README content if present |
| `read_asset` | `{ path, relative }` | `{ data }` | Binary file as base64-encoded string |

### Sessions

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `scan_sessions` | `{ path }` | `{ paths[] }` | Scan for session handoff files |
| `list_display_sessions` | — | `DisplaySession[]` | UI-safe session list for sidebar/session surfaces. |
| `list_runtime_sessions` | — | `RuntimeSession[]` | Runtime-authoritative session list including transcript/session metadata for coordination and compaction logic. |
| `get_runtime_session_snapshot` | — | `{ version, display_sessions[], runtime_sessions[], account_observations[], focus?, foreground_project_path, degraded, degraded_revision }` | One-shot seed of the hub snapshot. `foreground_project_path` is the legacy wire name for the hub's `focus_project_path`. |
| `wait_session_updates` | `{ since_version, since_degraded_revision, timeout_ms }` | `{ version, changed, sessions[], account_observations[], focus?, focus_project_path?, degraded, degraded_revision }` | Long-poll for a newer session snapshot version. `timeout_ms` defaults to 15000 and is clamped server-side. |
| `launch_session` | `{ project_path, mode, cli_tool?, tmux_layout?, command_override?, account_dir? }` | `{ tmux_session?, tmux_window, tmux_pane }` | Launch a CLI tool in a tmux pane |
| `stop_session` | `{ tmux_pane, cli_tool? }` | — | Stop a running CLI tool session |
| `navigate_to_session` | `{ tmux_session, tmux_window, tmux_pane }` | — | Focus a tmux pane |

**Launch modes** (`mode` field):
- `continue` — resume the last session (e.g., `claude --continue`)
- `fresh` — start a new session
- `resume` — tool-specific resume (e.g., `codex resume --last`)

**CLI tools** (`cli_tool` field, defaults to `claude`):
- `claude`, `codex`, `agy`, `grok`. An unrecognised value — including the pre-18a Google value — decodes to `Unknown` rather than to another harness, which is why every vocabulary change bumps the protocol.

### Accounts and usage (generic since protocol 11)

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `list_accounts` | `{ tool }` | `AccountsResult` (`{ accounts[], degraded, error? }`) | Provider accounts visible to the daemon's host, with cached in-memory usage attached. |
| `project_transcript` | `{ tool, project }` | `{ transcript }` | The newest provider transcript that owns the tool's project history. |
| `refresh_usage` | `{ tool }` | `{ started }` | Requests an on-demand, debounced provider usage refresh. |

On Windows, config dirs and transcripts live inside WSL, so the daemon owns these reads. These methods replaced the Claude-only ones in protocol 11 (shipped with app 0.7.0); the older names are intentionally incompatible. `tool` is any registered harness — all four have an account provider, but only the three with an `account_selector` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GROK_HOME`) can be switched; Antigravity returns one implicit account. `refresh_usage` answers `{"started": false}` for a tool with no usage provider (grok) and for a request inside the 5-second debounce (`daemon/usage_poller.rs:113-133`).

### Session activity stream (app bridge)

The daemon itself does not push `session_changed` TCP events. Instead, session activity uses versioned long-poll:
1. App backend opens a dedicated daemon connection (`DaemonSessionListener`), seeded once by `get_runtime_session_snapshot`.
2. App sends `wait_session_updates(since_version, since_degraded_revision, timeout_ms)` repeatedly.
3. Daemon responds immediately on newer snapshot version, or at timeout with `changed=false`.
4. App emits a frontend Tauri event `sessions-updated` — payload `{ version, sessions, degraded, observation_gap }` — on any of three edges:
   - `changed=true` (a newer snapshot version),
   - the hub's `degraded` flag moved in either direction,
   - `degraded_revision` advanced, revealing a blackout that began *and* ended inside one long poll.

   The last two fire with `changed=false` on purpose: the sessions did not change, but whether they are a live observation did, so the retained snapshot is re-emitted with the flags that say so. Each degraded edge emits once, not once per poll. `observation_gap` is set only on the recovering side (`blind_gap && !degraded`) — the edge *into* a blackout closes an interval that was still observed.
5. On every focus transition the app also emits `tmux-focus-changed` — `{ session, window, pane_id, project_id }`, with `project_id` resolved app-side from the hub's `focus_project_path`.

This keeps polling encapsulated inside daemon + app backend while the frontend stays event-driven.

**This bridge is the only live tmux-focus transport.** Focus is a hub-side `tmux list-clients` probe (`session_scanner/tmux.rs` — `list_clients`, `focus_from_clients`) folded into the versioned snapshot. The former hook chain — tmux hooks writing `tmux-focus.json`, watched by inotify — is deleted; no hook code remains on the focus path.

**Scanner blackout cursor (v10).** When a scan cycle cannot read its process inventory, the hub replays its last good snapshot rather than publishing an empty one. Such a cycle bumps no `version`, so it bumps `degraded_revision` instead and wakes waiters — the sessions did not change, but whether they are an observation did. The app carries `since_degraded_revision` so it can present replayed sessions as unobserved. Both mixed pairs are refused by the version gate: a v9 app never sends the cursor (its long poll would return immediately forever after a blackout), and a v9 daemon never sends the flags (a v10 app would read every replayed snapshot as a live observation).

### Tasks

| Method | Params | Result | Description |
|--------|--------|--------|-------------|
| `get_project_tasks` | `{ path, scan_cycle_id? }` | `TaskResult` (`{ tasks, errors, source_outcomes }`) | Aggregated tasks from all CLI tools for a project. `scan_cycle_id` is optional. |

### Per-cycle task scan caching (v6+)

`get_project_tasks` supports an optional `scan_cycle_id` (added in protocol v6 and still present in v10):

- When present, the daemon reuses cached `scan_sessions()` + `ClaudeSourceIndex` inputs for repeated project scans in the same cycle.
- When absent, the daemon performs a fresh input scan (backward compatible behavior).
- The daemon still accepts legacy params shaped as `{ path }`.

This reduces duplicated session/index work during one frontend task-scan pass while preserving compatibility with older clients.

## Platform launch

Every spawn prefixes the daemon's roots as environment variables, so app and daemon resolve the same data and Claude roots (`daemon_launch_env`). `TAURHAUS_DATA_DIR` is always sent when the app's root is overridden; `TAURHAUS_CLAUDE_DIR` only when it is set. For a WSL daemon both values are converted to Linux form first.

### Windows

The daemon runs inside WSL2, launched via `wsl.exe`:

```
wsl.exe -d <DISTRO> -- env TAURHAUS_DATA_DIR=<linux path> [TAURHAUS_CLAUDE_DIR=<linux path>] \
    ~/.local/bin/taurhaus-daemon --port 17233
```

- `CREATE_NO_WINDOW` flag prevents console flash
- WSL distro name is validated (alphanumeric, hyphens, underscores, dots only)
- WSL2 mirrored networking makes `localhost:17233` accessible from Windows

### macOS and Linux

The daemon runs natively as a subprocess, with the same roots set as process env:

```
TAURHAUS_DATA_DIR=<path> [TAURHAUS_CLAUDE_DIR=<path>] ~/.local/bin/taurhaus-daemon --port 17233
```

- macOS: binary must be re-signed after copying (`codesign --force --sign -`) due to macOS Sequoia linker-signature rejection
- macOS: uses `libproc` and `lsof` instead of `/proc` for process inspection

### Daemon binary CLI

| Invocation | Purpose |
|-----------|---------|
| `taurhaus-daemon [--port P] [--bind ADDR] [--data-dir PATH] [--idle-timeout SECS] [--verbose]` | Run the server. `--data-dir` sets `TAURHAUS_DATA_DIR` inside the daemon before any root resolves. |
| `taurhaus-daemon codex-notify <JSON>` | Codex `-c notify` target. Appends the turn-complete event to `<app_data>/codex-notify.jsonl` (5 MB cap, exclusive file lock) and logs `codex.notify.appended`. |
| `taurhaus-daemon --compact-hook` (alias `--claude-compact-hook`) | Same compaction hook bridge the app exposes, for hook wrappers that call the daemon binary. |

Managed Codex launches render the `notify` flag only when all four hold (`commands/terminal_settings.rs`): the launch is managed, the installed Codex version supports `notify`, the user's `config.toml` has no notifier of its own, and the daemon executable exists. Each miss logs differently:

| Case | Event |
|---|---|
| user's `config.toml` already sets `notify` | `launch.notify.ignored` — taurhaus preserves the user's notifier |
| daemon executable missing | `codex.notify.executable_missing` (warn) |
| Codex version does not support `notify`, or could not be resolved | nothing logged here — the flag is simply not rendered |

`just install-daemon` restarts a running daemon in place: it reads the old process's `/proc/<pid>/environ` and argv, preserves its `TAURHAUS_*`/`RUST_LOG` env, and always re-passes normalized `--data-dir`/`--port` so repeated installs cannot drift the daemon's roots or accumulate duplicate flags.

### Protocol version check

On connect, the app sends `ping` and checks `protocol_version` in the response. The gate is exact-match, not a floor: any version *different* from what the app expects (current: v13) is rejected, so a newer daemon is disconnected the same way an older one is, and the user is warned to rebuild the daemon (`just install-daemon`). Old daemons without the field deserialize as version 0.

The same check runs for the rest of the app's life, not only at startup: the health monitor pings for the protocol version rather than liveness (`daemon_lifecycle.rs`), and every reconnect confirms it before the daemon counts as connected — `DaemonProvider::reconnect_checked` is the gate the inline and manual paths use (runtime-snapshot IPC, task sync, the Start Daemon button), so reachability alone never adopts a daemon. A mismatched daemon is disconnected so the restart path can replace it — since v8 the hub snapshot is the only live tmux-focus transport, so a daemon that merely answers TCP is not a daemon the app can use. v9 added `set_codex_compaction_mode`; v10 added the scanner-blackout cursor; v11 replaced the Claude-only account methods with generic account methods (`list_accounts`, `project_transcript`, `refresh_usage`) and added `account_observations` to both session snapshot results; v12 replaced the retired Google value in the `CliTool` wire vocabulary with `agy`; v13 added `grok`. The last two are vocabulary-only changes, and they bump the version because either side decodes the other's tool value as `Unknown` — a session that silently loses its harness identity, not a method that fails loudly. The regression tests that pin this live in `daemon/protocol.rs` (`protocol_version_excludes_daemons_*`).

Separately, startup now validates that the connected daemon is serving from the current installed binary. A daemon still running from a replaced or deleted inode is terminated and restarted before Taurhaus keeps the connection.

## Key files

| File | Purpose |
|------|---------|
| `src-tauri/src/daemon/protocol.rs` | Wire format types, method constants, param/result structs |
| `src-tauri/src/daemon/server.rs` | TCP server (daemon-side request handling) |
| `src-tauri/src/daemon/handlers.rs` | Per-method handler dispatch |
| `src-tauri/src/daemon/session_activity.rs` | Daemon-owned versioned session snapshot hub (scan cycles, focus, degradation revision) |
| `src-tauri/src/daemon/session_listener.rs` | App-side long-poll client for session updates |
| `src-tauri/src/daemon/compaction.rs` | Daemon-owned Codex compaction runtime and the hooks/transcript mode switch driven by `set_codex_compaction_mode` |
| `src-tauri/src/daemon/codex_notify.rs` | `codex-notify` sink (bounded append-only JSONL) |
| `src-tauri/src/daemon/agy_hooks.rs` | Antigravity activity-hook sink — bounded append-only `<app data>/agy-hooks.jsonl` |
| `src-tauri/src/daemon/usage_poller.rs` | Per-(tool, account) usage polling behind `refresh_usage` |
| `src-tauri/src/session_scanner/tmux.rs` | `list_clients` / `focus_from_clients` focus probe |
| `src-tauri/src/daemon/event_listener.rs` | Event listener thread (app-side) |
| `src-tauri/src/daemon/launcher.rs` | Connection + auto-start logic |
| `src-tauri/src/daemon/auth.rs` | Token generation, reading, validation |
| `src-tauri/src/daemon/watch.rs` | File watcher management within daemon |
| `src-tauri/src/provider/daemon_client.rs` | DaemonProvider — TCP client implementing ProjectProvider trait |
| `src-tauri/src/daemon_lifecycle.rs` | Health check, reconnect, shutdown orchestration |

## Related documents

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — system overview
- [Platform abstraction](../platform-abstraction.md) — Linux/macOS dispatch details
- [IPC reference](ipc-reference.md) — Tauri IPC commands that proxy through the daemon
