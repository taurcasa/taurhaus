# Legacy Artifact Audit — 2026-03-09

> Status update (later on 2026-03-09): the independent `DaemonCompactionRuntime` 500 ms runtime scan identified here was subsequently removed. This document remains useful as a pre-cleanup audit snapshot, but any references to that loop describe historical state, not the current daemon.

## Scope

Exhaustive audit of the compaction-adjacent and daemon-adjacent paths after the event-driven Codex compaction migration.

Audited files:

- `src-tauri/src/daemon/auth.rs`
- `src-tauri/src/daemon/compaction.rs`
- `src-tauri/src/daemon/event_listener.rs`
- `src-tauri/src/daemon/handlers.rs`
- `src-tauri/src/daemon/launcher.rs`
- `src-tauri/src/daemon/mod.rs`
- `src-tauri/src/daemon/protocol.rs`
- `src-tauri/src/daemon/server.rs`
- `src-tauri/src/daemon/session_activity.rs`
- `src-tauri/src/daemon/session_listener.rs`
- `src-tauri/src/daemon/watch.rs`
- `src-tauri/src/session_scanner/cli_tool.rs`
- `src-tauri/src/session_scanner/compaction_extractor.rs`
- `src-tauri/src/session_scanner/compaction_watcher.rs`
- `src-tauri/src/session_scanner/control.rs`
- `src-tauri/src/session_scanner/idle/mod.rs`
- `src-tauri/src/session_scanner/idle/claude.rs`
- `src-tauri/src/session_scanner/idle/codex.rs`
- `src-tauri/src/session_scanner/idle/gemini.rs`
- `src-tauri/src/session_scanner/mod.rs`
- `src-tauri/src/session_scanner/proc_io.rs`
- `src-tauri/src/session_scanner/process.rs`
- `src-tauri/src/session_scanner/tmux.rs`
- `src-tauri/src/coordination/compaction_processor.rs`
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/stores/compaction.rs`
- `src-tauri/src/coordination/stores/inbox.rs`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/startup/compaction.rs`

## Bottom Line

I found a small number of genuine pre-event-driven leftovers and removed them.

I did **not** find another hidden production poll loop comparable to the removed `DaemonCompactionRuntime` 500 ms runtime scan.

Most of the remaining timers, loop ticks, reconciliation polls, caches, and long-poll waits are still justified boundary mechanisms. They are not legacy scaffolding from the old compaction architecture.

## Removed Artifacts

### 1. Dead extractor entrypoint

- Location: `src-tauri/src/session_scanner/compaction_extractor.rs`
- Artifact: `extract_compaction_signals(...)`
- Why it was legacy: no live callers remained after the service-based extractor path became canonical
- Action: removed

### 2. Misleading display-only compaction publisher name

- Location: `src-tauri/src/session_scanner/mod.rs`
- Artifact: `process_display_scan_compaction(...)`
- Why it was legacy: after `#778`, the helper is no longer display-only; it is the shared publisher for compaction runtime sessions
- Action: renamed to `publish_compaction_runtime_sessions(...)`

### 3. Redundant mutable wrapper for app-local watcher ownership

- Location: `src-tauri/src/startup/compaction.rs`
- Artifact: `CompactionWatcherState(pub Mutex<Vec<CompactionSignalWatcher>>)` plus `#[allow(dead_code)]`
- Why it was legacy: startup only stores watcher ownership in Tauri state; there is no mutation path and no reader path that justifies a `Mutex`
- Action: simplified to `CompactionWatcherState(pub Vec<CompactionSignalWatcher>)` and removed the dead-code suppression

### 4. Stale test naming from the older split-pipeline framing

- Location: `src-tauri/src/daemon/compaction.rs`
- Artifact: `daemon_runtime_delivers_codex_compaction_without_app_local_pipeline`
- Why it was legacy: the old name preserved outdated architecture language instead of describing the actual remaining daemon responsibility
- Action: renamed to `daemon_compaction_runtime_bootstrap_and_watchers_deliver_codex_compaction`

### 5. Stale scanner/process comments

- Locations:
  - `src-tauri/src/session_scanner/mod.rs`
  - `src-tauri/src/session_scanner/process.rs`
- Artifact: comments still referring specifically to Claude-only process/session scanning
- Why it was legacy: the scanner is multi-tool and now also acts as the shared runtime-session publisher for compaction
- Action: corrected comments to match the current architecture

## Previously Removed During This Audit Chain

These were already removed in the immediately preceding task chain and are part of the same cleanup story:

### 6. Independent 500 ms runtime scan inside `DaemonCompactionRuntime`

- Location: `src-tauri/src/daemon/compaction.rs`
- Artifact: periodic `scan_sessions_for_runtime()` loop with runtime-signature diff tracking
- Why it was legacy: compaction is already driven by the event-oriented extractor/watcher/processor chain, and runtime session publication was already available from the scanner path
- Action: removed in `#778`

### 7. Runtime-session diff/signature tracking that existed only to support the removed poll loop

- Location: `src-tauri/src/daemon/compaction.rs`
- Artifact: `RuntimeSessionSignature`, signature diff comparison, session-refresh wait calculations
- Why it was legacy: these structures only existed to suppress redundant work in a poll loop that no longer belongs in the runtime
- Action: removed in `#778`

## Reviewed and Kept With Justification

### `src-tauri/src/daemon/session_activity.rs`

Kept:
- `ACTIVE_SCAN_INTERVAL` / `IDLE_SCAN_INTERVAL`
- scanner cadence logic
- versioned snapshot + long-poll wait

Why it stays:
- this is still the canonical daemon-owned boundary scanner for live session activity
- there is no upstream push source for process/tmux/runtime activity itself
- this loop feeds UI-safe daemon session state and now also seeds compaction runtime-session publication indirectly

Conclusion:
- not legacy
- still a valid boundary poll

### `src-tauri/src/daemon/server.rs`

Kept:
- nonblocking `accept()` loop with `50 ms` backoff
- startup wait for first `SessionActivityHub` update before starting daemon compaction runtime

Why it stays:
- the accept backoff is listener/idle-timeout plumbing, not compaction legacy
- the startup wait is the new bridge to the shared scanner authority after removing the old compaction runtime poll loop

Conclusion:
- not legacy

### `src-tauri/src/daemon/launcher.rs`

Kept:
- daemon reachability polling during startup/reconnect
- stale daemon shutdown wait loops

Why it stays:
- these are bounded process-handshake loops around daemon lifecycle, not compaction architecture leftovers

Conclusion:
- not legacy

### `src-tauri/src/daemon/event_listener.rs`

Kept:
- blocking read with timeout
- handshake timeout

Why it stays:
- this is event delivery over TCP with bounded liveness timeouts
- not related to pre-event-driven compaction detection

Conclusion:
- not legacy

### `src-tauri/src/daemon/session_listener.rs`

Kept:
- `wait_session_updates` long-poll

Why it stays:
- this is the UI-facing event bridge above the daemon snapshot versioning model
- it is already event-oriented at the app boundary

Conclusion:
- not legacy

### `src-tauri/src/daemon/watch.rs`

Kept:
- directory watcher ownership
- git debounce tracking

Why it stays:
- separate file-watch subsystem
- no stale compaction logic here

Conclusion:
- no legacy compaction artifact found

### `src-tauri/src/daemon/handlers.rs`

Kept:
- `handle_list_runtime_sessions()` direct runtime scan
- task scan cache

Why it stays:
- `LIST_RUNTIME_SESSIONS` still has live daemon RPC consumers for exact runtime metadata, especially Windows runtime consumers
- this request path is on-demand RPC behavior, not a hidden background compaction poll loop
- task scan cache serves project-task request reuse, not compaction legacy

Conclusion:
- no removable pre-event-driven compaction artifact found

### `src-tauri/src/session_scanner/compaction_extractor.rs`

Kept:
- notify-driven transcript watch
- `SessionsUpdated` command path
- 5-second reconciliation timeout
- extractor file offsets / recent paired-boundary suppression / diagnostics state

Why it stays:
- this is the canonical event-driven signal extractor
- the timeout reconciliation is a recovery mechanism for missed watcher events and transcript watch drift
- the state structures are still live and feed correctness/diagnostics

Conclusion:
- not legacy

### `src-tauri/src/session_scanner/compaction_watcher.rs`

Kept:
- signal-log notify watch
- `loop_tick`
- reconciliation poll
- recent signal-id dedupe window
- watcher state persistence and diagnostics counters

Why it stays:
- this is the canonical event-driven signal consumer
- the 250 ms loop tick and 5-second reconcile are recovery/liveness mechanisms around `notify`, not a legacy replacement for detection
- recent ID tracking is still required for replay/idempotency safety

Conclusion:
- not legacy

### `src-tauri/src/session_scanner/mod.rs`

Kept:
- process/tmux cache
- state hysteresis tracker
- `scan_sessions_for_display()` and `scan_sessions_for_runtime()` split
- Windows daemon display/runtime RPC split

Why it stays:
- caches suppress real boundary cost and remain live
- hysteresis is current behavior for stable activity reporting
- display/runtime split is the current guard against UI-safe/runtime confusion, not legacy duplication

Conclusion:
- no additional removable compaction-legacy structure remained after renaming the compaction publisher helper

### `src-tauri/src/session_scanner/idle/*`, `proc_io.rs`, `process.rs`, `tmux.rs`

Kept:
- directory/file caches
- `/proc` IO hysteresis
- process fingerprint cache
- tmux cache

Why it stays:
- these are still part of the active scanner cost model and correctness model
- they predate the event-driven compaction chain, but they are not artifacts of it; they serve current session discovery/activity resolution

Conclusion:
- not legacy compaction artifacts

### `src-tauri/src/startup/compaction.rs`

Kept:
- non-Windows app-local extractor + watcher startup path

Why it stays:
- this is still the non-daemon platform path for app-local compaction ownership outside the WSL daemon model
- it is not redundant with the daemon runtime on Windows/WSL

Conclusion:
- not legacy
- state wrapper simplified, but the startup path itself must remain

### `src-tauri/src/coordination/compaction_processor.rs`

Kept:
- delivery guards, idempotency, runtime attachment checks, inbox append path

Why it stays:
- this is the active downstream processor for canonical compaction signals

Conclusion:
- no legacy artifact found

### `src-tauri/src/coordination/reinjection.rs`

Kept:
- shared structured card type
- Claude JSON rendering
- Codex imperative text rendering

Why it stays:
- these are the current output adapters for the active compaction delivery path

Conclusion:
- no legacy artifact found

### `src-tauri/src/coordination/stores/compaction.rs`

Kept:
- freshness window, idempotency state, delivery recording, diagnostics emission

Why it stays:
- active correctness and audit store

Conclusion:
- no legacy artifact found

### `src-tauri/src/coordination/stores/inbox.rs`

Kept:
- mesh inbox append/load/quarantine behavior

Why it stays:
- current delivery substrate for Codex post-compaction messages

Conclusion:
- no legacy artifact found

### `src-tauri/src/coordination/claude_hooks.rs`

Kept:
- compact hook install/bridge
- member resolution / snapshot resolution / delivery recording

Why it stays:
- active Claude compaction path

Conclusion:
- no legacy artifact found

## Intentionally Kept Historical Documents

I did **not** rewrite these dated analysis docs:

- `docs/analysis/daemon-cpu-deep-dive-2026-03-09.md`
- `docs/analysis/daemon-polling-audit-2026-03-09.md`

Reason:
- they are time-stamped historical analyses of the system state when written
- changing them would erase the investigative record
- the stale architecture risk is in live code/comments, not in dated reports

## Final Assessment

After this audit, I do not see another live pre-event-driven compaction poll loop or stale compaction-state structure still running in production in the audited paths.

What remains is:
- one daemon-owned live session/activity scanner
- one event-driven compaction extractor with bounded recovery reconciliation
- one event-driven signal watcher with bounded recovery reconciliation
- on-demand runtime-session RPC paths
- platform/bootstrap handshake loops

Those are current architecture, not leftover scaffolding.
