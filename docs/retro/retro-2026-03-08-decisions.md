# Retro 2026-03-08 Decisions

Inputs:
- [retro-2026-03-08-survey-findings.md](/home/mstie/projects/taurhaus/docs/retro/retro-2026-03-08-survey-findings.md)
- team discussion / consensus captured by `team-lead`

Purpose:
- finalize the retro into concrete decisions
- separate process-level changes from role-level guardrails
- provide direct input to the next role-definition phase

## Executive Summary

The retro confirmed that the team’s core execution model is working:
- direct action-oriented assignments
- exact deliverables
- completion-driven reporting

The decisions below do not change that model. They tighten the weak points repeatedly identified across survey responses:
- ownership ambiguity
- task-mode ambiguity
- stale idle-monitor noise
- unclear stop-vs-proceed behavior when validation is blocked by small unrelated issues

## Adopted Process Changes

## 1. Assignment Footer Standard

Adopted as the new baseline for task assignments.

Every task should include these 6 fields:

1. `Execution mode`
   - one of: `audit`, `recommend`, `implement`, `investigate`
2. `File ownership boundary`
   - which files/paths are in scope
3. `Adjacent-file-fix policy`
   - whether nearby-file fixes are allowed, and within what scope
4. `Completion signal`
   - what specifically marks the task done
5. `Validation expectation`
   - one of: `targeted tests`, `check-quick`, `report-only`, `runtime smoke`
6. `Response expectation`
   - `no_response_needed` or `report-on-completion`

### Why this was adopted

This directly addresses three repeated failure modes:
- tasks that start as review/audit but later drift into implementation
- uncertainty about whether an assignee may touch a nearby blocking file
- ambiguity about how far validation is expected to go

### Intended effect

- reduce assignment interpretation overhead
- reduce clarification loops
- make completion and verification more consistent

## 2. Ownership Override Rule

Adopted as a narrow exception to the normal “respect ownership boundaries” rule.

Allowed without separate approval:
- local
- low-risk
- non-design-changing
- strictly necessary to unblock validation of the assigned task

Examples of allowed override fixes:
- syntax errors
- missing imports
- config mistakes
- broken test fixtures
- capability / metadata entries

Not allowed under the override rule:
- behavioral changes in another person’s feature area
- schema or API redesign
- anything requiring judgment beyond unblocking validation

Mandatory requirement:
- every override fix must be explicitly reported in the completion summary

### Why this was adopted

The previous policy created repeated stalls when validation failed on small unrelated issues. The team does not want broad permission to cross ownership lines, but it also does not want avoidable deadlocks over obvious unblockers.

### Intended effect

- preserve ownership discipline
- reduce needless waiting on trivial blockers
- make exceptions visible and auditable

## 3. Idle-Monitor Policy (Revised)

Originally adopted on March 8, 2026 as a correction to noisy reminders. Updated on March 9, 2026 to reflect the newer communication-flow redesign: the right fix is better classification, not merely a longer grace period.

### Hard suppression

No idle reminders should ever be sent for:
- completed tasks
- formally blocked tasks
- freshly assigned tasks with recent progress or coordination activity

### State-based classification for `in_progress` tasks

For `in_progress` tasks, the monitor should classify the task state before deciding whether to remind:

- `healthy`
- `busy working`
- `uncertain`
- `stalled`
- `broken`

The classifier should consider:
- recent commentary/progress update
- recent task update
- active test/build session
- recent human coordination
- explicit compaction or context-reset activity when available
- runtime/activity snapshot freshness

### Reminder and escalation behavior

- do not remind while the state is `healthy` or `busy working`
- if the state is `uncertain`, prefer one targeted check rather than repeated nudges
- if the state becomes `stalled`, send at most one reminder, then escalate to a human if silence continues
- if the state is `broken`, escalate immediately

### Why this was adopted

The team agreed that repeated stale reminders are noisy, but a silently stalled agent is worse. The March 9 refinement is that timer-only cooldowns are too blunt: they suppress some noise, but they do not reliably distinguish active work, compaction, uncertainty, and real stalls.

### Intended effect

- hard-suppress clearly wrong reminders
- improve stall detection accuracy for `in_progress` work
- replace repeated nudges with state-aware reminders plus human escalation

## Bridge to Role Definitions

The retro decisions split cleanly into:
- process-level changes
- role-level guardrails
- items already handled by existing practice

## A. Process-Level Changes

These should be implemented in team operating procedure, not just in role wording:

### Assignment footer standard

This is clearly process-level because it changes how work is assigned, not how a specific role behaves.

### Idle-monitor policy

This is also process-level because it changes team automation behavior, state classification, and escalation policy.

## B. Role-Level Guardrails

These should be incorporated into role definitions and agent templates.

### 1. Override discipline

Roles should explicitly say:
- respect ownership by default
- only apply override fixes when they are local, low-risk, non-design-changing, and necessary for validation
- always report override fixes in the completion summary

### 2. Validation discipline

Roles should explicitly say:
- validation is part of task completion, not optional polish
- match validation depth to the declared task footer
- if runtime verification is required, do not stop at unit tests alone

### 3. Escalation behavior

Roles should explicitly say:
- escalate quickly when a blocker exceeds the override threshold
- report exact file/path/context when escalating
- do not sit idle under ambiguous ownership or validation conditions

### 4. Concrete completion reporting

Roles should explicitly say completion summaries must include:
- files changed
- validation run
- outcome
- any override fixes applied
- any residual risks or known follow-up gaps

## C. Already-Handled or Already-Aligned Findings

Some findings from the retro do not require major new decisions because the team is already directionally aligned:

### Direct action-oriented assignments

This remains the preferred team model and is reaffirmed, not changed.

### Deliverable-path specificity

Already a strong pattern. The footer standard strengthens it but does not replace it.

### Completion-driven reporting

Already expected and working well. The decisions above make it more uniform.

## Implications for the Next Role Design Phase

These retro decisions should be translated into role-definition changes immediately.

## 1. Developer-style roles

Should explicitly include:
- scoped execution discipline
- regression-first mindset where applicable
- validation to the level declared in the assignment footer
- fast escalation on overlap beyond override scope

## 2. Architect-style roles

Should explicitly include:
- cross-cutting diagnosis across frontend, backend, runtime, tmux, and mesh layers
- rigorous ownership behavior paired with fast escalation
- narrow unblocker repairs only within override scope
- outputs tied to concrete code paths, findings, or decisions

## 3. Team-facing behavioral rules for all roles

Should explicitly include:
- no idle acknowledgments to active assignees
- report blockers immediately when outside override scope
- do not over-stall on trivial local unblockers
- do not silently cross into behavioral or design changes in another stream

## Action Items

## Immediate process actions

1. Update assignment templates to include the 6-field footer standard.
2. Update idle-monitor behavior to implement the revised hard-suppress plus state-classified reminder policy.
3. Communicate the ownership override rule as an explicit team norm.

## Immediate role-definition actions

1. Add override-rule guardrails to the new role definitions.
2. Add explicit validation-discipline language to role definitions.
3. Add escalation-behavior expectations to role definitions.
4. Preserve direct-execution bias and concrete completion reporting in all rewritten roles.

## Key Learnings

The team does not need a new operating model.

It needs:
- more explicit task metadata
- more consistent automation behavior
- a sharper boundary between “allowed unblocker fix” and “ownership escalation”

Those are small process changes with outsized effect, and they are now the direct bridge from the retrospective into role design.
