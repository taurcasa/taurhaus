# Orchestration Protocol Design (Taurhaus)

Date: 2026-03-06  
Owner: architect  
Contributors: architect + mesh-expert (feasibility review)

Inputs:
- [`docs/ai-agent-characteristics.md`](/home/mstie/projects/taurhaus/docs/ai-agent-characteristics.md)
- coordination subsystem and stall detector implementation
- mesh-expert feasibility feedback (2026-03-05 23:28 UTC)

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
- Taurhaus stall detector + orchestration runtime telemetry.

## 3. Separation of Concerns

### Mesh infrastructure responsibility

Mesh should own protocol mechanics and transport-safe enforcement:

- Typed orchestration metadata envelope (compatibility-first extension fields).
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

### 4.1 Compatibility-first Orchestration Protocol v1

Do not break current inbox schema. Add protocol metadata as extension fields via `mesh send` flags, persisted alongside existing fields.

Compatibility placement contract:

- Store all protocol metadata under one namespaced extension key: `extensions.orchestration_v1`.
- Legacy consumers that ignore unknown extension keys remain unaffected.

V1 metadata contract (`extensions.orchestration_v1`):

- required: `protocol_version`, `message_id`, `intent`
- conditional by intent: `task_id`, `action_required`, `no_response_needed`, `first_step`, `deliverable`, `completion_signal`, `precedence`

Proposed flags (mapped into `extensions.orchestration_v1`):

- `--intent`
- `--task-id`
- `--action-required`
- `--no-response-needed`
- `--first-step`
- `--deliverable`
- `--completion-signal`
- `--precedence`

Default rollout mode: `warn` lint, not hard enforcement.

### 4.2 Linting and enforcement strategy

Add `mesh protocol lint` with modes:

- `warn` (default): emits lint findings, allows send.
- `enforce` (opt-in team setting): blocks send on error-level violations.

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

Phase 2: optional envelope + lint warn mode

- ship protocol extension fields and `mesh protocol lint` in warn mode.
- collect lint and transition latency metrics.

Phase 3: automation

- enable `assign -> executing` timeout auto-nudge.
- suppress using heartbeat and explicit status (`blocked|investigating`).

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

## Appendix B: Initial Lint Matrix

| Rule | Warn mode | Enforce mode |
|---|---|---|
| Actionable intent missing `task_id` | warn | block |
| Actionable intent missing `first_step` | warn | block |
| Assign missing `completion_signal` | warn | block |
| Info missing `no_response_needed=true` | warn | block |
| Ack-only message to active assignee | warn | block (after rollout hardening) |

## Key aligned proposals

- Compatibility-first protocol rollout with extension fields, not breaking schema changes.
- Warn-first linting with planned enforce mode.
- Strict task transitions via dedicated API, with migration mapping from legacy statuses.
- Role-drift detection primarily in taurhaus runtime; mesh provides supporting signals.
- Incremental rollout with measurable gates at each phase.
