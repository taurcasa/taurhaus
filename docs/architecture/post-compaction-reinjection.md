# Post-Compaction Operational Re-Injection

Date: March 8, 2026

## Goal

Restore the small but high-value operational context that fades after compaction:

- role identity
- top behavioral guardrails
- current task ID and objective
- execution mode
- file ownership boundary
- validation expectation
- active override / adjacent-fix permission state

This design is intentionally narrower than full context restoration.

It does not try to restore:

- full role YAML
- full task history
- full working memory
- UI compaction states
- idle suppression

Those are separate concerns and some of them require a reliable start-of-compaction signal that Codex does not expose.

## Decision Summary

## 1. Ownership model

Use a hybrid architecture with Taurhaus as the owner of:

- compaction-domain logic
- payload composition
- dynamic operational context state

Tool-specific adapters own only the final injection step:

- Claude Code: native hook adapter
- Codex: session-file detector plus direct tmux submit

Mesh is not the primary detection or delivery owner for this feature.

## 2. Detection strategy

- Claude Code: detect via `SessionStart(source=compact)` hook
- Codex: detect via appended `type:"compacted"` / `payload.type:"context_compacted"` in the active session JSONL

## 3. Delivery strategy

- Claude Code: inject via hook `additionalContext`
- Codex: inject via direct tmux submission of a bounded operational card

Do not use mesh inbox delivery as the primary reinjection path.

## 4. Payload composition

Compose the reinjection payload in Taurhaus from two sources:

- static member role data from coordination team config
- dynamic operational context from a new Taurhaus-owned operational snapshot

## 5. Canonical runtime snapshot

Add a new per-member snapshot under the team state tree:

- `~/.claude/teams/{team}/state/operational/{member}.json`

This becomes the canonical source for dynamic reinjection fields that are not reliably present in mesh tasks today.

## Why this architecture

## Why Taurhaus should own this

Taurhaus already owns the relevant hard parts:

- per-tool session detection and parsing
- active pane/runtime correlation
- team/member role metadata
- live mesh snapshot shaping
- coordination delivery rendering

Relevant existing boundaries:

- Codex session resolution already lives in [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs)
- Taurhaus already persists per-member role context on `Member`
- Taurhaus already routes operator delivery through the coordination backend and renderer

By contrast, mesh intentionally owns:

- inbox/task protocol
- per-agent notification delivery
- idle-monitor logic

It does not currently own:

- tool-specific session file parsing
- Codex/Claude compaction semantics
- role/template composition

Putting compaction parsing into mesh would duplicate volatile tool-specific parser logic into the wrong layer.

## Why mesh inbox delivery is the wrong primary transport

The current mesh path for non-Claude agents is:

1. write inbox message
2. mesh daemon injects a generic notification
3. agent must run `mesh read`

That is good for collaboration messages.

It is the wrong hot path for post-compaction reinjection because:

- it turns reinjection into a second-step pull
- the message body is not what gets injected into the pane
- it depends on the agent following another command immediately after compaction

The whole point of this feature is to avoid relying on the agent to realize it should ask again.

So phase 1 should not use mesh inbox delivery as the main reinjection mechanism.

## Why a file-only approach is insufficient

Writing a role card file and expecting the agent to read it later is the same failure mode as `AGENTS.md`:

- the model forgets exactly when it most needs the reminder

File state is useful as a source for composition.
It is not sufficient as the delivery mechanism.

## Why direct tmux submit is acceptable for Codex

For Codex, the confirmed signal is an after-compaction signal, not a before-signal.

That matters:

- by the time Taurhaus sees `compacted`, the session is back at a stable prompt boundary
- in the controlled observation, Codex printed `Context compacted` and returned to the prompt

That makes a bounded direct prompt injection viable for Codex in a way it would not be during arbitrary mid-turn execution.

This is still tool-fragile, but it is much less fragile than scraping for compaction and then relying on mesh read.

## Current Source-of-Truth Analysis

## Static role context

Already available on `Member` in coordination config:

- `role_id`
- `role_name`
- `focus_area`
- `context_summary`
- `behavior_summary`
- `instructions`
- `behavioral_contract`

This is sufficient to build the static half of the reinjection card.

## Current task context

Partly available today:

- mesh/shared task store already has:
  - `id`
  - `subject`
  - `status`
  - `owner`
  - `blocked_by`

But the requested footer fields are not canonical today:

- execution mode
- file ownership boundary
- adjacent-file policy / override permissions
- validation expectation
- response expectation

Those were standardized operationally in the retro, but they are not yet reliably persisted in a structured runtime store.

That is the main architectural gap.

## Decision: introduce an operational snapshot

Add a Taurhaus-owned per-member operational snapshot:

- `~/.claude/teams/{team}/state/operational/{member}.json`

Purpose:

- capture the dynamic fields needed for reinjection
- decouple reinjection from fragile parsing of assignment prose
- make the current operational contract inspectable and tool-agnostic

This snapshot is updated whenever Taurhaus knows the member’s active operational contract has changed.

## Proposed Schema

## `OperationalContextSnapshot`

```json
{
  "version": 1,
  "team_name": "taurhaus-team",
  "member_name": "architect",
  "updated_at": "2026-03-08T14:10:00.000Z",
  "task": {
    "id": "673",
    "subject": "Architecture: post-compaction operational re-injection",
    "status": "in_progress"
  },
  "assignment_footer": {
    "execution_mode": "recommend",
    "file_ownership_boundary": [
      "docs/architecture/post-compaction-reinjection.md"
    ],
    "adjacent_fix_policy": "no",
    "validation_expectation": "report-only",
    "response_expectation": "report-on-completion"
  },
  "ownership": {
    "override_allowed": false,
    "active_override_reason": null
  },
  "working_set": {
    "project_path": "/home/mstie/projects/taurhaus",
    "focal_files": [
      "docs/architecture/post-compaction-reinjection.md"
    ]
  }
}
```

Notes:

- `task` is intentionally minimal
- `assignment_footer` carries the standardized retro fields
- `ownership` handles temporary override state explicitly
- `working_set.focal_files` is optional and bounded

## `OperationalReinjectionCard`

This is the rendered logical payload, not necessarily the persisted raw snapshot:

```json
{
  "version": 1,
  "reason": "post_compaction",
  "generated_at": "2026-03-08T14:10:05.000Z",
  "team_name": "taurhaus-team",
  "member_name": "architect",
  "role": {
    "role_id": "taurhaus-architect",
    "role_name": "Taurhaus Architect",
    "focus_area": "Cross-layer diagnosis",
    "behavior_summary": "Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
  },
  "task": {
    "id": "673",
    "subject": "Architecture: post-compaction operational re-injection",
    "execution_mode": "recommend",
    "validation_expectation": "report-only"
  },
  "boundaries": {
    "file_ownership_boundary": [
      "docs/architecture/post-compaction-reinjection.md"
    ],
    "adjacent_fix_policy": "no",
    "override_allowed": false
  },
  "working_set": {
    "project_path": "/home/mstie/projects/taurhaus",
    "focal_files": [
      "docs/architecture/post-compaction-reinjection.md"
    ]
  }
}
```

## Composition ownership

Taurhaus should own composition in a dedicated service:

- `CompactionReinjectionService`

Inputs:

- `TeamConfigStore` / `Member`
- `OperationalContextSnapshot`
- current member runtime
- compaction detection event

Output:

- `OperationalReinjectionCard`
- tool-specific rendered text or hook output

Reasoning:

- only Taurhaus has both the role/template side and the runtime/task side
- mesh tasks alone do not contain the full assignment-footer contract
- mesh should not learn Taurhaus role semantics just to format this payload

## Detection Ownership

## Claude Code

Use a Taurhaus-managed hook bridge.

Mechanism:

1. Taurhaus installs or manages a `SessionStart` hook for managed Claude members.
2. The hook inspects the event payload.
3. If `source == "compact"`, it loads the member’s latest operational snapshot and role data.
4. It returns `additionalContext` containing the rendered compact card.

Decision:

- detection edge is the Claude hook
- ownership is still Taurhaus because Taurhaus defines the hook, snapshot, and renderer

This avoids waiting for the placeholder Claude native backend to become fully implemented.

Important local reality:

- [claude.rs](/home/mstie/projects/taurhaus/src-tauri/src/coordination/backend/claude.rs) is still a placeholder
- phase 1 should therefore use the hook bridge directly, not depend on a complete Claude-native coordination backend

## Codex

Use Taurhaus session scanning plus a codex-specific compaction watcher.

Mechanism:

1. Session scanner already resolves the active Codex JSONL per project.
2. A compaction watcher tails the active session file for:
   - `type:"compacted"`
   - `payload.type:"context_compacted"`
3. When detected, Taurhaus resolves the owning member/session.
4. Taurhaus builds the reinjection card.
5. Taurhaus injects it directly into the member’s tmux pane.

Decision:

- Codex detection lives in Taurhaus, not mesh

Reason:

- Taurhaus already owns session file semantics and per-tool parsers
- mesh should not absorb `.codex` JSONL parsing

## Re-Injection Mechanism by Tool

## Claude Code

Primary mechanism:

- `SessionStart(source=compact)` hook `additionalContext`

Why this is best:

- first-party supported
- delivered at the exact correct lifecycle point
- no tmux timing fragility
- no extra "read this inbox" step

## Codex

Primary mechanism:

- direct tmux submission of a bounded operational card

Format:

- deterministic, prefixed, compact prompt text
- not a generic prose blob

Example shape:

```text
[taurhaus] post_compaction_context
Role: Taurhaus Architect
Focus: Cross-layer diagnosis and boundary review.
Guardrail: Stay concrete and escalate ownership ambiguity quickly.
Task: #673 Architecture: post-compaction operational re-injection
Mode: recommend
Boundary: docs/architecture/post-compaction-reinjection.md
Validation: report-only
Override: none
```

Then submit with Enter as a new user turn.

Why this is best for phase 1:

- immediate
- does not depend on `mesh read`
- uses an already-available runtime primitive: tmux key injection
- aligns with the confirmed after-compaction prompt boundary

## Should we also mirror to mesh?

Optionally yes, but not on the critical path.

Recommended phase 1 behavior:

- inject directly to Codex pane
- optionally append a low-priority audit/event message elsewhere for observability

But do not make mesh inbox delivery required for success.

## Timing, Reliability, and Idempotency

## Timing

Target:

- inject within `1-2 seconds` of the after-compaction signal

Why:

- late enough to avoid racing the compaction tail event
- early enough to restore guardrails before the next long turn

## Reliability checks

Before Codex injection:

- member still attached
- pane still alive
- foreground process still matches Codex session
- event has not already been handled for this session

Before Claude hook reinjection:

- event source is `compact`
- latest snapshot exists
- generated card is not empty

## Idempotency

Double injection is noisy and potentially harmful.

Use a per-session delivery key:

- `tool + session_id + compaction_timestamp`

Persist it in Taurhaus state so retries are safe and duplicate file-watch events do not double-submit.

Suggested state path:

- `~/.claude/teams/{team}/state/compaction/{member}.json`

Store:

- last handled `session_id`
- last handled `compaction_timestamp`
- last delivery result

## What if injection races with new output?

For Codex:

- if a new turn has clearly started after the compaction event but before injection, skip or coalesce
- do not inject if the pane has already moved materially beyond the compaction boundary

Practical rule:

- if delivery is delayed past a short freshness window, skip
- freshness window recommendation: `10-15 seconds`

## Sequence Flows

## Claude Code flow

1. Claude compacts.
2. Claude fires `SessionStart(source=compact)`.
3. Taurhaus-managed hook runs.
4. Hook loads operational snapshot and role data.
5. Hook renders compact card.
6. Hook returns `additionalContext`.
7. Claude resumes with operational context restored.

## Codex flow

1. Codex appends `compacted` to session JSONL.
2. Taurhaus compaction watcher sees the append.
3. Taurhaus resolves member/session/pane.
4. Taurhaus loads:
   - team config member role data
   - operational snapshot
5. Taurhaus builds the reinjection card.
6. Taurhaus checks idempotency and pane freshness.
7. Taurhaus injects the card into the Codex pane as a new user turn.
8. Taurhaus records delivery outcome.

## Risks and Trade-Offs

## Risk: Codex relies on volatile implementation detail

True.

But this is already the confirmed viable signal, and it is stronger than terminal silence heuristics.

Mitigation:

- isolate Codex parsing in a single adapter
- key on explicit event names, not line positions or file size
- add regression tests using captured real JSONL snippets

## Risk: direct tmux injection is blunt

Also true.

Mitigation:

- use it only for post-compaction boundary, not arbitrary delivery
- keep payload compact and deterministic
- enforce idempotency and short freshness windows

## Risk: assignment-footer fields are not canonical yet

This is the biggest current gap.

Mitigation:

- create the operational snapshot first
- do not attempt NLP parsing of old assignment messages as the steady-state design

## Risk: Taurhaus daemon outage breaks Codex reinjection

Yes.

But copying Codex session parsing into mesh just to avoid this would put the logic in the wrong layer.

Recommended phase 1 acceptance:

- Codex reinjection requires Taurhaus runtime
- Claude reinjection remains more resilient because hook execution can read a last-written snapshot

## Phase 1 Architecture Decision

Use:

- Taurhaus-owned composition and state
- Claude hook adapter
- Codex session-watcher adapter
- direct tool-appropriate injection

Do not use:

- mesh as the compaction parser
- inbox/read loop as the primary reinjection transport
- timer-based periodic reinjection as the main solution

## Implementation Task Plan

## 1. Add canonical operational snapshot model and store

Scope:

- Medium

Goal:

- define `OperationalContextSnapshot`
- persist per-member snapshot under `state/operational/{member}.json`

Includes:

- schema
- read/write helpers
- test coverage

Dependency:

- none

## 2. Wire snapshot updates into coordination lifecycle and assignment paths

Scope:

- Large

Goal:

- keep the operational snapshot current when:
  - team initializes
  - member resumes
  - member is re-onboarded
  - task ownership changes
  - assignment footer / override state changes

Notes:

- this likely requires adding structured footer fields to the assignment path instead of leaving them only in prose

Dependency:

- task 1

## 3. Add compaction reinjection card composer and renderer

Scope:

- Medium

Goal:

- compose `OperationalReinjectionCard` from:
  - `Member`
  - operational snapshot
- render:
  - Claude hook `additionalContext`
  - Codex tmux text payload

Dependency:

- task 1

## 4. Add compaction idempotency state and audit events

Scope:

- Small

Goal:

- persist per-session handled compaction markers
- log:
  - detected
  - injected
  - skipped
  - stale

Dependency:

- tasks 1 and 3

## 5. Implement Codex compaction watcher in Taurhaus session scanner

Scope:

- Large

Goal:

- watch active Codex JSONL
- detect appended `compacted` / `context_compacted`
- resolve member and pane
- trigger reinjection pipeline

Dependency:

- tasks 1, 3, and 4

## 6. Implement Codex direct post-compaction tmux injection

Scope:

- Medium

Goal:

- inject rendered compact card as a bounded user turn
- include freshness and pane-state guards

Dependency:

- tasks 3, 4, and 5

## 7. Implement Claude `SessionStart(source=compact)` hook bridge

Scope:

- Large

Goal:

- manage hook installation for managed Claude members
- on compact resume, return rendered `additionalContext`
- use the same composer as Codex

Dependency:

- tasks 1, 3, and 4

## 8. Add structured compaction-aware regression tests

Scope:

- Medium

Goal:

- Codex JSONL parser tests with captured compaction records
- idempotency tests
- stale delivery skip tests
- Claude hook output tests
- renderer snapshot tests

Dependency:

- tasks 3 through 7

## 9. Optional follow-on: expose reinjection audit in UI/inspection surfaces

Scope:

- Small to medium

Goal:

- show recent compaction reinjection events in inspection/debug UI

Dependency:

- tasks 4 through 7

Not required for the first functional rollout.

## Recommended dependency chain

1. operational snapshot store
2. snapshot update wiring
3. composer + renderer
4. idempotency/audit
5. Codex watcher
6. Codex injection
7. Claude hook bridge
8. regression tests
9. optional UI inspection

## Bottom Line

The clean design is:

- Taurhaus owns the compaction reinjection domain
- static role context comes from coordination config
- dynamic operational context comes from a new canonical snapshot
- Claude uses the native hook lifecycle
- Codex uses Taurhaus session-file detection plus direct post-compaction tmux submit

That keeps tool-specific parser logic where it already belongs, avoids a fragile mesh-read roundtrip, and solves the actual high-value problem: restoring the compact operational contract that agents lose after compaction.
