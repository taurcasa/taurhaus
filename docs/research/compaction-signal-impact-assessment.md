# Compaction Signal Impact Assessment

Date: March 8, 2026

## Executive Summary

A reliable compaction signal unlocks more than role-card re-injection.

The highest-value capabilities are not UI polish. They are operational correctness improvements across idle detection, activity state, team supervision, and session health.

Top recommendation:

1. Use compaction as an explicit suppressor for false-idle and false-stall transitions.
2. Surface compaction as a short-lived first-class runtime state in Taurhaus and mesh.
3. Use compaction as a bounded reinjection point for operational context beyond roles.
4. Track compaction frequency as a session-health metric.
5. Add compaction awareness to mesh team-daemon logic before adding richer UI.

## Why this matters

Right now Taurhaus and mesh mostly infer liveness from:

- file mtime
- `/proc` I/O hysteresis
- TCP activity
- heartbeats
- mesh-specific status and reminder markers

Compaction creates a known temporary gap:

- the agent is alive
- the session is doing real work
- but it may look briefly quiet or context-reset

Without compaction awareness, the stack can misclassify that gap as:

- idle
- stalled
- context drift
- apparent status flapping

That is not hypothetical. We already have real observed problems in this repo around:

- noisy idle-monitor reminders
- stale/incorrect activity inference
- role/context loss after compaction

## Current State of the Stack

## Taurhaus session activity

Current docs and code references show:

- Claude uses session file mtime
- Codex uses session JSONL mtime plus `/proc` I/O hysteresis
- Gemini uses chat file mtime plus TCP activity

Relevant references:

- [docs/features/session-management.md](/home/mstie/projects/taurhaus/docs/features/session-management.md)
- [proc_io.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/proc_io.rs)
- [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs)

## Mesh idle monitoring

Mesh currently reasons from:

- heartbeat freshness
- explicit mesh status
- task ownership
- idle reminder marker freshness
- optional Taurhaus-exported activity snapshots

Relevant references:

- [/home/mstie/projects/mesh/src/idle_monitor.rs](/home/mstie/projects/mesh/src/idle_monitor.rs)
- [/home/mstie/projects/mesh/src/team_daemon.rs](/home/mstie/projects/mesh/src/team_daemon.rs)
- [/home/mstie/projects/mesh/docs/architecture/idle-monitor-analysis.md](/home/mstie/projects/mesh/docs/architecture/idle-monitor-analysis.md)

Important observed reality:

- mesh idle-monitor noise was already a real pain point
- missing or weak activity snapshots cause premature nudge behavior
- the system already needs better "alive but temporarily quiet" classification

So compaction awareness is not speculative. It directly addresses a current class of false signals.

## Prioritized Capability List

## 1. False-idle / false-stall suppression during compaction

### What it enables

- prevents an agent from being marked idle while compacting
- prevents mesh idle-monitor from nudging during a known context-reset window
- prevents short-lived active -> idle -> active state flapping

### Why it matters

This solves a real observed problem.

We already know:

- mesh idle monitoring is noisy
- Taurhaus and mesh both infer activity from indirect signals
- compaction creates a deliberate activity gap

Compaction should therefore be treated as:

- explicit liveness
- temporary unavailability
- not idle

### Difficulty

Moderate

### Why not trivial

It requires:

- a per-session `compacting` window in Taurhaus runtime state
- tool-specific detector adapters
- export of compaction state into mesh activity snapshots or runtime status

### Priority

Highest

## 2. First-class `compacting` runtime state in the UI and status model

### What it enables

- sidebar and Mesh canvas can show `compacting` instead of incorrectly showing `idle`
- team lead can tell "agent is compressing context" instead of "agent stopped working"
- session state machine becomes more truthful: `active`, `compacting`, `idle`, `offline`

### Why it matters

This solves a real observed problem, not just cosmetics.

Without this, the UI lies during compaction:

- users see false inactivity
- leads may intervene unnecessarily
- transient silence looks like drift or failure

### Difficulty

Moderate

### Why not significant

Most of the cost is status plumbing, not new architecture:

- extend backend snapshot shape
- extend frontend state mapping
- add a timed visual treatment

### Priority

Very high

## 3. Reinjection of bounded operational context beyond role cards

### What it enables

After compaction, Taurhaus can re-inject more than role identity:

- active task ID / objective
- assignment footer summary
- current ownership boundary
- current validation expectation
- current working set or focal files
- current blocker/escalation state

### Why it matters

This solves a real observed problem.

From long-running team operation in this repo:

- task facts survive compaction better than precise guardrails
- role identity persists more than exact behavioral constraints
- ownership and validation expectations are exactly the kind of details that drift

So the valuable reinjection target is not "all context again." It is a short operational card containing:

- what I am doing
- what boundaries apply
- what success looks like

### Difficulty

Moderate

### Why not significant

The architecture mostly exists already:

- role-card concept is already being considered
- task metadata already exists
- assignment footer standard now exists

The work is mainly choosing the compact form and tool-specific injection point.

### Priority

Very high

## 4. Compaction frequency as a session-health metric

### What it enables

- detects context thrashing
- flags sessions that are too overloaded
- helps explain declining output quality or repeated forgetting
- supports health indicators like:
  - compactions/hour
  - time-since-last-compaction
  - repeated compact-then-idle pattern

### Why it matters

This solves a likely real problem, though less directly observed than idle noise.

We do not yet have a specific user-facing incident in this repo that says:

- "this agent compacted too often and we measured it"

But this is strongly grounded, not hypothetical hand-waving:

- compaction is a direct signal of context pressure
- repeated compaction over short intervals is almost certainly unhealthy

### Difficulty

Trivial to moderate

### Why it is relatively cheap

Once compaction events are detected:

- counting and bucketing them is easy
- surfacing them meaningfully is the harder part

### Priority

High

## 5. Mesh idle-monitor compaction awareness

### What it enables

- mesh can suppress or defer idle nudges during compaction
- mesh can distinguish:
  - no output because stuck
  - no output because compacting

### Why it matters

This solves a real observed problem.

Mesh currently has an explicit idle-monitor noise problem and missing uncertainty handling. Compaction awareness is exactly the kind of suppressor it lacks.

### Difficulty

Moderate

### Implementation shape

Best path:

- Taurhaus exports compaction state into `state/activity/{member}.json`
- mesh idle monitor suppresses nudge while:
  - `compacting=true`
  - or `last_compaction_at` is within a short freshness window

### Priority

High

## 6. Team-lead and task supervision visibility

### What it enables

- lead can see when an agent compacted mid-task
- can distinguish "went quiet" from "context maintenance happened"
- helps when reviewing progress interruptions or missed instructions

### Why it matters

This is partly a real problem and partly a force multiplier.

Observed reality:

- team-lead often has to infer too much from silence
- compaction is exactly the kind of silent transition that can otherwise look suspicious

### Difficulty

Moderate

### Best form

Not as chat spam.

Best as:

- live status
- timeline entry
- optional health/inspection panel

### Priority

Medium-high

## 7. Handoff/checkpoint trigger

### What it enables

- compaction can act as a natural checkpoint boundary
- session handoff metadata could note:
  - compacted at time X
  - current task Y
  - current ownership boundary Z

### Why it matters

This is valuable, but less urgent.

It helps long-session resilience, but does not solve the most painful current issues as directly as idle suppression or status correctness.

### Difficulty

Moderate

### Priority

Medium

## 8. UI-only compaction affordances

### What it enables

- node badge or pulse on Mesh canvas
- sidebar micro-indicator
- tooltip: "Compacting context"

### Why it matters

Useful, but lower value alone.

If the system does not first use compaction to fix actual state classification, a badge is just decorative truth without operational benefit.

### Difficulty

Trivial to moderate

### Priority

Medium-low by itself

## 9. Automatic ownership / working-set reassertion

### What it enables

- re-inject current file ownership, touched files, or assigned module boundaries after compaction

### Why it matters

This is plausible and useful, but partly hypothetical.

We have seen ownership ambiguity as a real problem, but we have not directly measured compaction as the main cause of ownership-file drift.

### Difficulty

Significant

### Why significant

Requires:

- stable working-set capture
- deconfliction with changing tasks
- compact formatting so reinjection does not create more context pressure

### Priority

Lower than task/role/validation reinjection

## 10. Automatic compaction-triggered handoff generation

### What it enables

- create a synthetic handoff at each compaction boundary

### Why it matters

Mostly hypothetical right now.

It is tempting, but likely over-engineered unless there is a concrete workflow using those synthetic checkpoints.

### Difficulty

Significant

### Priority

Low

## Recommended Top 5 Capabilities

## 1. Idle/stall suppression during compaction

Best value because it fixes an already-observed false-positive class in mesh and Taurhaus activity logic.

## 2. First-class `compacting` runtime state

Best truthfulness improvement. It makes the UI and lead supervision accurate instead of misleading.

## 3. Reinjection of bounded operational context

Best direct productivity gain after role-card reinjection:

- task
- footer
- ownership
- validation

### Recommended compact card

- task objective
- current task ID
- ownership boundary
- validation expectation
- escalation rule
- focal files

## 4. Compaction-aware mesh idle monitor

Best cross-stack leverage because mesh is already suffering from noisy idle reminders.

## 5. Compaction frequency health metric

Best low-cost observability feature. It can reveal unhealthy sessions and context thrashing early.

## What solves real observed problems vs hypothetical ones

## Real observed problems

- post-compaction role/context drift
- idle-monitor noise
- ambiguous silence during active work
- inaccurate state inference from indirect signals

Capabilities directly solving those:

- false-idle suppression
- `compacting` runtime state
- bounded operational reinjection
- mesh compaction suppressor

## Grounded but not yet directly observed

- compaction frequency as explicit health metric
- lead-visible compaction timeline
- compaction-aware handoff/checkpointing

These are still worthwhile, but second-order.

## What mesh currently lacks

Mesh today does not appear to have any explicit compaction handling.

It reasons from:

- heartbeat freshness
- activity snapshots
- task ownership
- explicit status
- reminder freshness

That means compaction awareness would materially improve:

- idle suppression
- uncertainty handling
- interpretation of temporary quiet windows

Best integration point:

- Taurhaus exports compaction state in the per-member activity snapshot
- mesh reads it and suppresses idle nudges / marks temporary alive-but-compacting state

## Suggested Implementation Order

## Phase 1

- add backend compaction event normalization per tool
- add `compacting_until` / `last_compaction_at` to session runtime state
- suppress idle transitions during compaction window

## Phase 2

- expose `compacting` in sidebar and Mesh canvas
- export compaction state to mesh activity snapshots
- suppress mesh idle nudges during compaction

## Phase 3

- add bounded operational reinjection card
- add compaction frequency health metrics

## Phase 4

- optional lead timeline / checkpointing / richer UI

## Bottom Line

The most valuable thing compaction detection unlocks is not a prettier role system.

It unlocks a more truthful runtime model:

- the agent is alive
- the agent is temporarily re-packing context
- the system should not treat that as idle, stalled, or directionless

If Taurhaus uses the signal that way first, then role reinjection, health metrics, and UI cues become much more reliable and much more useful.
