# Cross-Project Synthesis: Role System Lessons From ECC And BMAD

Date: 2026-03-21
Owner: architect-1
Task: `#1417`

## Scope

Synthesized four research inputs:

- [everything-claude-code-role-research-2026-03-21.md](/home/user/projects/taurhaus/docs/analysis/everything-claude-code-role-research-2026-03-21.md)
- [bmad-method-dev-perspective-2026-03-21.md](/home/user/projects/taurhaus/docs/analysis/bmad-method-dev-perspective-2026-03-21.md)
- design-lead message findings on ECC: graduated severity, role variants, confidence thresholds
- product-check-1 message findings on BMAD: communication style, quality gates, phase structure, shared skills

Goal: turn both projects into concrete guidance for improving the taurhaus role system, not just summarize what they do.

## Executive summary

The two external projects are complementary:

- ECC is strongest at reusable role packaging, explicit role contracts, runtime subagent profiles, and coordination-specialist roles.
- BMAD is strongest at artifact discipline, readiness gates, story-rich handoff context, and checklist-driven workflow control.

Taurhaus is already stronger than both at live runtime orchestration and assignment-time steering, but it is weaker in three places:

1. reusable role archetypes
2. explicit mode and workflow contracts
3. standardized pre-execution artifacts and readiness gates

The best direction is a hybrid:

- keep taurhaus runtime mesh and structured steering
- add ECC-style archetypes and mode overlays
- add BMAD-style constitutions, story packets, and readiness checklists

## Comparison table

| Aspect | ECC | BMAD | Taurhaus today | Recommendation |
|---|---|---|---|---|
| Core model | Role-packaging and harness system | Document-first workflow methodology | Live runtime orchestration with structured task steering | Keep taurhaus runtime model; borrow packaging from ECC and workflow discipline from BMAD |
| Role definition | Markdown role cards with frontmatter plus mission/workflow/checks/escalation | Skill personas with identity, principles, critical actions, and workflow steps | Strong assignment-time steering, weaker reusable archetype catalog | Add first-class reusable role archetypes with explicit contracts |
| Task mode | Separate `contexts/*.md` overlays like research/review | Phase and workflow stages imply mode | `focusArea` partly covers mode, but not explicitly | Split role identity from operating mode |
| Coordination roles | Explicit orchestration roles like `loop-operator`, `chief-of-staff` | Mostly phase roles and facilitator-style party mode | Lead/member runtime exists, but role catalog is lighter | Add coordination-specialist archetypes |
| Runtime execution profiles | `.codex/agents/*.toml` for sandbox, reasoning, evidence expectations | Less runtime-oriented | Assignment steering exists, but no reusable execution-profile layer | Add subagent runtime profiles |
| Handoff quality | Good reusable role instructions, less artifact-heavy | Excellent artifact pipeline and story packets | Task context is often conversational plus ad hoc docs | Add standardized handoff packets |
| Readiness gates | Some role checks, lighter phase gating | Strong implementation-readiness and validation gates | Limited explicit readiness structure before execution | Add readiness gates before execution begins |
| Review culture | Specialist reviewer roles and verification agents | Adversarial review and definition-of-done checklists | Review exists, but artifact/checklist support is thinner | Add review modes and checklists as first-class role outputs |
| Learning/evolution | Project-scoped instinct learning and evolution into skills/agents | Method evolves through templates and structured artifacts | Role updates are manual | Add telemetry-backed role-template evolution later |
| Best thing to steal | Layered role system | Constitution + story/checklist discipline | Runtime mesh | Combine all three, but do not replace mesh |

## What we should change

### 1. Add a three-layer role model

This is the clearest synthesis from both projects.

Role execution in taurhaus should have three explicit layers:

1. `role_archetype`
   - stable reusable specialist package
2. `mode_overlay`
   - research, implementation, review, incident, coordination
3. `assignment_steering`
   - `focusArea`, `contextSummary`, `behaviorBoundaries`, task-specific constraints

Why:

- ECC proves reusable archetypes and mode overlays are valuable.
- Our current system already does assignment steering well.
- This structure preserves what we already do best while making roles reusable and composable.

### 2. Upgrade role definitions from persona cards to operational contracts

Current taurhaus role definitions should become more explicit about execution behavior.

Recommended role schema additions:

- `core_workflow`
- `required_checks`
- `stop_conditions`
- `escalation_triggers`
- `required_artifacts`
- `handoff_expectations`
- `verification_expectations`

Why:

- ECC shows the value of role-specific contracts.
- BMAD shows the value of hard workflow constraints.

This would make our roles less vibe-based and more operational.

## Concrete new role field recommendations

The role schema should grow in a way that separates identity, behavior, workflow, and execution strictness.

### Core identity fields

- `name`
- `purpose`
- `summary`
- `communication_style`
- `default_tools`
- `default_model_profile`

`communication_style` is worth adding explicitly. BMAD treats style as part of persona clarity, and that is useful when we want a reviewer, coordinator, or incident lead to behave consistently without repeating tone guidance in every assignment.

### Workflow and contract fields

- `core_workflow`
- `required_checks`
- `quality_gates`
- `required_artifacts`
- `definition_of_done`
- `handoff_expectations`
- `verification_expectations`
- `phase_scope`

`quality_gates` and `phase_scope` are the main BMAD-inspired additions. They let roles express where they belong in a delivery flow and what must be true before they can claim completion.

### Boundary and escalation fields

- `behavior_boundaries`
- `boundary_severity`
- `stop_conditions`
- `escalation_triggers`
- `escalation_severity_levels`
- `must_verify`
- `must_not_edit`
- `must_read_first`

This is where the design-lead's graduated-severity idea is most useful. Not every boundary should be binary. Some constraints should be:

- `hard`
- `strong`
- `advisory`

That makes role behavior stricter without forcing everything into absolute prohibitions.

### Confidence and autonomy fields

- `confidence_thresholds`
- `autonomy_level`
- `ask_vs_act_policy`
- `fallback_behavior`

`confidence_thresholds` are especially valuable for research, review, and incident roles. They allow the role to know when to proceed, when to verify, and when to escalate.

### Composition fields

- `inherits_from`
- `role_variant_of`
- `shared_skills`
- `compatible_modes`
- `preferred_handoffs`

These fields turn the role system from a flat list into a composable graph. ECC points toward reusable packaging; BMAD points toward phase specialization. Variants and inheritance are how we capture both cleanly.

### 3. Add BMAD-style pre-execution artifacts

The biggest gap neither our runtime mesh nor ECC fills well is standardized implementation context.

We should add:

- a mesh/project constitution artifact
- a richer task/story packet
- a definition-of-done checklist
- an implementation-readiness checklist

Suggested taurhaus equivalents:

- `mesh-context.md` or project constitution
- task execution brief generated at assignment/start time
- review checklist attached to completion/review states
- readiness gate before a team or worker enters implementation mode

Why:

- BMAD is strongest exactly here.
- This directly improves worker alignment without weakening autonomy.

### 4. Add reusable execution profiles for subagents

Profiles should be independent from both persona and current task.

Suggested profile dimensions:

- sandbox level
- web verification requirement
- expected evidence level
- reasoning depth
- allowed mutation surface
- test/verification strictness

Why:

- ECC’s `.codex/agents/*.toml` is a strong model.
- Today, too much of this lives implicitly in the role prompt or task wording.

### 5. Add explicit coordination and review archetypes

Both repos suggest that not every valuable role is a coder.

New role ideas worth adding:

- `coordinator`
  - owns sequencing, dependency tracking, lane health, and handoffs
- `incident-operator`
  - handles degraded states, recovery sequencing, and operator communications
- `verification-reviewer`
  - skeptical correctness/test/regression review with adversarial bias
- `docs-verifier`
  - primary-source confirmation for API/framework/process claims
- `release-shepherd`
  - release readiness, gate tracking, changelog/release-note cohesion
- `quick-dev`
  - compressed low-ceremony execution role with mandatory final review

Why:

- ECC contributes coordination and verification specialist ideas.
- BMAD contributes the quick-flow and adversarial-review patterns.

## Improvements to existing taurhaus role definitions

### `behaviorBoundaries` should become more structured

Right now the field is useful but underspecified. It should support machine-readable categories like:

- `must_verify`
- `must_not_edit`
- `must_read_first`
- `required_checklists`
- `stop_if`
- `handoff_to`
- `artifact_outputs`

That would make steering easier to enforce and easier to compare across roles.

### `focusArea` should narrow subject matter, not carry workflow mode

We should keep `focusArea`, but it should answer:

- what the agent is concentrating on

It should not have to also answer:

- whether the agent is researching, implementing, reviewing, or coordinating

That belongs in `mode_overlay`.

### `contextSummary` should have standardized sections

BMAD’s artifact discipline suggests `contextSummary` should be templated more often.

Recommended sections:

- objective
- current state
- constraints
- known risks
- relevant files/systems
- expected output
- verification expectations

This would produce more reliable starts and cleaner resumability.

## New role ideas

The most promising additions inspired by both projects are:

- `adversarial-reviewer`
  - assumes defects exist and must produce evidence-backed findings or explain why none were found
- `implementation-readiness-checker`
  - validates that architecture, task packet, constraints, dependencies, and verification plan are ready before coding starts
- `docs-verifier`
  - confirms framework/API/process claims against primary sources
- `incident-operator`
  - manages degraded-state recovery, sequencing, operator notes, and rollback posture
- `coordinator`
  - owns team sequencing, dependency health, handoffs, and phase movement
- `quick-dev`
  - optimized low-ceremony implementer for small tasks, but still bound to final review and DoD checks
- `release-shepherd`
  - tracks release readiness, gate completion, and cross-artifact consistency
- `story-packager`
  - prepares rich execution briefs from plans, architecture, and prior work before implementation begins

These are not all equal priority. The highest-value early additions are `implementation-readiness-checker`, `adversarial-reviewer`, `coordinator`, and `docs-verifier`.

## Role editor improvements

### Add variants and inheritance

The editor should support:

- base archetypes
- derived variants
- local overrides

Examples:

- `reviewer` -> `adversarial-reviewer`
- `implementer` -> `quick-dev`
- `coordinator` -> `incident-operator`

This lets us avoid duplicating whole role definitions just to change strictness, style, or workflow emphasis.

### Add graduated boundaries

Boundary editing should support severity levels instead of only plain text constraints.

Recommended levels:

- `hard`
- `default`
- `advisory`

Examples:

- `hard`: must not edit files outside assigned scope
- `default`: verify external claims against primary docs
- `advisory`: prefer parallel helper roles when available

### Add confidence controls

The editor should allow per-role confidence behavior such as:

- minimum confidence to answer without verification
- minimum confidence to continue without escalation
- stricter confidence floor for high-risk domains

This would make research and review roles noticeably better.

### Add shared-skill attachment

BMAD’s shared-skill idea is useful if we translate it carefully.

Roles should be able to reference reusable skill blocks such as:

- `primary_source_verification`
- `definition_of_done_check`
- `readiness_gate`
- `regression_review`

That keeps the archetypes smaller while preserving consistency.

## Structural changes to role management

### Add a role catalog and a mode catalog

Instead of only assigning ad hoc role text, taurhaus should maintain:

- a reusable role catalog
- a reusable mode catalog
- a library of execution profiles

Assignments then compose those pieces.

### Add role bundles for common team shapes

ECC implies bundles; BMAD implies phased chains. We should make bundles explicit.

Useful starter bundles:

- implementation bundle
  - planner
  - implementer
  - reviewer
- research bundle
  - explorer
  - docs verifier
  - synthesizer
- incident bundle
  - incident operator
  - diagnostician
  - comms lead
- quality bundle
  - implementer
  - adversarial reviewer
  - regression verifier

### Add artifact generation to assignment/start flows

BMAD’s best idea is not its personas; it is the artifact pipeline around them.

When a task starts, taurhaus should be able to generate:

- execution brief
- constraints checklist
- verification plan
- handoff expectations

This should be part of the role system, not separate from it.

## Workflow improvements

The research points to four workflow upgrades.

### 1. Add phase awareness

Roles should know whether they are being used in:

- discovery
- planning
- implementation
- review
- incident response
- release

This is BMAD’s phase discipline translated into a taurhaus-compatible model.

### 2. Add readiness gates

Before implementation starts, taurhaus should be able to answer:

- is the task packet complete
- are dependencies resolved
- are constraints clear
- is the verification plan defined
- is the blast radius acceptable

If not, the task should bounce to a preparatory role instead of letting the worker improvise.

### 3. Add definition-of-done and review checklists

Completion should not be only “tests passed” or “task done.”

We should support role-linked checklists for:

- correctness
- regression risk
- docs updates
- verification evidence
- handoff completeness

### 4. Add richer shared context artifacts

We need a stronger equivalent of BMAD’s story packet and constitution model:

- mesh or project constitution
- execution brief
- known-risk summary
- verification plan
- handoff summary

This would reduce repeated briefing work and make resume/recovery cleaner.

## What not to adopt

We should avoid three tempting but wrong copies.

1. Do not replace live mesh orchestration with BMAD-style facilitator or party-mode conversation.
   Our advantage is real runtime execution, ownership, and recovery.
2. Do not flatten taurhaus into only markdown role cards.
   ECC’s packaging is useful, but our structured steering is one of our strengths.
3. Do not make the role system fully static.
   Both projects show value in reusable structure, but taurhaus still needs dynamic assignment-time control.
4. Do not force every role boundary to be absolute.
   Graduated severity is better than turning the schema into a wall of binary prohibitions.
5. Do not promote project-local conventions to global defaults too early.
   ECC’s project-scoped learning idea is a good warning here.

### Add role-template evolution later, backed by completed-task evidence

This is a later step, but ECC’s instinct idea is worth adapting.

Longer term, taurhaus should mine:

- repeated steering patterns
- repeated review findings
- repeated verification needs

Then suggest updates to:

- role archetypes
- mode overlays
- execution profiles
- checklist templates

The key BMAD/ECC lesson here is scope:

- keep project-specific patterns project-scoped
- only promote repeated cross-project patterns to global defaults

## Concrete recommendations

### We should add

1. A reusable role-archetype catalog with explicit operational fields.
2. A separate mode-overlay system for research, implementation, review, incident, and coordination.
3. A reusable execution-profile layer for subagents.
4. A project or mesh constitution artifact loaded by implementation-oriented roles.
5. A standardized task execution brief and definition-of-done checklist.
6. An implementation-readiness gate before serious execution begins.
7. An adversarial review mode for review-oriented roles.
8. Explicit coordination-specialist roles, not just coding roles.
9. Role variants/inheritance in the role editor.
10. Confidence thresholds and graduated boundary severity in the role schema.

### We should change

1. `behaviorBoundaries` from freeform guidance into a richer operational contract.
2. `focusArea` so it describes subject focus only, not role mode.
3. `contextSummary` so it follows a more consistent artifact shape.
4. Role assignment so it composes archetype + mode + assignment steering instead of relying so heavily on one blended instruction payload.

### We should not change

1. We should not replace runtime mesh with BMAD-style facilitator workflows.
2. We should not replace structured steering with ECC-style markdown roles alone.
3. We should not import either project verbatim.

The right move is synthesis, not imitation.

## Prioritized action items

### First

- define the three-layer model: archetype, mode overlay, assignment steering
- enrich the role schema with `communication_style`, `quality_gates`, `confidence_thresholds`, `phase_scope`, and graduated boundary severity
- add role variants/inheritance to the editor

### Second

- add constitution and task execution brief artifacts
- add readiness-gate and definition-of-done checklist support
- add `implementation-readiness-checker`, `adversarial-reviewer`, and `docs-verifier`

### Third

- add execution profiles and role bundles
- add phase-aware workflow templates
- add coordination-specialist roles like `coordinator` and `incident-operator`

### Later

- add telemetry-backed role evolution
- promote repeated project-local patterns into global defaults only when evidence supports it

## Recommendation

Treat ECC as the source of role architecture patterns.
Treat BMAD as the source of workflow artifact discipline.
Treat taurhaus mesh as the execution substrate.

If we combine those three cleanly, we get a role system that is:

- more reusable than today
- more disciplined before execution starts
- easier to compose into teams
- easier to verify
- better positioned to evolve from observed practice
