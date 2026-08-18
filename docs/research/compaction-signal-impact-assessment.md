# Compaction Signal Impact Assessment

Date: March 9, 2026

Updated after the March 9, 2026 event-driven compaction rollout.

## Executive Summary

This assessment changed materially after the current compaction work landed.

The main question is no longer "should Taurhaus add compaction detection?" That is done for the current stack. The next-value question is "where should the new signal change runtime behavior?"

Top recommendation:

1. Use the existing compaction signal to improve activity classification before adding more UI.
2. Export a first-class `compacting` or `recently_compacted` state into team activity snapshots.
3. Make mesh reminder logic classification-based, not timer-based.
4. Expand the bounded post-compaction resume card only after classification is trustworthy.
5. Track compaction frequency and unresolved deliveries as health signals.

## Why this matters

Compaction creates a real temporary gap:

- the agent is alive
- the session is doing real work
- the session may briefly look quiet or context-reset

Without explicit handling, the stack can still misclassify that gap as:

- idle
- stalled
- context drift
- apparent status flapping

That matters because the repo already has observed problems around:

- noisy idle-monitor reminders
- stale or weak activity inference
- post-compaction task/guardrail loss

## Current State of the Stack

## Taurhaus now has explicit compaction handling

As of March 9, 2026, Taurhaus has already shipped the hard part of the pipeline:

- event-driven Codex compaction extraction from session JSONL records
- watcher-based signal consumption instead of the old redundant `500ms` loop
- shared runtime-session cache instead of duplicate display/compaction scanning
- structured compaction observability events and audit surfaces
- bounded post-compaction resume-card delivery

Relevant references:

- [CHANGELOG.md](/home/user/projects/taurhaus/CHANGELOG.md)
- [compaction_watcher.rs](/home/user/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs)
- [compaction_events.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/compaction_events.rs)
- [reinjection.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/reinjection.rs)
- [session_scanner/mod.rs](/home/user/projects/taurhaus/src-tauri/src/session_scanner/mod.rs)
- [daemon/compaction.rs](/home/user/projects/taurhaus/src-tauri/src/daemon/compaction.rs)

## What is still incomplete

The remaining gap is not signal capture. It is system-wide consumption of that signal.

The stack still needs:

- a cleaner runtime classification than active vs idle by time alone
- activity snapshot export that carries compaction state explicitly
- mesh-side reminder logic that understands `healthy`, `busy working`, `uncertain`, `stalled`, and `broken`
- UI/status mapping that shows temporary context maintenance truthfully

This is why simple "wait longer before nudging" changes are not enough. A longer timer hides some false positives, but it does not improve classification accuracy.

## Highest-Value Next Capabilities

## 1. Classification-based idle/stall suppression

### What it enables

- prevents active -> idle -> active flapping during compaction
- suppresses reminders during known context-maintenance windows
- distinguishes "alive but compacting" from "silent and stuck"

### Why it matters

This is the direct fix for the most painful observed problem: false idle/stall noise.

### Implementation shape

- set per-session `last_compaction_at` and a short freshness window
- treat that window as explicit liveness in the classifier
- let reminder logic consume the classifier output instead of raw timers

### Priority

Highest

## 2. First-class `compacting` runtime state

### What it enables

- truthful sidebar / runtime status
- fewer unnecessary interventions from leads
- clearer distinction between temporary maintenance and actual drift

### Why it matters

The signal already exists. Not surfacing it means the runtime still lies at the exact moment when operators most need clarity.

### Priority

Very high

## 3. Mesh reminder logic driven by state, not just silence

### What it enables

- one consistent policy across progress updates, active builds, compaction, and real stalls
- fewer stale nudges
- cleaner escalation to a human only when the state is genuinely uncertain or stalled

### Why it matters

This is where the Taurhaus signal becomes team-level value.

### Priority

Very high

## 4. Bounded operational reinjection card

### What it enables

After compaction, Taurhaus can reassert the facts that drift most easily:

- current task ID and objective
- ownership boundary
- validation expectation
- focal files
- blocker / escalation rule

### Why it matters

This already exists in basic form. The next step is to keep it short, operational, and consistent with the classifier so it does not add noise or extra context pressure.

### Priority

High

## 5. Compaction health metrics

### What it enables

- compactions per hour
- unresolved or replayed signal counts
- repeated compact-then-idle patterns
- early detection of overloaded or degraded sessions

### Why it matters

This is lower-value than better classification, but it is cheap once the signal path is already explicit and logged.

### Priority

High

## What changed since the earlier March 8 assessment

The earlier version assumed Taurhaus still needed to build:

- backend compaction event normalization
- detector adapters
- a watcher path
- reinjection plumbing

That is no longer true for the current codebase.

The detector path is now:

1. extractor
2. watcher
3. processor
4. reinjection / audit consumers

The redundant `500ms` daemon scan loop is gone, and the shared runtime-session cache means compaction consumers no longer require a separate scanner path.

## Recommended Implementation Order

## Phase 1: already delivered

- event-driven compaction normalization
- watcher/processor delivery
- shared runtime-session cache
- bounded resume-card delivery
- compaction audit surface

## Phase 2: next

- add `compacting` / `recently_compacted` fields to exported activity state
- update the runtime classifier to treat compaction as explicit liveness
- drive mesh reminder suppression from classifier output

## Phase 3: follow-on

- surface `compacting` in sidebar and runtime views
- expand health metrics and lead-facing inspection
- refine the operational resume card only where it improves recovery quality

## What not to do

- do not reintroduce periodic polling into the compaction path
- do not treat "just increase the cooldown" as the main fix
- do not expand the resume card until the classifier is trustworthy
- do not add badge-only UI work before the reminder logic improves

## Bottom Line

The most valuable thing compaction detection now unlocks is not detection itself. That work is already in place.

The real value is a more accurate runtime classifier:

- the agent is alive
- the agent is repacking context
- the system should not interpret that as idle or stalled

If Taurhaus and mesh consume the signal that way, reminder noise drops, supervision becomes more truthful, and post-compaction recovery gets materially better.
