# W4 experiment 5 — two concurrent stages, two worktrees, one team (2026-08-31)

Experiment 5 of [`../w4-managed-stages-design.md`](../w4-managed-stages-design.md): do two stages in two worktrees on one team run concurrently without touching each other's tree or inbox? It ran live against a real Codex subscription; the third attempt passed end to end in 3 m 32 s — the first two failed **in setup**, each exposing a real member-delivery hazard recorded below. **The isolation and concurrency claims all held.**

The lane is `e2e/specs/managed-stage-parallel.js` (paid, named-only; #106 built it, #107 and #108 hardened its bring-up after attempts 1–2).

## Commands

```bash
E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-parallel
```

## Setup as measured (attempt 3, 2026-08-31 15:08–15:11 UTC)

| | |
|---|---|
| Host | Linux (WSL2), app + daemon from `main`, mesh 0.2.24, Codex 0.151 `gpt-5.6-sol` |
| Team | Claude lead (never took a turn) + `codex-alpha` (session `01a0585c-5867…`) + `codex-beta` (`01a0585c-d118…`) — **serialized cold-starts** (#107), retrying bind nudge (#108) |
| Worktrees | `stage-alpha` and `stage-beta`, separate git worktrees of the fixture repo under the run's temp root |
| Assignments | one per member (per experiment 4's second-assignment finding), created and assigned together, `--effort medium --deadline 10`, distinct greet-file deliverables |

## Measured — every value from records

| Fact | Value | Source |
|---|---|---|
| Assignments | 15:08:21.378 / .380 (2 ms apart) | task metadata `assigned_at` |
| Notices delivered | 15:08:21.951 / .957 | attention projection |
| RESULTs | alpha 15:10:46.215, beta 15:10:45.926 — **distinct `completion.at`** | lead inbox + task records |
| **Concurrency overlap** (delivered → RESULT intersection) | **143.97 s** (15:08:21.957 → 15:10:45.926) — each stage ran essentially its whole life while the sibling ran | delivered windows |
| Tree isolation | alpha's head commit adds only `greet-alpha.{js,test.js}`; beta's only `greet-beta.{js,test.js}`; both working trees clean; neither tree contains the other's files | per-worktree `git diff`/status |
| Inbox isolation | `codex-alpha` inbox holds assignment notices for task 1 only; `codex-beta` for task 2 only | per-member inboxes |
| Run-tree read-back | both `stage:codex:*` labels under one `Managed stage` phase with the run's real task ids and overlap | **synthesized scanner-contract fixture** — see caveats |

## What the two failed attempts taught (the experiment's second yield)

1. **Shared-home cold-start race** (attempt 1): two Codex 0.151 instances cold-starting one fresh `CODEX_HOME` simultaneously corrupt the state DB (`state_5.sqlite: migration 22: duplicate column name: agent_path`). A real team initializing 2+ Codex members against a fresh home can hit this — **product follow-up: serialize or pre-warm Codex cold-starts in the launch pipeline.**
2. **Parked-composer delivery** (attempt 2): the bind prompt *and mesh's own onboarding notice* sat unsubmitted in the Codex composer — `delivered` per the projection, unsubmitted per the pane. Together with experiment 4's second-assignment finding, this pins the W4 follow-up precisely: **mesh records tmux acceptance, not member uptake; send-verification (or an uptake probe) is the gap.**

## Caveats

- The run-tree item is a **scanner-contract read-back over a summary the lane synthesizes** (carrying the run's real task ids and windows), not a lead-emitted run tree — the lead session takes no turns in this lane. The scanner's own parsing is covered by `workflow_runs` unit tests; a lead-emitted tree remains unmeasured.
- A managed implement step files under a different run-tree phase than the exec transport (`Managed stage`); a missing "Implement" phase for a staged lane is a rename, not a gap.
- Evidence archived with the session: `exp5-run{,2,3}.log`; attempts 1–2 artifacts preserved.

## Verdict

Experiment 5 **passes**: two managed stages ran genuinely concurrently (143.97 s overlap) with byte-verified worktree and inbox isolation and distinct completions on one team. With experiment 4, all W4 gating experiments are green; the member-uptake verification gap is the one carried follow-up.
