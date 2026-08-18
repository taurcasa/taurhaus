# Platform Compatibility Review

Date: 2026-03-28
Reviewer: `dev-3`
Scope: Windows host + WSL runtime, Linux, macOS, with emphasis on path normalization, daemon/runtime boundaries, watcher behavior, environment detection, and build pipeline assumptions.

## Executive Summary

The strongest parts of the current design are:
- `src-tauri/src/provider/path.rs` centralizes Windows drive, WSL UNC, and Linux conversions and has a solid test corpus.
- Windows project-tree watching is intentionally deferred to the daemon for WSL paths, which is the right general direction.
- Startup daemon selection already prefers the distro embedded in registered project paths over the ambient default distro.

The main problems are not in the low-level conversion helpers themselves. They are in higher-level callers that still reconstruct platform roots independently or compare raw path strings after translation should already have happened.

Severity summary:
- High: 2
- Medium: 2
- Low: 1

## Findings

### 1. High: Windows coordination runtime ignores `TAURHAUS_CLAUDE_DIR` in process-control paths

Affected platforms:
- Windows host + WSL runtime
- Especially test/dev isolation or any non-default Claude root

Evidence:
- `PlatformPaths::claude_dir()` honors `TAURHAUS_CLAUDE_DIR` at [src-tauri/src/provider/platform_paths.rs:36](/home/user/projects/taurhaus/src-tauri/src/provider/platform_paths.rs#L36).
- Coordination stores also honor the override at [src-tauri/src/coordination/state.rs:264](/home/user/projects/taurhaus/src-tauri/src/coordination/state.rs#L264), [src-tauri/src/coordination/stores/operational.rs:160](/home/user/projects/taurhaus/src-tauri/src/coordination/stores/operational.rs#L160), and [src-tauri/src/coordination/stall_detector/paths.rs:7](/home/user/projects/taurhaus/src-tauri/src/coordination/stall_detector/paths.rs#L7).
- But the runtime/process path used for daemon pid files, control credentials, and `--claude-dir` resolution hardcodes `mesh_cli::resolve_windows_mesh_teams_dir()` on Windows at [src-tauri/src/coordination/runtime/process.rs:325](/home/user/projects/taurhaus/src-tauri/src/coordination/runtime/process.rs#L325) and [src-tauri/src/coordination/runtime/process.rs:342](/home/user/projects/taurhaus/src-tauri/src/coordination/runtime/process.rs#L342).
- Those values are then fed into mesh daemon launch arguments at [src-tauri/src/coordination/runtime/system.rs:174](/home/user/projects/taurhaus/src-tauri/src/coordination/runtime/system.rs#L174) and [src-tauri/src/coordination/runtime/system.rs:228](/home/user/projects/taurhaus/src-tauri/src/coordination/runtime/system.rs#L228).

Why this is a bug:
- On Windows, different subsystems disagree about where `~/.claude` lives.
- If `TAURHAUS_CLAUDE_DIR` is set, stores and path helpers will point to the override, but runtime process control still points at the default WSL-derived UNC root.
- That can break daemon pid discovery, control-token lookup, and mesh daemon launches in exactly the environments where overrides are used for isolation.

Recommended fix:
- Replace the Windows-specific branch in `resolve_host_claude_dir()` and `resolve_mesh_cli_claude_dir_arg()` with `PlatformPaths::claude_dir()` or a single shared authority derived from it.
- Add a Windows-targeted regression test that sets `TAURHAUS_CLAUDE_DIR` and verifies pid/control paths and `--claude-dir` all resolve under the override.

### 2. High: Coordination on Windows still binds to the default WSL distro, while startup daemon bootstrap can bind to a project distro

Affected platforms:
- Windows host with more than one WSL distro

Evidence:
- Startup daemon bootstrap prefers the distro embedded in registered project paths at [src-tauri/src/startup/setup.rs:72](/home/user/projects/taurhaus/src-tauri/src/startup/setup.rs#L72) through [src-tauri/src/startup/setup.rs:119](/home/user/projects/taurhaus/src-tauri/src/startup/setup.rs#L119).
- Coordination bridge helpers discover WSL home and teams roots by running `wsl` with no explicit distro at [src-tauri/src/coordination/mesh_cli.rs:77](/home/user/projects/taurhaus/src-tauri/src/coordination/mesh_cli.rs#L77), [src-tauri/src/coordination/mesh_cli.rs:148](/home/user/projects/taurhaus/src-tauri/src/coordination/mesh_cli.rs#L148), and [src-tauri/src/coordination/mesh_cli.rs:171](/home/user/projects/taurhaus/src-tauri/src/coordination/mesh_cli.rs#L171).

Why this is a bug:
- The app already has logic to choose a runtime distro from actual project paths.
- But the coordination bridge re-detects a distro independently and falls back to the ambient default.
- In a mixed-distro setup, daemon bootstrap can target distro A while coordination pid files, team roots, and mesh CLI invocations resolve into distro B.
- That is a real split-brain risk across Windows host state, WSL runtime state, and daemon state.

Recommended fix:
- Thread the chosen startup/runtime distro through `mesh_cli` callers instead of rediscovering it.
- Make `resolve_windows_mesh_teams_dir()` accept an explicit distro parameter, with the ambient default only as a last-resort fallback.
- Add a regression test that simulates two distro names and asserts daemon selection and coordination root resolution stay aligned.

### 3. Medium: Task scan flows still filter sessions with raw path equality instead of normalized identity

Affected platforms:
- Windows/WSL
- Linux/macOS when trailing slash or separator normalization differs

Evidence:
- Daemon-side project task scan filters sessions with `s.project_path == params.path` at [src-tauri/src/daemon/handlers.rs:410](/home/user/projects/taurhaus/src-tauri/src/daemon/handlers.rs#L410).
- Local task sync does the same with `s.project_path == project_path` at [src-tauri/src/services/task_sync.rs:377](/home/user/projects/taurhaus/src-tauri/src/services/task_sync.rs#L377).
- The codebase otherwise treats project matching as a normalized identity problem via `normalize_project_path()` in many neighboring flows.

Why this is a bug:
- These scans run after path translation has already crossed runtime boundaries.
- A trailing slash, UNC-vs-Linux form, or repeated-separator difference can exclude live sessions from the task scan.
- The result is silent under-reporting of Claude/Codex task state rather than an obvious hard failure.

Recommended fix:
- Normalize both sides before filtering.
- Prefer a small helper for “same project identity” so future task-related code paths do not reintroduce raw compares.
- Add regression tests covering at least:
  - `\\wsl.localhost\Ubuntu\home\user\proj` vs `/home/user/proj`
  - `C:\repo\` vs `/mnt/c/repo`
  - trailing-slash mismatches on native paths

### 4. Medium: WSL detection for daemon compaction depends only on `WSL_DISTRO_NAME`

Affected platforms:
- Linux processes running inside WSL

Evidence:
- `running_under_wsl()` is only `cfg!(target_os = "linux") && std::env::var_os("WSL_DISTRO_NAME").is_some()` at [src-tauri/src/daemon/compaction.rs:137](/home/user/projects/taurhaus/src-tauri/src/daemon/compaction.rs#L137).

Why this is fragile:
- This works for normal interactive WSL shells, but it assumes that environment variable is always preserved.
- Long-lived background processes, service-style launches, or alternate entrypoints can lose that variable even though they are still running inside WSL.
- If that happens, compaction runtime startup is skipped entirely.

Recommended fix:
- Use a stronger WSL detector with an explicit fallback, for example:
  - `WSL_DISTRO_NAME`, then
  - `/proc/sys/kernel/osrelease` or `/proc/version` inspection for `microsoft`, or
  - an explicit launch-time flag when the daemon is spawned from Windows.
- Add tests for the detector so the fallback logic is locked down.

### 5. Low: Build pipelines work, but they depend on ambient machine state and mutate live runtime state during packaging

Affected platforms:
- Windows build lane
- macOS remote build lane

Evidence:
- Windows packaging installs and restarts the live daemon as part of `build-windows.sh` at [scripts/build-windows.sh:28](/home/user/projects/taurhaus/scripts/build-windows.sh#L28).
- macOS build steps assume remote `ssh`, `rsync`, login-shell PATH wiring, codesign availability, and a sibling Mesh checkout at [justfile:592](/home/user/projects/taurhaus/justfile#L592) and [justfile:720](/home/user/projects/taurhaus/justfile#L720).

Why this matters:
- I did not find a clear correctness bug in the scripts themselves.
- The fragility is operational: packaging is coupled to live user/runtime state and to preconfigured remote hosts.
- That makes reproducibility weaker than the rest of the path/runtime design.

Recommended fix:
- Add explicit preflight checks that fail fast on missing remote prerequisites and report the exact missing dependency.
- Consider making Windows packaging skip `_install-daemon-from-build` by default unless explicitly requested, mirroring the safe-by-default E2E approach.

## Area Notes

### Path normalization

Overall assessment:
- Strong.

Notes:
- `provider/path.rs` covers the important Windows drive, WSL UNC, verbatim path, and separator cases well.
- The main residual risk is not conversion correctness. It is callers bypassing normalized identity after conversion.

### Platform-conditional logic

Overall assessment:
- Mostly coherent.

Notes:
- The `cfg(target_os)` split is generally explicit and readable.
- The biggest weakness is duplicated platform-root discovery logic outside `PlatformPaths`.

### Daemon behavior

Overall assessment:
- Strong direction, with one important Windows multi-distro gap.

Notes:
- Windows daemon bootstrap, auth-token fallback, and console-window suppression are all deliberate and well structured.
- The mismatch is not “daemon vs no daemon”; it is “which distro owns the daemon and coordination state”.

### File watching

Overall assessment:
- No code-level correctness defect found in watcher registration logic during this review.

Notes:
- The design correctly defers WSL project trees to daemon watches on Windows.
- Residual risk remains wherever Windows still locally watches WSL UNC paths, because those paths are more sensitive to host/runtime drift than native Linux paths.

### Build pipeline

Overall assessment:
- Functional but operationally brittle.

Notes:
- The scripts encode the intended host boundaries correctly.
- The main weakness is reliance on ambient host state rather than a single reproducible preflight contract.

## Hardening Priorities

1. Unify all Windows coordination path resolution behind one Claude-root authority.
2. Thread the selected runtime distro through every Windows host ↔ WSL coordination call instead of rediscovering the default distro.
3. Replace remaining raw project-path equality checks with normalized identity helpers.
4. Strengthen WSL environment detection for daemon-side compaction.
5. Add build-lane preflight checks and reduce live-environment mutation during packaging.
