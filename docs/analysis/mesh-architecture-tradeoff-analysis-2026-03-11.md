# Mesh Architecture Tradeoff Analysis - 2026-03-11

## Scope

This document compares two Mesh task and communication architecture options:
- the approved linked-journal Mesh architecture from [mesh-joint-architecture-proposal-2026-03-11.md](/home/mstie/projects/taurhaus/docs/analysis/mesh-joint-architecture-proposal-2026-03-11.md)
- the earlier storage-heavy task-centric workflow model from [mesh-task-communication-architecture-draft-2026-03-11.md](/home/mstie/projects/taurhaus/docs/analysis/mesh-task-communication-architecture-draft-2026-03-11.md)

The goal is not to re-argue whether Mesh needs richer workflow semantics. Both approaches agree that it does.

The goal is to make the architectural tradeoffs explicit:
- what each approach optimizes for
- where each approach is stronger or weaker
- which approach is the better fit for Mesh specifically

## Short Definitions

### Approved linked-journal approach

Core idea:
- keep current task snapshots and inboxes as operational current-state surfaces
- evolve `state/task_mutations.jsonl` into the authoritative task workflow journal
- evolve `state/protocol_index.jsonl` into the authoritative message/task/ack correlation journal
- add only small per-task projections where they solve an immediate operational problem:
  - recovery
  - attention

### Storage-heavy task-centric approach

Core idea:
- center the entire workflow around a richer task domain model
- add separate canonical entities and stores such as:
  - `TaskAssignment`
  - `TaskEvent`
  - `MessageRecord`
  - `TaskRecoveryContext`
  - `TaskAttentionState`
- treat task workflow as the primary system, with task snapshot and inbox behavior increasingly derived from that canonical model

## Common Ground

Both approaches agree on the following design goals:
- assignment must become first-class
- progress, blocked state, and completion semantics should be explicit
- recovery should be task-first
- idle-monitor should consume workflow evidence, not only loose heuristics
- Taurhaus should be able to read clearer native Mesh semantics downstream
- the system must remain local, file-backed, inspectable, and recoverable

That matters because the real decision is not whether Mesh should become more workflow-aware.
It should.

The decision is which implementation path best fits Mesh.

## Pros Of The Approved Linked-Journal Approach

### 1. Lower implementation complexity

It extends mechanisms Mesh already has:
- task snapshots
- inbox queues
- `task_mutations.jsonl`
- `protocol_index.jsonl`
- daemon-driven projections and delivery

That means less new storage machinery, fewer overlapping truth surfaces, and less rebuild logic.

### 2. Lower migration risk

The approved model is additive.
It can preserve:
- current task file shape
- current inbox behavior
- current CLI mental model
- current daemon responsibility boundaries

It does not require Mesh to flip to a wholly new persistence model in one step.

### 3. Better operability for the current system

Mesh already operates as:
- a filesystem-first CLI
- a daemon set that reacts to files and journals
- a compatibility-sensitive local coordination system

The linked-journal approach matches those realities instead of trying to replace them.

### 4. Better fit for incremental correctness improvements

Workflow correctness improves as journals gain richer semantic events and linkage.
That is a tractable path:
- richer assignment semantics
- better message/task correlation
- task-first recovery
- inspectable attention state

Each step is locally testable and does not require a new grand abstraction layer to land first.

### 5. Lower long-term drift risk between stores

The more canonical stores a system adds, the more chances it has to drift.
The linked-journal model keeps the number of authoritative historical surfaces small:
- task workflow journal
- protocol correlation journal

Everything else is a projection or operational queue.

### 6. Better alignment with current downstream consumers

Taurhaus already reads file-backed current state and can evolve to read richer journals and projections.
The approved approach improves those sources without forcing downstream consumers to learn a much larger canonical storage graph all at once.

## Cons Of The Approved Linked-Journal Approach

### 1. Semantics remain somewhat distributed

Even in the approved model, the workflow is not represented as one clean normalized aggregate.
Meaning is still distributed across:
- task snapshot
- workflow journal
- protocol journal
- recovery projection
- attention projection
- inbox queue

That is manageable, but it is still conceptually layered.

### 2. Historical querying can be less elegant

If someone wants a perfectly normalized workflow history with explicit typed assignment and message entities, the linked-journal model is less direct.
Some queries may still require correlation logic instead of simple lookup in a dedicated canonical store.

### 3. Some semantics arrive through projection rather than original object purity

For example:
- assignment is first-class semantically via `assignmentId` and lifecycle events, not necessarily a separate assignment object store in phase 1
- attention state is a projection, not an independent canonical domain store

That is pragmatic, but some architects will view it as less pure.

### 4. Future expansion may require later normalization

If Mesh eventually grows far beyond current scale or complexity, it may still decide to promote some semantics into dedicated stores later.
The approved model does not forbid that, but it postpones the decision.

## Pros Of The Storage-Heavy Task-Centric Approach

### 1. Cleaner domain explicitness

This model makes workflow concepts visibly first-class:
- assignment
- acknowledgment
- message linkage
- recovery state
- attention state
- task lifecycle events

That makes the domain easier to explain at a conceptual level.

### 2. Stronger direct queryability

A normalized workflow model can be easier to query for rich questions such as:
- which assignments are unseen?
- which task messages satisfied completion?
- what is the latest attention state for each assignment?

In theory, these answers can be obtained without as much projection or correlation logic.

### 3. Better fit for a future dedicated workflow engine

If Mesh were intentionally evolving into a larger workflow platform with many more features, the storage-heavy model could become a better long-term foundation.
It leaves more room for deep workflow semantics and richer analytics as first-class concepts.

### 4. Stronger conceptual boundary between workflow state and delivery state

The earlier draft clearly distinguishes:
- canonical workflow records
- delivery projections
- recovery state
- attention state

That can reduce ambiguity when the system is designed from the beginning around those boundaries.

## Cons Of The Storage-Heavy Task-Centric Approach

### 1. Higher implementation complexity

This approach adds multiple new canonical concepts and stores early.
That means more work to implement:
- write paths
- rebuild paths
- consistency rules
- migration rules
- backward compatibility behavior
- test coverage across all of them

For current Mesh, that is a large architectural step.

### 2. Higher migration risk

Mesh already has working snapshots, inboxes, journals, daemons, and CLI flows.
A storage-heavy redesign risks destabilizing those surfaces while trying to improve semantics.

The more new canonical stores introduced at once, the more migration edges must stay correct.

### 3. Higher operability cost

Operationally, Mesh benefits from having a small number of trusted files and journals.
The storage-heavy model increases the number of places an operator or tool must inspect to understand current truth.

That can make debugging and recovery harder during rollout.

### 4. More overlap with what existing journals already do

Current Mesh already has the beginnings of the right backbone:
- `task_mutations.jsonl`
- `protocol_index.jsonl`

The storage-heavy model risks partially duplicating their role instead of extending them.
That increases the chance of redundant truth surfaces.

### 5. Identity migration pressure is stronger

The task-centric model wants stable agent identity to matter everywhere quickly.
That is architecturally reasonable, but the current Mesh CLI and routing model is still strongly name-based.

Pushing too much identity normalization too early increases migration complexity.

### 6. Long-term maintenance cost may be higher if the extra stores do not pull their weight

Every canonical store creates ongoing obligations:
- schema evolution
- rebuild logic
- compatibility guarantees
- tests
- corruption handling
- drift handling

If the workflow benefits can be achieved with fewer stores, the extra storage surface is hard to justify.

## Dimension-By-Dimension Comparison

## 1. Implementation complexity

Linked-journal approach:
- lower complexity
- reuses current storage and runtime patterns
- easier to stage in multiple small changes

Storage-heavy approach:
- higher complexity
- introduces more canonical concepts and more storage surfaces
- requires broader refactoring before value is realized

Winner for Mesh:
- linked-journal

## 2. Migration risk

Linked-journal approach:
- lower risk
- easier backward compatibility story
- current CLI and daemon flows remain recognizable

Storage-heavy approach:
- higher risk
- more new state to migrate and reconcile
- more chances to destabilize working behavior

Winner for Mesh:
- linked-journal

## 3. Operability

Linked-journal approach:
- stronger operational fit
- fewer authoritative stores to inspect
- easier to explain how current truth is assembled

Storage-heavy approach:
- potentially cleaner in theory, but heavier in practice during rollout
- more files and state families to reason about during incidents

Winner for Mesh today:
- linked-journal

## 4. Correctness

Linked-journal approach:
- strong if journals are enriched and projections are clearly defined
- correctness improves incrementally
- some semantics still require disciplined correlation rules

Storage-heavy approach:
- can provide a cleaner canonical model if fully and correctly implemented
- correctness ceiling may be higher in theory
- correctness floor during migration is lower because more machinery must stay consistent

Winner for Mesh now:
- linked-journal

Long-term theoretical winner in a different system shape:
- storage-heavy, if Mesh ever intentionally becomes a larger workflow engine and can justify the added canonical storage complexity

## 5. Recovery behavior

Linked-journal approach:
- strong once `recovery/{task}.json` exists and is fed by workflow plus protocol evidence
- directly supports compaction-aware resume with minimal extra surface

Storage-heavy approach:
- also strong, because recovery is a dedicated first-class store
- more explicit by construction

Winner:
- near tie

Practical winner for Mesh:
- linked-journal, because it gets most of the recovery benefit with less architectural cost

## 6. Downstream consumption

Linked-journal approach:
- easier for Taurhaus to adopt incrementally
- preserves existing current-state files while adding richer journals and projections

Storage-heavy approach:
- can eventually be more semantically explicit for downstream consumers
- but requires downstream tools to ingest a larger new canonical model
- gives tools like Taurhaus more obvious domain records and less reconstruction once the larger model is fully in place

Winner for current integration path:
- linked-journal

## 7. Extensibility

Linked-journal approach:
- extensible, but intentionally conservative
- can still add richer semantics and even more normalization later if truly needed

Storage-heavy approach:
- more extensible at the domain-model level from the start
- easier to imagine future features hanging off separate canonical entities

Winner:
- storage-heavy in pure architectural headroom
- linked-journal in practical extensibility for Mesh's next several phases

## 8. Long-term maintenance cost

Linked-journal approach:
- lower expected maintenance cost if the journal boundaries stay clean
- fewer canonical stores to evolve and keep consistent

Storage-heavy approach:
- higher maintenance cost unless the richer model pays back clearly in capabilities that cannot be reached otherwise

Winner for Mesh:
- linked-journal

## Situations Where Each Approach Is The Better Fit

## The linked-journal approach is the better fit when:
- the system already has working snapshots, journals, and file-backed runtime flows
- migration safety matters more than domain-model purity
- the team wants faster, lower-risk improvement of workflow semantics
- current operator ergonomics and inspectability matter
- downstream consumers need an additive path rather than a hard storage pivot

This describes Mesh today.

## The storage-heavy task-centric approach is the better fit when:
- the system is being designed as a workflow engine first, not evolving from an existing CLI/runtime model
- the product intends to center future capabilities on deeply normalized workflow entities
- the team can afford a broader storage and migration rewrite
- the extra canonical stores unlock near-term product value that cannot be reached through enriched journals plus projections

This does not describe current Mesh as well, but it could describe a future system if Mesh's role changes materially.

## Final Recommendation

For Mesh, the approved linked-journal architecture is the correct choice.

Reason:
- it captures most of the workflow benefits the storage-heavy model is aiming for
- it does so with materially less implementation complexity, migration risk, and long-term maintenance burden
- it aligns with Mesh's current filesystem-first, CLI-first, daemon-and-projection operating model

The storage-heavy model is still useful as a design pressure source.
It identified the right semantic gaps:
- assignment
- workflow events
- task-linked communication
- recovery
- attention

This recommendation has an explicit condition:
- the linked-journal architecture is only the right answer if Mesh actually lands first-class semantic upgrades rather than stopping at field-delta journaling

That means the implementation target must explicitly include:
- enriched `task_mutations.jsonl` with semantic event types
- enriched `protocol_index.jsonl` with `assignmentId` plus ack, delivery, and completion linkage
- recovery and attention projections as real operational read models
- early internal `agent_id` linkage even while names remain the user-facing routing surface

But for Mesh, those gaps should be closed by enriching the current journals and adding only the projections that pay for themselves immediately.

The storage-heavy task-centric model remains a plausible future direction only if Mesh later grows beyond a CLI-local workflow system into a broader multi-consumer workflow platform where the extra canonical stores clearly pay for themselves.

## Explicit Sign-Off

### mesh-architect

Status:
- approved

Rationale:
- the linked-journal model is the better fit for Mesh's actual runtime architecture and migration constraints
- the document still gives the storage-heavy model credit for clarifying the semantic target state

### architect-1

Status:
- approved

Rationale:
- the comparison is fair to the earlier storage-heavy task-centric draft
- the final recommendation preserves the original design intent while making the Mesh-native tradeoffs explicit
- the document includes the conditions required for linked-journal success
- the storage-heavy model is scoped correctly as a plausible future direction only if Mesh evolves into a broader multi-consumer workflow platform
