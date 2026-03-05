# Orchestration Protocol Design (Taurhaus)

Date: 2026-03-06  
Owner: architect  
Inputs: [`docs/ai-agent-characteristics.md`](/home/mstie/projects/taurhaus/docs/ai-agent-characteristics.md), coordination subsystem architecture, stall detector behavior, live team workflow experience.

## 1. Vision & Mission

### Vision

Excellent multi-agent orchestration in taurhaus means:

- Every task assignment is operational, unambiguous, and self-executing.
- Team lead stays in orchestration role under pressure and after compaction.
- Task state is authoritative and consistent across channels.
- Model-specific quirks are absorbed by protocol and guardrails, not ad-hoc human memory.

### Mission

Define and implement an orchestration protocol that converts fragile chat habits into deterministic task execution loops.

### Measurable goals

1. `0` stall-on-ack incidents per rolling 14 days.
2. Median assignment overhead `< 2` message round-trips per task (assignment -> delivery).
3. `>= 95%` of assignments transition to `executing` within 3 minutes of delivery.
4. `0` lead role-drift incidents after compaction (lead starts implementation without delegation).
5. `0` task-state divergence incidents (task system says one thing, messages/docs another).
6. `>= 90%` of assignments pass protocol lint on first send.

### Scope

In scope:

- Protocol-level message contract (assignment, nudge, info, completion).
- Guardrails in taurhaus/mesh orchestration layer.
- Team-level conventions and task templates.
- Per-model adaptation profile (Codex/Claude/Gemini framing).

Out of scope:

- Changing base model internals.
- Replacing mesh transport primitives end-to-end.
- Solving all productivity issues via infrastructure only (human conventions still required).

## 2. Current State Assessment

### Observed failure modes

| Failure mode | Root cause | Current workaround | Workaround cost |
|---|---|---|---|
| Stall-on-ack | Ack-only messages end execution loop for Codex agents | Lead avoids acknowledgments and manually appends follow-up directives | High cognitive load; easy to regress under pressure |
| Task-read-not-execute | Assignment messages are meta/instructional, not operational | Lead re-sends with imperative + first step | Extra round-trip and latency |
| Priority inversion under stacked directives | No explicit precedence between task brief, AGENTS/CLAUDE policy, and lead message | Manual clarification per incident | Frequent ambiguity and slower start |
| Split-directive fragmentation | One task is sent across many micro-messages; context is diffused | Lead repeats consolidated instructions | Message noise and avoidable rework |
| Post-compaction role drift (lead) | Lead loses operating posture and starts coding instead of delegating | Memory reminders and manual correction | Throughput bottleneck; delayed responses to agents |
| Single-source-of-truth drift | Task status, chat text, and docs diverge | Manual reconciliation in chat | Hidden stale state, duplicate work |
| Idle ambiguity | “Idle” is inferred from silence without protocol state semantics | Manual pings (“check messages”) | Produces read-and-stop behavior rather than execution |
| Command/message syntax fragility | Shell-like message text can be malformed when executed/copy-pasted | Manual resend/correction | Wasted cycle, occasional misfire |

### Notes from runtime experience

- The system already has useful guardrail primitives (`coordination/stall_detector.rs`, event/audit scaffolding), but orchestration protocol validation is still mostly social/manual.
- Current delivery templates in coordination are good onboarding scaffolding, but assignment lifecycle semantics are not enforced as a typed protocol contract.

## 3. Separation of Concerns

### Mesh infrastructure responsibility

Mesh infrastructure should own protocol mechanics and enforcement:

- Typed message envelope and required fields.
- Message intent taxonomy (`assign`, `nudge`, `info`, `close`, `handoff`).
- Task-state transition API and idempotent updates.
- Protocol linting/validation before send.
- Delivery/ack telemetry and timeout hooks.
- Automated orchestration guardrails (stall detection hooks, role drift alerts, anti-pattern checks).

### Project-level responsibility (taurhaus repo/team)

Project/team should own domain conventions:

- Role definitions (lead vs developers vs architect).
- Task quality gates (`just check-quick`, `just agent-quality` where applicable).
- Deliverable structure and file path standards.
- Repo policy precedence rules and conflict handling.
- Domain-specific acceptance criteria templates.

### Per-model adaptation responsibility

Model adaptation layer should own framing strategy, not task truth:

- Mapping canonical protocol fields into model-specific messaging templates.
- Known anti-pattern suppression (ack-only for Codex, over-specified design prompts for Gemini, role-drift nudges for Claude lead).
- Output normalization (completion summary shape, confidence flags, blocker format).

### Boundary rule

- Infrastructure enforces mechanics.
- Project defines intent and standards.
- Model adapters transform presentation.

No layer should duplicate another layer's source of truth.

## 4. Design Proposals

### 4.1 Protocol changes (standardized message contract)

Define **Orchestration Protocol v1** envelope for every directed work message:

Required fields:

- `protocol_version`
- `message_id`
- `team`
- `from`
- `to`
- `intent` (`assign|nudge|info|close|handoff`)
- `task_id` (required for `assign|nudge|close`)
- `action_required` (bool)
- `first_step` (required when `action_required=true`)
- `deliverable` (artifact path or explicit output contract)
- `completion_signal` (what the assignee must do on finish)
- `no_response_needed` (bool, required for non-action informational messages)
- `priority`
- `deadline` (optional)
- `precedence` (ordered list for conflict resolution)

Protocol semantics:

1. `assign` must include `first_step`, `deliverable`, `completion_signal`.
2. `info` must set `action_required=false` and `no_response_needed=true`.
3. `nudge` must reference existing `task_id` and include concrete next action.
4. `close` must include closure reason and final status.

### 4.2 Automated guardrails

#### A) Assignment lint gate (pre-send)

Block or warn on:

- Missing `first_step` in actionable message.
- Ack-only body while target has active in-progress task.
- Instruction split across multiple consecutive messages without a single actionable payload.
- Missing `task_id` for actionable intents.

#### B) Task-read-not-execute detector

If `assign` delivered and no `task -> executing` transition within timeout (default 180s):

- auto-send protocolized nudge with explicit first command/action.
- increment orchestration warning counter.

#### C) Lead role-drift detector

Use orchestration signals to detect lead doing implementation while queue has pending coordination events:

- Trigger when lead opens source/test-heavy command sequence while unresolved inbound assignment/reply backlog exceeds threshold.
- Emit reminder event: `orchestration.role_drift.detected`.
- Suggest delegation action with one-click task draft.

#### D) Single-source-of-truth enforcement

- Task system is authoritative for `status`, `owner`, `active_form`, `blocked_by`.
- Messages cannot mutate effective state unless accompanied by task transition call.
- UI should show “message/task drift” warning when inconsistency detected.

#### E) Priority conflict resolver

Encode precedence explicitly in protocol:

1. Safety/security constraints
2. Repository policy files
3. Current task acceptance criteria
4. Lead free-text instructions
5. Heuristic optimizations

If conflict detected, assignee returns structured blocker instead of guessing.

### 4.3 Convention changes (team operating protocol)

#### Assignment checklist (mandatory)

Before lead sends assignment:

1. Objective stated in one sentence.
2. Exact artifact/deliverable path named.
3. Concrete first action specified.
4. Completion signal specified.
5. `no_response_needed` policy explicit.

#### Communication anti-pattern enforcement

Disallow in lead playbook:

- “You have task X” without executable first step.
- Pure acknowledgment to active assignee.
- “Check messages” without explicit required action.
- Multi-message fragmented directive when one structured assignment can be sent.

#### Completion contract convention

Every assignee completion message includes:

- `task_id`
- artifacts changed
- verification performed (or explicit skipped reason)
- residual risks/blockers

### 4.4 Infrastructure changes (mesh system)

1. Add protocol-aware send command:
   - `mesh send --intent assign --task-id ... --first-step ... --deliverable ...`
2. Add orchestration lint command:
   - `mesh protocol lint --message <json|file>`
3. Add task transition API with strict state machine:
   - `assigned -> executing -> completed|blocked|failed`
4. Add drift monitor service:
   - compares task state and message stream; emits divergence events.
5. Add role-drift hooks in orchestration runtime:
   - queue-depth + lead command pattern heuristics.
6. Add protocol telemetry stream:
   - round-trip counts, transition latency, lint failures, auto-nudges.

### 4.5 Failure mode -> proposal mapping

| Failure mode | Primary fix |
|---|---|
| Stall-on-ack | `intent=info` + `no_response_needed=true` + ack-lint guard |
| Task-read-not-execute | Required `first_step` + execution timeout auto-nudge |
| Priority inversion | Explicit `precedence` field + conflict escalation contract |
| Split directives | Single structured assignment payload requirement |
| Post-compaction role drift | Lead role-drift detector + delegation reminder guardrail |
| Task state divergence | Task transition API as authority + drift monitor |
| Idle ambiguity | Stateful task lifecycle + actionable nudge intents |
| Syntax fragility | Typed fields instead of shell-fragile free-text contracts |

### 4.6 Rollout plan

Phase 1 (convention-first, immediate):

- Adopt assignment checklist and anti-pattern rules.
- Enforce `no_response_needed` for broadcast/info.
- Track baseline metrics manually.

Phase 2 (protocol scaffolding):

- Introduce protocol envelope and lint in mesh/taurhaus integration layer.
- Instrument transition latency and round-trips.

Phase 3 (guardrail automation):

- Enable task-read-not-execute auto-nudge.
- Enable role-drift and drift monitor alerts.
- Enforce strict task/message consistency.

## 5. Success Criteria

### Metrics dashboard (weekly)

Track:

- `stall_on_ack_count`
- `assignment_to_executing_p50_secs`
- `assignment_round_trips_p50`
- `protocol_lint_fail_rate`
- `task_message_drift_count`
- `role_drift_incident_count`

Target thresholds:

1. `stall_on_ack_count = 0`.
2. `assignment_round_trips_p50 < 2`.
3. `assignment_to_executing_p50_secs < 180`.
4. `protocol_lint_fail_rate < 5%` after week 2.
5. `task_message_drift_count = 0` for 14 consecutive days.
6. `role_drift_incident_count = 0` for 14 consecutive days.

### “Done” definition

This initiative is done when:

1. Protocol v1 envelope and lint gate are enforced for all actionable assignments.
2. Task lifecycle transitions are authoritative and reflected consistently in UI/messages.
3. Automated guardrails are active for ack anti-patterns, non-execution stalls, and lead role drift.
4. Metrics meet thresholds for at least two consecutive weekly windows.
5. Team can onboard a new lead/agent and maintain throughput without undocumented tribal rules.

## Key proposal summary

- Shift from “message style guidance” to a typed orchestration protocol contract.
- Make task state authoritative and message flow derivative.
- Convert known failure modes into deterministic lint and guardrail checks.
- Separate responsibilities cleanly across infrastructure, project conventions, and model adapters.
- Measure protocol quality directly; treat orchestration quality as a first-class product surface.
