# Daemon CPU Deep Dive — 2026-03-09

> Status update (later on 2026-03-09): this analysis accurately described the daemon before the independent `DaemonCompactionRuntime` 500 ms runtime-session loop was removed. Treat the duplicated-scan findings here as historical pre-fix context, not current-state architecture.

## Objective

Identify what is actually burning CPU in the daemon's steady-state path, classify each recurring loop/function by reduction strategy, and propose concrete next steps with expected impact.

Scope here is steady-state only, not startup burst behavior.

## Bottom Line

The daemon is not primarily burning CPU on TCP accept, watcher reconciliation, tmux enumeration, or process-list enumeration anymore.

The dominant steady-state cost is repeated session idle classification, especially transcript/file-based idle resolution, running at 500 ms cadence.

The second major problem is architectural duplication:

1. `SessionActivityHub` runs `scan_sessions_for_display()` roughly every 500 ms while any session is active.
2. `DaemonCompactionRuntime` separately runs `scan_sessions_for_runtime()` every 500 ms.
3. Both paths reuse the same expensive process/tmux/input discovery stack and, more importantly, the same per-session idle resolution work.

That duplicated scanning explains most of the observed `~49%` of one CPU core.

## Evidence

### Resource monitor capture

From `/tmp/taurhaus-resource-monitor.csv` for `taurhaus-daemon`:

- rows with CPU samples: `6677`
- average CPU: `46.16%`
- median CPU: `45.01%`
- p95 CPU: `64.8%`
- max CPU: `242.98%` (startup/outlier burst, not the main focus)

Memory, threads, and watches were stable; this is a loop/cadence problem, not churn.

### Active daemon scanner metrics

From the current daemon log run in:

- `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- run id: `run_45f18923c307478e860aa8293b29e3d5`

`session_scanner.scan.completed` stats for that run:

| Metric | Avg | Median | P95 | Max |
| --- | ---: | ---: | ---: | ---: |
| `process_scan_ms` | `0.26` | `0` | `0` | `50` |
| `tmux_ms` | `0.32` | `0` | `0` | `50` |
| `classify_ms` | `119.24` | `113` | `165` | `217` |
| `idle_ms` | `117.49` | `111` | `163` | `213` |
| `process_signal_ms` | `0.04` | `0` | `0` | `1` |
| `ownership_ms` | `0.45` | `0` | `2` | `3` |
| `duration_ms` | `160.05` | `158` | `212` | `2231` |

Cache hit rate in the same run:

- process cache hits: `770 / 774`
- tmux cache hits: `769 / 774`

That matters because it proves the hot path is no longer `ps` or `tmux list-panes`. The cost is in classification, and classification is almost entirely `idle_ms`.

### Cadence proof

For the same run, consecutive `session_scanner.scan.completed` events were spaced at:

- average interval: `0.508s`
- median interval: `0.500s`
- p95 interval: `0.559s`
- `<= 0.6s`: `847` of `853` intervals

So this is not an occasional 500 ms fast lane. In real steady-state usage, the display scanner is effectively running at the fast cadence almost continuously.

### CPU budget estimate

Using the observed display scan cost:

- display scanner budget: `160.05 ms / 0.508 s ~= 31.5%` of one core
- idle classification alone: `117.49 ms / 0.508 s ~= 23.1%` of one core

That already explains most of the daemon budget without counting the separate runtime compaction scan.

## Ranked CPU Consumers

### 1. Display session scanner idle classification

Files:

- `src-tauri/src/daemon/session_activity.rs`
- `src-tauri/src/session_scanner/mod.rs`
- `src-tauri/src/session_scanner/idle/`

Why it is hot:

- `SessionActivityHub` scans at `500 ms` while any session is active.
- In the active run, this path spent `~160 ms` per cycle.
- `~117 ms` of that `~160 ms` is `idle_ms`, so the hot work is transcript/file-based session-idle resolution, not process or tmux discovery.

What the code is doing:

- `scan_sessions_for_display()`
- `classify_display_runtime_sessions_with(...)`
- `detect_runtime_idle_for_process(...)`
- tool-specific idle resolvers in `idle/claude.rs`, `idle/codex.rs`, `idle/gemini.rs`

Why `idle_ms` is expensive:

- Claude checks main JSONL and subagent directory mtimes.
- Gemini checks chat directory latest-file mtimes.
- Codex is worst:
  - scans up to 7 date directories
  - sorts JSONL files by mtime
  - opens/parses first line of candidate transcripts to match `cwd`
  - in multi-session cases, also probes per-PID file ownership via `/proc`

Classification:

- `b` partially event-driven in the future
- `a` reduce frequency immediately

Why:

- The app/UI does need fresh session state.
- But `500 ms` is too aggressive for transcript/file idle detection at current implementation cost.
- There is no evidence that `500 ms` is required to preserve product behavior.

Expected impact:

- Moving this fast lane from `500 ms` to `1000 ms` would roughly halve this dominant cost.
- Based on current numbers, that alone likely saves around `15-16` percentage points of a core.

### 2. Duplicate runtime scan for compaction

Files:

- `src-tauri/src/daemon/compaction.rs`
- `src-tauri/src/session_scanner/mod.rs`

Why it is hot:

- `DaemonCompactionRuntime` still calls `scan_sessions_for_runtime()` every `500 ms`.
- That runtime scan reuses the same cached process/tmux inputs but re-runs per-process idle detection.
- `scan_sessions_for_runtime()` calls `detect_runtime_idle_for_process(...)` again for every session.

Why this matters:

- The previous audit removed unchanged fanout after the runtime scan.
- It did **not** remove the second scan itself.
- So we still pay the transcript-idle resolution cost twice:
  - once for display activity
  - once for compaction attachment/runtime session metadata

Classification:

- `b` should be made event-driven from the display/runtime scanner authority

Why:

- Compaction processing does not need an independent full session scan loop.
- It needs current runtime attachment data.
- That runtime-rich data should be produced once and shared, not rediscovered by a second loop.

Expected impact:

- This is the second major win.
- Because the runtime scan duplicates the same idle detection stack, removing it could plausibly save another `10-20` percentage points of a core depending on how often its result set actually changes.
- Combined with reducing the display cadence, this is the path to taking the daemon well below the current `~49%`.

### 3. Codex project-to-transcript resolution strategy

Files:

- `src-tauri/src/session_scanner/idle/codex.rs`

Why it matters:

- The expensive part is not just “scan more slowly.”
- The Codex resolver currently reconstructs transcript ownership from the filesystem on the hot path.
- It scans recent day directories and matches transcripts back to project path by opening/parsing files.

This is architecturally expensive because:

- transcript binding is effectively attachment state
- attachment state already belongs to Taurhaus runtime
- the hot path is re-deriving it from disk every cycle

Classification:

- `b` should move toward persisted attachment/runtime authority

Expected impact:

- medium to high, especially for multi-session Codex-heavy teams
- reduces both display and compaction scan cost once attachment state is authoritative

## Low-Impact or Already-Minimal Loops

### Daemon TCP accept loop

File:

- `src-tauri/src/daemon/server.rs`

Current behavior:

- nonblocking `accept()`
- `50 ms` sleep on `WouldBlock`

Classification:

- `c` already minor

Why:

- It wakes often, but the scanner evidence shows the CPU is elsewhere.
- This loop is worth cleaning up only after scanner duplication is fixed.

Potential future cleanup:

- blocking accept + shutdown pipe/eventfd

Expected impact:

- low

### Compaction signal watcher

File:

- `src-tauri/src/session_scanner/compaction_watcher.rs`

Current behavior:

- `notify`-driven
- `250 ms` loop tick
- `5 s` reconciliation interval

Classification:

- `c` already minimal enough

Why:

- this path is mostly blocking on filesystem events
- it is not showing up in measured scanner metrics

Expected impact from further tuning:

- low

### Session update listener and filesystem event listener

Files:

- `src-tauri/src/daemon/session_listener.rs`
- `src-tauri/src/daemon/event_listener.rs`

Classification:

- `c` already minimal and should stay

Why:

- long-poll / blocking read
- no evidence they are dominating CPU

## Actionable Recommendations

### 1. Reduce display scanner fast cadence from 500 ms to 1000 ms

Change:

- `src-tauri/src/daemon/session_activity.rs`
- increase `ACTIVE_SCAN_INTERVAL`

Why first:

- smallest code change
- immediate measurable win
- directly attacks the largest proven CPU consumer

Expected impact:

- likely biggest same-day reduction
- approximately halve the display scanner contribution

### 2. Remove the independent compaction runtime scan

Change:

- make compaction runtime consume a shared runtime-rich scanner snapshot instead of calling `scan_sessions_for_runtime()` itself every 500 ms

Practical direction:

- extend `SessionActivityHub` (or sibling shared scanner state) to keep `RuntimeSession` data, not only `DisplaySession`
- derive the display-safe view from that shared runtime snapshot
- let compaction runtime subscribe to changes instead of rescanning

Expected impact:

- very high
- removes the largest duplicate work in the daemon

### 3. Stop re-deriving Codex transcript ownership from disk on the hot path

Change:

- persist runtime attachment/transcript binding as authoritative state
- use that directly in hot loops

Practical direction:

- rely on runtime attachment state for:
  - `session_id`
  - `jsonl_path`
  - pane/session binding
- reserve filesystem scan for recovery/reconciliation, not every steady-state cycle

Expected impact:

- medium to high
- especially important once the duplicate runtime scan is removed, because it also reduces remaining display-scan cost

### 4. Only then consider minor loop cleanup

Examples:

- TCP accept loop sleep interval / blocking accept design
- compaction watcher loop tick tuning

Expected impact:

- low
- not worth prioritizing before scanner/cadence work

## Classification Summary

| Consumer | Classification | Decision |
| --- | --- | --- |
| Display session scanner cadence | `a` reduce frequency now, `b` partial event-driven later | Reduce fast cadence; keep daemon-owned scanner |
| Display idle classification internals | `b` | Move more attachment/transcript state out of hot path |
| Compaction runtime full runtime scan | `b` | Replace with shared scanner snapshot / event-driven subscription |
| Codex transcript resolution | `b` | Replace repeated disk inference with authoritative runtime attachment |
| TCP accept loop | `c` | Leave alone for now |
| Compaction watcher | `c` | Leave alone for now |
| Session listener / event listener | `c` | Leave alone |

## Conclusion

The daemon's steady-state CPU problem is now narrow and clear:

- one expensive display scanner loop running at `~500 ms`
- one second runtime compaction loop repeating almost the same idle-resolution work

This is no longer a broad “too many polling loops” issue. It is primarily a duplicated scanner architecture problem plus an overly aggressive fast cadence for expensive transcript-idle classification.

If only one fix is made next, make it **reduce the display scanner cadence**.

If the goal is to materially solve the `~49%` CPU problem, the real fix is **remove the separate compaction runtime scan and share one runtime-rich scanner snapshot**.
