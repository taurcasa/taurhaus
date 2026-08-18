# Taurhaus Startup Freeze Investigation - 2026-03-13

## Objective

Investigate the Windows production startup freeze reported after the recent install/update, correlate the newest screenshot with the production Windows log and recent resource-monitor data, fix the real root cause, and record the remaining risk.

## Evidence Used

- Screenshot: `/home/user/projects/taurhaus/Screenshot 2026-03-13 134513.png`
- Production Windows log: `C:\Users\user\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- Resource monitor capture: `/tmp/taurhaus-resource-monitor-v2.csv`

## What The Screenshot Shows

The shell chrome is present:

- custom titlebar renders
- sidebar frame renders
- project-list skeleton renders

But the app is clearly not healthy:

- the main content area is effectively blank except for the idle "Select a project" placeholder
- the screenshot timestamp (`2026-03-13 13:45:25 +0100`) lands inside a period where the backend log stops making foreground progress after startup

This does not look like an immediate crash. It looks like a startup path that reached partial UI paint and then stopped servicing the next real foreground request.

## Correlated Log Timeline

The relevant run is `run_31f0a175a9ef4c5d9dd8afa4cb3db4a3`.

### Startup itself completed quickly

- `12:44:29.236Z` `startup.app.started`
- `12:44:29.375Z` daemon fast-path connect succeeded
- `12:44:31.052Z` `startup.orchestration.completed`

So the freeze was not caused by bootstrap never finishing. Taurhaus reached "startup complete" in about `1509 ms`.

### The first foreground command then stalled behind daemon traffic

Immediately after startup:

- `12:44:31.052Z` background task `activity_reseed` started
- `12:44:31.052Z` daemon RPC `r2` sent: `git_status`
- `12:44:31.387Z` frontend requested `get_foreground_project`
- `12:44:31.387Z` daemon RPC `runtime-session-snapshot` sent from that foreground path

After that point, there is no matching `ipc.command.completed` for `get_foreground_project`, and there is no daemon response/timeout for the foreground snapshot request in this run.

The next daemon RPC log line for the run is only:

- `12:45:04.560Z` daemon RPC `r3` sent: `ping`

That is a `33 s` gap after `r2` was sent and a `33 s` gap after the foreground `get_foreground_project` request entered the backend.

## Resource Monitor Correlation

The resource monitor around the same window shows:

- `taurhaus.exe` PID `69308`
  - `13:44:29+01:00`: `37.21 MB`, `20` threads, `313` handles
  - `13:44:32+01:00`: `43.80 MB`, `32` threads, `855` handles
  - `13:44:34+01:00` through `13:44:39+01:00`: CPU effectively idle (`0.00` to `0.06`)
- `taurhaus-daemon` PID `3973612`
  - same window: CPU roughly `8.63%` to `15.09%`, RSS `14 MB`, `22` threads

This is the opposite of a classic frontend CPU runaway:

- the Windows app process is mostly idle while "frozen"
- the daemon is alive and doing work

So the problem is not that `taurhaus.exe` is saturating CPU. The problem is that a foreground request is blocked waiting on daemon coordination.

## Root Cause

The real freeze was shared-connection head-of-line blocking in the daemon client.

More concretely:

1. Taurhaus uses a single shared `DaemonProvider` TCP connection guarded by one mutex.
2. Startup background reseed immediately issued a daemon-backed `git_status` request on that shared connection.
3. Foreground startup/UI paths such as `get_foreground_project` also used the same shared connection for status-like requests (`get_runtime_session_snapshot`).
4. Status-like requests did not fail fast when the shared connection was already busy; they waited behind the long-running request.
5. That blocked the foreground IPC long enough for Windows to make the app feel frozen/unresponsive, even though bootstrap had already "completed."

The key symptom is the `33 s` hole after the background `git_status` send, which closely matches the `30 s` daemon git timeout budget plus overhead.

## Why This Happened Now

The trigger is startup ordering:

- startup background reseed runs immediately after bootstrap
- a foreground project-resolution request arrives almost immediately after the frontend mounts
- both contend for the same daemon connection

This is why the screenshot shows a half-initialized app instead of a clean crash.

## Fix Implemented

Changed the daemon status lane to fail fast when the shared connection is already busy, instead of queueing the foreground/status request behind a long-running daemon RPC.

### Code changes

- `src-tauri/src/provider/daemon_client.rs`
  - `send_status_request(...)` now uses `try_lock()` on the shared daemon connection mutex
  - if another request already owns the connection, status requests return a bounded "busy" transport error immediately
  - the provider is **not** marked disconnected for this busy case
  - added regression test `status_request_fails_fast_when_connection_is_busy`
- `src-tauri/src/daemon_api.rs`
  - added `is_busy_transport_error(...)`
- `src-tauri/src/commands/command_center/session_listing.rs`
  - runtime session snapshot requests now treat the busy case as a cache fallback, not as a reconnect trigger
- `src-tauri/src/commands/coordination/live_status.rs`
  - coordination runtime snapshot treats the busy case as a benign skip instead of logging it as a daemon outage

## Why This Fix Is Correct

This is the narrowest fix that addresses the actual failure mode:

- it does not redesign daemon transport
- it does not change normal git/task/session behavior
- it only changes the "status-like" lane that should never block the foreground behind a long background request

That matches the intended design already documented elsewhere in the codebase for `get_daemon_status`: status reads should not queue behind long shared daemon work.

## Verification

Executed:

- `cargo test status_request_fails_fast_when_connection_is_busy --manifest-path src-tauri/Cargo.toml`
- `cargo test daemon_provider_ping_via_git_status --manifest-path src-tauri/Cargo.toml`
- `cargo test daemon_runtime_session_snapshot_uses_snapshot_method_and_returns_payload --manifest-path src-tauri/Cargo.toml`
- `cargo check --tests --manifest-path src-tauri/Cargo.toml`

All passed.

## Remaining Risk

This fix removes the specific startup freeze path caused by a foreground status request queueing behind a long daemon RPC on the shared connection.

What it does **not** do:

- it does not make long daemon git operations cheap
- it does not eliminate all background startup churn
- on a cold start with no cached session snapshot yet, a busy foreground snapshot request may still resolve to "no foreground project" temporarily rather than blocking

That is the intended tradeoff here:

- temporary missing foreground context is acceptable
- blocking the foreground long enough to make the whole app appear frozen is not

## Bottom Line

The startup freeze after Windows install was not a raw CPU runaway. It was a foreground UI request getting stuck behind startup background daemon traffic on the single shared daemon connection.

The implemented fix makes that status/snapshot lane non-blocking under contention, which is the correct boundary for keeping Taurhaus responsive during startup.
