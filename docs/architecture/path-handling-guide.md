# Path Handling Guide

Short reference for adding path-sensitive features without re-learning the Windows/WSL/Linux boundary rules.

Background: see `docs/analysis/path-handling-audit-2026-03-08.md` for the full inventory and audit findings.

## Use This First

If your feature touches any of these, start with the existing authorities instead of inventing new path logic:
- `src-tauri/src/provider/path.rs` for project identity / normalization / Windows<->WSL conversion
- `src-tauri/src/provider/platform_paths.rs` for app-data, daemon-token, tool-home, teams, log, hook, and daemon-binary roots

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
- `PlatformPaths::daemon_token_path()` — `<app_data>/daemon.token`; the old platform data `taurhaus/daemon.token` is read-only migration fallback
- `PlatformPaths::log_path()`
- `PlatformPaths::codex_notify_path()` — `<app_data>/codex-notify.jsonl`
- `PlatformPaths::claude_dir()`
- `PlatformPaths::claude_dir_override()` — `Some` only when `TAURHAUS_CLAUDE_DIR` is set
- `PlatformPaths::teams_dir()`
- `PlatformPaths::codex_dir()` — `$CODEX_HOME` or `~/.codex` (WSL-UNC on Windows); root for the Codex `hooks.json` installer
- `PlatformPaths::agy_dir()` — `TAURHAUS_AGY_DIR` or `~/.gemini`; the override is taurhaus-only because agy exposes no supported home selector
- `PlatformPaths::coordination_template_root(teams_dir)`
- `PlatformPaths::tool_session_root(tool)`
- `PlatformPaths::daemon_binary_path()`
- `PlatformPaths::hook_script_dir()`
- `PlatformPaths::hook_settings_path()`

## Which Path Form To Use

### Linux-native required

Use Linux-native paths for anything executed inside WSL/Linux or compared as project identity:
- daemon binary paths
- tmux commands (there are no tmux hook payloads any more — the focus hook chain was deleted; focus is a hub-side `tmux list-clients` probe)
- Codex session JSONL paths
- CLI runtime file paths under Linux/WSL homes (`~/.claude*`, `~/.codex*`, `~/.gemini/antigravity-cli`, `~/.grok*`)
- project-path equality checks
- backend storage keys derived from project identity

Examples:
- `/home/user/projects/taurhaus`
- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/...`

### Windows-native allowed

Windows-native paths are fine when the consumer is Windows/UI-facing and no cross-platform identity comparison is happening yet:
- Tauri `app_data_dir()` on Windows
- file-picker output from Windows UI
- display-only path text in the UI
- external Windows shell / Explorer integration

Examples:
- `C:\Users\user\project`
- `\\wsl.localhost\Ubuntu\home\user\project`

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

Reference implementation: `just install-daemon` reads the running daemon's `/proc/<pid>/environ` and argv and restarts it with the same `TAURHAUS_DATA_DIR`, `--data-dir` and `--port`, rather than re-deriving a root of its own.

Rule:
- operational tooling should consume the same resolved roots as the app, not a parallel guess

## `TAURHAUS_CLAUDE_DIR` vs `CLAUDE_CONFIG_DIR`

These are two different variables and confusing them silently splits state.

`TAURHAUS_CLAUDE_DIR` moves **taurhaus's** Claude root only. Claude Code itself reads `CLAUDE_CONFIG_DIR` and, with that unset, `~/.claude` — whatever taurhaus was pointed at. Three consequences:

1. **Launching.** A managed Claude launch renders a `CLAUDE_CONFIG_DIR=<dir>` prefix only when the resolved account has an explicit config dir — a non-default account or an override. The default account deliberately resolves to `None` and renders no prefix, so Claude inherits its own unset-variable behaviour (`session_scanner/launch.rs`, `session_scanner/accounts/claude.rs`). The same rule is data-driven for every harness with an `account_selector` in the registry — `CODEX_HOME` for Codex, `GROK_HOME` for Grok. If the base command already carries the variable, taurhaus keeps it and logs `launch.selector.ignored` (`session_scanner/launch.rs`, `LaunchNote::SelectorIgnored`).
2. **Reading identity/activity.** Session identity and state are read under the *process's own* `CLAUDE_CONFIG_DIR` (`/proc/<pid>/environ` on Linux, `ps -Eww` on macOS), falling back to `tool_session_root(Claude)`. Never assume the app's root is the session's root.
3. **The daemon.** It is spawned with `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR` forwarded (converted to Linux form for a WSL daemon); `--data-dir` sets `TAURHAUS_DATA_DIR` inside the daemon. Startup logs `daemon.data_root.mismatch` (warn) when the app and daemon roots diverge anyway.

The daemon token follows that same captured data root. Both processes resolve
`<TAURHAUS_DATA_DIR>/daemon.token`; on Windows the WSL reader uses the exact
Linux-form root passed by the launcher. When the active app-data root is the
ordinary platform default — including the default the app pins at startup — it
can then try the pre-migration `$HOME/.local/share/taurhaus/daemon.token` as a
read-only fallback. A root redirected elsewhere never resolves or reads that
legacy path, and token generation never writes it.

`TAURHAUS_AGY_DIR` is different from a harness selector. It redirects only
taurhaus's Antigravity hook and identity reads, which gives tests a disposable
root. The agy CLI does not honour it, and the registry deliberately declares no
account selector for Antigravity.

Team inboxes always live under the single `PlatformPaths::teams_dir()`, so team members run on the default config dir.

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
