# Mesh Task-Management Capabilities Review — 2026-03-11

## Scope

Task `#938` asked for an end-to-end review of Mesh task management for AI-only team operation.

This review covers:

- native Mesh task-management capabilities in `/home/mstie/projects/mesh`
- the current task, message, ack, idle-monitor, and daemon flows
- the task-domain architecture that Mesh effectively has today
- the architectural gaps that make team-lead coordination weaker than it should be
- how Taurhaus currently depends on Mesh task storage and where Taurhaus is adding behavior on top

This is not a review of human PM workflows. It is focused on AI team-lead plus AI assignee operation.

## Executive Summary

Mesh already has strong low-level building blocks for AI task coordination:

- durable filesystem-backed task storage
- explicit task assignment
- append-only task mutation journaling
- per-agent inbox delivery with acknowledgment tracking
- generated assignment and nudge messages
- daemon-driven wake delivery
- team-daemon idle monitoring
- protocol lint/correlation/drift modules that try to reason about task-message correctness

The strongest gap is not “Mesh has no task system.” Mesh clearly does.

The strongest gap is that task semantics are split across too many partial stores:

- task JSON snapshot
- task mutation journal
- inbox messages
- inbox ack state
- protocol index
- idle markers and runtime/activity files

The `Task` object is only a thin snapshot. The real workflow semantics live outside it and are reconstructed later by daemons, correlation, lint, drift detection, and Taurhaus.

That is workable for “assign task, wake agent, read message,” but weak for:

- durable task lifecycle management
- precise AI recovery after compaction
- robust team-lead visibility
- strong linkage between assignment, acknowledgment, progress, and completion
- project-scoped task querying across multi-project teams

My main recommendation is:

1. keep the filesystem model
2. keep the task snapshot files
3. promote the missing lifecycle relations into first-class entities and events instead of inferring them from prose and side files

In short: Mesh should evolve from “task files plus coordination heuristics” into “task-centered workflow state with message delivery derived from it.”

## Native Mesh: Current Capability Map

## Storage Model And Entities

Native Mesh stores task-management state in a small filesystem graph under `~/.claude/`.

### Primary task entities

- Team config: `~/.claude/teams/{team}/config.json`
  - members
  - lead identity
  - activity heartbeat fields
  - explicit member status fields
  - active/inactive membership
- Task snapshot: `~/.claude/tasks/{team}/{id}.json`
  - `id`
  - `subject`
  - optional `description`
  - `status`
  - optional `owner`
  - optional `activeForm`
  - `blocks`
  - `blockedBy`
  - optional opaque `metadata`
- Task mutation journal: `~/.claude/teams/{team}/state/task_mutations.jsonl`
  - `task_id`
  - `actor`
  - `timestamp`
  - `changed_fields`
- Inbox files: `~/.claude/teams/{team}/inboxes/{agent}.json`
  - per-message `id`
  - sender
  - text
  - summary
  - priority
  - `ackedAt`
  - `ackedBy`
- Protocol index: `~/.claude/teams/{team}/state/protocol_index.jsonl`
  - canonicalized message/task correlation rows
  - task/message linkage
  - copied task owner/status/dependency data
  - embedded orchestration metadata

### Derived runtime entities

- Member daemon PID files: `~/.claude/teams/{team}/daemons/{member}.pid`
- Team daemon PID file: `~/.claude/teams/{team}/daemons/team.pid`
- Idle markers: `state/{member}.idle_reminded`
- Uncertainty markers: `state/{member}.uncertain`
- Runtime/activity snapshots read by idle monitor from `state/`

### Important observation

Mesh does not have one canonical task domain store.

It has:

- one snapshot store for current task state
- one journal for “what task fields changed”
- one message store for operator communication
- one protocol store for reconstructed message/task linkage

That split is the root cause of most higher-level gaps.

## Native CLI And Runtime Flows

### Task creation

`mesh task create`:

- requires an active member identity
- creates a new numeric task file in `~/.claude/tasks/{team}/`
- always starts with `status = pending`
- records a mutation journal row
- optionally includes `activeForm`
- notifies the lead as a best-effort inbox message if the creator is not the lead

What it does not do:

- no first-class assignment
- no metadata authoring for `first_step`, `deliverable`, or `completion_signal`
- no project scoping
- no parent/child/subtask relationship

### Task update

`mesh task update`:

- requires an active member identity
- mutates status, owner, `blocks`, and `blockedBy`
- appends to `blocks`/`blockedBy`; there is no remove/replace flow
- records a mutation journal row
- notifies the lead as a best-effort inbox message

Important weakness:

- `update` does not enforce a strict status model
- arbitrary status strings can be written here even though other code assumes legacy states such as `pending`, `in_progress`, `completed`, `deleted`

### Task assignment

`mesh task assign`:

- requires active team-lead identity
- validates owner name exists in team config
- allows assignment status only `pending` or `in_progress`
- writes owner and status into task JSON
- appends mutation journal entry
- sends a generated actionable message to the assignee inbox
- member daemons separately detect assignment via journal and can inject a wake notification into tmux

Important nuance:

- assignment targets only member name, not stable `agent_id`
- assignment allows registered inactive members because existence check is weaker than active-member check
- this supports cross-project inactive owners, but it also means “owner” really means “team-config name string,” not “currently active execution endpoint”

### Task listing and querying

Native Mesh task query surface is intentionally small:

- `mesh task get <id>`
- `mesh tasks`
- `mesh tasks --all`
- `mesh tasks --mine`
- `mesh tasks --status <status>`

Default behavior:

- non-lead teammates see only their own tasks by default
- team lead sees all tasks by default

Missing native query capability:

- no `--owner <name>`
- no `--unassigned`
- no `--blocked`
- no `--blocked-by <id>`
- no `--changed-since`
- no task-history query
- no task journal query
- no protocol/drift query CLI

### Message and acknowledgment flow

Mesh has a real message/ack system:

- `mesh send`
- `mesh read`
- `mesh ack`
- `mesh ack-status`

Useful current behavior:

- actionable messages get linted / augmented with embedded orchestration metadata
- `INFO ONLY:` messages get `no_response_needed=true`
- acknowledgment is tracked durably on the message record

Task-related message behavior today:

- task assignment creates an inbox message
- idle nudge creates an inbox message
- lead notifications about create/update/assign are also inbox messages
- message ack status can be queried by `message_id`

But the task system itself does not know:

- whether the current assignment message has been seen
- whether the assignment was accepted
- whether a nudge has already been acknowledged for this task
- whether the task completion signal satisfied the original assignment

That knowledge exists only indirectly or must be reconstructed.

### `mesh read --unread` fallback behavior

If there are no unread inbox messages, `mesh read --unread` synthesizes a resume reminder from owned `in_progress` tasks:

- “No new messages, but you have active work”
- task number and subject
- `mesh task get ...` resume command

If there are no unread messages and no active tasks, Mesh injects an idle reminder.

This is pragmatic and useful for AI agents after compaction, but it is still generic:

- it does not encode last completed step
- it does not surface blocked reason
- it does not bind to the last assignment message
- it does not reconstruct a task-specific recovery bundle

### Idle monitor and nudge behavior

The team daemon runs idle-monitor every 30 seconds.

For each active member, Mesh:

- looks for owned `pending` or `in_progress` tasks
- picks a primary task
- evaluates suppression / defer / nudge / escalate using member status, heartbeat, runtime health, activity snapshots, and marker files
- sends generated nudge messages to the assignee inbox
- sends escalation messages to the lead inbox

This is a real task-attention loop, not just messaging.

But the task object itself has no native “attention” state. Nudge state is external:

- marker files
- inbox messages
- idle-monitor logic

### Daemon and compaction-related recovery support

Per-member daemons:

- watch inboxes
- watch tasks directory
- detect new assignments from mutation journal deltas
- inject compaction-safe wake text into tmux panes

The assignment notification format is strong:

- agent identity reminder
- team identity reminder
- task number, subject, and status
- direct `mesh task get`
- direct `mesh task update --status completed`
- direct `mesh send`

This is clearly designed for AI recovery after context loss.

That is one of Mesh’s strongest current capabilities.

But again, recovery is message-derived, not task-derived:

- compaction-safe assignment context is formatted from the current task snapshot
- it is not a first-class persisted task recovery record

## What Mesh Supports Well Today

## Strong native capabilities

### 1. Durable, inspectable, local-first storage

Mesh task state is plain JSON plus journals, protected with file locks and atomic writes.

Benefits:

- easy to inspect
- easy to recover
- easy to watch from external tools
- no hidden daemon database required

### 2. Real assignment flow

`task assign` is not just “set owner.”

It also:

- validates lead role
- writes the task mutation
- sends actionable instructions
- can wake the assignee via daemon delivery

That is a meaningful AI-team primitive.

### 3. Useful inbox and ack primitives

Assignment, nudge, escalation, and human/agent messages all land in one inbox model.

Acknowledgment tracking exists and is durable.

### 4. Idle-monitor tied to owned tasks

Nudges are task-aware, not generic pings.

Mesh only nudges if there is owned active work.

### 5. Recovery-oriented delivery language

The generated messages are clearly written for AI agents surviving compaction or tmux wake-ups.

The system already assumes:

- context will be lost
- agents need exact restart commands
- task state must be recoverable from outside the model context window

That is correct for this domain.

### 6. Hidden but valuable protocol-analysis layer

`correlation.rs`, `lint.rs`, `protocol_index.rs`, and `transition_drift.rs` show that Mesh already knows it needs:

- explicit task/message linkage
- structured orchestration metadata
- completion drift detection
- canonicalized records for later analysis

Those are valuable foundations.

## Strongest Capability Gaps

## 1. The task is a snapshot, not a workflow object

Current `Task` fields are too weak for the workflow Mesh is trying to support.

Missing first-class concepts:

- assignment event
- assignment acceptance
- execution start
- blocked state as a task state, not only member status
- completion evidence
- closure reason
- reopen / retry
- nudge/escalation history
- recovery context

Because these are missing, Mesh keeps rebuilding them from:

- prose message text
- inbox ack state
- mutation journal entries
- protocol index rows

That is the central architectural weakness.

## 2. The owner relation is weak

Today, `Task.owner` is an optional member name string.

That is insufficient for AI-team operations because it does not distinguish:

- stable member identity vs display name
- active execution endpoint vs inactive registered member
- ownership vs current executor
- cross-project handoff target vs current team pane

The system partially compensates elsewhere, but the task model itself stays weak.

## 3. Structured assignment metadata exists in theory, but not in the normal CLI path

Generated assignment and nudge messages know about:

- `first_step`
- `deliverable`
- `completion_signal`

But those come from `task.metadata`, and native `mesh task create/update` does not expose a real metadata authoring surface.

Result:

- the most useful assignment fields are not first-class in normal task authoring
- most tasks fall back to generic “Run: mesh task get X”
- richer instructions have to be layered externally or encoded indirectly

This is a direct capability gap for AI-only task operation.

## 4. Task lifecycle is under-specified and under-enforced

Current legacy statuses are effectively:

- `pending`
- `in_progress`
- `completed`
- `deleted`

Problems:

- `task update` can still write arbitrary statuses
- blocked work is represented as member status, not task status
- failed vs canceled vs closed are collapsed or absent
- there is no accepted/claimed state between assignment and execution
- no strict transition validation is applied in the write path

Mesh already has `transition_drift.rs`, which is a sign this problem is known. It just is not yet part of the canonical mutation path.

## 5. Message/ack state is not first-class task state

Message ack is tracked, but task workflow does not consume it directly.

Examples:

- assignment ack does not transition the task to “accepted”
- nudge ack does not clear a task-attention state
- completion message does not itself close the loop unless a separate task update happens
- `ack-status` is message-centric, not task-centric

For AI-only teams, this is a big gap. “Did the agent see the task?” is not secondary metadata. It is operationally important.

## 6. Query and reporting capability is too thin for the lead

For team leads, native task query is currently minimal.

Missing:

- team queue views by owner
- blocked/unblocked views
- unassigned queue
- overdue/idle/nudged views
- “tasks with no ack”
- “tasks assigned but not started”
- mutation history per task
- assignment history per member

The underlying data exists in pieces, but the command surface does not expose it.

## 7. Recovery after compaction is better than average, but still too generic

Mesh is strong at “you have task #X; here is the command.”

It is weaker at “resume the exact workflow state.”

Missing persisted recovery context:

- last meaningful progress note
- last outstanding ask from lead
- current substep / next step
- why the task is blocked
- whether the latest assignment superseded an older one
- whether there is unacked escalation or follow-up

This gap is visible in real use. As an assignee, I can recover the task shell quickly, but I still need to reread messages and task JSON separately to reconstruct the real working state.

## 8. Native Mesh has no project-scoped task model

Mesh tasks are team-scoped by directory.

There is no first-class project field on the task.

That is acceptable for Mesh alone, but it becomes weak when:

- one team spans multiple projects
- one inactive cross-project owner is still a valid assignee
- Taurhaus wants to show one project board

At that point, project meaning is inferred externally rather than declared natively.

## AI User Stories: Current Support vs Gaps

## Story 1: Team lead assigns a concrete task with objective, first action, deliverable, and completion criteria

Current support:

- lead can assign a task
- assignee receives a strong actionable message
- assignment can include structured fields if task metadata already contains them

Current gap:

- normal task authoring does not expose those structured fields directly
- the best assignment behavior depends on opaque metadata that native Mesh does not manage well

Assessment:

- partially supported

## Story 2: Assignee loses context, runs `mesh read`, and resumes correctly

Current support:

- assignment notifications are compaction-safe
- `mesh read --unread` shows unread assignments
- if there are no unread messages, `mesh read --unread` can synthesize active-task fallback

Current gap:

- fallback is only task number plus subject plus `mesh task get`
- recovery is not bound to a persisted next-step or progress checkpoint

Assessment:

- supported, but shallow

## Story 3: Assignee needs to show progress without closing the task

Current support:

- task status can move to `in_progress`
- member can set explicit status like `working`, `blocked`, `investigating`
- member can send follow-up messages

Current gap:

- progress note is not a first-class task field
- task state and member status are separate systems
- blocked is a member property, not a task lifecycle state

Assessment:

- weakly supported

## Story 4: Team lead needs to know which assigned tasks were seen, accepted, started, blocked, or finished

Current support:

- lead can list tasks
- lead can inspect one task
- lead can query ack status by message ID
- lead can infer some state from status and messages

Current gap:

- no first-class acceptance state
- no task-centric ack view
- no started-at / accepted-at / blocked-at fields
- no unified lead dashboard or query surface

Assessment:

- weakly supported

## Story 5: Team daemon should nudge only when there is real unattended work

Current support:

- owned active tasks gate nudge behavior
- explicit status and heartbeat suppress nudges
- activity/runtime signals can suppress or escalate

Current gap:

- task model has no attention state
- idle-monitor operates from external signals and marker files
- repeated-nudge behavior remains structurally awkward
- no assignment acceptance or progress checkpoint to anchor “real inactivity”

Assessment:

- functionally supported, architecturally weak

## Story 6: Team lead needs to recover after agent compaction and still understand task state

Current support:

- messages are designed for AI recovery
- task subject/status/owner remain durable
- protocol-analysis modules exist

Current gap:

- no canonical task conversation ledger
- no first-class linkage from assignment message to completion signal
- lead must reconstruct from task snapshot plus inbox plus ack state

Assessment:

- partially supported

## Story 7: Cross-project AI teams need shared tasks without project bleed

Current support:

- native Mesh allows team-scoped shared tasks
- inactive cross-project members can still be valid owners

Current gap:

- no native project scope on the task
- downstream tools must infer project meaning from team config member paths

Assessment:

- native Mesh supports team scope, but not project scope

## Actual Task Architecture Today

## Real entities and relations

Today’s effective task architecture looks like this:

### Declared entities

- `Team`
- `Member`
- `Task`
- `TaskMutationEntry`
- `InboxMessage`
- `ProtocolRecord`

### Actual relations that exist

- team contains members via `config.json`
- task belongs to team implicitly via `~/.claude/tasks/{team}/`
- task owner references member by `name`
- task dependencies reference other tasks by task ID strings
- task mutation entries reference task by ID
- task mutation entries reference actor by name
- inbox message recipient is implied by inbox file path
- inbox ack is attached to message ID
- protocol record can reference both `message_id` and `task_id`

### Missing or weak relations

- task to assignee by stable `agent_id`
- task to project
- task to assignment event
- task to acceptance event
- task to execution event
- task to blocked reason
- task to completion evidence
- task to closure reason
- task to nudge/escalation events
- task to message ack state
- task to recovery context

That missing set is exactly why the system needs correlation, lint, and drift detection modules later.

## Recommended Task Architecture

Mesh should keep its local-file model, but promote the missing workflow relations into first-class entities.

## Recommended core entities

### 1. TaskSnapshot

Current task JSON can stay, but with a stronger schema:

- `task_id`
- `team_id`
- optional `project_scope`
- `title`
- `description`
- `state`
- `current_assignment_id`
- optional `current_executor_agent_id`
- optional `current_executor_name`
- priority / urgency
- dependency IDs
- current recovery summary
- created/updated timestamps

### 2. TaskAssignment

Separate assignment entity:

- `assignment_id`
- `task_id`
- `assigned_by_agent_id`
- `assigned_to_agent_id`
- display-name snapshots
- assigned timestamp
- accepted timestamp
- started timestamp
- superseded timestamp
- completion target / signal

### 3. TaskEvent

Unify today’s mutation journal and protocol-index intent rows into one authoritative task event stream.

Representative event kinds:

- `task_created`
- `task_updated`
- `task_assigned`
- `assignment_seen`
- `assignment_accepted`
- `execution_started`
- `task_blocked`
- `task_unblocked`
- `task_completed`
- `task_failed`
- `task_closed`
- `nudge_sent`
- `escalation_sent`
- `recovery_context_updated`

### 4. TaskMessageLink

Make task-message linkage first-class:

- `message_id`
- `task_id`
- optional `assignment_id`
- intent
- sender / recipient IDs
- whether ack is required
- whether ack was satisfied

### 5. TaskRecoveryContext

Persist AI-resume state explicitly:

- latest objective
- current next step
- latest deliverable target
- completion condition
- latest blocked reason
- latest lead instruction summary
- latest relevant message IDs

### 6. TaskAttentionState

Persist what idle-monitor currently externalizes:

- last nudge timestamp
- last escalation timestamp
- current attention level
- suppression reason
- last strong-progress signal

## Recommended state model

Replace the weak legacy model with a stricter lifecycle:

- `unassigned`
- `assigned`
- `accepted`
- `executing`
- `blocked`
- `review_ready`
- `completed`
- `failed`
- `closed`
- `canceled`

The important addition for AI teams is not complexity for its own sake.

It is the ability to distinguish:

- seen vs unseen work
- accepted vs ignored work
- executing vs blocked work
- completed vs merely messaged-about completion

## How Tasks Should Tie Into Messages, Nudges, Ack, And Compaction

## Assignment messages

Assignment should be derived from `TaskAssignment`, not from ad hoc task snapshot formatting alone.

Required linkage:

- one assignment message references one `assignment_id`
- ack can satisfy “assignment seen”
- follow-up completion can satisfy that same assignment’s completion criteria

## Acknowledgments

Ack should remain message-level, but task workflow must consume it.

Examples:

- assignment message ack -> `assignment_seen`
- escalation ack by lead -> `escalation_seen`
- no-response-needed info message -> does not affect task state

Without this link, ack is informative but operationally weak.

## Idle monitor and nudges

Idle-monitor should operate on task attention state, not only “owned task plus external suppressors.”

A good nudge decision should consider:

- active assignment exists
- assignment has been seen or not
- last meaningful progress event
- blocked state
- latest recovery summary age
- prior nudges/escalations for the same assignment

Nudge should emit:

- a task-linked message
- a task-attention event
- no duplicate nudge if the same assignment has no new evidence

## Completion signals

Completion should not rely only on “task update completed” or only on “completion-like message.”

It should require or at least strongly track:

- task completion event
- optional completion message
- optional lead ack / acceptance when relevant

This is exactly the gap the current `transition_drift.rs` module is trying to detect after the fact.

## Compaction recovery

Compaction recovery should be task-first:

- `mesh read --unread` should surface outstanding task-linked messages
- if no unread messages exist, fallback should use `TaskRecoveryContext`
- daemon wake notifications should include task-linked recovery context
- lead and assignee should be able to reconstruct the same active assignment without rereading multiple stores manually

The current generated messages are a good transport format. They just need stronger underlying task linkage.

## Taurhaus Integration Boundary

Taurhaus is not using a Mesh task RPC API. It is layering on top of the shared file protocol.

## What Taurhaus relies on from Mesh natively

- team-scoped task files under `~/.claude/tasks/{team}/`
- team config under `~/.claude/teams/{team}/config.json`
- the shared task JSON shape

## What Taurhaus adds on top

- project scoping by scanning task directories and classifying source keys
- cross-tool normalization across Claude-session tasks, Mesh-team tasks, Codex plan steps, and Gemini TODOs
- SQLite persistence and archival history
- UI task board and history view
- commit/file enrichment
- event-driven refresh and DB caching

## Important Taurhaus constraints caused by Mesh’s current model

### 1. Team tasks are only implicitly project-scoped

Taurhaus maps Mesh team task directories to projects by reading team member paths from team config.

If one team spans multiple projects, one team task directory can map to multiple project paths.

That is not a Taurhaus invention. It is a consequence of Mesh not declaring project scope on the task itself.

### 2. Taurhaus reads snapshots, not workflow relations

Taurhaus sees:

- task JSON fields
- owner
- dependencies

It does not get a native assignment ledger, acceptance state, or task attention state from Mesh.

So Taurhaus can display tasks well, but it cannot recover semantics that Mesh never persisted.

### 3. Freshness is file-scan driven

Because Mesh writes files directly and Taurhaus scans/imports them, UI freshness is eventual.

That is acceptable for the architecture, but it makes explicit event relations even more important.

## Observed Usage Friction

These are not hypothetical.

They are the kinds of friction that show up while operating as an AI assignee in this system:

- assignment is very usable in the moment, but still arrives as prose that I have to parse instead of as first-class task state
- after compaction, I can recover the task command quickly, but not the exact progress context unless I reread multiple messages
- acknowledgment is visible, but it does not actually change the task workflow state
- “blocked” is easy to express as member status, but that does not make the task itself blocked
- team-lead coordination still depends on remembering message IDs, because task and message lifecycle are not unified

This is the clearest sign that Mesh is already beyond “simple todo files” and needs a stronger task domain.

## Recommended Follow-Up Tasks

## Highest-value follow-up tasks

### 1. Define and enforce a canonical task lifecycle

Implement:

- strict allowed states
- strict legal transitions
- write-path validation in `task update` / `task assign`

Why first:

- almost every other workflow depends on reliable task state

### 2. Promote assignment metadata to first-class CLI/API fields

Implement native support for:

- `first_step`
- `deliverable`
- `completion_signal`
- optional `project_scope`
- optional priority

Why:

- current assignment messages want these fields already
- AI task execution quality depends on them

### 3. Unify task mutation journal and protocol linkage into a canonical task event stream

Keep snapshot files, but add authoritative workflow events:

- assignment
- ack seen
- started
- blocked
- completed
- nudged
- escalated
- recovery updated

Why:

- removes the need to reconstruct semantics from side channels

### 4. Replace owner-by-name with assignment to stable agent identity

Store:

- `agent_id`
- display-name snapshot
- active/inactive execution target semantics

Why:

- owner string is too weak for AI team operation

### 5. Add task-centric lead query surfaces

At minimum:

- `mesh tasks --owner <name>`
- `mesh tasks --unassigned`
- `mesh tasks --blocked`
- `mesh tasks --attention-needed`
- `mesh task history <id>`
- `mesh task status <id>` including latest ack/nudge/escalation state

Why:

- team-lead visibility is the biggest day-to-day operational gap

### 6. Persist task recovery context and use it in `mesh read` fallback

Make recovery fallback task-aware:

- last meaningful progress note
- current next step
- latest lead instruction
- blocked reason

Why:

- this directly improves AI compaction recovery

### 7. Tighten Taurhaus project attribution for Mesh team tasks

Either:

- add project scope natively to Mesh tasks, or
- make one canonical team project explicit in config

Why:

- current multi-project inference is too weak for durable project boards

### 8. Add end-to-end tests for the real AI workflow loop

Cover:

- assign -> daemon wake -> read -> ack -> start -> blocked -> nudge suppression -> complete -> completion signal
- compaction recovery with no unread messages but active assignment
- multi-project team task attribution into Taurhaus

Why:

- the current system spans multiple stores; unit tests alone are not enough

## Bottom Line

Mesh already has a credible AI-team task-management foundation.

Its current strength is operational pragmatism:

- durable files
- good assignment delivery
- inbox and ack support
- task-aware nudges
- compaction-safe messaging

Its current weakness is domain coherence:

- the core task model is too small
- the important workflow relations are spread across side stores
- downstream systems and operators have to reconstruct meaning that should be explicit

The next phase should not replace Mesh task storage.

It should make task lifecycle, assignment, ack, recovery, and attention state first-class so that:

- the lead can manage work by task, not by message archaeology
- the assignee can recover after compaction by reading task state, not by parsing prose
- Taurhaus can consume richer native Mesh semantics instead of inferring them
