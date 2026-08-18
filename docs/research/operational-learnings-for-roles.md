# Operational Learnings For Role Design

Date: `2026-03-08`

Purpose:
- consolidate operational lessons from real Taurhaus multi-agent work
- provide raw material for role-definition drafting
- separate what is directly observed from what is inferred for future roles

Sources reviewed:
- [CLAUDE.md](/home/user/projects/taurhaus/CLAUDE.md)
- [retro-2026-03-08-survey-findings.md](/home/user/projects/taurhaus/docs/retro/retro-2026-03-08-survey-findings.md)
- [retro-2026-03-08-decisions.md](/home/user/projects/taurhaus/docs/retro/retro-2026-03-08-decisions.md)
- [retro-quality-sprint-2026-03-05.md](/home/user/projects/taurhaus/docs/archive/retro-quality-sprint-2026-03-05.md)
- [visual-testing-pipeline-lessons.md](/home/user/projects/taurhaus/docs/archive/retros/visual-testing-pipeline-lessons.md)
- [layout-engine-pipeline-retro.md](/home/user/projects/taurhaus/docs/archive/retros/layout-engine-pipeline-retro.md)
- [ai-agent-characteristics.md](/home/user/projects/taurhaus/docs/archive/ai-agent-characteristics.md)
- [design-workflow.md](/home/user/projects/taurhaus/docs/archive/design-workflow.md)
- [agent-role-visibility.md](/home/user/projects/taurhaus/docs/archive/design/agent-role-visibility.md)
- [role-context-steering-review.md](/home/user/projects/taurhaus/docs/archive/design/role-context-steering-review.md)
- [ai-friendliness-audit-codex.md](/home/user/projects/taurhaus/docs/archive/audits/ai-friendliness-audit-codex.md)
- [ai-friendliness-audit-claude.md](/home/user/projects/taurhaus/docs/archive/audits/ai-friendliness-audit-claude.md)

Notable doc-state finding:
- `MEMORY.md` was referenced in the assignment but is not present in this workspace
- several design docs referenced by current instructions now live under `docs/archive/`
- that documentation drift is itself an operational learning: role definitions should avoid depending on unstable doc paths

## Cross-Cutting Learnings

These apply to nearly every role.

### What consistently works

- direct action-oriented task assignments
- exact deliverable paths
- explicit completion signals
- scoped validation expectations
- end-to-end execution without conversational overhead
- concrete completion summaries tied to changed files and validation results

### What consistently fails

- repeated stale idle-monitor nudges
- assignment messages split across multiple micro-messages
- vague “check messages” or acknowledgment-seeking pings
- ambiguous execution mode (`audit` vs `recommend` vs `implement`)
- ownership ambiguity in shared files

### Cross-role guardrails already validated by practice

- every assignment needs explicit execution mode
- every assignment needs a file ownership boundary
- validation depth must be stated, not implied
- local low-risk override fixes are allowed only when necessary to unblock validation and must be reported
- roles should escalate quickly when a blocker exceeds that threshold

## Role: Team Lead (Claude)

Status:
- directly observed in real team operation

## What works

- system-level synthesis and decomposition into concrete tasks
- adapting communication style to different agents
- coordinating multi-step pipelines when the contract is clear up front
- turning retros and audits into actionable process changes
- keeping assignments operational rather than conversational

Observed successful behaviors:
- lead with imperative instructions
- include deliverable path and completion signal
- bundle action, scope, and validation expectations in one message
- stay available for communication rather than disappearing into implementation

## What does not work

- role drift into hands-on implementation
- deep code investigation by the lead when a developer could do it faster
- over-detailed, context-heavy opening messages that do not start with an action
- process changes announced piecemeal during active work

Observed recurring failure mode:
- after compaction or under pressure, Claude team-lead tends to start coding/investigating instead of orchestrating

## Friction points

- communication bottleneck when the lead is busy doing implementation work
- heartbeat/ping style messaging creates noise and stalls
- assignment clarity drops if the lead sends background before the actual concrete first action
- if ownership exceptions are not called out in the assignment, agents stall waiting for clarification

## Behavioral guardrails

- orchestrate, do not implement, unless there is an explicit exception
- keep assignment messages action-first and compact
- never send pure acknowledgment messages to active assignees
- state execution mode, ownership boundary, adjacent-fix policy, validation expectation, and completion signal
- batch process changes into deliberate broadcasts rather than drip-feeding them midstream
- intervene on blockers fast; do not let agents sit silently under ambiguity

## Role-definition implications

The Claude lead role should optimize for:
- task routing
- unblock velocity
- pipeline design
- communication discipline
- anti-role-drift guardrails

## Role: Team Lead (Codex)

Status:
- partly inferred future role
- based on observed Codex behavior in developer/architect lanes plus multi-CLI lead exploration

## What would likely work

- structured execution against a clearly defined orchestration protocol
- operationally precise task routing if the rules are explicit
- reliable follow-through on a concrete lead workflow with templates
- disciplined reporting if completion format is fixed

Reason for confidence:
- Codex is strong when the operating contract is explicit and procedural
- many lead duties can be reduced to protocol-following if the role definition is concrete enough

## What would likely not work

- open-ended “manage the team however you think best” phrasing
- long, meta, or highly conversational tasking
- reliance on soft social cues instead of explicit protocol
- ambiguous balancing between orchestration and hands-on work

## Friction points

- Codex is sensitive to message protocol and can stop on acknowledgment-style interactions
- if multiple constraints are stacked without priority ordering, it may optimize for rule compliance over actual orchestration outcome
- local repo policy files strongly influence behavior, so any lead role definition must be unambiguous about precedence

## Behavioral guardrails

- define the lead workflow as an explicit protocol, not a loose leadership style
- forbid pure acknowledgment exchanges
- make decision ownership explicit
- require the lead to stay in routing/unblocking/reporting mode unless a task explicitly authorizes implementation
- require compact, imperative assignment style
- require a single-source-of-truth task system reference instead of duplicating task state in chat

## Role-definition implications

A Codex lead role is viable only if it is:
- highly procedural
- message-disciplined
- guarded against acknowledgment stalls and execution drift

This role should not be drafted as “Claude lead, but on Codex.” It needs a more explicit protocol surface.

## Role: Developer

Status:
- directly observed through developer1/developer2/developer3 retros and pipeline behavior

## What works

- focused, scoped implementation tasks
- TDD or regression-test-first work when the failure mode is clear
- explicit in-scope/out-of-scope boundaries
- concrete quality gate expectations, especially `just check-quick`
- end-to-end closure on one slice rather than fragmented substeps

Observed successful patterns:
- developers perform well when asked to read a specific file, trace a path, write a failing test, then implement
- runtime/integration verification improves outcomes when the task touches Tauri, daemon, or UI wiring

## What does not work

- broad ambiguous “fix this area” prompts
- tasks that begin as verification-only but secretly expect net-new implementation
- shared hotspot files without ownership clarity
- full-gate contention in shared worktrees
- repeated idle-monitor nudges during active work

## Friction points

- worktree overlap, especially on controllers and tests
- unclear rule for how to handle unrelated red tests or nearby compile breaks
- ambiguity about whether to stop or proceed when blocked by another agent’s file
- long-running global checks causing false-red noise

## Behavioral guardrails

- keep scope tight and concrete
- prefer regression-first or targeted failing tests when fixing behavior
- validate to the level declared in the assignment footer
- use the ownership override rule only for local, low-risk, non-design-changing unblockers
- report exact blockers fast when outside that threshold
- close the loop with a concrete completion summary, not “done”

## Role-definition implications

Developer roles should encode:
- execution discipline
- scoped autonomy
- strong validation habits
- explicit escalation behavior
- low tolerance for ambiguous scope creep

## Role: Architect

Status:
- directly observed in current team operation

## What works

- cross-cutting diagnosis across frontend, backend, runtime metadata, tmux, and mesh/daemon layers
- audit and review work tied to concrete code paths
- architecture notes that become implementation guardrails
- targeted fixes in coordination-heavy or system-boundary areas
- turning ambiguous runtime symptoms into precise root-cause reports

Observed successful patterns:
- architecture work is most valuable when it is close to real system behavior, not abstract theory
- this role performs well on investigations, audits, design reviews, and narrow fixes where multiple layers interact

## What does not work

- purely abstract design tasks disconnected from code reality
- ambiguous boundary with developer-owned files during blocked validation
- tasks framed as “design/review only” that later silently expand into implementation
- prolonged waiting on ownership clarifications for trivial unblockers

## Friction points

- runtime/daemon/pidfile drift generates extra operational debugging work
- ownership boundaries blur when validation is blocked by adjacent files
- cross-cutting investigations often reveal bugs outside the nominal task boundary
- doc drift makes “current source of truth” harder to identify quickly

## Behavioral guardrails

- stay concrete: findings must point to actual files, code paths, or runtime observations
- escalate ownership ambiguity fast
- use override repairs only within the narrow agreed threshold
- distinguish clearly between recommendation, review finding, and implemented fix
- prefer artifacts that can feed the next phase directly: audit docs, design notes, regression coverage, precise task follow-ups

## Role-definition implications

The architect role should be defined as:
- system investigator
- architecture reviewer
- boundary fixer for cross-layer issues

It should not collapse into a generic senior developer role. The distinguishing value is cross-cutting diagnosis plus disciplined escalation.

## Role: Designer / UX Specialist

Status:
- directly observed through Gemini UI-specialist workflow and design-process retros

## What works

- design ownership rather than implementation-only orders
- design-first loop: brief -> proposal -> approval -> implement -> review
- functional requirements plus creative freedom
- explicit request for wireframes, token values, and both-theme design treatment
- visual review against a high bar after implementation

Observed successful patterns:
- Gemini performs best when allowed to lead visual design instead of receiving pixel-level coding instructions
- design-only phases can run in parallel with engineering work
- implementation after design approval yields better UI results than “just build it”

## What does not work

- over-specified briefs that reduce the specialist to a code monkey
- vague design proposals with adjectives instead of actual values
- dark-mode-only thinking
- visual work that changes behavior, tests, or API shape

## Friction points

- initial proposals can be too vague without explicit demand for concrete values
- implementation quality is strong but still needs review for theme parity and token discipline
- design work can conflict with E2E/test work if implementation starts before stabilization

## Behavioral guardrails

- keep design and implementation phases separate
- never change behavior, API shape, or test IDs during visual work
- require dark and light mode in the proposal
- require concrete tokens, spacing, and interaction specs
- use review scoring to reject merely “good enough” UI

## Role-definition implications

The designer/UX role should be framed as:
- design lead
- not just frontend implementer
- responsible for visual systems, states, grouping, and interaction quality

This role needs guardrails that preserve functional boundaries while granting aesthetic latitude.

## Role: Specialist

Status:
- mixed observed/inferred category
- grounded in existing specialist patterns like reviewer, researcher, UI specialist, mesh/domain-focused roles

## What works

- very narrow context lane
- clear statement of what domain context the specialist accumulates
- strong behavioral boundaries around when to escalate
- focused artifacts: review findings, research notes, domain audits, or targeted recommendations

Observed evidence:
- role-context work now centers on focus area, context summary, and behavior summary rather than capability tags
- runtime role visibility is most useful when it explains context lane and boundaries, not theoretical abilities

## What does not work

- capability-tag framing without a real context lane
- broad specialist titles with no durable domain identity
- roles that duplicate a general developer lane but with different wording

## Friction points

- if a specialist role is defined as a bag of capabilities, the team cannot tell what context it is meant to preserve
- hidden or weak runtime visibility makes specialist identity less useful during live team operation
- poor setup/assignment UX reduces role adoption even when the role concept is strong

## Behavioral guardrails

- define specialist roles around context steering, not capability labels
- require short focus area, context summary, and behavior boundary
- make escalation boundaries explicit
- keep outputs narrow and domain-linked
- avoid role designs that pretend the model has exclusive abilities

## Role-definition implications

Specialist roles should be drafted as:
- context lanes
- not skill taxonomies
- with runtime-visible summaries that help the lead route work and diagnose behavior

## Operational Design Constraints For All Roles

These should shape every new role definition.

## 1. Message protocol matters as much as task content

Especially for Codex-family roles:
- lead with the first action
- avoid acknowledgment traps
- keep operational messages compact and complete

## 2. Validation expectations must be explicit

Role behavior degrades when “done” is ambiguous.

Every role should know whether a task expects:
- report-only
- targeted tests
- `check-quick`
- runtime smoke

## 3. Ownership discipline needs a narrow escape hatch

The team now has a validated pattern:
- respect boundaries by default
- allow small local validation unblockers
- escalate anything design-changing or judgment-heavy

This should be encoded in all non-lead roles.

## 4. Role identity should preserve context, not advertise ability

This is the most important conceptual shift from the recent role-system work.

Roles are valuable because they:
- steer future task routing
- preserve domain memory
- survive compaction/handoffs
- communicate expected behavior to the lead

They are not valuable because they claim exclusive ability.

## 5. Documentation drift is itself an operational risk

Observed in this task:
- `MEMORY.md` missing
- several “current” docs only available in `docs/archive/`
- some path references in live docs still point to moved files

Implication for role design:
- roles should not depend on fragile path-specific memory
- role instructions should prefer stable behavioral rules over brittle file-location assumptions

## Recommended Inputs To The Next Drafting Phase

When drafting the actual role definitions, prioritize:

1. team-lead-claude:
   - anti-implementation drift
   - action-first messaging
   - explicit task protocol ownership

2. team-lead-codex:
   - strict orchestration protocol
   - anti-ack stall rules
   - explicit decision/assignment ownership

3. developer:
   - scoped execution
   - regression/validation discipline
   - fast escalation outside override scope

4. architect:
   - cross-layer diagnosis
   - boundary-aware implementation
   - concrete artifact production

5. designer/UX:
   - design leadership with implementation boundaries
   - concrete proposal discipline
   - theme parity and review bar

6. specialists:
   - context-steering semantics
   - narrow lane definitions
   - explicit runtime-readable summaries
