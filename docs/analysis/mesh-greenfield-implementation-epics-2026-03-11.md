# Mesh Greenfield Implementation Epics - 2026-03-11

## Scope

This document turns the approved greenfield Mesh architecture into an execution plan.

It is intentionally not a migration plan for the current Mesh storage model.
The working assumption is:
- the greenfield workflow core is the product target
- legacy backwards compatibility is not a design goal inside the core
- compatibility belongs at adapter boundaries, not as fallbacks inside core logic

The Claude Code adapter is the explicit example of that rule:
- Mesh workflow semantics remain canonical
- Claude disk files stay an external ingress/egress surface
- unsupported Claude semantics stay unsupported instead of being faked internally

## Operating Rules

### 1. No legacy fallbacks in the core

The greenfield implementation should not accumulate compatibility branches for old Mesh storage or old workflow behavior.

Allowed:
- explicit external adapters
- cutover tooling
- one-time migration or import helpers if needed

Not allowed:
- permanent dual-write core paths
- silent fallback from canonical workflow semantics to legacy approximations
- weakening Mesh-native semantics to match Claude file limitations

### 2. Unknown stays unknown

If an external tool or partial runtime signal does not provide enough information to support full workflow semantics, the system must preserve uncertainty explicitly.

Examples:
- no synthetic ack from a file mtime
- no synthetic acceptance from a task status string
- no synthetic actor identity from a display name

### 3. Epic-first execution

We will use larger assignment units.
In the current Mesh task system, that means:
- one Mesh task acts as one epic container
- the epic owner works through the internal task framework inside the epic
- smaller child tasks are created only when a slice truly needs separate delegation

### 4. Reach out before idle

If an owner is blocked, unclear, or has exhausted the currently executable lane inside an epic, they should message the lead before going idle.

### 5. Commit at stable checkpoints

We are moving quickly, but speed is not an excuse for giant unstructured worktrees.

Commit rule:
- commit when a slice is independently valid and test-backed
- prefer multiple coherent commits inside an epic over one end-of-epic dump
- do not keep unrelated changes bundled just because they happened close together in time

Reasonable commit checkpoints:
- schema or contract lock plus failing tests
- walking skeleton landed and passing
- one lifecycle slice complete
- one projection or adapter surface complete
- hardening or cleanup pass complete

## Program Shape

We should execute the greenfield build through five epics.

| Epic | Primary owner | Core outcome | Main dependencies |
| --- | --- | --- | --- |
| G1 Workflow Core and Event Backbone | dev-1 | Canonical workflow domain, command handling, event log, replay | none |
| G2 Projection Layer and Workflow Views | dev-2 | Lead board, agent inbox, recovery, attention, Taurhaus-facing read models | G1 schema checkpoint |
| G3 Delivery Runtime and Workflow Automation | dev-3 | Delivery, read/ack capture, nudge/escalation behavior, runtime orchestration | G1 schema checkpoint |
| G4 External Adapter Platform and Claude Code Adapter | mesh-architect | Adapter abstraction, capability model, provenance, Claude adapter | G1 external-event checkpoint |
| G5 Cutover, CLI Flows, and Legacy Removal | architect-1 | Greenfield CLI/user flows, cutover, deletion of legacy assumptions | G1 baseline; overlaps with G2-G4 |

Parallelization rule:
- G1 starts first because it defines the canonical command and event surface
- G2, G3, G4, and G5 can all start once G1 lands the first schema checkpoint
- G5 should begin cutover planning immediately and then absorb integration and deletion work as the other epics land

## Shared Task Framework For Every Epic

Every epic should work through the same internal structure.

### Stage A: Contract lock

Output:
- concrete scope for the epic
- command, event, projection, or adapter contracts relevant to that epic
- acceptance scenarios and regression cases

Exit condition:
- downstream implementers can work without guessing the shape of the interface

### Stage B: Walking skeleton

Output:
- minimal end-to-end implementation of the epic's core path
- tests proving the path exists

Exit condition:
- the epic has a thin vertical slice that exercises the real architecture, not a stub pile

### Stage C: Lifecycle slices

Output:
- the full set of semantic operations owned by the epic
- one slice at a time, each closed with tests before the next expands

Exit condition:
- all required user-visible and operator-visible behaviors for the epic exist

### Stage D: Hardening

Output:
- replay/idempotency checks
- malformed input handling
- crash or restart behavior
- concurrency/ordering behavior where relevant

Exit condition:
- the epic survives realistic failure modes rather than only happy paths

### Stage E: Cleanup and deletion

Output:
- remove superseded code paths
- remove temporary scaffolding that is no longer justified
- remove hidden fallback behavior that undermines the greenfield model

Exit condition:
- the epic is not held together by temporary bridges that quietly became permanent

### Stage F: Handoff and close

Output:
- concise report
- commit list
- residual risks
- explicit note of anything that still blocks downstream epics

Exit condition:
- another agent can continue from the result without archaeology

## Epic G1: Workflow Core and Event Backbone

### Objective

Implement the canonical workflow core:
- explicit domain objects
- command validation
- append-only event backbone
- replay into canonical task state

### Must deliver

- canonical models for `Actor`, `Task`, `Assignment`, `TaskMessage`, `RecoveryContext`, `AttentionCase`, and `WorkflowEvent`
- command catalog for create, assign, deliver, see, accept, start, progress, block, review, complete, close, acknowledge, nudge, escalate
- append-only durable event storage
- deterministic replay
- first-pass concurrency and idempotency rules

### Task framework inside the epic

1. Lock the command and event vocabulary, including assignment-state transitions.
2. Land a minimal event journal plus replay engine.
3. Implement task creation and assignment lifecycle slices end to end.
4. Implement progress, blocked, review, completion, and closure slices.
5. Harden ordering, duplicate command handling, and crash/replay recovery.
6. Remove old workflow-truth assumptions from the touched path.

### Epic close criteria

- current responsibility state is queryable from canonical workflow state
- assignment lifecycle is explicit and test-backed
- replay reconstructs durable workflow state without inbox archaeology
- no touched path falls back to legacy workflow truth

### Recommended commit cadence

- commit 1: schema/contract lock plus failing scenarios
- commit 2: event journal and replay skeleton
- commit 3+: lifecycle slices in logical groups
- final commit: hardening and cleanup

## Epic G2: Projection Layer and Workflow Views

### Objective

Build the read models that make the workflow core usable:
- lead workflow view
- assignee inbox/action queue
- recovery bundle
- attention queue
- Taurhaus-facing workflow projections

### Must deliver

- projection builder wired to canonical workflow events
- query surfaces for lead, assignee, recovery, attention, and Taurhaus consumers
- projection repair and replay behavior
- explicit projection invalidation and rebuild rules

### Task framework inside the epic

1. Lock projection contracts against the G1 event vocabulary.
2. Land one thin projection path from event replay to query output.
3. Add lead board and assignee inbox projections.
4. Add recovery and attention projections.
5. Add Taurhaus-facing projection surfaces.
6. Harden rebuild, partial corruption recovery, and stale-read behavior.

### Epic close criteria

- lead queries do not require reconstruction across unrelated stores
- assignee resume state comes from a dedicated recovery model
- Taurhaus can consume workflow facts without reverse-engineering
- projections rebuild cleanly from the event backbone

### Recommended commit cadence

- commit 1: projection contracts and first replayed view
- commit 2: lead + inbox views
- commit 3: recovery + attention views
- commit 4: Taurhaus projections and rebuild hardening

## Epic G3: Delivery Runtime and Workflow Automation

### Objective

Make workflow delivery and automation consume canonical workflow state instead of inventing their own.

### Must deliver

- delivery runtime bound to assignments and task-linked messages
- explicit delivery, read, and ack capture
- nudge and escalation behavior driven by `AttentionCase`
- suppression and cooldown semantics
- runtime orchestration that respects workflow truth instead of side heuristics

### Task framework inside the epic

1. Lock the runtime contract with G1 assignment and message semantics.
2. Land a walking skeleton that delivers one assignment and captures read/ack state.
3. Add nudge and escalation flow on top of attention projections.
4. Add blocked-state and progress-aware suppression behavior.
5. Harden runtime retries, duplicate deliveries, and crashed-agent recovery.
6. Remove legacy heuristic paths where workflow-native automation now exists.

### Epic close criteria

- delivery/read/ack are queryable workflow facts
- nudges and escalations reason over canonical attention state
- blocked tasks suppress inappropriate noise
- runtime behavior no longer depends on ambiguous side files for touched flows

### Recommended commit cadence

- commit 1: runtime contract + failing scenarios
- commit 2: assignment delivery and read/ack capture
- commit 3: nudge/escalation automation
- commit 4: retry, suppression, and recovery hardening

## Epic G4: External Adapter Platform and Claude Code Adapter

### Objective

Build the adapter boundary that keeps Mesh canonical while allowing external task-tool interoperability.

### Must deliver

- `ExternalWorkflowAdapter` abstraction
- external object mapping registry
- adapter capability registry
- provenance and replay-guard model
- conflict reporting surface
- Claude Code task-file ingress/egress adapter

### Task framework inside the epic

1. Lock the adapter capability model and external observation event vocabulary with G1.
2. Land the adapter skeleton with provenance and loop-prevention support.
3. Implement Claude task-file ingress watcher and normalizer.
4. Implement Claude-compatible egress projector.
5. Add degraded-capability handling, unsupported-operation reporting, and conflict surfacing.
6. Keep communication-adapter seams separate from the task-file adapter seam.

### Epic close criteria

- Claude task files round-trip supported snapshot semantics cleanly
- unsupported semantics stay explicit instead of collapsing Mesh workflow truth
- provenance and replay guards prevent echo loops
- the adapter boundary is reusable for future external CLI tools

### Recommended commit cadence

- commit 1: adapter contracts and capability model
- commit 2: provenance registry and replay guards
- commit 3: Claude ingress
- commit 4: Claude egress and degraded-capability handling

## Epic G5: Cutover, CLI Flows, and Legacy Removal

### Objective

Turn the greenfield model into the actual operating path and delete the assumptions it replaces.

### Must deliver

- CLI or command-layer flows aligned to greenfield workflow semantics
- operator flows for assign, accept, start, progress, block, review, complete, and attention handling
- cutover plan from old Mesh workflow assumptions to the new core
- explicit deletion ledger for obsolete paths
- no-legacy guardrails that stop fallback reintroduction

### Task framework inside the epic

1. Lock the operator and CLI flows against the G1-G4 contracts.
2. Land one walking skeleton through the real user path: assign -> accept -> progress -> complete.
3. Move remaining major flows onto the greenfield core.
4. Delete or disable superseded legacy paths instead of leaving them as hidden fallback.
5. Add guardrails and regression checks that fail if old assumptions re-enter touched flows.
6. Finalize operator docs and handoff guidance for the new execution model.

### Epic close criteria

- the main operating path runs through the greenfield core
- legacy workflow truth is not still active behind the scenes
- the Claude adapter is compatibility, not a crutch for old Mesh behavior
- the system has a deliberate cutover story instead of permanent dual behavior

### Recommended commit cadence

- commit 1: cutover contract and deletion ledger
- commit 2: primary workflow path cut over
- commit 3: remaining flow cutovers
- final commit: deletion and guardrails

## Suggested Start Order

### Wave 1: start immediately

- G1 Workflow Core and Event Backbone
- G4 External Adapter Platform and Claude Code Adapter
- G5 Cutover, CLI Flows, and Legacy Removal
- G3 Delivery Runtime and Workflow Automation

### Wave 2: start as soon as G1 lands the schema checkpoint

- G2 Projection Layer and Workflow Views

Reason:
- G2 should consume a real event and command vocabulary instead of guessing one
- the other wave-1 epics can begin with contracts, guards, and thin skeletons while G1 locks the core

## Program-Level Guardrails

Before the full program is considered complete, we should also have:
- a cross-epic acceptance scenario matrix for assignment lifecycle, recovery, attention, review/handoff, and adapter degradation
- explicit "no hidden fallback" review before each epic closes
- a clear note of what remains Mesh-native and what is adapter-limited
- a standing expectation that owners message the lead before going idle or going dark under ambiguity

## Final Recommendation

We should treat these epics as the new execution backbone for the greenfield Mesh program.

That means:
- assign the epics directly as larger work chunks
- keep internal sub-work inside the epic unless delegation is actually needed
- enforce no-legacy-fallback discipline at epic close
- keep commits small enough to preserve maneuverability while still moving quickly

If we follow this structure, we get the greenfield benefits the architects designed for without recreating the current coordination overhead in a new form.
