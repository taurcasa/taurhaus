# Backend Freeze Audit - 2026-03-11

## Scope

Task `#924` targeted backend-driven UI freezes and general Windows flakiness after the recent task-management changes. The explicit requirement was to audit the main blocking and background paths, fix the actual causes, and include the daemon-bridge drop issue instead of only masking symptoms.

## Symptoms Seen In Production

1. Task and session-driven views felt intermittently frozen or sluggish.
2. `list_cli_sessions` and `get_foreground_project` were repeatedly taking about `2.1-2.4s` on Windows.
3. When the daemon bridge dropped, the app fell into a reconnect/fallback storm instead of recovering cleanly.
4. The same failure window also triggered many `record_session_activity` IPCs and made the app feel unstable even after the original trigger passed.

## Timing Evidence

The Windows JSONL app log showed a repeated pattern during degraded periods:

- `daemon.connection.lost` with transport reason:
  - `Failed to terminate daemon request line: An established connection was aborted by the software in your host machine. (os error 10053)`
- then repeated request-path slowdowns:
  - `list_cli_sessions`: about `2130-2400ms`
  - `get_foreground_project`: about `2150ms`
- then repeated reconnect/bootstrap churn

Representative failure windows:

- `2026-03-10T22:16:21Z`
- `2026-03-10T22:47:25Z`
- `2026-03-10T23:39:03Z`
- `2026-03-10T23:54:37Z`

The worst window also showed repeated daemon bootstrap attempts:

- `23:54:42Z` spawn
- `23:54:50Z` spawn
- `23:54:59Z` spawn

That proves the app was not just reconnecting to an existing daemon. It was repeatedly trying to spawn new daemon processes after a bridge loss.

## Root Cause

There were two coupled bugs.

### 1. Expensive Windows fallback work was still reachable from hot request paths

When the daemon provider became disconnected, these request paths could still trigger local Windows fallback work against WSL session state:

- [session_listing.rs](/home/user/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs)
- [mod.rs](/home/user/projects/taurhaus/src-tauri/src/commands/command_center/mod.rs)

That fallback is relatively expensive on Windows/WSL and is acceptable only as an exceptional path. Under bridge churn it became effectively continuous.

### 2. The daemon health logic conflated connection health with daemon process health

The real architectural bug was in [daemon_lifecycle.rs](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs).

Old behavior in the disconnected branch:

1. one immediate `daemon.reconnect()` attempt
2. if that failed, assume daemon restart was needed
3. spawn a new WSL daemon process
4. sleep `2s`
5. try one more reconnect
6. if that failed, loop again soon afterward

This was wrong for Windows/WSL because a stale long-lived provider socket can die even when the daemon process is not conclusively dead yet. The code therefore treated a connection failure as a process failure and repeatedly launched new daemon processes.

That restart churn amplified the original fault and kept the frontend stuck in fallback polling/recovery behavior.

## Secondary Contributor

The frontend Tauri fallback polling path was too aggressive during bridge loss:

- [sessionStore.svelte.js](/home/user/projects/taurhaus/src/lib/sessionStore.svelte.js)
- [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte)

Issues:

1. Tauri fallback polling cadence was still `500ms`.
2. stopping the fallback poll flushed tracked session activity every time, producing bursts of `record_session_activity` IPCs.
3. the session updates bridge did not emit a current snapshot immediately after reconnect, so the frontend could remain in fallback mode even after the daemon was back.

## Fixes Implemented

### A. Remove expensive local fallback from daemon-backed request paths

Changed:

- [session_listing.rs](/home/user/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs)
- [mod.rs](/home/user/projects/taurhaus/src-tauri/src/commands/command_center/mod.rs)
- new cache: [session_snapshot_cache.rs](/home/user/projects/taurhaus/src-tauri/src/session_snapshot_cache.rs)

Behavior now:

1. If a live daemon runtime snapshot exists, use it.
2. If the daemon connection drops, try inline reconnect and retry snapshot fetch.
3. If live fetch still fails, use the last cached runtime snapshot.
4. If there is a daemon provider but no live/cached snapshot, return empty / `None` instead of performing local WSL fallback scans.
5. Local scanning remains only for truly non-daemon environments.

This removes the expensive Windows fallback from the hot request path during daemon churn.

### B. Emit a fresh session snapshot immediately after bridge reconnect

Changed:

- [daemon_lifecycle.rs](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs)

Behavior now:

- `start_session_updates_bridge()` emits the current session snapshot immediately after reconnect, even if no versioned delta happened yet.

This is important because otherwise the frontend can stay in fallback mode waiting for a `sessions-updated` event that never comes.

### C. Slow Tauri fallback polling and stop activity-flush storms

Changed:

- [sessionStore.svelte.js](/home/user/projects/taurhaus/src/lib/sessionStore.svelte.js)
- [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte)

Behavior now:

- Tauri fallback poll interval is `5000ms` instead of `500ms`.
- bridge-transition stop calls no longer flush tracked session activity
- normal callers still keep the default flush behavior unless they opt out

This turns a bridge-loss episode from a near-continuous request storm into a bounded degraded mode.

### D. Fix the daemon health policy so it does not restart on the first reconnect miss

Changed:

- [daemon_lifecycle.rs](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs)

Behavior now:

1. On disconnect, the health monitor first waits for sustained reconnect reachability using `reconnect_existing_provider_until_reachable(...)`.
2. Only if that sustained reconnect path fails does it attempt a daemon restart.
3. After restart, it again waits for sustained reconnect reachability before deciding recovery failed.

This is the key root-cause fix.

The boundary is now correct:

- provider socket failure means `connection unhealthy`
- it does **not** immediately mean `daemon process must be restarted`

## Architectural Outcome

The main improvement is not one individual optimization. It is restoring the correct ownership boundaries:

- hot request paths should consume daemon-owned runtime snapshots, not rescan locally during bridge loss
- the daemon health monitor should recover the provider connection first and only restart the daemon when sustained reconnectability fails
- the frontend should not hammer the backend while bridge state is unclear

## Validation

Focused validation run:

- `cargo test --manifest-path src-tauri/Cargo.toml commands::command_center -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml daemon_lifecycle -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml daemon::launcher -- --nocapture`
- `bun vitest run src/lib/sessionStore.test.js src/lib/shell.test.js src/lib/shell/events.test.js`

All passed.

Final gate:

- `just check-quick`

Result: passed.

## Residual Risk

The specific low-level reason Windows/WSL aborts a long-lived provider socket with `10053` is still outside the app's direct control. The app-side root cause we could and did fix is the recovery architecture around that event.

That is the right boundary for this task:

- we do not need to prove why the OS occasionally aborts the socket
- we do need to ensure that one dead socket does not freeze the app or cause daemon restart churn

This task closes that app-side failure mode.
