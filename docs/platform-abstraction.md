# Platform Abstraction

Current platform handling is split into three explicit layers, but root/path authority is intentionally centralized: `PlatformPaths` is the single source of truth for durable filesystem roots and launcher paths.

## 1. Process Inspection Surface: `src-tauri/src/platform/`

`platform/` is the compile-time OS boundary for process-level facts:

- `process_cwd(pid)`
- `process_tty(pid)`
- `process_rchar(pid)`
- `has_established_443(pid)`

Structure:

```text
src-tauri/src/platform/
  mod.rs
  linux.rs
  darwin.rs
  windows.rs
  types.rs
```

This boundary is still pure `#[cfg(target_os)]` dispatch. There are no trait objects here.

Platform behavior:

| Platform | Implementation | Notes |
|----------|----------------|-------|
| Linux | `/proc`-based | Used directly on Linux and inside the WSL daemon |
| macOS | `libproc` + `lsof` | Native daemon/session scanning path |
| Windows | explicit stubs | The app does not inspect WSL processes directly; daemon/runtime feeds provide the real session data |

## 2. Path Translation and Identity: `src-tauri/src/provider/path.rs`

`provider/path.rs` owns path-shape translation and project identity normalization:

- WSL UNC <-> Linux (`\\wsl$\\...`, `\\wsl.localhost\\...`)
- Windows drive <-> `/mnt/<drive>/...`
- `normalize_project_path()` for stable cross-platform matching

Use this layer when the question is:

- “Do these two paths identify the same project?”
- “What Linux path should the daemon receive?”
- “What Windows/UNC path should the app show or access?”

## 3. Root Authority: `src-tauri/src/provider/platform_paths.rs`

`PlatformPaths` is the authoritative root resolver for platform-sensitive file locations:

- app data root
- structured JSONL log path
- Claude home / teams dir
- tool session roots
- daemon binary path
- Claude hook script/settings paths

Important nuance:

- file-backed roots resolve to paths the current process can access directly
- on Windows, WSL-backed tool state resolves to UNC paths for the app
- the daemon binary path is different: it resolves to the WSL Linux path because launcher commands execute it inside WSL

This is now the required entry point for durable root resolution. New path-sensitive features should call `PlatformPaths` rather than rediscovering roots ad hoc in commands, scripts, providers, or startup code.

## Daemon Lifecycle by Platform

| Aspect | Windows | macOS | Linux |
|--------|---------|-------|-------|
| GUI app | native `.exe` | native `.app` | native desktop app |
| Daemon location | WSL `~/.local/bin/taurhaus-daemon` | native `~/.local/bin/taurhaus-daemon` | native `~/.local/bin/taurhaus-daemon` |
| Spawn path | `wsl.exe ... <linux-binary>` | direct subprocess | direct subprocess |
| Console behavior | hidden-window spawn for background work | normal background subprocess | normal background subprocess |
| Process inspection authority | daemon/runtime feeds | native process probes | native process probes |

Windows is intentionally asymmetric:

- the app is native Windows
- the daemon is Linux in WSL
- tool/session roots are often accessed from Windows via UNC paths
- project matching must normalize Windows, UNC, and Linux forms before comparing

## Session View Split

Platform abstraction now includes a session-data split:

- `DisplaySession` / `list_display_sessions`: UI-safe session view
- `RuntimeSession` / `list_runtime_sessions`: transcript-aware runtime view

This matters most on Windows, where the daemon provides both feeds and only the runtime feed preserves `session_id` and `jsonl_path`.

## File Watching Split

The watcher model is intentionally split:

| Work | Owner |
|------|-------|
| Native/local project watchers | app |
| WSL project watchers | daemon |
| Activity-based watch reconciliation | app startup/runtime logic |
| Compaction signal watching | dedicated compaction watcher path |

This is why “platform abstraction” is not just `platform/`: path authority, daemon routing, and watcher ownership all participate. The key boundary is:

- `PlatformPaths` decides where important roots live
- `provider/path.rs` translates between Windows, WSL UNC, and Linux path shapes
- `platform/` inspects live process facts once the right paths/processes are known

## Terminal Management

Terminal behavior is unified at the decision-tree level, with platform-specific launch/activate mechanics underneath:

| Platform | Supported emulators |
|----------|---------------------|
| Windows | Windows Terminal + custom |
| macOS | iTerm2, Ghostty, Terminal.app, custom |
| Linux | user-managed / no dedicated taurhaus surface |

The invariant is unchanged: respect the configured emulator, and only launch a new attach path when an existing attached client is not available.

## Build and Distribution

| Target | Build path |
|--------|------------|
| Windows | `just build-windows` via native Windows build under WSL interop |
| macOS | `just build-macos` / `just build-macos-universal` on the remote Mac |
| Linux | development/test target; not the release-first platform |

Cross-compiling Windows from WSL remains out of bounds. macOS builds still require native Mac hardware.

## Rules for New Features

When adding platform-sensitive behavior:

1. Use `PlatformPaths` for every durable root or launcher path: app data, logs, `~/.claude`, teams dir, hook paths, daemon binary path, and per-tool session roots.
2. Use `provider/path.rs` for identity normalization and WSL/Windows/Linux translation.
3. Use `platform/` only for process/socket/TTY inspection primitives.
4. Use `DisplaySession` only for UI surfaces.
5. Use `RuntimeSession` for coordination, transcript ownership, task sync, and compaction.

Related references:

- [path handling guide](architecture/path-handling-guide.md)
- [path handling audit](analysis/path-handling-audit-2026-03-08.md)
