# Communication Analysis: taurhaus-team / mesh 0.2.1

## Slice Definition

- Team: `taurhaus-team`
- Mesh version: `0.2.1`
- Date range: from `2026-03-06 19:00 CET` (`2026-03-06T18:00:20Z`) onward
- Message count: `578`
- Unique assigned tasks observed: `88`
- Task mutation events: `85`

## Executive Assessment

- Overall score: `3.3 / 5`
- Classification: `Mixed`
- One-sentence summary: the latest slice shows strong task completions and generally fast loop closure, but communication quality is dragged down by reminder-heavy execution, duplicate assignment traffic, and a few stale instructions that create avoidable round-trips.

## Dimension Scores

| Dimension | Score | Key evidence |
| --- | --- | --- |
| Message pattern quality | `3.5` | Strong lead<->agent traffic, but high reminder load and some duplicate assignment traffic |
| Responsiveness and loop closure | `3.5` | `87 / 91` assignment loops had a detectable same-task reply; sampled median first-reply time about `3.86` minutes |
| Signal-to-noise ratio | `2.8` | `88` idle-monitor reminders (`15.2%` of all messages) plus duplicated assignment messages and stale onboarding instructions |
| Task lifecycle efficiency | `3.3` | Most tasks mutate cleanly (`85` mutations across `88` assigned tasks; only one task had more than two mutations), but some tasks require many nudges |
| Anti-pattern detection | `2.6` | Duplicate task assignment, reminder storms, late blocker surfacing, and instruction drift are all present |
| Effective pattern detection | `4.2` | Completion reports are often artifact-bearing, root-cause-oriented, and include verification/test evidence |

## Quantitative Findings

### Direction mix

- `261` lead -> agent messages (`45.2%`)
- `219` agent -> lead messages (`37.9%`)
- `88` system -> agent reminder messages (`15.2%`)
- `10` agent -> agent messages (`1.7%`)

Interpretation:

- The slice is highly lead-driven, but not one-way; agents do report back frequently.
- The reminder share is high enough to materially affect signal-to-noise.

### Actor concentration

Top senders:

- `team-lead`: `261`
- `mesh-idle-monitor`: `88`
- `developer1`: `60`
- `developer2`: `51`
- `architect`: `51`

Top recipients:

- `team-lead`: `220`
- `developer1`: `94`
- `developer2`: `93`
- `architect`: `67`
- `developer3`: `58`

Interpretation:

- Team-lead is the coordination hub.
- Most work appears to flow through lead<->assignee communication rather than peer-to-peer coordination.

### Response timing

Using assignment JSON messages as the start of a task loop and the first same-task agent -> lead message as the first substantive reply:

- Assignments observed: `91`
- Matched same-task first replies: `87`
- Sample median first-reply latency: about `3.86` minutes

Fast examples:

- `#474` (asset-generator): about `0.3` minutes to first reply
- `#420` (mesh-expert): about `0.4` minutes
- `#443` (architect): about `0.42` minutes

Slow examples:

- `#473` (architect path): about `11.06` minutes
- `#492` (architect): about `12.1` minutes
- `#494` (developer2): about `50.53` minutes

Interpretation:

- The baseline response time is good.
- The long tail is dominated by tasks that attracted repeated reminder traffic or ownership confusion.

### Task lifecycle signals

- Task mutation actors in this slice: `developer1 26`, `developer2 21`, `architect 19`, `developer3 12`, `communication-analyst 3`, `asset-generator 3`, `mesh-expert 1`
- Only one task in the mutation stream had more than two mutation events: task `#498` with `3`

Interpretation:

- Once work begins, tasks usually progress with low status churn.
- The main lifecycle cost is not repeated mutation flipping; it is pre-progress communication overhead.

### Reminder load

- Tasks with at least one idle-monitor nudge: `50`
- That is about `56.8%` of the `88` assigned tasks seen in the slice
- Most nudged tasks:
  - `#494`: `12` nudges
  - `#432`: `5`
  - `#468`: `4`
  - `#492`: `3`
  - `#504`: `3`

Interpretation:

- Reminder usage is not rare cleanup traffic; it is a common part of the execution loop in this slice.

## What Worked Well

### 1. Completion reports are usually high quality

Strong completion messages consistently include:

- explicit task id
- artifact paths
- root cause or design rationale
- validation or test evidence

Representative examples:

- `2026-03-07T00:51:19.689Z` `architect -> team-lead` on `#473`: clear root cause, exact code paths changed, backward-compat behavior, and concrete validation commands
- `2026-03-07T09:58:49.673Z` `architect -> team-lead` on `#492`: clear diagnosis of stale runtime health, exact backend fix, regression coverage, and validation steps
- `2026-03-07T11:59:29.962Z` `communication-analyst -> team-lead` on `#505`: artifact path, key findings, and explicit “no code tests were run” verification note

Why it matters:

- Team-lead can verify work immediately.
- These messages compress review time and reduce follow-up questions.

### 2. Assignment messages often contain the full execution contract

The best team-lead assignments include:

- objective
- deliverable path
- concrete first step
- completion signal
- response expectation

Representative example:

- `#505` assignment plus follow-up gave a clear document path, scope, and completion behavior. The resulting loop closed cleanly in about four minutes.

Why it matters:

- Agents can start without clarification.
- The completion shape is known up front, which improves close-out quality.

### 3. Mutation churn is low once work is underway

Only one task exceeded two mutation events in the current slice.

Why it matters:

- Once tasks are genuinely in motion, the team is not repeatedly thrashing owner/status fields.
- The problem is more “getting the loop started cleanly” than “tasks bouncing indefinitely.”

## What Did Not Work Well

### 1. Reminder-heavy execution is a real drag on quality

The clearest example is task `#494`:

- assigned at `2026-03-07T09:46:44.651Z`
- first idle nudge at `2026-03-07T09:46:46.461Z`
- total idle nudges observed: `12`
- substantive blocker/status report not sent until `2026-03-07T10:37:16.261Z`

Why this is bad:

- The team spent a large amount of communication budget on “resume task” traffic.
- The real blocker surfaced late, so the lead could not help earlier.

### 2. Duplicate assignment traffic creates waste and ambiguity

The slice contains duplicated assignment JSON messages for at least these task ids:

- `#436`
- `#437`
- `#473`

Most problematic example:

- `#473` was assigned to `developer1` at `2026-03-07T00:33:42.925Z`
- the same task was later assigned to `architect` at `2026-03-07T00:40:15.980Z`

Additional traffic around `#473` included:

- multiple `ACTION REQUIRED` follow-ups
- an idle-monitor nudge
- explicit “are you receiving this?” / status-check messages

Why this is bad:

- It blurs ownership.
- It increases message count without creating new technical information.
- It makes later analysis harder because the same task has overlapping coordination threads.

### 3. Some instructions were stale relative to the actual CLI

Representative example:

- the onboarding operator notice sent at `2026-03-07T11:50:47.401Z` instructed `communication-analyst` to use `mesh task list`
- the installed CLI does not expose `mesh task list`
- this caused an avoidable clarification round-trip at `2026-03-07T11:51:37.607Z`

Why this is bad:

- The message was otherwise well structured, but stale procedural details still caused friction.
- A stale onboarding template is high-leverage noise because it affects every newly attached agent.

## Redundancies And Waste Identified

### Reminder traffic

- `88` reminder messages in the slice
- reminders touched `50` tasks

This is the largest single recurring waste pattern.

### Repeated assignment layers

Several tasks received more than one of:

- structured JSON assignment
- `ACTION REQUIRED` follow-up
- idle-monitor resume message
- extra status-check ping

Representative example: `#428`

- assignment at `21:05:23`
- idle nudge at `21:05:28`
- manual lead follow-up at `21:05:36`
- second idle nudge at `21:08:58`
- completion at `21:09:00`

The work still completed quickly, but four coordination messages preceded the useful reply.

### Duplicate onboarding / operator notices

Developer3 received the onboarding operator notice twice in the same slice:

- `2026-03-07T00:30:24.348Z`
- `2026-03-07T11:26:50.735Z`

This may be expected after resume/re-attach, but it is still noise unless the repeated onboarding changes behavior or restores a broken loop.

## Communication Stalls And Their Causes

### Stall pattern 1: silent execution plus aggressive remindering

Observed in `#428`, `#492`, and especially `#494`.

Likely cause:

- agents do not always emit a fast “starting / investigating” status message
- idle-monitor nudges arrive before the lead sees proof of progress

Result:

- reminder traffic substitutes for loop closure

### Stall pattern 2: ownership ambiguity

Observed most clearly in `#473`.

Likely cause:

- the same task is routed to more than one agent
- follow-up messages switch from ownership clarification to status chasing

Result:

- response latency increases and message volume grows before the actual fix report lands

### Stall pattern 3: blocker surfaced late

Observed in `#494`.

Likely cause:

- blocker existed during final validation, but it was reported only after many resume nudges

Result:

- team-lead could not unblock the task quickly
- the system continued to emit low-value reminders during the hidden-blocker period

## Representative Good Examples

- `#473` completion by architect at `2026-03-07T00:51:19.689Z`
  - Strong because it provides root cause, exact fix location, normalization semantics, CLI verification, and test evidence.
- `#492` completion by architect at `2026-03-07T09:58:49.673Z`
  - Strong because it ties the bug to a stale runtime health record, explains why the UI looked wrong, states the precise backend fix, and cites regressions.
- `#505` completion by communication-analyst at `2026-03-07T11:59:29.962Z`
  - Strong because it includes the deliverable path, concise key findings, and an explicit verification statement.

## Representative Bad Examples

- Onboarding operator notice at `2026-03-07T11:50:47.401Z`
  - Bad because it contains stale `mesh task list` instructions that no longer match the CLI.
- Task `#473` assignment thread across `developer1` and `architect`
  - Bad because the same task appears in overlapping ownership flows with repeated nudges and status checks.
- Task `#494` reminder sequence from `2026-03-07T09:46:46.461Z` through `2026-03-07T10:34:44.662Z`
  - Bad because it shows reminder accumulation without early blocker escalation.

## Recommendations

### Keep

- Keep the current assignment checklist structure.
- Keep artifact-bearing completion reports with explicit validation/test notes.
- Keep root-cause-first completion writing; it is one of the strongest patterns in the slice.

### Change

- Remove stale CLI instructions from onboarding/operator notices, especially `mesh task list`.
- Require an early status message for newly assigned work, even if only “starting / investigating / blocked on X”.
- Avoid sending both a task-assignment JSON message and a second manual assignment message unless the second one changes owner, scope, or urgency.
- Suppress idle-monitor nudges for a short grace period after assignment or after a fresh progress update.

### Investigate Next

- Whether idle-monitor thresholds are too aggressive for normal focused work.
- Whether duplicate task assignment to multiple owners is intentional or a routing bug.
- Whether a structured “blocked” message template would reduce long reminder sequences like `#494`.
