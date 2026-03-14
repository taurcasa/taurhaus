# Session UI Resilience Experiments

## Objective

Identify the current session-presence update path, add one concrete measurement point for short daemon disconnects, and record the first UI resilience options.

## Current UI / Session Presence Path

The current runtime-session presence path is:

1. The daemon owns the runtime session snapshot and version counter.
2. `src-tauri/src/daemon_lifecycle.rs:start_session_updates_bridge()` long-polls daemon `wait_session_updates`.
3. The bridge emits Tauri `sessions-updated` events and, after reconnect, emits one immediate current snapshot through `emit_current_session_snapshot(...)`.
4. `src/lib/shell/events.svelte.js` registers the `sessions-updated` and `daemon-status` listeners.
5. `src/Shell.svelte`:
   - marks `sessionBridgeLive = false` when daemon status is no longer `connected`
   - marks `sessionBridgeLive = true` on the next `sessions-updated`
   - applies the snapshot via `applyDaemonSessionUpdate(...)`
6. `src/lib/sessionStore.svelte.js` updates the in-memory session map, which drives sidebar/session presence UI.

That means short daemon disconnects surface in the UI as a gap between:

- the first disconnect observation that clears the live bridge flag
- the first restored `sessions-updated` snapshot after reconnect

## Measurement Added

Added one bounded backend measurement at the session-updates bridge boundary in:

- `src-tauri/src/daemon_lifecycle.rs`

New behavior:

- when the session-updates bridge observes the daemon transition from connected to disconnected, it records the first disconnect timestamp
- when the bridge successfully emits the first post-reconnect current snapshot, it emits a structured log event:

`daemon.session_updates_bridge.recovered`

Event fields:

- `duration_ms`: elapsed time from first observed disconnect to first restored snapshot emission
- `snapshot_version`: daemon snapshot version pushed to the UI
- `session_count`: number of sessions in the restored snapshot

This is the correct first metric for UI resilience because it measures the actual session-presence recovery boundary, not only raw socket reconnect time.

## Why This Measurement Matters

There are already daemon connection lifecycle events such as `daemon.connection.lost` and `daemon.connection.established`, but those only describe transport state. The UI becomes trustworthy again only after the session-updates bridge has pushed a fresh runtime snapshot. This new metric captures that end-to-end gap directly.

## First Resilience Options

### Option 1: Grace window before presence clears

Hold the last known session-presence snapshot for a short grace period, such as 2-5 seconds, before visually downgrading the UI to disconnected. This would suppress flicker for short daemon restarts.

Tradeoff:

- better perceived stability for brief disconnects
- risks showing stale presence without an explicit stale indicator

### Option 2: Stale-but-retained presence state

Keep the last snapshot visible, but mark session presence as stale once `daemon-status` leaves `connected`. This avoids hard disappearance while remaining honest that the data is not live.

Tradeoff:

- clearest operator model for short gaps
- requires visual language for stale presence across sidebar and mesh runtime surfaces

### Option 3: Disconnect debounce only for session UI

Do not delay the daemon status badge, but debounce only the session-presence downgrade until either:

- the disconnect exceeds a short threshold, or
- a reconnect fails to restore a snapshot in time

Tradeoff:

- keeps transport status honest
- adds one more state boundary between daemon badge and session-presence UI

## Recommendation

The safest next experiment is Option 2: retain the last snapshot with an explicit stale treatment while the bridge is disconnected, and use the new `daemon.session_updates_bridge.recovered.duration_ms` metric to decide whether the grace window should later be shortened or expanded.

## Files Changed

- `src-tauri/src/daemon_lifecycle.rs`
- `docs/analysis/session-ui-resilience-experiments-2026-03-14.md`

## Validation

- `cargo fmt --all`
- `cargo test session_bridge_recovery_tracker --lib`
