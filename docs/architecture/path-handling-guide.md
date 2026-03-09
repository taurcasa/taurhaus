# Path Handling Guide

Short reference for adding path-sensitive features without re-learning the Windows/WSL/Linux boundary rules.

Background: see `docs/analysis/path-handling-audit-2026-03-08.md` for the full inventory and audit findings.

## Use This First

If your feature touches any of these, start with the existing authorities instead of inventing new path logic:
- `src-tauri/src/provider/path.rs` for project identity / normalization / Windows<->WSL conversion
- `src-tauri/src/provider/platform_paths.rs` for app-data, Claude-dir, teams-dir, log, hook, and daemon-binary roots

Use `provider/path.rs` when you are comparing or converting project/session paths:
- project identity / project matching
- Windows <-> WSL conversion
- UNC handling
- drive-letter handling
- normalizing paths before comparison

Current canonical helpers:
- `normalize_project_path()`
- `to_linux()`
- `to_windows()`
- `wsl_unc_to_linux()`
- `linux_to_wsl_unc()`
- `windows_drive_to_linux()`
- `linux_mount_to_windows()`

Current root authority:
- `PlatformPaths::app_data_root()`
- `PlatformPaths::log_path()`
- `PlatformPaths::claude_dir()`
- `PlatformPaths::teams_dir()`
- `PlatformPaths::tool_session_root(tool)`
- `PlatformPaths::daemon_binary_path()`
- `PlatformPaths::hook_script_dir()`
- `PlatformPaths::hook_settings_path()`

## Which Path Form To Use

### Linux-native required

Use Linux-native paths for anything executed inside WSL/Linux or compared as project identity:
- daemon binary paths
- tmux commands and tmux hook payloads
- Codex session JSONL paths
- Claude/Gemini runtime file paths under Linux/WSL homes
- project-path equality checks
- backend storage keys derived from project identity

Examples:
- `/home/mstie/projects/taurhaus`
- `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/...`

### Windows-native allowed

Windows-native paths are fine when the consumer is Windows/UI-facing and no cross-platform identity comparison is happening yet:
- Tauri `app_data_dir()` on Windows
- file-picker output from Windows UI
- display-only path text in the UI
- external Windows shell / Explorer integration

Examples:
- `C:\Users\mstie\project`
- `\\wsl.localhost\Ubuntu\home\mstie\project`

Rule:
- if the path is about execution or identity, normalize it
- if the path is only for display, keep the original form unless/until it crosses a runtime boundary

## Identity vs Display

Do not treat these as the same thing.

### Identity path

Used for:
- matching sessions to projects
- matching team members to projects
- backend lookups / cache keys
- cross-platform comparisons

Requirement:
- run through `normalize_project_path()` first

### Display path

Used for:
- showing the path in the UI
- preserving the user-visible OS-native form

Requirement:
- do not assume display paths are safe for equality checks

## `normalize_project_path()` Contract

What it guarantees:
- trims leading/trailing whitespace
- converts backslashes to forward slashes
- collapses repeated separators
- strips trailing separators except root `/`
- converts `\\wsl$\...` and `\\wsl.localhost\...` to Linux form
- converts Windows drive paths like `D:\foo` to `/mnt/d/foo`

What it does not guarantee:
- no filesystem existence check
- no symlink/canonical-path resolution
- no case-folding beyond path-family detection
- no semantic equivalence for arbitrary relative paths
- no guarantee that a display path should be replaced with the normalized form in UI

Use it for identity, not for user-facing presentation cleanup.

## Script Root Discovery Rules

Scripts must not guess active roots heuristically if app authority is available.

Bad:
- scanning a few likely log locations and picking the newest one
- reconstructing the Claude/team root separately from the app

Good:
- consume `TAURHAUS_DATA_DIR` / `TAURHAUS_CLAUDE_DIR` when explicitly provided
- otherwise resolve roots through `PlatformPaths`
- if a script cannot call app code, document any temporary heuristic and keep it clearly secondary to app authority

Rule:
- operational tooling should consume the same resolved roots as the app, not a parallel guess

## Common Pitfalls

### `\\wsl$` vs `\\wsl.localhost`

Both are WSL UNC forms. Treat them as equivalent for project identity after normalization.

### Drive letter case

`C:\foo` and `c:\foo` should normalize to the same Linux mount path family. Do not compare raw Windows strings.

### Trailing slashes

`/home/me/proj` and `/home/me/proj/` should compare equal only after normalization.

### `/mnt/<drive>` mapping

If a Windows path is going to Linux/WSL execution, convert it first. Do not send raw `C:\...` paths into daemon/tmux/runtime code.

### Raw string comparison

Do not compare:
- Windows path vs Linux path
- UNC path vs Linux path
- `projectPath` vs `project_path` fields
- display path vs identity path

Normalize first, then compare.

### Feature-local path discovery

Do not add new one-off logic for:
- app-data root
- log file root
- Claude home / teams dir
- hook script location
- daemon binary path

That is how cross-platform drift gets reintroduced.

## Practical Checklist For New Features

Before merging a path-sensitive feature, verify:
- which side consumes this path: UI, Windows host, WSL/Linux runtime, or both
- whether this value is identity or display
- whether comparison uses `normalize_project_path()`
- whether runtime execution receives Linux-native paths
- whether scripts/tooling are consuming app-authoritative roots
- whether UNC, drive-path, and trailing-slash cases are covered in tests

## Default Rule

If you are unsure:
1. treat project matching as an identity problem
2. normalize through `provider/path.rs`
3. keep OS-native formatting only at the UI/display edge
4. do not invent a second path authority for the same feature
