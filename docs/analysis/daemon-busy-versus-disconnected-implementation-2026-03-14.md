# Daemon Busy Versus Disconnected Implementation

**Date:** 2026-03-14
**Task:** #1288

## Summary

Implemented the first end-to-end `busy` versus `disconnected` daemon status
slice.

Before this change, Taurhaus already distinguished a busy shared daemon
connection from a real transport loss inside `DaemonProvider`, but the
UI-facing status snapshot collapsed both cases into the same
connected/disconnected model.

After this change:

- the provider exposes an explicit `is_busy()` probe
- `get_daemon_status` can now return `status: "busy"`
- frontend IPC normalization preserves that status
- the shell and sidebar render `busy` distinctly from `disconnected`

## Trace Of The Old Mapping

### Provider layer

`src-tauri/src/provider/daemon_client.rs` already had the key distinction:

- `send_status_request(...)` used `try_lock()` on the shared connection mutex
- if the mutex was already held, it returned:
  - `"Daemon connection busy with another request"`
- that fast-fail path intentionally did **not** mark the provider disconnected
- only non-busy transport errors flowed into `mark_disconnected(...)`

So the provider already knew:

- busy = connection healthy but occupied
- disconnected = transport unhealthy or missing

### UI-facing status layer

`src-tauri/src/commands/daemon.rs` lost that distinction.

`daemon_status_snapshot(...)` previously mapped provider state as:

- no daemon -> `not_configured`
- connected daemon -> `connected`
- everything else -> `disconnected`

That meant a provider that was connected-but-busy still surfaced as plain
`connected` to the UI status query.

### Frontend rendering path

The frontend path is:

1. `get_daemon_status` IPC command
2. `src/lib/ipc/system.js` normalization
3. `src/Shell.svelte` status filtering and banner state
4. `src/lib/Sidebar.svelte` footer badge copy

Because the IPC snapshot never emitted `busy`, the UI could not represent it.
The only degraded transport states visible in the shell/footer were
`reconnecting`, `disconnected`, and `failed`.

## Implemented Slice

### Backend

Added `DaemonProvider::is_busy()` in
`src-tauri/src/provider/daemon_client.rs`.

Behavior:

- returns `true` only when:
  - the provider is still connected, and
  - the shared connection mutex is currently occupied
- returns `false` for disconnected or poisoned states

Then updated `daemon_status_snapshot(...)` in
`src-tauri/src/commands/daemon.rs` to map:

- connected + busy -> `busy`
- connected + not busy -> `connected`
- not connected -> `disconnected`

This keeps `busy` explicitly on the healthy side of the transport boundary.

### Frontend

Updated the UI-facing status handling so `busy` is preserved and rendered:

- `src/lib/ipc/system.js`
  - normalization now passes through `busy` like other daemon status strings
- `src/Shell.svelte`
  - `busy` is retained as a surfaced status, not filtered away
  - the main banner now includes `busy`
  - banner copy distinguishes:
    - busy: daemon occupied by another request
    - reconnecting/disconnected: transport recovery path
- `src/lib/Sidebar.svelte`
  - added footer badge label:
    - `Daemon busy`

## Verification

Backend targeted tests:

- `cargo test --lib provider::daemon_client::tests::busy_probe_reports_busy_without_marking_disconnected -- --test-threads=1`
- `cargo test --lib provider::daemon_client::tests::status_request_fails_fast_when_connection_is_busy -- --test-threads=1`
- `cargo test --lib commands::daemon::tests::daemon_status_snapshot_reports_busy_without_treating_it_as_disconnect -- --test-threads=1`
- `cargo test --lib commands::daemon::tests::daemon_status_snapshot_returns_connected_without_waiting_for_daemon_ping -- --test-threads=1`

Frontend targeted tests:

- `bunx vitest run src/lib/ipc.test.js src/lib/shell.test.js src/lib/Sidebar.component.test.js`

Formatting:

- `cd src-tauri && cargo fmt`

All of the above passed.

## Scope Boundary

This is intentionally the first narrow slice, not the full daemon-state model.

What is covered now:

- explicit `busy` surfaced through the synchronous daemon status query path
- explicit `busy` visible in shell and sidebar UI

What is not covered yet:

- emitting `busy` from daemon lifecycle background events
- richer differentiation such as `timeout` versus `disconnected`
- session bridge behavior that may want to treat `busy` less severely than
  transport loss

## Practical Result

Taurhaus can now represent:

- `busy`: daemon connection healthy but occupied
- `disconnected`: daemon transport unavailable

That gives the UI its first true busy-versus-disconnected distinction without
changing the broader daemon recovery model yet.
