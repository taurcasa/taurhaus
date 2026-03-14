# Session Activity Daemon Stability Experiment Matrix

**Date:** 2026-03-14
**Task:** #1281

## Conclusion

The current session-detection and daemon-owned activity path is coherent enough
to run as one bounded experiment lane, but it should not be treated as shipped
just because the pieces exist in source.

The real ship question is not "does Taurhaus detect sessions at all." It is:

1. does the scanner stay stable across real process churn and multi-session
   attribution cases,
2. does the daemon keep exporting and delivering authoritative snapshots under
   restart and reconnect pressure, and
3. do the exported activity files stay fresh and semantically honest enough for
   downstream idle handling.

This document turns the current implementation into a concrete experiment matrix
and one explicit ship gate.

## Current Code Baseline

The experiment envelopes should be measured against the code that exists today:

- `src-tauri/src/session_scanner/idle/mod.rs`
  - Claude and Gemini file activity threshold: `5s`
  - Codex file activity threshold: `10s`
- `src-tauri/src/session_scanner/mod.rs`
  - reported session state uses two-poll bidirectional hysteresis
  - Codex multi-session projects suppress shared file activity unless a
    deterministic file owner can be proven
  - scanner input cache windows:
    - PID fingerprint cache TTL: `2s`
    - tmux pane cache max age: `2s`
  - `ps` subprocess timeout: `2s`
- `src-tauri/src/daemon/session_activity.rs`
  - active scan interval: `500ms`
  - idle scan interval after stable idle: `1500ms`
  - stable-idle downgrade after `30` idle cycles
  - activity snapshot refresh without shape change: every `30s`
- `src-tauri/src/coordination/activity_export.rs`
  - activity export is the only canonical writer for `state/activity/*.json`
  - exported confidence classes: `active`, `likely_working`, `uncertain`,
    `idle`, `dead`
- `src-tauri/src/daemon/launcher.rs`
  - startup connect-or-spawn timeout: `5s`
  - reconnect poll interval during spawn: `500ms`
  - native startup can evict a stale daemon binary and reconnect
- `src-tauri/src/daemon/session_listener.rs`
  - app-side session updates are long-polled via `wait_session_updates`

These values define the acceptable timing and behavior envelope for the
experiments below. If the observed behavior falls outside them, that is either
an implementation bug or a documentation bug. Either way it is a no-ship
condition until reconciled.

## Experiment Matrix

| ID | Focus | Experiment | Primary evidence | Ship gate |
|----|-------|------------|------------------|-----------|
| `S1` | Claude and Gemini transition stability | Run one active burst, then stop output cleanly. Measure active-to-idle and idle-to-active transitions against the current `5s` threshold plus two-poll hysteresis. Repeat with one transient blip shorter than one full confirmation window. | `wait_session_updates` version stream, session snapshot timestamps, transcript file mtimes, `session_scanner metrics` logs | No single transient poll may flip reported state. Reported transitions must stay inside the expected code envelope and must not go active or idle earlier than the configured threshold logic allows. |
| `S2` | Codex single-session versus multi-session attribution | First run one Codex session in a project, then run two Codex sessions in the same project and only keep one truly active. Verify that shared transcript writes do not make the quiet peer look definitively active. | runtime snapshots, `project_unattributed_active`, `activity_confidence`, `activity_attribution`, per-PID open-file checks | Single-session Codex may use file activity normally. Multi-session Codex must either identify the true owner or leave the quiet peer unattributed; it must not mark both panes as confidently active. |
| `S3` | Process and tmux churn tolerance | Exercise shim plus native child on one TTY, disappearing processes during scan, tmux pane renames, focus changes, and quick pane teardown/recreate. | display snapshots, runtime snapshots, deduped `(tty, cli_tool)` inventory, scanner timing logs | No duplicate session should survive for the same `(tty, cli_tool)` pair. Vanished PIDs must drop out cleanly. Scan latency must remain bounded instead of hanging on `ps`, tmux, or `/proc` churn. |
| `D1` | Daemon cadence and authoritative snapshot freshness | Keep the daemon in an active state, then let it settle into long idle. Measure scan cadence shift, snapshot version increments, and export frequency. | daemon logs, session snapshot version history, on-disk activity snapshot mtimes | Active scans should stay near `500ms`, stable idle scans should relax toward `1500ms`, and unchanged activity exports must not churn faster than the current `30s` refresh contract. |
| `D2` | Activity export semantic honesty | For one member each, exercise: recent IO, live non-shell process with recent output, live pane with unattributed project activity, dead pane, and genuinely idle pane. Verify the exported JSON classification. | `state/activity/<member>.json`, observed pane state, session snapshot fields | Exported files must match the v1 schema exactly and classify each case honestly. No case should be upgraded from uncertain to active without the evidence the current classifier requires. |
| `D3` | Startup stale-daemon eviction | Start Taurhaus against an older or deleted daemon binary, then let startup validation run. | startup logs, `/proc/<pid>/exe`, daemon connection events, daemon PID/exe before and after | Taurhaus must not continue talking to a stale daemon binary once validation can prove drift. The live connection must rotate to the expected binary or fail visibly. |
| `D4` | Live reconnect and long-poll recovery | During active use, kill or restart the daemon while the app is consuming `wait_session_updates`. Measure reconnect, version continuity, and whether the app resumes authoritative snapshots instead of sticking on stale ones. | `daemon.connection.*` events, session-listing behavior, post-restart version stream, reconnect logs | The app must either reconnect and resume versioned snapshots cleanly or surface a visible degraded state. Silent permanent staleness is a no-ship failure. |
| `D5` | Snapshot and probe failure degradation | Force pane-probe failures, missing team membership data, activity directory write failures, and malformed long-poll responses. | warning logs from `activity_export.rs` and `session_listener.rs`, resulting snapshot files, app state after failure | Failures must stay bounded and visible. A probe or export failure may reduce confidence or skip a write, but it must not fabricate recent activity, crash the daemon loop, or poison subsequent updates. |

## Required Evidence Bundle

Each experiment above should produce the same minimum evidence set:

- one short run log with exact timestamps
- one saved snapshot stream or extracted version timeline
- the relevant on-disk activity snapshot files when the experiment touches
  export behavior
- one concise result note: `pass`, `fail`, or `inconclusive`
- if failed, one exact violated invariant

Without that evidence, the result should be treated as anecdotal and not as a
ship signal.

## Ship Criteria

This lane is ready to ship only if all of the following are true at the same
time:

1. `S1` passes for Claude and Gemini without early flips or one-poll flicker.
2. `S2` passes for both the single-session and multi-session Codex cases, with
   no false confident cross-attribution.
3. `S3` shows no duplicate `(tty, cli_tool)` survivors and no unbounded scan
   stalls during churn.
4. `D1` confirms the daemon is using the intended scan cadence and that steady
   state export churn stays at or below the current `30s` refresh rule.
5. `D2` confirms the activity snapshot files stay on the canonical v1 schema
   and preserve the intended `active` versus `likely_working` versus
   `uncertain` boundaries.
6. `D3` proves that startup can evict a stale daemon binary instead of talking
   to old code indefinitely.
7. `D4` proves that a live daemon restart does not leave the app stuck on stale
   session state.
8. `D5` proves that the failure cases degrade to lower confidence, skipped
   export, or explicit warning, rather than silent false activity.

If any one of those is still missing, the honest verdict is not "almost
shippable." It is "not yet shipped."

## Explicit No-Ship Conditions

Any of these should block shipment immediately:

- a quiet Codex peer in a shared project is reported as confidently active
- the app can stay connected to a stale daemon binary after startup validation
- daemon restart leaves session views frozen without a visible degraded state
- unchanged activity snapshots are still being rewritten at active-scan cadence
- exported snapshot confidence upgrades beyond what the current classifier can
  justify
- one transient poll can flip reported session state

## Recommended Execution Order

Run the experiments in this order:

1. `S1`
2. `S2`
3. `D1`
4. `D2`
5. `D3`
6. `D4`
7. `S3`
8. `D5`

That order is intentional. It proves the semantic core first, then the daemon
delivery path, then the more chaotic churn and degradation cases last.

## Bottom Line

The current code already defines a fairly clear contract:

- explicit file-age thresholds per tool
- explicit hysteresis before reported state changes
- daemon-owned authoritative session snapshots
- one canonical activity-export writer
- explicit stale-daemon validation at startup

The remaining work is not to invent more architecture. It is to prove that
these contracts still hold under live churn, restart, and attribution pressure.
This matrix is the minimum experiment set that should be treated as a real ship
gate for the session-activity and daemon-stability lane.
