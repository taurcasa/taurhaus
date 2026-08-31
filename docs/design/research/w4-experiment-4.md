# W4 experiment 4 — deadline semantics measured end to end (2026-08-31)

Experiment 4 of [`../w4-managed-stages-design.md`](../w4-managed-stages-design.md): is a deliberately stalled member nudged at half time, marked stale at the deadline, with `stage()` returning `timeout` while the session survives for a resumed attempt? It ran live on the development host against a real Codex subscription, twice. **It is — every deadline semantic held exactly as designed** — and the two runs also surfaced a reproducible member-side delivery hazard recorded below.

The lane is `e2e/specs/managed-stage-deadline.js` (paid, named-only; #103 built it, #104 reordered it after attempt 1).

## Commands

```bash
E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-deadline
```

## Setup as measured

| | |
|---|---|
| Host | Linux (WSL2), app + daemon built from `main`, daemon protocol 14 |
| mesh | 0.2.24 (`1d6af0e`), `--deadline` metadata + completion recognition |
| Codex | 0.151, `gpt-5.6-sol`, one member (`codex-deadline`), launched low |
| Team | Claude lead `e2e-lead` (isolated credential-free root, never took a turn) + the Codex member |
| Isolation | per-run temp root (data, claude, codex scratch, projects, tmux server), worker-owned daemon port, ownership-ledger cleanup; the operator's environment untouched |
| Stall assignment | `--deadline 1` (pass cadence 30 s → nudge due ≥ 30 s, stale due ≥ 60 s after start); first step: `mesh task start`, then wait silently |
| Suppression assignment | `--deadline 3`; one chained member command: 120 s heartbeat at 8 190 B/s (clearing the 1 kB/s activity gate) ending in `mesh task complete` |

## Measured — stall path (attempt 2, 2026-08-31 11:50–11:54 UTC)

Every timestamp is a record's own, never the test process's stopwatch.

| Fact | Value | Source |
|---|---|---|
| Assignment → notice delivered | 11:50:05.994 → 11:50:06.581 (**0.59 s**) | mesh attention projection |
| Member ran `mesh task start` | 11:52:08.811 | task record `startedAt` |
| Operational import (deadline + `assigned_at` stamped) | 11:52:10.067 (**1.26 s** after start) | operational snapshot |
| Half-due (start + 30 s) | 11:52:40 | arithmetic on records |
| **Nudge** — event / inbox message / marker | 11:52:59.575 / .562 / .481 — the first self-heal pass after half-due; **eventCount 1, inboxCount 1** across three passes | `deadline.nudge.sent`, member inbox, snapshot |
| Full due (start + 60 s) | 11:53:10 | arithmetic |
| **Stale** — mesh task + snapshot + event | 11:53:28.899–.981 — the first pass after the deadline; **eventCount 1**, no further nudge | task record, snapshot, `deadline.task.staled` |
| `stage()`-shaped poll verdict | `{"status":"timeout"}` | task-record poll |
| Session survival | pane `%2` pid 3451804 alive, daemon 3451891, same session id at afterStart / afterNudge / afterStale | runtime record + pane probe |
| Self-heal pass cost | 80–95 ms per pass, 2 teams scanned | pass events |

## Measured — suppression path (attempt 1, 2026-08-31 10:43–10:48 UTC)

| Fact | Value | Source |
|---|---|---|
| Assignment → delivered | 10:43:30.764 → 10:43:31.354 | attention projection |
| Member started | 10:45:45.896; import 1.0 s later, `deadline_minutes: 3` | task record, snapshot |
| Half-due | 10:47:16.837 | arithmetic |
| Eligible pass at 10:47:24.837 read activity observed 10:47:23.260 = **active** | nudge suppressed | pass event joined to the activity snapshot the pass read |
| Deadline actions | **0** | event scan |
| Completion | `completedAt` 10:47:46.520, well inside the 60 s post-heartbeat slack | task record |

## A reproducible finding: the second assignment to a used Codex member starts no turn

Across both attempts, whichever case ran **first** on the fresh member succeeded (2/2), and whichever ran **second** failed identically (0/2): mesh recorded the notice `delivered` (attention projection, sub-second), the member's runtime stayed `healthy` with the pane alive — and the member completed **zero Codex turns** in the following 240 s (`turnCountDelta=0`). Attempt 2 removed the mid-turn race hypothesis: the quiescence-settled delivery landed in a quiet pane and still produced no turn. The pane-level mechanics are not pinned (teardown deletes the temp root; the lane captures app-side records only) — **named follow-up**: pane-content capture on start-timeout, and send-verification for assignment notices (mesh records tmux acceptance, not member uptake). This matters to `stage()` directly: the design reuses one member across implement → fix rounds, exactly the second-assignment shape.

## Notes for readers

- The two paths were each proven on a different attempt; no single run has both green — a consequence of the finding above, not of the deadline machinery, which behaved identically in every armed instance.
- A managed implement step files under a different run-tree phase than the exec transport; a run tree without an "Implement" phase for a staged lane is a rename, not a gap.
- Evidence: `managed deadline measured` blocks and failure artifacts for both attempts are archived with the session (attempt logs `exp4-run.log` / `exp4-run2.log`, artifacts `exp4-attempt1-artifacts/` / `exp4-attempt2-artifacts/`).

## Verdict

Experiment 4 **passes**: nudge exactly once at the first pass after half-time, stale exactly once at the first pass after the deadline, stale supersedes further nudges, the stage verdict is `timeout`, and the member session survives untouched for a resumed attempt. The deadline machinery is ready for W4 use; member-notice uptake on reassignment is the one open hazard, filed as a follow-up.
