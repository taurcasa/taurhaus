# Orchestration Protocol Design (Taurhaus)

> [!WARNING]
> ARCHIVED — historical proposal, NOT active implementation target.
> This document records a superseded v0.2.0-oriented design exploration.
> Do not treat the embedded-block parser, protocol index/journal, transition validator,
> drift detector, or suppression evaluator sections as current implementation commitments.

Date: 2026-03-06  
Owner: architect  
Contributors: architect + mesh-expert (feasibility review)

Inputs:
- [`docs/ai-agent-characteristics.md`](/home/user/projects/taurhaus/docs/ai-agent-characteristics.md)
- team-lead complete signal/data inventory (2026-03-06)
- mesh-expert feasibility guidance on current mesh constraints

## 1. Vision & Mission

### Vision

Excellent multi-agent orchestration in taurhaus means:

- Assignments are operational and self-executing.
- Lead stays in orchestration role under pressure and after compaction.
- Task state remains authoritative across tasks, messages, and UI signals.
- Known model/tool quirks are handled by protocol and guardrails.

### Mission

Define a reliable orchestration protocol that works with current real interfaces:

- `SendMessage(type, recipient, content, summary)` only
- task files + task update APIs as structured authority
- fixed inbox schema that cannot preserve unknown fields

## 2. Hard Constraints (Inventory-Grounded)

1. `SendMessage` has only 4 fields. No direct protocol metadata transport fields exist.
2. Inbox rewrite paths drop unknown JSON fields. Inbox cannot be protocol metadata storage.
3. Structured task data exists and is authoritative (`TaskCreate`, `TaskUpdate`, task JSON files).
4. Runtime/health signals exist (`runtime/{agent}.json`, heartbeat/status APIs, idle reminder files, taurhaus telemetry).
5. Protocol lint, task/message correlation, and transition validation do not exist yet and must be built.

## 3. Design Decisions (Answers To Key Questions)

### Q1. Embedded content block: still right approach?

Yes, but narrowed.

- Use embedded block for message semantics that cannot come from task tools (`intent` for non-task messages, `no_response_needed`, optional explicit `task_id`, optional execution directives).
- Do not rely on embedded block as primary assignment lifecycle truth.
- Assignment lifecycle truth comes from task updates/files.

### Q2. Is time-window sender/recipient correlation reliable enough?

Not by itself.

Reliable rule set:

1. Primary correlation key: explicit `task_id` in embedded block.
2. Secondary correlation key: exactly one candidate assignment context for same sender->recipient in bounded window.
3. If secondary is ambiguous or empty: no silent inference; emit blocker/lint error and require explicit `task_id`.

This keeps behavior deterministic and avoids heuristic guessing.

### Q3. What mesh infrastructure must be built?

See Section 7 for exact build list. Core additions are:

- protocol index/journal (separate from inbox)
- embedded block parser + validator
- task/message correlator
- transition edge validator
- nudge suppression evaluator using runtime + idle reminder signals
- drift detector + telemetry

### Q4. Map every protocol field to concrete source

See Section 5 field-source mapping table.

### Q5. Include runtime health/last_seen and idle_reminded in suppression

Included in Section 6 suppression algorithm with explicit precedence and thresholds.

## 4. Task-Anchored Hybrid v1 (Current-Interface Compatible)

### 4.1 Source-of-truth split

- Task lifecycle authority: task tools + task files.
- Message semantics authority: embedded `orchestration_v1` block in `content`.
- Protocol persistence authority: mesh protocol index/journal (not inbox).

### 4.2 Canonical normalized record (stored in protocol index)

Canonical record key:

- `record_id` (mesh-generated, primary)
- optional `message_id` (when available from mesh delivery path)
- optional `task_id`

Canonical fields:

- `protocol_version`
- `message_id`
- `intent`
- `task_id`
- `action_required`
- `no_response_needed`
- `first_step`
- `deliverable`
- `completion_signal`
- `precedence`
- `sender`
- `recipient`
- `message_type`
- `timestamp`
- `task_owner`
- `task_status`
- `blocked_by`
- `blocks`

### 4.3 Field authority precedence

1. Task lifecycle/dependency fields: task data is authoritative.
2. Message semantics fields: embedded block is authoritative.
3. Free-text outside block: never authoritative; warning/suggestion only.

No optional `SendMessage` schema extension is required for v1.

## 5. Protocol Field -> Concrete Data Source Mapping

| Protocol field | Concrete source | Rule |
|---|---|---|
| `protocol_version` | mesh protocol normalizer constant | fixed to `1` |
| `message_id` | mesh delivery layer existing message id | optional (present when available) |
| `sender` | message envelope sender | required |
| `recipient` | `SendMessage.recipient` | required |
| `message_type` | `SendMessage.type` | optional for inbox-only observed events; default to `message` when not surfaced |
| `timestamp` | inbox message timestamp / send event timestamp | required |
| `intent` (task assignment) | task-mutation journal + correlated `TaskUpdate` owner/status + optional embedded block consistency check | infer `assign` when deterministic assignment context exists |
| `intent` (non-task) | embedded block `intent` | required for actionable non-task messages |
| `task_id` | embedded block `task_id` OR correlated task id from task context | required for assignment/nudge/close |
| `action_required` | derived from intent (`assign|nudge` => true, `info` => false) unless explicitly set in embedded block | deterministic derive |
| `no_response_needed` | embedded block field (or default true for explicit `info`) | required for info/broadcast protocol compliance |
| `first_step` | task `metadata.orchestration.first_step` OR embedded block `first_step` | required for assign/nudge |
| `deliverable` | task `metadata.orchestration.deliverable` OR embedded block `deliverable` | required for assign |
| `completion_signal` | task `metadata.orchestration.completion_signal` OR embedded block `completion_signal` | required for assign |
| `precedence` | task `metadata.orchestration.precedence` OR embedded block `precedence` | recommended for actionable intents |
| `task_owner` | task file `owner` | task-authoritative |
| `task_status` | task file `status` | task-authoritative |
| `blocked_by` | task file `blockedBy` | task-authoritative |
| `blocks` | task file `blocks` | task-authoritative |
| `mutation_actor` | task-mutation journal | required for authoritative sender correlation |
| `mutation_timestamp` | task-mutation journal | required for authoritative time correlation |

Notes:

- If both task metadata and embedded block provide `first_step`/`deliverable`/`completion_signal`, lint warns on mismatch and uses embedded block as message semantics authority.
- Task lifecycle fields are never overridden by embedded block text.
- `activeForm` is fallback hint only and is never a primary source for required execution fields.

## 6. Deterministic Correlation + Nudge Suppression

### 6.1 Assignment correlation algorithm

Inputs:

- task events (`TaskCreate`/`TaskUpdate`)
- task file snapshots
- task-mutation journal entries (`task_id`, actor, timestamp, changed fields)
- `SendMessage` event (`sender`, `recipient`, `content`, `summary`, `timestamp`)

Algorithm:

1. Parse embedded block if present.
2. In enforce mode, actionable messages must include explicit `task_id` in block.
3. If block has `task_id`, bind directly to that task id.
4. Else find assignment candidates where:
   - task `owner == recipient`
   - task status in active set (`pending` for queued assignment, `in_progress` for active execution)
   - mutation actor aligns with message sender (from mutation journal)
   - event in tight bounded window (`<= 90s`)
5. If exactly one candidate, correlate.
6. If zero or >1 candidates, mark as uncorrelated and emit blocker/lint error for actionable message.

Authoritative correlation requirement:

- task files alone are insufficient for actor/time correlation.
- mesh must persist task-mutation journal at write time.
- if journal is unavailable, correlation is best-effort only (warn mode) and cannot be authoritative.

### 6.2 Nudge suppression (must use runtime + state signals)

Suppress idle/stall nudge when any strong-active signal is true:

1. Member status API indicates `working|blocked|investigating` with live TTL.
2. Heartbeat freshness `<= 3m`.
3. Taurhaus stall detector reports recent pane/tool activity.
4. `config.json` member `lastActivityAt <= 2m`.

Runtime signal handling:

- `runtime/{agent}.json.last_seen_at` is delivery/attachment freshness only; not proof of read or execution progress.
- `runtime/{agent}.json.health` is used for dead/offline escalation logic, not as positive work-progress suppressor.

Duplicate-nudge suppression:

1. If `state/{agent}.idle_reminded` exists and file mtime is within cooldown (`<= 10m`), skip repeat nudge (anti-spam only).
2. Clear/remove idle reminder flag after fresh activity is observed.

Escalate instead of nudge when:

- runtime health is non-healthy for >5m, or
- stall detector indicates no output and no active process signal across suppression window.

### 6.3 Task transition validation (deterministic)

Authoritative transition API target:

- `pending -> in_progress -> completed|deleted`

Canonical strict lifecycle mapping (for protocol semantics):

- `pending -> assigned`
- `in_progress -> executing`
- `completed -> completed`
- `deleted -> failed|closed` (policy-defined closure mapping)

Validation rules:

1. Reject illegal backward edges in enforce mode.
2. Allow idempotent replay of same transition.
3. Emit drift event if message semantics indicate completion but task status remains non-terminal.

## 7. Mesh Build Plan (New Code vs Existing)

### 7.1 Already exists (reuse)

- message send + message ids + read tracking
- task CRUD + dependencies
- heartbeat API
- member status API
- idle reminder primitive

### 7.2 Must build (new)

1. Protocol index/journal store:
   - persistent normalized protocol records keyed by `record_id` (+ optional `message_id`/`task_id` indexes)
   - separate from inbox JSON
2. Task-mutation journal:
   - append-only write-time log: `task_id`, actor, timestamp, changed fields
   - required for authoritative sender/time correlation
3. Embedded block parser:
   - deterministic parser for `[orchestration_v1] ... [/orchestration_v1]`
4. Correlation service:
   - joins send events with task updates using deterministic rules
5. Protocol lint engine:
   - `warn` and `enforce` modes on normalized records
6. Transition validator:
   - strict task transition edges + canonical legacy mapping
7. Drift detector:
   - detects task/message completion mismatches and orphan transitions
8. Suppression evaluator:
   - evaluates heartbeat/status/runtime/config/stall/idle-reminded signals
9. Telemetry counters:
   - lint failures, correlation ambiguity count, suppression-hit rate, assignment->executing latency, drift count

## 8. Rollout (Reality-First)

Phase 1:

- adopt embedded block template and task metadata keys for execution semantics
- run lint in `warn` mode

Phase 2:

- ship protocol index/journal + deterministic correlator
- enforce blocker on ambiguous assignment correlation

Phase 3:

- enable strict transition validator + drift detector
- enable nudge suppression evaluator with runtime/idle-reminded inputs

Phase 4:

- switch key lint rules to `enforce` after telemetry stabilization

## 9. Embedded Block Template (Current SendMessage-Compatible)

```text
[orchestration_v1]
intent: assign|nudge|info|close
task_id: <task-id-or-empty>
no_response_needed: true|false
first_step: <single concrete first action>
deliverable: <artifact path or output contract>
completion_signal: <required completion response>
precedence: <ordered policy list>
[/orchestration_v1]

<human-readable body>
```

Parser requirements:

- deterministic key parsing; ignore unknown keys with lint warn
- block is optional for non-actionable casual messages
- block required for actionable non-task messages
- for task assignments, block is required in enforce mode (must include explicit `task_id`)
- if task assignment block omits execution fields, task metadata must satisfy required execution fields

## 10. Success Criteria

1. `stall_on_ack_count = 0` (14-day rolling)
2. `assignment_to_executing_p50_secs < 180`
3. `protocol_lint_fail_rate < 5%` after week 2
4. `correlation_ambiguity_count = 0` in enforce mode
5. `task_message_drift_count = 0` for 14 consecutive days
6. `lead_role_drift_incidents = 0` for 14 consecutive days
