# Poll vs Event-Driven Audit

Task: `#738`
Owner: `architect`
Date: `2026-03-08`

## Executive Summary

Not all polling in Taurhaus is the same.

This audit separates polling into three classes:

1. **Core state polling**
   - steady-state loops that drive live product behavior
2. **Low-frequency safety-net polling**
   - periodic reconciliation for drift recovery
3. **One-shot handshake polling**
   - short-lived bootstrap/command wait loops

Main conclusion:
- only a small subset of current polling is a strong event-driven replacement candidate
- the highest-value replacement was compaction detection, and that is already the right redesign target
- most remaining polling either observes sources with no reliable native signal (`/proc`, file mtimes, process liveness), or is low-frequency and already appropriately scoped

Top recommendations:
1. Replace poll-based **compaction triggering** with the dedicated event-driven signal pipeline already designed in [event-driven-compaction-detection.md](/home/mstie/projects/taurhaus/docs/architecture/event-driven-compaction-detection.md)
2. Replace the **30s coordination self-heal monitor** with explicit lifecycle triggers plus a narrower fallback sweep
3. Keep **session/activity polling**, **stall detection polling**, and **daemon health polling** for now
4. Treat file watching as **already event-driven**, with periodic reconcile kept only as a safety net

## Evaluation Criteria

Recommend event-driven replacement when most of these are true:
- polling can miss time-sensitive events
- user-visible latency matters
- the data source already has a trustworthy event boundary
- the current poll loop does meaningful repeated work to find rare events

Keep poll-based when most of these are true:
- the source has no reliable event signal
- seconds-level delay is acceptable
- the loop is already adaptive or low-frequency
- event-driven replacement would add platform complexity for little product value

## Inventory Summary

| Pattern | What it polls | Interval / cadence | Candidate event source | Recommendation |
|---|---|---|---|---|
| SessionActivityHub display loop | process list, tmux panes, idle/runtime signals | `500ms` active, `1500ms` stable idle | none cleanly available today | Keep |
| Session idle / activity heuristics | `/proc` IO, TCP sockets, transcript mtimes | piggybacks on scanner cadence | tool-native hooks/telemetry, not currently available | Keep |
| Foreground / active attribution | process IO + transcript activity + tmux focus state | piggybacks on scanner cadence; tmux focus already event-driven | more tool-native per-session signals | Future consideration |
| Daemon health monitor | daemon connectivity via ping/reconnect | `30s` normal, `2s` fast recovery | socket failure / listener failure / connection state | Keep for now |
| Session updates bridge | long-poll daemon versioned session snapshot | `20s` wait timeout, `1s` retry | already event-driven above daemon | Keep |
| File watcher layer | notify events with debounce | event-driven; internal poll interval configured for backend | already notify/inotify/FSEvents based | Keep |
| Activity watch reconcile | watched-project set drift | `60s` periodic safety net + explicit triggers | explicit project/settings/watch lifecycle events | Keep low-frequency safety net |
| Coordination self-heal monitor | team runtime/daemon drift | `30s` | startup, resume, daemon-recovered, mesh/team config changes, session attach/detach | Replace |
| Task scanners | Claude/Codex/Gemini task sources | on-demand, not timer-owned | file watchers / hooks could precompute cache | Keep |
| Stall detector | member inactivity/stall thresholds | default `30s` | no single event for “absence of progress” | Keep |
| Bootstrap/command handshakes | daemon startup, pane exit, command availability | short-lived `10-500ms` sleeps | none worth adding | Keep |

## Detailed Assessment

## 1. SessionActivityHub display loop

What it polls:
- `scan_sessions_for_display()` in the daemon-owned session hub
- process scanning
- tmux pane listing
- idle/runtime heuristics derived from transcript mtimes and `/proc`

Cadence:
- `500ms` active
- `1500ms` after 30 stable idle cycles

Key file:
- `src-tauri/src/daemon/session_activity.rs`

Reliability assessment:
- Polling does not inherently lose session existence information because session state is not a discrete rare event like compaction
- It can produce classification lag, but not the same “miss forever” failure mode

Latency assessment:
- `500ms` is appropriate for live session presence and activity pill updates
- User value is real-time-ish, but sub-second is already sufficient

Efficiency assessment:
- This is the heaviest steady-state poller in the backend
- However, it already uses adaptive cadence and powers multiple features at once

Could event-driven replace it?
- Not cleanly today
- True replacement would require reliable process lifecycle events, tmux pane lifecycle events, and tool-native activity signals across Linux/macOS/WSL
- That is a much larger architecture move than compaction detection

Recommendation:
- **Keep poll-based**

Reason:
- There is no equivalent trustworthy unified event source today
- Replacing this now would add major platform complexity for uncertain reliability gains

Effort if replaced:
- Very high

## 2. Session activity / idle heuristics

What it polls:
- `/proc/PID/io` for Claude and Codex IO hysteresis
- `/proc/PID/fd` + TCP tables for Gemini API-connection activity
- transcript file mtimes for Claude/Codex/Gemini fallback state

Cadence:
- tied to the session scanner cadence above
- thresholds include `5s` general mtime and `10s` Codex mtime window

Key files:
- `src-tauri/src/session_scanner/proc_io.rs`
- `src-tauri/src/session_scanner/idle/mod.rs`
- `src-tauri/src/session_scanner/idle/*.rs`

Reliability assessment:
- These heuristics are not ideal, but they are observing sources that do not expose a stable unified event API
- For Codex/Claude/Gemini, the main problem is absence of a canonical “active/inactive” event, not poll design quality

Latency assessment:
- Current latency is acceptable for activity pills and hover state
- Hysteresis intentionally trades some immediacy for stability

Efficiency assessment:
- Moderate ongoing cost because these checks happen every scan cycle
- But the work is already amortized by the session scanner

Could event-driven replace it?
- Only if the tools expose durable hooks, telemetry, or explicit lifecycle APIs
- Today they do not, at least not across all three tools in a uniform way

Recommendation:
- **Keep poll-based**

Reason:
- no reliable event source exists for “this process is actively working right now”

Effort if replaced:
- Very high

## 3. Foreground / active attribution detection

What it polls:
- session activity signals and runtime metadata inside the scanner
- tmux focus state participates, but focus itself is already hook-driven

Cadence:
- piggybacks on session scanner cadence

Relevant boundary:
- tmux focus file updates are already event-driven via hooks
- attribution still depends on poll-based process/runtime state

Reliability assessment:
- current design is mixed-mode already: event-driven focus + polled work detection
- that is the right split for now

Latency assessment:
- acceptable for sidebar foreground indication

Efficiency assessment:
- marginal extra cost beyond the scanner itself

Recommendation:
- **Future consideration**, not current replacement target

Reason:
- the most valuable event-driven piece (tmux focus) already exists
- the remaining signals still depend on poll-only sources

Effort if replaced further:
- High

## 4. Daemon health monitor polling

What it polls:
- daemon connectivity with `ping()` and reconnect/restart logic

Cadence:
- `30s` normal
- `2s` while disconnected or recovering

Key file:
- `src-tauri/src/daemon_lifecycle.rs`

Reliability assessment:
- health polling is a coarse safety net, not the primary app data path
- the app also has the session updates bridge and connection attempts, so this is somewhat duplicative

Latency assessment:
- 30s normal detection is fine for a background health monitor
- fast recovery loop already reduces perceived downtime after disconnects

Efficiency assessment:
- low cost

Could event-driven replace it?
- partially
- connection/listener failures could drive more of this from transport-level events
- but restart policy still benefits from a periodic supervisory loop

Recommendation:
- **Keep poll-based for now**

Reason:
- cost is low and the loop is operationally simple
- not the kind of polling currently hurting product correctness

Effort if replaced:
- Medium

## 5. File watcher debounce / batch processing

What it polls:
- effectively nothing at the product level; it is already driven by `notify`
- `with_poll_interval(Duration::from_secs(2))` is a watcher backend configuration detail, not a product-owned recurring scan of the tree
- git changes are debounced, not polled

Key files:
- `src-tauri/src/fs/watcher.rs`
- `src-tauri/src/event_processor.rs`

Reliability assessment:
- already event-driven where it matters

Latency assessment:
- good enough; debounce exists deliberately to coalesce storms

Efficiency assessment:
- strong as-is

Recommendation:
- **Keep**

Reason:
- this is already an event-driven architecture with bounded debounce windows

Effort if replaced:
- None warranted

## 6. Task scanners (`claude.rs`, `codex.rs`, `gemini.rs`)

What they poll:
- strictly speaking, they do not own a timer loop
- they scan task sources on demand when task data is requested or recomputed

Key files:
- `src-tauri/src/task_scanner/mod.rs`
- `src-tauri/src/task_scanner/claude.rs`
- `src-tauri/src/task_scanner/codex.rs`
- `src-tauri/src/task_scanner/gemini.rs`

Reliability assessment:
- on-demand scanning is acceptable because task views tolerate slight staleness better than compaction detection

Latency assessment:
- not a real-time hot path in the same way session activity or compaction is

Efficiency assessment:
- acceptable because it is request-driven rather than timer-driven

Could event-driven replace it?
- yes, but only as a cache invalidation optimization:
  - Claude task directory watcher
  - Codex transcript delta watcher
  - Gemini TODO watcher
- that would be an optimization, not a correctness rescue

Recommendation:
- **Keep**

Reason:
- not an actual steady-state polling loop today
- event-driven rewrite would be change-for-change's-sake right now

Effort if replaced:
- Medium

## 7. Activity watch reconcile safety net

What it polls:
- watched project set drift / activity watch registration drift

Cadence:
- startup reconcile once
- periodic reconcile every `60s`
- also triggered explicitly on relevant project/settings commands

Key file:
- `src-tauri/src/startup/watchers.rs`

Reliability assessment:
- this is a safety net for watcher drift, not a primary state pipeline
- explicit triggers already handle the common cases

Latency assessment:
- seconds-to-minute delay is acceptable because this is recovery logic

Efficiency assessment:
- cheap

Recommendation:
- **Keep low-frequency safety net**

Reason:
- event-driven triggers already exist for the main cases
- the 60s loop is a sensible repair backstop, not wasteful steady-state churn

Effort if replaced entirely:
- Medium, with little payoff

## 8. Coordination self-heal monitor

What it polls:
- team runtime and daemon drift via `run_background_self_heal_pass()`

Cadence:
- initial delay `5s`
- then every `30s`

Key file:
- `src-tauri/src/startup/mod.rs`

Reliability assessment:
- this loop exists because runtime/daemon state can drift, but the drift often originates from known lifecycle transitions:
  - startup
  - resume
  - daemon recovery
  - team config changes
  - member attach/detach
- periodic polling is a blunt instrument here

Latency assessment:
- 30s delay can leave state stale longer than necessary

Efficiency assessment:
- moderate wasted work because it scans even when nothing changed

Event source candidates:
- startup completion
- daemon recovered event
- team initialize/add/remove/disband
- runtime member record mutation
- session attach/detach changes

Recommendation:
- **Replace**

Reason:
- this is the strongest remaining event-driven opportunity after compaction
- the domain already has meaningful lifecycle edges to trigger bounded self-heal directly

Suggested replacement:
- event-driven self-heal queue triggered by lifecycle mutations
- retain an infrequent fallback sweep only as a recovery guard, e.g. every 10-15 minutes

Effort:
- Medium

## 9. Stall detector polling

What it polls:
- multi-signal member state snapshots and thresholds
- runtime state, session scanner state, mesh-side signals

Cadence:
- configurable, default `30s`

Key file:
- `src-tauri/src/coordination/stall_detector.rs`

Reliability assessment:
- stall detection is fundamentally about absence over time
- absence is one of the classic cases where polling remains appropriate

Latency assessment:
- 30s is reasonable for soft/hard stall thresholds

Efficiency assessment:
- low to moderate; targeted to tracked members only

Could event-driven replace it?
- not fully
- individual signals can become more event-rich, but the decision still requires elapsed-time evaluation

Recommendation:
- **Keep poll-based**

Reason:
- this is the right kind of problem for periodic evaluation

Effort if replaced:
- High, with unclear benefit

## 10. Session updates bridge long-poll

What it polls:
- daemon versioned session snapshot via `wait_session_updates`

Cadence:
- `20s` long-poll wait timeout
- `1s` reconnect retry

Key file:
- `src-tauri/src/daemon_lifecycle.rs`
- `src-tauri/src/daemon/session_listener.rs`

Assessment:
- this is already effectively event-driven above the daemon boundary
- the long-poll is just transport plumbing

Recommendation:
- **Keep**

Reason:
- no meaningful replacement needed unless the transport itself changes to push sockets

Effort if replaced:
- High for little gain

## 11. Bootstrap and command handshake polling

Examples:
- daemon startup `poll_until_reachable()` (`500ms`)
- session stop waits for shell return / process exit
- various test/command readiness waits with `10-100ms` sleeps

Assessment:
- these are short-lived handshake loops, not architectural product polling

Recommendation:
- **Keep**

Reason:
- local command/process readiness usually does not justify a more elaborate event source

Effort if replaced:
- Low to medium, but low value

## Recommendations Ranked

### 1. Replace: coordination self-heal monitor

Why first:
- real lifecycle edges already exist
- lower latency and less wasted scanning
- medium effort, clear payoff

### 2. Replace: compaction detection trigger path

Status:
- already identified and architected separately
- see [event-driven-compaction-detection.md](/home/mstie/projects/taurhaus/docs/architecture/event-driven-compaction-detection.md)

Why second:
- highest correctness payoff overall
- but it is already in motion as its own dedicated redesign

### 3. Future consideration: foreground attribution improvements

Why not now:
- only partial event-driven improvement remains
- core work detection is still poll-bound

### 4. Keep: session/activity scanner and idle heuristics

Why:
- no robust cross-tool event source exists
- current adaptive cadence is a pragmatic compromise

### 5. Keep: daemon health monitor

Why:
- low cost
- operational safety net
- not a correctness pain point today

### 6. Keep: stall detector

Why:
- time-window absence detection is naturally periodic

### 7. Keep: task scanners as on-demand scans

Why:
- not actually a background poll loop
- event-driven would be optimization, not rescue

## Bottom Line

The codebase does contain recurring polling, but only two categories stand out as meaningful event-driven replacement targets:
- compaction triggering
- coordination self-heal sweeps

Everything else is either:
- already event-driven enough at the product boundary
- observing sources with no reliable native event signal
- or cheap low-frequency safety-net work that is acceptable as-is

So the correct architectural stance is selective, not ideological:
- replace polling where it misses rare, important edges
- keep polling where the world does not provide a trustworthy event
