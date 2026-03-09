# Daemon Performance Audit — 2026-03-09

Task: `#807`
Scope: `taurhaus-daemon` steady-state performance on Linux/WSL after the recent event-driven compaction migration.
Primary data source: `/tmp/taurhaus-resource-monitor-v2.csv`
Supporting telemetry: `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

## Executive summary

The backend daemon is still too expensive in steady state. After warmup, it sits at roughly `23-24%` of one logical CPU core with a stable resident set around `71 MB`, `77` threads, `~204` open FDs, and `~75k` inotify watches. The dominant cost is no longer the removed legacy compaction loop. The remaining hot path is the daemon-owned display session scanner, which still runs at an effective `500ms` cadence most of the time and spends most of each cycle in per-session idle classification.

The core architecture problem is now clear:

1. `SessionActivityHub` still polls too frequently for steady-state supervision.
2. The adaptive `500ms -> 1500ms` downgrade rarely engages in real workloads.
3. Per-session idle classification, especially Codex transcript resolution, dominates scan time.
4. Thread, FD, and inotify counts are high but look like capacity/footprint issues, not the primary CPU driver.

## Measured baseline

Full capture window from `/tmp/taurhaus-resource-monitor-v2.csv`:

- Start: `2026-03-09T04:26:46+01:00`
- End: `2026-03-09T12:57:07+01:00`
- Duration: about `510` minutes
- Daemon samples: `12030`

Whole-capture daemon stats:

- CPU mean: `24.41%`
- CPU median: `23.86%`
- CPU p95: `31.19%`
- RSS median: `70.71 MB`
- Threads median: `77`
- Open FDs median: `204`
- Inotify watches median: `74872`

Warm steady-state subset (`threads >= 70` and `inotify_watches >= 70000`):

- Samples: `8763`
- CPU mean: `23.38%`
- CPU median: `23.45%`
- CPU p95: `27.50%`
- RSS median: `70.96 MB`
- Threads median: `77`
- Open FDs median: `204`
- Inotify watches median: `74872`

Latest sampled daemon row:

- `2026-03-09T12:57:07+01:00`
- CPU: `33.71%`
- RSS: `72.21 MB`
- Threads: `77`
- Open FDs: `206`
- Inotify watches: `79108`

Interpretation:

- The daemon does not show a current runaway leak pattern.
- Memory, threads, FDs, and watch counts ramp up and then flatten.
- CPU remains the real steady-state problem.

## Log-derived scanner behavior

Using current-run `session_scanner.scan.completed` events from the Windows app log for run `run_8d42853f48ab4e25a546c276f8c3a731`:

- Event count: about `11.7k`
- Observed `session_count`: constant `16`
- Scan interval mean: `0.521s`
- Scan interval median: `0.500s`
- Scan interval p95: `0.601s`
- `<= 0.6s` intervals: `11081 / 11718`

Per-cycle timing:

- `duration_ms` mean: `142.45`
- `duration_ms` median: `152`
- `duration_ms` p95: `209`
- `idle_ms` mean: `104.72`
- `idle_ms` median: `104`
- `classify_ms` mean: `105.48`
- `classify_ms` median: `105`
- `tmux_ms` mean: `11.26`
- `tmux_ms` median: `0`
- `process_scan_ms` mean: `0.01`
- `ownership_ms` mean: `0.04`
- `process_signal_ms` mean: `0`

Interpretation:

- The steady-state scanner is still effectively a `500ms` loop.
- The dominant per-cycle cost is idle/classification work, not process enumeration, ownership logic, or compaction processing.
- Tmux refreshes create occasional spikes, but they are not the main baseline cost.

## Current code-path assessment

### High: Session activity scanning is still the dominant steady-state CPU cost

The daemon starts `SessionActivityHub` at startup and keeps a single background scanner thread alive. That thread still calls `scan_sessions_for_display()` every cycle, then sleeps according to the adaptive cadence logic. See [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:16), [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:85), [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:194).

The current cadence rules are:

- `500ms` when anything changed or any session is not idle
- `1500ms` only after `30` consecutive all-idle unchanged cycles

That means any active session, any churn in display metadata, or any classification wobble immediately snaps the loop back to `500ms`. The measured intervals show that this downgrade rarely happens in real operation.

### High: Per-session idle classification dominates scan time

The display scan path still performs full classification for each detected process. See [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:661), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:688), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:731), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:749).

The log timings show `idle_ms` and `classify_ms` are the dominant cycle cost. Process scanning and ownership logic are effectively negligible in comparison.

The most expensive resolver is still Codex. On cache miss or ambiguity, Codex resolution can:

- scan up to `7` date directories
- enumerate and sort JSONL files by mtime
- open candidate files and parse the first line for `session_meta.payload.cwd`
- in multi-session cases, probe whether the PID has the file open

See [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:60), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:81), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:128), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:176), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:191), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:246).

This is defensible for correctness, but too expensive at current polling frequency.

### Medium: Tmux mapping still creates burst cost, but not baseline cost

The scanner uses a short-lived tmux pane cache with max age `2s`. If the process fingerprint is stable and tmux metadata is fresh, the cache is reused. Otherwise, the scanner refreshes pane metadata. See [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:213), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:442), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:480).

The logs support this split:

- `tmux_ms` median is `0`
- `tmux_ms` p95 is about `50ms`

So tmux is a spiky secondary contributor, not the main sustained burn.

### Medium: Thread count is high but stable

The daemon stabilizes around `77` threads. The main daemon server accepts TCP connections in a nonblocking loop and spawns per-connection handlers. See [server.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/server.rs:55), [server.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/server.rs:93), [server.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/server.rs:107).

This is not showing runaway churn in the monitor capture, so it looks more like accumulated service/watcher/background infrastructure footprint than an active bug. It is still worth tightening over time, but it is not the primary reason for the `~24%` steady-state CPU.

### Medium: Inotify/watch footprint is large, but the evidence points to memory/FD pressure more than CPU

The daemon sits around `75k` inotify watches and occasionally reaches `88k`. File watching is recursive and project-wide, with `.gitignore`-aware filtering. See [watch.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/watch.rs:16), [watch.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/watch.rs:62), [watch.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/watch.rs:85), [watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs:77), [watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs:128), [watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs:192).

There is no evidence in this capture that notify watchers themselves are the main CPU burn. The current evidence points to scanner cadence and idle classification first. The watch count is still a real systems concern because it drives:

- memory footprint
- FD pressure
- larger reconcile surfaces

### Low: Compaction runtime is no longer the main daemon CPU problem

The current daemon compaction runtime no longer contains the previously removed redundant `500ms` runtime-session scan loop. It now does three bounded things:

- bootstraps the extractor once from current runtime sessions
- watches team topology for watcher creation/removal
- reconciles watcher topology on the reconciliation interval

See [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs:36), [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs:67), [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs:82), [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs:107).

The extractor and watcher are also now event-oriented, with `5s` reconciliation loops rather than hot transcript rescans in the daemon path. See [compaction_extractor.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs:33), [compaction_extractor.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs:249), [compaction_watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs:28), [compaction_watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs:157).

So the older deep-dive conclusion about duplicate compaction polling is now historical, not current.

### Low: TCP accept loop and long-poll delivery are not current hotspots

The daemon server accept loop sleeps `50ms` on `WouldBlock`, and the session update API itself is Condvar-based long-polling rather than active polling. See [server.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/server.rs:120), [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:145), [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:320), [handlers.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/handlers.rs:332).

These paths may add incidental overhead, but they do not align with the measured hot metrics.

## What changed since the earlier deep dive

The older daemon CPU deep dive correctly identified one major waste source at the time: the redundant `DaemonCompactionRuntime` `500ms` runtime scan. That is no longer present.

Current problem statement:

- old issue fixed: duplicate compaction runtime polling removed
- current issue remains: display-session scanning cadence is still too aggressive for steady state
- current dominant cost: per-session idle resolution at near-constant `500ms`

That distinction matters because the next optimization pass should not keep chasing already-removed compaction scaffolding.

## Recommended implementation plan

### 1. Reduce steady-state display scan pressure first

Target: make `SessionActivityHub` spend most real steady-state time above `500ms`.

Recommended changes:

- Raise the idle cadence more aggressively once the system is stable.
- Stop requiring `30` perfect unchanged all-idle cycles before backing off.
- Separate “any active session exists” from “all sessions need fresh full classification”.
- Add a slower cadence tier for long-lived stable states.

Pragmatic phase-1 target:

- keep `500ms` only for active transition windows
- move stable mixed/idle monitoring closer to `1.5s-3s`

### 2. Split cheap liveness from expensive classification

Right now the daemon effectively pays the full classification tax each scan cycle. The system needs a cheaper steady-state path.

Recommended direction:

- maintain a lightweight fast path for process/pane presence and obvious active hints
- run the full idle classifier less frequently or only for sessions whose cheap signals changed
- cache Codex transcript ownership more aggressively per attached runtime session rather than per project lookup

### 3. Special-case Codex resolver cost

Codex is the main correctness-vs-cost tradeoff. The current project-path-to-transcript search is too expensive to keep hitting in a hot loop.

Recommended direction:

- lean on authoritative runtime attachment state wherever possible
- persist transcript binding more directly for attached managed members
- avoid broad per-project historical transcript scans in steady state

### 4. Treat watch count as a separate footprint project

The watch count is high enough to deserve its own capacity reduction pass, but not as the primary CPU fix for this task.

Recommended direction:

- inventory which watch roots are actually required concurrently
- reduce duplicate recursive watch coverage where possible
- keep `.gitignore` awareness, but evaluate whether some directories can move to coarser invalidation

### 5. Tighten thread model later, not first

The thread count is high but stable. Do not optimize this before fixing scanner cadence and classification cost, because that is where the measured CPU is actually going.

## Bottom line

The daemon is better than it was before the compaction-runtime cleanup, but it is still over budget in steady state. The current `~24%` single-core load is primarily a `SessionActivityHub` problem, not a compaction-runtime problem.

If only one thing gets fixed next, it should be this:

- cut the frequency of full display-session idle classification in steady state

That change has the strongest evidence behind it and the highest chance of materially reducing daemon CPU without weakening correctness.
