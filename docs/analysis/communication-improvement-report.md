# Communication Improvement Report

This report synthesizes the communication-file catalog, version timeline, reusable analysis framework, and the applied `mesh 0.2.1` slice analysis for `taurhaus-team`.

Primary evidence comes from the latest slice report at [communication-analysis-latest.md](/home/mstie/projects/taurhaus/docs/analysis/communication-analysis-latest.md).

## Executive Summary

- The latest `mesh 0.2.1` slice is productive but only `3.3 / 5` (`Mixed`) overall: strong completions are offset by reminder-heavy coordination and assignment redundancy.
- Completion quality is the team’s strongest communication behavior. Good reports routinely include artifact paths, root cause, fix summary, and validation evidence.
- The biggest waste pattern is reminder traffic: `88` idle-monitor messages in the latest slice, touching `50` tasks (`56.8%` of assigned tasks in the slice).
- Duplicate assignment / ownership traffic is a second major drag. At least tasks `#436`, `#437`, and `#473` were assigned more than once, and `#473` was routed to two different owners.
- The current conventions are close to good, but they need sharper rules around early status reporting, blocker escalation, duplicate assignment suppression, and command-template freshness.

## What Works

### 1. Assignment messages often define the execution contract well

The best task messages already include:

- objective
- exact deliverable
- first action
- completion signal
- response expectation

Why to keep it:

- This pattern reduces clarification.
- It makes completion quality visibly better downstream.

### 2. Completion messages are consistently high value

Best-in-slice examples:

- `#473`: root cause, exact fix, compatibility notes, and validation commands
- `#492`: diagnosis, system behavior explanation, regression coverage, and verification
- `#505`: artifact path, concise findings, and explicit verification note

Why to keep it:

- Review becomes fast.
- The team lead can verify results without a second message.

### 3. Once tasks are moving, churn is low

In the latest slice:

- `85` task mutation events covered `88` assigned tasks
- only one task exceeded two mutation events

Why to keep it:

- The team does not appear to thrash task status once work is underway.
- The problem is initiation and coordination overhead, not constant task flipping.

## What Does Not Work

### 1. Reminder-heavy execution is too common

Evidence:

- `88` idle-monitor messages in the latest slice
- `50` nudged tasks
- task `#494` alone received `12` nudges before the real blocker/status report arrived

Why it matters:

- Reminders replace proper loop closure.
- The communication budget shifts from useful updates to “resume task” traffic.

### 2. Duplicate assignment traffic creates ambiguity

Evidence:

- tasks `#436`, `#437`, and `#473` each had duplicate assignment messages
- task `#473` was assigned to both `developer1` and `architect`

Why it matters:

- Ownership becomes blurry.
- The lead spends extra messages reasserting or checking ownership instead of progressing the work.

### 3. Stale operator instructions create avoidable friction

Evidence:

- the onboarding notice used for `communication-analyst` still referenced `mesh task list`
- the installed CLI does not support that command
- this caused an unnecessary clarification round-trip before real work began

Why it matters:

- Template drift is high-impact because it affects every newly attached or resumed agent.

### 4. Blockers are not surfaced early enough

Evidence:

- `#494` was effectively blocked during validation, but the blocker was reported only after many nudges
- the team could not help until the issue was finally articulated

Why it matters:

- Hidden blockers amplify reminder noise.
- The lead cannot unblock what has not been reported.

## Redundancies

### Reminder traffic

Waste pattern:

- repeated `mesh-idle-monitor` “Resume task” messages after assignment, often before any explicit progress update

How to eliminate it:

- require an early progress signal from the assignee
- add a grace period after assignment and after any fresh progress update before nudging again

### Multi-layer assignment traffic

Waste pattern:

Several tasks receive all of the following in quick succession:

- JSON task assignment
- `ACTION REQUIRED` manual follow-up
- idle-monitor reminder
- status check ping

How to eliminate it:

- choose one primary assignment vehicle
- only send a second assignment-like message when scope, owner, or urgency actually changed

### Duplicate onboarding notices

Waste pattern:

- developer3 received the same onboarding notice twice in the same slice

How to eliminate it:

- distinguish “fresh onboarding” from “resume / reattach”
- on resume, send only delta instructions or a short reactivation note

## Prioritized Recommendations

### P0. Remove stale command instructions from onboarding and operator notices

Action:

- audit all onboarding/operator templates against the current CLI
- remove unsupported commands such as `mesh task list`
- add a template maintenance check whenever the CLI surface changes

Evidence:

- communication-analyst onboarding used a nonexistent command and caused an immediate clarification round-trip

Expected impact:

- removes high-leverage startup friction for every new or resumed agent

### P0. Suppress duplicate assignment layers unless something materially changed

Action:

- after a structured task-assignment message, do not send a second assignment-style message unless one of these changed:
  - owner
  - deliverable
  - urgency
  - blocking dependency
- if ownership changes, explicitly state: `Owner changed from <old> to <new>`

Evidence:

- duplicate assignments observed on `#436`, `#437`, and `#473`
- `#473` showed the clearest ownership ambiguity and extra follow-up traffic

Expected impact:

- lower message count
- clearer ownership
- easier downstream analysis of task loops

### P0. Require a first progress signal quickly after assignment

Action:

- require each assignee to send one substantive status update within a short window after assignment
- acceptable statuses:
  - `starting`
  - `investigating`
  - `blocked`
- pure acknowledgment does not count

Evidence:

- `50` tasks received nudges in the latest slice
- `#428`, `#492`, and especially `#494` show reminder traffic substituting for an early progress signal

Expected impact:

- fewer reminders
- faster loop closure
- earlier blocker visibility

### P1. Add an explicit blocker report template

Action:

- standardize blocker messages to include:
  - task id
  - current state
  - blocker
  - what is needed from the lead
  - whether parallel work remains

Evidence:

- `#494` surfaced its blocker late, after repeated idle nudges

Expected impact:

- quicker unblocking
- fewer repeated “resume task” messages

### P1. Tune idle-monitor behavior around fresh assignments and fresh progress

Action:

- add a post-assignment grace window before the first nudge
- reset the nudge timer after any substantive progress report
- cap repeated nudges unless the lead explicitly requests continued escalation

Evidence:

- `88` reminders in the latest slice
- `#494` received `12`
- `#428` completed quickly but still accumulated four coordination messages before the useful reply

Expected impact:

- materially better signal-to-noise
- fewer reminders landing during legitimate focused work

### P2. Treat resumed agents differently from fresh agents

Action:

- split template paths into:
  - fresh onboarding
  - resumed session reactivation
- resumed agents should receive a short “reactivation” note with only deltas, current assignment, and any changed commands

Evidence:

- duplicate onboarding notices observed in the same slice

Expected impact:

- lower noise for already-contextualized agents
- less accidental template drift reuse

## Proposed Rule Changes

These are concrete changes worth adding to `CLAUDE.md` and `AGENTS.md`.

### 1. Add a “No Duplicate Assignment Layering” rule

Proposed wording:

- Do not send both a structured task assignment and a second assignment-style message unless scope, owner, urgency, or blocking status changed.
- If ownership changes, explicitly state: `Owner changed from <old> to <new>`.

### 2. Add a mandatory “first progress signal” rule

Proposed wording:

- After receiving an assignment, send one substantive progress update quickly (`starting`, `investigating`, or `blocked`) before going silent for extended work.
- Pure acknowledgment does not satisfy this requirement.

### 3. Add a blocker message contract

Proposed wording:

- When blocked, report:
  - task id
  - current state
  - blocker
  - exact help needed
  - whether any parallel work remains

### 4. Add a command-template freshness rule

Proposed wording:

- Do not include workflow commands in onboarding/operator templates unless they are verified against the current installed CLI surface.
- When the CLI changes, update onboarding/operator templates in the same change.

### 5. Clarify resume vs onboarding messaging

Proposed wording:

- Use onboarding only for first attachment.
- For resumed agents, send a reactivation message containing only the current assignment, changed instructions, and any required recovery step.

## Recommended Immediate Next Steps

1. Update onboarding/operator templates to remove unsupported commands.
2. Add the first-progress-signal and blocker-report rules to `CLAUDE.md` and `AGENTS.md`.
3. Change team-lead practice so a JSON task assignment is the default primary assignment message, with manual follow-up reserved for real scope/owner changes.
4. Adjust idle-monitor to use a grace period after assignment and after substantive status updates.
5. Re-run this same framework on the next comparable slice after the rule changes to see whether reminder share and duplicate assignment rates drop.
