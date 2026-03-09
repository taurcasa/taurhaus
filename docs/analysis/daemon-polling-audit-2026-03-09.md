# Daemon Polling Audit — 2026-03-09

## Scope

Audit all polling-style loops in the daemon-owned session and compaction paths, classify each as:

- must-poll boundary work
- should be event-driven
- should be diff-based

Apply fixes where the current implementation was doing unnecessary internal fanout on unchanged state.

## Outcome

Two hot-path fixes were warranted and implemented:

1. `src-tauri/src/daemon/session_activity.rs`
   - kept the display-session scan cadence
   - stopped unconditional activity snapshot export on every scan cycle
   - activity snapshot export now runs only when the session activity signature actually changes

2. `src-tauri/src/daemon/compaction.rs`
   - kept the runtime-session scan cadence
   - stopped pushing unchanged Codex runtime session sets into the compaction extractor every 500 ms
   - stopped reconciling team watchers every 500 ms
   - team-watcher reconcile now runs on the watcher reconciliation interval instead of the scan interval

## Loop Classification

| Area | File | Current mechanism | Classification | Decision |
| --- | --- | --- | --- | --- |
| Display session scanner | `src-tauri/src/daemon/session_activity.rs` | `scan_sessions_for_display()` at 500 ms / 1500 ms cadence | Must-poll boundary, diff-based fanout | Keep poll, keep daemon-owned cadence, export only on diff |
| Compaction runtime scan | `src-tauri/src/daemon/compaction.rs` | `scan_sessions_for_runtime()` every 500 ms | Must-poll boundary, diff-based fanout | Keep poll, push extractor updates only on Codex runtime-session diff |
| Compaction watcher roster reconcile | `src-tauri/src/daemon/compaction.rs` | team config scan every 500 ms | Should be event-driven eventually | Reduced to watcher reconcile cadence; future improvement is config-dir watch |
| Daemon TCP accept loop | `src-tauri/src/daemon/server.rs` | nonblocking `accept()` + 50 ms backoff on `WouldBlock` | Must-poll boundary | Keep; needed for idle-timeout shutdown without a second control thread |
| Session update delivery | `src-tauri/src/daemon/session_listener.rs` | long-poll `wait_session_updates` RPC | Already event-driven at app layer | Keep |
| Filesystem event delivery | `src-tauri/src/daemon/event_listener.rs` | blocking socket read with timeout | Already event-driven | Keep |
| Daemon bootstrap reachability | `src-tauri/src/daemon/launcher.rs` | bounded poll-until-reachable during startup/restart | Must-poll handshake | Keep |
| Compaction extractor | `src-tauri/src/session_scanner/compaction_extractor.rs` | `notify` events + 5 s reconciliation timeout | Event-driven with bounded recovery poll | Keep |
| Compaction signal watcher | `src-tauri/src/session_scanner/compaction_watcher.rs` | `notify` events + 250 ms loop tick + 5 s reconcile | Event-driven with bounded recovery poll | Keep for now |

## What Was Wrong

### 1. Session activity loop exported snapshots even when nothing changed

Before this audit, `SessionActivityHub` always called `export_activity_snapshots_for_sessions(...)` on every daemon scan cycle, even if:

- the session list was unchanged
- state/activity signatures were unchanged
- no consumer-visible update was emitted

That meant a 500 ms steady-state loop could keep rewriting the activity snapshot store without any new information.

This was not boundary work. It was purely internal fanout and should have been gated by a diff.

### 2. Daemon compaction loop drove unchanged work every 500 ms

Before this audit, the daemon compaction runtime did all of the following every 500 ms:

- scanned runtime sessions
- pushed the full runtime session set into `compaction_extractor::update_active_runtime_sessions(...)`
- rescanned all teams to reconcile watcher startup/shutdown

The scan itself is boundary work. The downstream update and watcher reconcile are not.

This produced unnecessary internal wakeups and repeated config scans even when:

- the active Codex transcript set had not changed
- no team config had changed
- no watcher topology change was needed

## Fixes Applied

### `src-tauri/src/daemon/session_activity.rs`

Change:

- snapshot export now runs only when `activity_changed(...)` reports a real change or the hub is not initialized yet

What remains polling:

- the scanner cadence itself

Why that poll remains valid:

- session activity still comes from live process/tmux/runtime observation
- there is no single push source for “all session activity changed”

### `src-tauri/src/daemon/compaction.rs`

Changes:

- added a compact Codex runtime-session signature set keyed by:
  - `project_path`
  - `tty`
  - `cli_tool`
  - `tmux_pane`
  - `session_id`
  - `jsonl_path`
- only call `update_active_runtime_sessions(...)` when that signature set changes
- moved team watcher reconciliation off the 500 ms scan cadence and onto the watcher reconciliation interval

What remains polling:

- runtime session scan itself
- periodic watcher topology reconcile

Why:

- runtime session attachment is still derived from boundary scanning
- watcher topology still depends on team config changes, and no config-dir watch exists yet in this daemon path

## macOS Compaction Story

### 1. What platform gate exists in `src-tauri/src/daemon/compaction.rs`?

`DaemonCompactionRuntime::maybe_start()` is gated by:

```rust
cfg!(target_os = "linux") && std::env::var_os("WSL_DISTRO_NAME").is_some()
```

That means the daemon-owned compaction runtime only starts inside WSL Linux.

It does **not** start on:

- macOS
- native Linux outside WSL
- Windows itself

### 2. Do extractor, watcher, or processor have platform-specific code?

Mostly no.

- `src-tauri/src/session_scanner/compaction_extractor.rs`
  - platform-neutral overall
  - one Windows-specific rename fallback:
    - `is_windows_unsupported_rename_error(...)`
- `src-tauri/src/session_scanner/compaction_watcher.rs`
  - no platform gating
  - uses `notify`, which is cross-platform
- `src-tauri/src/coordination/compaction_processor.rs`
  - no platform gating in the processor logic itself

So the compaction chain components are largely portable. The main platform restriction is at startup/wiring time, not inside the core extractor/watcher/processor logic.

### 3. Is there a macOS compaction path at all?

Yes, but it is **app-local**, not daemon-owned.

`src-tauri/src/startup/compaction.rs` does this:

- skips only on `cfg!(target_os = "windows")`
- on macOS and Linux, starts:
  - the compaction extractor service
  - per-team compaction signal watchers

So on macOS:

- compaction detection and injection are available while the Taurhaus app process is running
- there is no daemon-owned compaction runtime equivalent

Practical implication:

- macOS is **not** “compaction completely missing”
- macOS **is** missing the daemon-owned/background compaction path that WSL uses

## Remaining Recommendations

1. Move daemon compaction watcher topology to event-driven config-dir watching.
2. Keep the session scanner cadence daemon-owned, but continue pushing only diffs downstream.
3. If macOS should have the same background behavior as WSL, remove the WSL-only startup assumption in `daemon/compaction.rs` and start the daemon compaction runtime on native macOS/Linux as well.
4. Keep extractor and watcher recovery polls; they are valid boundary recovery mechanisms, not the main trigger path.

## Validation

- `cargo fmt --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml daemon::compaction -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml daemon::session_activity -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml --tests`

All passed.
