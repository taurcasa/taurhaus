# W4 — taurhaus-managed non-Claude stages: design

Status: design for review, 2026-08-29. Row W4 in [`workflows-integration-plan.md`](workflows-integration-plan.md); builds on W1 (procedures), W2 (run scanner), W3 (agent definitions), W5 (assignment effort in mesh 0.2.22/0.2.23 and taurhaus #68).

## Problem

A workflow's non-Claude stage today is an Opus "babysitter" agent that writes a prompt file, launches `codex exec` detached with a pidfile, polls a log, and resumes it up to three times. It works, but it is where the wall-clock timeouts, the lost work on a terminal crash, and the duplicated per-lane wrapper code came from, and none of it is visible in taurhaus: no session, no activity, no compaction reinjection, no later steering.

## Design

A stage is an **assignment to a managed member**, not a subprocess. The workflow script calls one primitive — `stage(task)` — implemented by a Claude Code subagent whose only job is to hand the task to taurhaus and wait for the result; taurhaus and mesh do everything else through paths that already exist.

**Delivery contract** (mesh 0.2.23): the stage creates a mesh task on the workflow's team with `--effort <level> --why <reason>`, a `first-step`, a `deliverable` (the return shape the script expects), and a `completion-signal` (a single inbox message to the lead that starts with `RESULT <task-id>` followed by a JSON block); `mesh task assign` picks a member of the requested harness, mesh applies `/effort` (Claude/Antigravity/Grok) or taurhaus resumes Codex with the flag and mesh holds the notice until `appliedEffort` matches.

**Completion signal**: the member's `RESULT <task-id>` message; the stage returns its JSON block to the script (schema-validated by the same StructuredOutput path the procedures use today). A member that reports a blocker sends `BLOCKED <task-id>` with a reason; the stage returns `{status: "blocked", reason}` and the script decides (fix-round, escalate, abandon).

**Timeouts and retries**: the assignment carries a deadline (`--deadline <minutes>`, default from the script's `size`: small 20, feature 60); taurhaus's self-heal pass sends one nudge at half the deadline and marks the task `stale` at the deadline; the stage returns `{status: "timeout"}`; no automatic re-run — the script owns retries, the member keeps its session and context for a resumed attempt (`stage(task, {resume: taskId})`).

**Worktree handling**: a stage names the checkout it works in (`worktree`), created by the script's own setup step exactly as the lanes do today (`git worktree add`, resource binaries, `bun install`); a member launched for a stage starts in that checkout, so two stages never share a working tree. Members are per-worktree, not per-stage: a team for a feature PR has one Codex member in the worktree, reused across implement → fix rounds.

**Observability**: the member is a normal managed session — sidebar, canvas, activity, compaction reinjection, the W2 run tree on the *lead* session (the workflow runs in it), and the task card with the effort and why (W5b). Steering later is `mesh send` to the member from the lead's pane, as with any team.

**What changes where**
- mesh: `task create/assign --deadline`, the `RESULT/BLOCKED <task-id>` completion-signal convention recognised in `task get` (small).
- taurhaus: the nudge-and-stale rule in the self-heal pass; the task card shows the deadline; a `stage` helper in the procedures' shared lib (W1) replacing the Codex wrapper: create task → assign → wait for the completion message via the inbox → return JSON; the Codex wrapper stays available behind `args.transport: "exec"` for hosts without a team.
- procedures: `feature-pr` / `small-change` / `fix-round` / `research-sweep` use `stage` for non-Claude lanes when `args.team` names a team; otherwise unchanged.

## Experiments that gate implementation

3. A managed Codex member completes a bounded implementation task end to end through the inbox contract (assign → RESULT with commits) in a scratch team and worktree, with the effort applied before pickup (mesh 0.2.23) — measures wall clock vs the `codex exec` wrapper. **Measured 2026-08-30 (#71, `e2e/specs/managed-stage-codex.js`, [`research/w4-experiment-3.md`](research/w4-experiment-3.md)):** the hold, resume and delivery happened in the required order (hold 1.91 s of which resume 1.63 s), the member's `RESULT` carried a verified commit in 34.05 s end to end for two Codex turns, and a member launched at the requested level was never held.
4. Deadline semantics: a deliberately stalled member is nudged at half time, marked stale at the deadline, and `stage()` returns `timeout` while the session survives for a resumed attempt. The pure decision prerequisite is now `coordination/task_deadline.rs`; it is fenced from the placeholder health framework and deliberately remains unwired until this experiment.
5. Worktree isolation: two stages in two worktrees on one team run concurrently without touching each other's tree or inbox; the lead's run tree shows both.

## Not building
Automatic retries, cross-machine stages, a UI to launch workflows, changes to Claude-side subagents.
