# Windows App Stall Investigation - 2026-03-11

## Objective

Investigate the recurring Windows stall where `taurhaus.exe` becomes unresponsive, the title bar turns grey, and the app often has to be killed manually.

## Evidence Used

- Windows app log: `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- Resource monitor capture: `/tmp/taurhaus-resource-monitor-v2.csv`
- Relevant code paths:
  - `src-tauri/src/session_scanner/process.rs`
  - `src-tauri/src/session_scanner/mod.rs`
  - `src-tauri/src/daemon/session_activity.rs`
  - `src-tauri/src/commands/tasks.rs`
  - `src-tauri/src/daemon/handlers.rs`
  - `src-tauri/src/commands/coordination.rs`
  - `src/lib/components/meshTabController.svelte.js`
  - `src/lib/projectSelection.js`

## Executive Summary

The strongest confirmed pattern is not a `taurhaus.exe` CPU or memory runaway. The hotspot is the WSL-side `taurhaus-daemon` session scanner, which runs at a 500 ms cadence while sessions are active and occasionally stalls for about 2 to 4 seconds on process scanning. Multiple frontend/background features then amplify that same daemon pressure:

1. Mesh runtime status polls every 2 seconds.
2. Task reads schedule background task rescans that call back into session scanning.
3. Project selection and hover prefetch repeatedly reload overview sections.

The user's task-detection suspicion is partially valid, but it is not the primary root cause. Task detection is a secondary amplifier of the same lower-level daemon/session-scan problem.

## Confirmed Patterns

### 1. The UI process does not look resource-bound

From `/tmp/taurhaus-resource-monitor-v2.csv`:

- `taurhaus.exe` max CPU: `3.28%` at `2026-03-10T23:51:58+01:00`
- `taurhaus.exe` max RSS: `85.66 MB` at `2026-03-11T01:38:39+01:00`
- `taurhaus.exe` max threads: `66` at `2026-03-11T01:37:00+01:00`
- `taurhaus.exe` max handles: `921` at `2026-03-10T23:52:09+01:00`

That does not match a straightforward UI-process CPU spike or memory blow-up.

### 2. The daemon is the main hot process

From the same monitor capture:

- `taurhaus-daemon` max CPU: `85.94%` at `2026-03-11T03:12:48+01:00`
- Other daemon spikes: `80.76%` at `2026-03-10T23:29:22+01:00`, `59.91%` at `2026-03-11T00:42:19+01:00`

This points to WSL-side work starving responsiveness indirectly rather than the Tauri window process doing the expensive work itself.

### 3. Session scanning is continuously active and occasionally very slow

From the JSONL log:

- `session_scanner.scan.completed`: `2648` events in the current capture
- Average duration: `123.8 ms`
- P95 duration: `171 ms`
- Max duration: `4097 ms`

Representative slow scans:

- `2026-03-11T01:16:05.036Z`: `duration_ms=2165`
- `2026-03-11T01:32:45.919Z`: `duration_ms=2163`
- `2026-03-11T01:33:01.020Z`: `duration_ms=4097`, `process_scan_ms=2009`, `session_count=0`
- `2026-03-11T01:34:12.298Z`: `duration_ms=4091`, `process_scan_ms=2035`, `session_count=0`

The 2.0 second signature matches the hard timeout in `src-tauri/src/session_scanner/process.rs:112-154`, where `ps` subprocesses are killed after `Duration::from_secs(2)`.

That same scanner is driven continuously by the daemon session hub in `src-tauri/src/daemon/session_activity.rs:17-20` and `src-tauri/src/daemon/session_activity.rs:224-283`, which keeps a `500 ms` active cadence.

### 4. Mesh runtime status polling is frequent and expensive enough to amplify stalls

From the JSONL summary:

- `coordination_get_live_team_status`: `134` completed calls
- Average duration: `193.2 ms`
- P95 duration: `476 ms`
- Max duration: `978 ms`

Representative burst:

- `2026-03-11T02:12:04.981Z` to `2026-03-11T02:12:05.959Z`: live-team-status call took `978 ms`
- Nearby calls repeatedly landed in the `393-478 ms` range

This poll is driven by `src/lib/components/meshTabController.svelte.js:1556-1589`, which schedules runtime refresh every `RUNTIME_STATUS_POLL_MS` while the runtime view is visible. The backend path is `src-tauri/src/commands/coordination.rs:197-215` plus `src-tauri/src/commands/coordination.rs:556-620`, which reaches the daemon snapshot endpoint on each refresh.

The snapshot read itself is cheap, but the daemon keeps that snapshot fresh with the same scanner loop above. So the mesh runtime view keeps consuming a resource already under stress.

### 5. Task detection is a real amplifier, but not the primary cause

From the JSONL summary:

- Daemon `get_project_tasks` RPCs: `51`
- Average duration: `148.2 ms`
- P95 duration: `432 ms`
- Max duration: `2058 ms`

Representative slow task-scan window:

- `2026-03-11T01:33:01.784Z` to `2026-03-11T01:33:03.843Z`: `get_project_tasks` daemon RPC took `2058 ms`

Task refresh architecture:

- Every successful `get_project_tasks` IPC schedules a background refresh in `src-tauri/src/commands/tasks.rs:58-85`.
- That refresh calls `scan_tasks_from_files(...)` in `src-tauri/src/commands/tasks.rs:252-327`.
- On daemon-backed WSL paths, task scanning reaches `src-tauri/src/daemon/handlers.rs:392-449`, which calls `scan_sessions_for_runtime()` when cache misses.
- `scan_sessions_for_runtime()` uses the same process/tmux scan path in `src-tauri/src/session_scanner/mod.rs:889-930`.

So task detection does correlate with the stalls, but because it re-enters the same session-scanner machinery that is already showing 2 second timeout behavior.

### 6. There is extra background churn unrelated to task detection

One long run (`run_82c73e995ebe4b80a092742fc268380e`) also logged:

- `get_recent_commits`: `412`
- `get_latest_session`: `412`
- `get_relationships`: `412`
- `get_project`: `242`
- `list_sessions`: `242`
- `get_readme`: `242`

That pattern lines up with project selection and hover prefetch behavior in `src/lib/projectSelection.js:64-145`, which always fans out six parallel section requests and only deduplicates while a given request is already in flight. There is no short-lived post-resolution cache, so repeated hover/select patterns can keep refetching the same data.

This looks like secondary background pressure, not the main stall trigger, but it reduces overall headroom.

## Strongest Root Cause / Hypothesis Ranking

### 1. Primary: daemon session scanner overload on WSL, especially around `ps` timeouts

Confidence: high

Why:

- The monitor shows daemon CPU spikes, not UI-process spikes.
- The log shows repeated `session_scanner.scan.completed` events with a clear `~2000 ms` timeout signature.
- The scanner loop is intentionally aggressive at `500 ms` while active.
- Multiple user-visible features depend on that scanner output.

Most likely failure mode:

- WSL-side process enumeration or `/proc`-adjacent reads occasionally stall.
- The daemon scanner falls behind.
- Frontend requests that depend on daemon freshness become slow enough to make the app feel hung.
- Windows marks the window as non-responsive when message processing is delayed long enough, even if the root pressure is not a pure UI CPU spike.

### 2. Secondary: mesh runtime polling keeps hitting the stressed subsystem

Confidence: high

The mesh runtime tab polls every 2 seconds and those calls often land in the `400-900 ms` range. That is not enough alone to prove the freeze, but it is a strong amplifier when the daemon is already unhealthy.

### 3. Secondary: task detection/background task refresh re-enters the same scanner path

Confidence: medium-high

The user's suspicion was directionally correct. Task refreshes do correlate with slow windows, but the deeper issue is that task scans share the same session-scan machinery, so task detection is not an isolated root cause.

### 4. Tertiary: project overview/prefetch chatter wastes headroom

Confidence: medium

The repeated overview-section fetches are not the main stall signature, but they are a clear source of unnecessary background work.

### 5. Unconfirmed / weak: DB lock contention or titlebar-specific rendering bugs

Confidence: low

Why not:

- `ipc.lock.wait` is almost absent; `get_project_tasks/db` waits were `0 ms`.
- The only notable lock wait was `get_commit_files/db`, max `717 ms`, and it is not frequent enough to explain the broad stall pattern.
- There is no log evidence pointing to custom titlebar logic as the cause.

## Recommended Remediation Order

### 1. Reduce scanner pressure first

Highest-value change:

- Back off the daemon session scanner when repeated process-scan timeouts occur.
- Reuse the last known good snapshot instead of immediately re-entering the same 500 ms loop after a timeout.
- Emit structured timeout events into JSONL instead of only a generic warning, so this can be tracked directly.

Why first:

- It attacks the common root for mesh status, task detection, and session updates.

### 2. Stop mesh runtime from polling every 2 seconds against daemon freshness

Preferred direction:

- Move mesh runtime status toward event-driven or long-poll updates instead of fixed 2 second polling.
- Reuse the existing daemon session update/version model rather than re-querying live status continuously.

Why second:

- This removes a constant read load from the subsystem already showing stress.

### 3. Decouple task reads from automatic background task rescans

Preferred direction:

- Do not schedule a file/task rescan on every successful `get_project_tasks` read.
- Refresh on explicit invalidation signals: session changes, task files changing, tab activation, or a slower stale-while-revalidate budget.

Why third:

- Task detection is not the main cause, but it is a confirmed amplifier and the current read path is more expensive than it needs to be.

### 4. Add a short TTL cache for project selection / hover prefetch

Preferred direction:

- Keep the current in-flight dedupe, but add a short resolved-result cache for `getProject`, `getRecentCommits`, `getLatestSession`, `listSessions`, `getReadme`, and `getRelationships`.

Why fourth:

- This is low-risk headroom recovery and should reduce noisy IPC/RPC churn.

### 5. Add hang-focused instrumentation before changing anything more invasive

Add:

- Structured `session_scanner.process_timeout` events with command name and elapsed time
- Scanner loop lag / overrun metrics
- Per-feature refresh counters for mesh runtime polls, task refreshes, and project prefetches
- Optional UI-thread heartbeat or freeze detector for `taurhaus.exe`

Why:

- Current logs are strong enough to rank the likely causes, but not enough to prove exact UI-thread starvation mechanics.

## Low-Risk Fix Implemented During Investigation

None.

Reason:

- The strongest remediations change cross-cutting runtime behavior in the daemon/session/mesh/task paths.
- That needs an implementation decision, not an opportunistic one-line patch.

## Validation Performed

- Confirmed both required artifacts existed and were current:
  - `/tmp/taurhaus-resource-monitor-v2.csv`
  - `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- Summarized daemon/UI resource peaks from the monitor capture.
- Summarized IPC and daemon RPC durations from the JSONL log.
- Pulled specific slow windows and matched them to code paths.
- Verified task-detection flow and mesh runtime flow against source.
- Checked lock-wait evidence to rule out DB contention as the main explanation.

## Bottom Line

The strongest current explanation is:

- WSL daemon session scanning is intermittently timing out on process enumeration.
- Mesh runtime polling and task/background refreshes multiply the impact of that stress.
- Task detection is part of the problem, but as a secondary consumer of the same stressed scanner path, not as the primary root cause by itself.
