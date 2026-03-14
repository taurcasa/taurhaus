# Windows Startup White Screen Investigation

## Task

**Task:** #1291  
**Owner:** dev-1  
**Status:** in_progress

Investigate the current Windows startup regression where Taurhaus launches to a white screen and hangs before normal UI startup completes.

## Scope

Treat this as a production-quality root-cause investigation, not a superficial symptom check.

Required investigation angles:

- Windows production log review
- current resource/usage monitor output if it is still being produced
- startup pipeline review across frontend bootstrap, backend startup phases, daemon/provider initialization, and runtime session sync
- Windows/WSL/UNC path normalization and provider-path handling
- regression review against the recent bounded session-resilience changes on `feature/session-activity-daemon-stability`

## Required evidence

- Canonical Windows log path:
  - `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- WSL-visible equivalent:
  - `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- Resource monitor output if current:
  - `/tmp/taurhaus-resource-monitor-v2.csv`

## Specific normalization check

Do not treat this as a generic startup hang until the path layer is explicitly cleared.

Verify whether the recent changes accidentally regressed any of:

- Windows drive path -> Linux path normalization
- WSL UNC path -> Linux path normalization
- provider-path reuse during startup/bootstrap
- daemon/client path identity checks
- any startup code path that now mixes:
  - `\\wsl.localhost\...`
  - `/home/...`
  - `/mnt/...`
  - provider-native paths

## Completion bar

This task is only complete when it includes:

- the real failing startup path or contention point
- concrete evidence from logs and/or code
- the root-cause fix if the issue is in current Taurhaus code
- regression coverage where appropriate
- a clear note on whether Windows/WSL path normalization was involved or ruled out

## Initial live findings

The first pass over the current Windows production log showed that the failing run was not a simple backend startup crash:

- `startup.app.started` was present
- database initialization completed
- daemon bootstrap was delayed, with repeated reconnect requests before the WSL daemon actually spawned

But the same run had **no frontend log-bridge events at all**:

- no `ipc.log.received` for `frontend_log`
- no `[logger] frontend log bridge initialized`

That is materially different from healthy runs, where the frontend bridge appears almost immediately.

## Path-normalization status

The recent bounded session-resilience slice did **not** directly modify the shared path-normalization layer:

- `src-tauri/src/provider/path.rs`
- `src/lib/pathUtils.js`

So Windows/WSL/UNC normalization is still being treated as an explicit check, but it was **not** the leading suspect from the first evidence pass.

## Root-cause direction

The strongest failure shape was:

- backend startup still alive
- frontend/WebView startup apparently failing before normal application logging and rendering were established

That makes this closer to a fragile frontend startup path than to the earlier daemon-contention-only freezes.

## Fix landed

Bounded hardening landed in the frontend startup path:

- `src/lib/logger.js`
  - the logger bridge no longer statically depends on Tauri IPC at module-evaluation time
  - the `frontend_log` invoke is now resolved lazily
  - logger bridge failures remain non-fatal
- `src/main.js`
  - app bootstrap now dynamically imports `App.svelte`
  - uncaught startup errors and unhandled rejections render a visible failure overlay instead of leaving a silent white screen
- `src/lib/startupFailure.js`
  - shared startup-failure fallback rendering

## Regression coverage

Added:

- `src/lib/startupFailure.test.js`
- expanded `src/lib/logger.test.js` to cover the hardened lazy bridge path

Verified:

- `bunx vitest run src/lib/logger.test.js src/lib/startupFailure.test.js src/App.test.js`
- `just check-quick`

## Current assessment

This is a meaningful hardening fix even if the exact original WebView startup exception was not preserved in the existing log.

It does two useful things at once:

1. removes one fragile early-startup dependency from the logger bridge
2. prevents the user from being left with a silent white screen if startup still fails for a different reason later

The next required step is a fresh Windows build/install from this hardened state and then a live startup verification against the installed app.
