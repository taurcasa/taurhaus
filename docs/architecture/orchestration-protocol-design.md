# Orchestration Protocol Design (Taurhaus)

Date: 2026-03-06  
Owner: architect  
Contributors: architect + mesh-expert (feasibility review)

Inputs:
- [`docs/ai-agent-characteristics.md`](/home/mstie/projects/taurhaus/docs/ai-agent-characteristics.md)
- coordination subsystem and stall detector implementation
- mesh-expert feasibility feedback (2026-03-05 23:28 UTC)
- team-lead transport constraint note (2026-03-05 23:44 UTC): Claude `SendMessage` currently supports only `type`, `recipient`, `content`, `summary`

## 1. Vision & Mission

### Vision

Excellent multi-agent orchestration in taurhaus means:

- Assignments are operational and self-executing, not conversationally ambiguous.
- Lead remains an orchestrator, especially under pressure and after compaction.
- Task state is authoritative and consistent across UI, messages, and automation.
- Model-specific quirks are handled by protocol + guardrails, not memory or luck.

### Mission

Define a compatibility-first orchestration protocol and rollout that eliminates known coordination failure modes without breaking existing mesh consumers.

### Measurable goals

1. `0` stall-on-ack incidents per rolling 14 days.
2. Median assignment overhead `< 2` message round-trips per task.
3. `>= 95%` of assignments transition to `executing` within 3 minutes.
4. `0` lead role-drift incidents after compaction.
5. `0` task-state divergence incidents (task DB vs message flow).
6. `>= 90%` protocol-lint pass rate on first send.

### Scope

In scope:

- Orchestration protocol envelope and lint rules.
- Guardrails for assignment quality, stalls, and drift.
- Layer boundaries between mesh infra, taurhaus conventions, and per-model framing.
- Migration path from permissive current behavior to enforceable protocol.

Out of scope:

- Modifying base model internals.
- Big-bang breaking changes to existing mesh inbox consumers.
- Replacing mesh transport primitives.

## 2. Current State Assessment

### Observed failure modes

| Failure mode | Root cause | Current workaround | Workaround cost |
|---|---|---|---|
| Stall-on-ack | Ack-only messages terminate Codex execution loop | Lead avoids pure acknowledgments manually | High cognitive load; brittle |
| Task-read-not-execute | “You have task X” framing is informational, not operational | Re-send with imperative first step | Extra round-trips |
| Priority inversion | Conflicting directives (task brief vs repo policy vs message) lack explicit precedence | Manual clarification | Delayed starts, confusion |
| Split directives | One instruction spread over multiple micro-messages | Manual consolidation | Message noise, missed context |
| Post-compaction role drift (lead) | Lead resumes implementation instead of orchestration | Manual reminders/corrections | Orchestrator bottleneck |
| Task state drift | Task status, message stream, and docs diverge | Manual reconciliation | Hidden inconsistency, duplicate work |
| Idle ambiguity | Silence interpreted without typed intent/state | Manual “check messages” pings | Produces read-and-stop loops |

### Root takeaway

Most failures are protocol-quality failures, not model-capability failures.

### Existing capabilities we can leverage now

Already available in mesh/taurhaus stack:

- Message IDs and explicit ack/ack-status.
- Heartbeat API (activity freshness).
- Explicit member status (`blocked|investigating|working`) with TTL semantics.
- Idle reminder/task fallback nudge primitives.
- Structured task tools (`TaskCreate`, `TaskUpdate`) with rich metadata (`taskId`, `owner`, `status`, `description`, `activeForm`, `metadata`, dependencies).
- Taurhaus stall detector + orchestration runtime telemetry.

## 3. Separation of Concerns

### Mesh infrastructure responsibility

Mesh should own protocol mechanics and transport-safe enforcement:

- Typed orchestration metadata envelope (compatibility-first extension fields).
- Field derivation/correlation pipeline (task events + messages -> canonical protocol fields).
- `mesh protocol lint` and mode control (`warn` vs `enforce`).
- Task transition API + edge validation + idempotency.
- Message/task drift detection at protocol layer.
- Telemetry for delivery, transition latency, lint outcomes.

### Taurhaus/project responsibility

Taurhaus/team should own domain conventions and operating policy:

- Role definitions and task assignment checklist.
- Quality gate conventions (`check-quick`, `agent-quality`, etc.).
- Repository policy precedence declaration.
- UI surfacing of drift/stall signals and remediation UX.

### Per-model adaptation responsibility

Adapter layer should own framing transformation, not truth:

- Render protocol payload into model-specific message style.
- Suppress model-specific anti-patterns.
- Normalize completion/blocker reporting format.

### Boundary rule

- Infrastructure enforces mechanics.
- Project policy defines intent.
- Model adapters shape presentation.

No duplication of authoritative task state across layers.

## 4. Design Proposals

### 4.1 Transport-compatible Orchestration Protocol v1

Current hard constraint:

- Claude lead currently sends via `SendMessage(type, recipient, content, summary)` only.
- No direct metadata fields are currently attachable from that interface.

Compatibility placement contract (authoritative persisted form):

- Target canonical message placement: `extensions.orchestration_v1`.
- Legacy consumers that ignore unknown extension keys remain unaffected.
- Current mesh constraint: inbox typed deserialize/re-serialize paths drop unknown fields on rewrite.
- Therefore, v1 launch must not depend on unknown-field passthrough in inbox JSON until mesh persistence is fixed.
- Interim persistence: protocol-normalized records keyed by `message_id` in mesh protocol index/audit data; mirror to `extensions.orchestration_v1` once lossless persistence is available.

Canonical v1 metadata contract (`extensions.orchestration_v1`):

- required: `protocol_version`, `message_id`, `intent`
- conditional by intent: `task_id`, `action_required`, `no_response_needed`, `first_step`, `deliverable`, `completion_signal`, `precedence`

Protocol source model (task-anchored hybrid v1):

1. Task-derived profile (preferred for assignments):
   - correlate `TaskCreate`/`TaskUpdate` events with subsequent `SendMessage`.
   - example assignment signal: task owner set to recipient and task status moved to active (`assigned`/`in_progress`) in near-time window.
   - derive assignment core fields (`intent=assign`, `task_id`, `action_required=true`) from structured task events.
   - execution details are expected from message semantics payload; task metadata/description are fallback hints only.
2. Content-embedded profile (default fallback for general messages and current Claude constraints):
   - embed a deterministic metadata block at the top of `content`, namespaced as `orchestration_v1`.
   - mesh parses and normalizes this into `extensions.orchestration_v1` server-side.
3. Structured message profile (future, if `SendMessage` gains optional fields):
   - optional protocol fields are supplied directly by client/tool.
   - mesh writes them directly to `extensions.orchestration_v1`.

Source precedence rules:

1. Task tool data (highest, authoritative for lifecycle truth):
   - `task_id`, owner, status/lifecycle, dependencies, `active_form`
   - source: `TaskCreate`/`TaskUpdate` and task records
2. SendMessage optional protocol fields (future additive layer):
   - if implemented later, canonical for message semantics
3. Message convention payload (`content` embedded `orchestration_v1` block):
   - canonical message semantics path for current 4-parameter interface
4. Free-text heuristics (lowest):
   - warning/suggestion only, never authoritative

Field authority mapping:

- task-lifecycle fields (`task_id`, owner, status/lifecycle, dependencies):
  - task tool data first; other sources may reference but not override task truth.
- message-semantics fields (`intent`, `action_required`, `no_response_needed`, `first_step`, `deliverable`, `completion_signal`, `precedence`):
  - structured message profile when available
  - otherwise parsed embedded `orchestration_v1` block
  - heuristics only for warnings/suggestions when explicit semantics are missing.

Conflict rule when multiple sources are present:

- task data is authoritative for lifecycle fields.
- for message-semantics fields, structured profile (when available) is authoritative over embedded payload.
- lint emits mismatch findings for diverging sources.

Default rollout mode: `warn` lint, not hard enforcement.

### 4.2 Linting and enforcement strategy

Add `mesh protocol lint` with modes:

- `warn` (default): emits lint findings, allows send.
- `enforce` (opt-in team setting): blocks send on error-level violations.

Lint input sources:

- structured message protocol fields (when present).
- task-derived fields from correlated `TaskCreate`/`TaskUpdate`.
- parsed `content` embedded block for 4-parameter `SendMessage` senders.

Lint evaluation rule:

- evaluate required fields on the canonical normalized record after derivation.
- for assignment intents, `intent`, `task_id`, and lifecycle context may be satisfied by task-derived data.
- for assignment execution fields (`first_step`, `deliverable`, `completion_signal`), require structured message fields or embedded content block; lint if missing.
- for non-task general messages, embedded or structured fields are required to satisfy protocol checks.

Conflict rules:

- protocol `task_id` conflicting with correlated task record/event -> lint error (block in enforce mode).
- embedded block conflicting with structured message profile -> structured wins; lint mismatch.
- message indicates completion while task state is not transitioned -> drift event.

Ack-only detection rollout:

- Phase A: heuristic warn-first (canned short-ack patterns).
- Phase B: promote to enforce for actionable conversations.

### 4.3 Task transition ownership and migration

Target authoritative API:

- `mesh task transition --task-id ... --to assigned|executing|completed|blocked|failed|closed`

Allowed edges (strict mode):

- `assigned -> executing -> completed|blocked|failed|closed`
- idempotent replays of same transition allowed.

Canonical legacy -> strict transition mapping:

- `pending -> assigned`
- `in_progress -> executing`
- `completed -> completed`
- `deleted -> failed|closed` (policy-defined by team/project)

Migration bridge:

- Continue applying canonical mapping for legacy statuses (`pending/in_progress/completed/deleted`) during transition window.
- Tighten to strict edge validation once adoption and telemetry are stable.

### 4.4 Automated guardrails

#### A) Assignment quality gate (pre-send)

Warn/block rules:

- actionable intent missing `first_step`.
- actionable intent missing `task_id`.
- missing `completion_signal` for assignment.
- informational broadcast missing `no_response_needed=true`.

#### B) Task-read-not-execute detector

If assignment delivered and no `task -> executing` within timeout:

- auto-nudge with concrete first step.
- suppress nudge when fresh heartbeat/status indicates active or blocked/investigating work.
- default freshness windows for suppression:
  - heartbeat `<= 3m`
  - status `<= 10m`

#### C) Drift monitor

Detect and surface when task authoritative state and message protocol state disagree:

- message says done but task not transitioned.
- task transitioned without completion signal message.

#### D) Lead role-drift detection

Primary detector location: taurhaus orchestration runtime.

Why: taurhaus sees pane/session command telemetry and queue depth context; mesh alone does not.

Mesh contributes supporting signals:

- backlog depth
- heartbeat freshness
- status freshness

### 4.5 Convention changes (team behavior)

Assignment checklist (mandatory):

1. Objective in one sentence.
2. Exact deliverable path/output contract.
3. Concrete first action.
4. Completion signal.
5. Explicit response expectation (`no_response_needed` where applicable).

Anti-pattern enforcement:

- no pure acknowledgments to active assignees.
- no “check messages” without explicit action.
- no split assignment across many micro-messages when one complete payload is possible.

### 4.6 Failure mode -> solution mapping

| Failure mode | Primary fix |
|---|---|
| Stall-on-ack | info intent + `no_response_needed=true` + ack lint |
| Task-read-not-execute | required first step + execution timeout nudge |
| Priority inversion | explicit precedence field + conflict contract |
| Split directives | single structured assignment payload |
| Role drift | taurhaus role-drift detector + delegation reminder |
| Task/message drift | authoritative transition API + drift monitor |
| Idle ambiguity | typed status/heartbeat-aware nudge suppression |

### 4.7 Rollout plan (aligned with mesh-expert feasibility)

Phase 1: conventions now

- apply assignment checklist and anti-pattern rules immediately.
- enforce `no_response_needed` discipline for informational traffic.

Phase 2: task-tool integration + content-embedded profile + lint warn mode

- implement task/message correlation to derive assignment protocol fields from `TaskCreate`/`TaskUpdate`.
- ship canonical `orchestration_v1` content block template for lead-assignment messages.
- parse/normalize embedded block into canonical protocol record.
- run `mesh protocol lint` in warn mode against parsed canonical fields.
- collect lint and transition latency metrics.

Phase 3: automation + optional structured profile

- enable `assign -> executing` timeout auto-nudge.
- suppress using heartbeat and explicit status (`blocked|investigating`).
- if mesh confirms `SendMessage` extensibility, add optional structured protocol fields and dual-write validation.

Phase 4: enforce mode + strict transitions

- enable lint enforce for actionable intents.
- adopt strict `task transition` edges as team default.

## 5. Success Criteria

### Metrics

Weekly dashboard:

- `stall_on_ack_count`
- `assignment_round_trips_p50`
- `assignment_to_executing_p50_secs`
- `protocol_lint_fail_rate`
- `task_message_drift_count`
- `lead_role_drift_incidents`

Targets:

1. `stall_on_ack_count = 0`
2. `assignment_round_trips_p50 < 2`
3. `assignment_to_executing_p50_secs < 180`
4. `protocol_lint_fail_rate < 5%` after rollout week 2
5. `task_message_drift_count = 0` for 14 consecutive days
6. `lead_role_drift_incidents = 0` for 14 consecutive days

### Done definition

The design is “done” when:

1. Protocol v1 metadata + lint mode control are implemented and adopted.
2. Task transition API is authoritative for actionable lifecycle state.
3. Auto-nudge and drift monitors are running with suppression safeguards.
4. Role-drift detection is active in taurhaus with mesh support signals.
5. Metrics meet thresholds for two consecutive weekly windows.

## Appendix A: Protocol Field Table (v1)

| Field | Assign | Nudge | Info | Close | Required? |
|---|---|---|---|---|---|
| `protocol_version` | yes | yes | yes | yes | always |
| `message_id` | yes | yes | yes | yes | always |
| `intent` | yes | yes | yes | yes | always |
| `task_id` | yes | yes | no | yes | by intent |
| `action_required` | yes | yes | no | no | by intent |
| `no_response_needed` | optional | optional | yes | optional | by intent |
| `first_step` | yes | yes | no | no | by intent |
| `deliverable` | yes | optional | no | optional | by intent |
| `completion_signal` | yes | optional | no | optional | by intent |
| `precedence` | optional | optional | no | optional | recommended for actionable intents |

Transport note:

- For current Claude lead flows, these fields are carried in `content` via embedded `orchestration_v1` block and normalized server-side.
- For task-related assignment flows, core lifecycle fields may be derived from `TaskCreate`/`TaskUpdate` correlation.
- For future extended `SendMessage`, fields may be passed directly as optional structured parameters.
- Until inbox unknown-field persistence is fixed, canonical normalized protocol data should be persisted outside lossy inbox rewrite paths.

## Appendix B: Initial Lint Matrix

| Rule | Warn mode | Enforce mode |
|---|---|---|
| Actionable intent missing `task_id` | warn | block |
| Actionable intent missing `first_step` | warn | block |
| Assign missing `completion_signal` | warn | block |
| Info missing `no_response_needed=true` | warn | block |
| Ack-only message to active assignee | warn | block (after rollout hardening) |

## Appendix C: Embedded Content Template (current default transport)

```text
[orchestration_v1]
protocol_version: 1
message_id: <uuid>
intent: assign|nudge|info|close
task_id: <task-id-or-empty>
action_required: true|false
no_response_needed: true|false
first_step: <single concrete first action>
deliverable: <artifact path or output contract>
completion_signal: <required completion response>
precedence: <ordered policy list>
[/orchestration_v1]

<human-readable assignment body follows>
```

Parser requirements:

- deterministic key parsing, tolerant to field order.
- unknown keys ignored with warn-level lint.
- missing required fields follow lint mode policy (`warn` vs `enforce`).

## Appendix D: Task-Tool Correlation Rules (assignment path)

Correlation key:

- same sender, recipient equals task owner, and message timestamp near task mutation window.

Initial default window:

- `TaskUpdate`/`TaskCreate` to `SendMessage` correlation window: `<= 5m`.

Derived assignment minimum:

- `intent=assign`
- `task_id=<taskId>`
- `action_required=true`

Derived assignment details (in order):

1. structured `SendMessage` optional protocol fields (if available later)
2. embedded `orchestration_v1` content block (current required source)
3. task metadata/activeForm/description extraction as fallback hints (warn-only if still incomplete)

Safety behavior:

- primary task match key: explicit `task_id` from structured/embedded profile when present.
- secondary match key (if no explicit `task_id`): bounded time-window + sender/recipient/owner correlation.
- ambiguous multi-task match -> emit structured blocker (no silent inference) and require explicit task reference.
- no match -> treat as general message path.

Assignment rule:

- when task correlation infers assignment, require executable message semantics (`first_step`, `deliverable`, `completion_signal`) in structured or embedded message payload.

## Key aligned proposals

- Hybrid rollout that uses task-tool structured context for assignments and content embedding for general 4-field `SendMessage` traffic.
- v1 launch is feasible without extending `SendMessage`; optional fields are additive when/if available.
- Warn-first linting with planned enforce mode.
- Strict task transitions via dedicated API, with migration mapping from legacy statuses.
- Role-drift detection primarily in taurhaus runtime; mesh provides supporting signals.
- Incremental rollout with measurable gates at each phase.
