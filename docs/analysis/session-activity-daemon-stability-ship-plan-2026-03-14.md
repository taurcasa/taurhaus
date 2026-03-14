# Session Activity Daemon Stability Ship Plan

**Date:** 2026-03-14
**Task:** #1285

## Verdict

The broad lane is currently **NO-SHIP** as a full "session activity and daemon
stability are solved" claim.

The inputs do not support that claim yet:

- `#1281` defined the full ship matrix and explicit no-ship conditions.
- `#1282` did **not** reproduce a raw WSL daemon transport failure under the
  tested load; it reproduced shared-client contention.
- `#1283` confirmed the current tool split instead of supporting a unified
  cross-tool signal model.
- `#1284` found the safest first product slice at the UI boundary: retain the
  last session snapshot across short disconnects, but mark it stale.

So the right ship plan is not a broad daemon rewrite. It is one bounded
implementation sequence:

1. ship the UI/session resilience improvement now,
2. add visibility for shared-client contention so it stops being mistaken for
   daemon loss,
3. do **not** reopen scanner semantics or daemon transport architecture unless
   the next evidence pass proves they are actually the problem,
4. only call the whole lane shipped after the remaining matrix items pass.

## What The Experiment Set Actually Says

## 1. Current scanner direction is mostly confirmed

`#1283` supports keeping the current tool-specific activity model:

- Codex should continue to lean on transcript binding plus per-PID IO when
  deciding which session is active.
- Claude should continue to treat `/proc/<pid>/io` bursts as the primary active
  signal, with transcript mtimes as supporting evidence rather than the main
  source of truth.

That means the current scanner split is directionally right. The experiments do
not justify a new one-size-fits-all signal model.

## 2. Current daemon symptom is probably being mislabeled

`#1282` matters because it changes the default suspect.

Under the tested WSL load:

- the isolated daemon stayed reachable,
- an independent probe connection kept answering,
- transport/protocol errors stayed at zero,
- but app-like shared-connection probes hit immediate `busy` outcomes.

That means the most reproducible failure shape is:

- **foreground status work competing on a shared client connection**

not:

- **the daemon process falling over under modest load**

So the next implementation slice should attack contention visibility and UI
behavior before it touches daemon startup, reconnect timing, or server-side
transport assumptions.

## 3. The first safe product fix is at the UI boundary

`#1284` identifies the cleanest immediate improvement:

- keep the last known session snapshot during short disconnects
- mark it stale while the session bridge is not live
- measure the real recovery window from disconnect observation to first restored
  snapshot emission

That is a better first slice than clearing all presence immediately or trying
to "fix" daemon loss that the experiments did not actually prove.

## Ship / No-Ship Decisions

| Area | Decision | Why |
|------|----------|-----|
| Full lane ship claim | `NO-SHIP` | The full matrix from `#1281` is not closed. The current results narrow the problem, but they do not prove scanner, daemon delivery, export semantics, churn handling, stale-daemon rotation, and reconnect recovery end to end. |
| Tool-specific signal split | `SHIP AS CURRENT DIRECTION` | `#1283` supports Codex transcript binding plus IO and Claude proc-IO-first behavior. Reopening that now would create churn without new evidence. |
| UI stale snapshot retention | `SHIP NOW` | `#1284` identifies this as the lowest-risk user-facing improvement and the most honest way to absorb short disconnects. |
| Shared-client contention visibility | `SHIP NOW` | `#1282` reproduced contention, not transport death. The product needs to distinguish `busy` from `disconnected`. |
| Daemon transport rewrite | `NO-SHIP` | The tested evidence does not support making the daemon the primary suspect yet. |
| Scanner semantic overhaul | `NO-SHIP` | The experiments support keeping the current signal split while the remaining matrix items are executed. |

## Implementation Sequence

## Phase 1: Ship the bounded UI resilience slice

Objective:

- session presence should not disappear instantly during a short daemon bridge
  gap
- the UI must stay honest that the snapshot is stale rather than live

Primary surfaces:

- `src/Shell.svelte`
- `src/lib/shell/events.svelte.js`
- `src/lib/sessionStore.svelte.js`
- any session/sidebar components that currently assume "no fresh bridge means no
  sessions"

Required behavior:

- keep the last daemon snapshot in memory when `daemon-status` leaves
  `connected`
- introduce an explicit stale presentation state instead of clearing presence
- restore normal live state on the first fresh `sessions-updated` event

Why first:

- this directly improves operator experience
- it matches the strongest conclusion from `#1284`
- it does not require a transport or scanner rewrite

Ship decision for Phase 1:

- **Ship once implemented and verified**

## Phase 2: Make contention visible and stop misclassifying it as disconnect

Objective:

- distinguish shared-client busy/lock contention from actual daemon loss

Primary surfaces:

- `src-tauri/src/commands/command_center/session_listing.rs`
- `src-tauri/src/provider/daemon_client.rs`
- `src-tauri/src/commands/daemon.rs`
- frontend daemon/session status presentation

Required behavior:

- preserve the current "busy means fall back to cached snapshot" behavior
- add explicit telemetry and, where surfaced to the UI, explicit classification
  that this was a busy shared lane rather than a dead daemon
- avoid driving the session UI into a hard disconnected state when the daemon is
  merely busy and the cache is still valid

Why second:

- `#1282` says this is the first real failure shape to attack
- it keeps the team from spending time on the wrong layer

Ship decision for Phase 2:

- **Ship after Phase 1**

## Phase 3: Re-run the missing ship-gate experiments against the updated UI and client behavior

Objective:

- close the matrix that `#1281` defined instead of inferring shipment from two
  or three promising slices

Minimum items to execute and record:

- `S1` Claude/Gemini transition stability
- `S2` Codex single-session versus multi-session attribution
- `S3` process/tmux churn tolerance
- `D1` daemon cadence and export freshness
- `D2` activity export semantic honesty
- `D3` startup stale-daemon eviction
- `D4` live reconnect and long-poll recovery
- `D5` failure degradation behavior

Why third:

- these are the actual broad-lane ship gates
- without them, any "done" claim is still overstated

Ship decision for Phase 3:

- **Do not ship the broad lane without this**

## Phase 4: Only then decide whether deeper daemon or scanner changes are needed

This phase is explicitly conditional.

Only reopen deeper architecture if the post-Phase-2 matrix still shows one of
these:

- real transport loss rather than shared-client contention
- stale-daemon recovery failure
- version-stream recovery failure after daemon restart
- scanner churn failures that survive the current tool-specific model

If none of those appear, do not create a daemon redesign project just because
the word "disconnect" was used in the original symptom report.

## Explicit Defers

These should be deferred unless the remaining experiments force them open:

- changing the current Codex versus Claude signal split
- shortening or lengthening scanner thresholds without evidence
- changing daemon startup or reconnect timeouts preemptively
- broad server-side concurrency redesign
- claiming the WSL daemon is unstable under load without reproducing a real
  transport failure

## Final Ship Gate

The full lane can move from `NO-SHIP` to `SHIP` only when all of these are true:

1. the stale-retained UI slice is in and validated,
2. busy shared-lane behavior is visible and no longer mislabeled as disconnect,
3. the `#1281` matrix items have been executed with evidence,
4. no explicit no-ship condition from `#1281` is still failing.

Until then, the honest status is:

- **ship the bounded UI and contention-visibility improvements**
- **do not ship a broad "daemon stability solved" story**

## Bottom Line

The experiment set narrows the problem sharply:

- keep the current scanner direction,
- stop blaming raw daemon transport first,
- ship stale-but-retained session presence now,
- make shared-client contention visible next,
- and reserve any deeper daemon or scanner surgery for the evidence that comes
  out of the remaining matrix.

That is the smallest honest plan that matches the current results.
