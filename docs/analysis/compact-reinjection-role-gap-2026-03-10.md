# Compact Reinjection vs Role Contract Fidelity

## Summary

Current compaction reinjection is intentionally narrow. It restores task,
ownership, and working-set context, but it does not faithfully restore the
active role contract.

That is not a rendering accident. It is the current architecture:

- role templates carry rich instructions and behavioral rules
- initial onboarding/role-context delivery includes much more of that contract
- compact reinjection composes from a deliberately reduced subset

The result is that post-compaction behavior can collapse toward a generic
"continue the task" mode even when the active role template encodes stricter
workflow, escalation, or anti-pattern rules.

The right fix is not to paste full template YAML into compaction cards. The
right fix is to add a bounded runtime compact summary layer for roles and merge
that with the existing operational task/footer/working-set context.

## 1. Current State

### Role template contract today

Role templates already carry the information needed for richer reinjection.
The active schema in
[types.rs](/home/user/projects/taurhaus/src-tauri/src/templates/types.rs)
includes:

- `instructions`
- `focus_area`
- `context_summary`
- `behavior_summary`
- `behavioral_contract`
- `capabilities`

Those fields propagate into managed members through the coordination domain in
[domain.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/domain.rs)
and the roster join in
[roster.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/roster.rs).

### What agents get at startup

Initial role delivery is materially richer than compaction reinjection.

The renderer in
[delivery.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/delivery.rs)
includes:

- role id
- full instructions
- behavioral contract sections
- capabilities

for both:

- non-Claude onboarding
- Claude role-context delivery

So the system already has a concept of a richer role contract at runtime.

### What operational snapshots keep

The operational snapshot model in
[operational.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/stores/operational.rs)
stores only:

- task id / subject / status
- assignment footer
- ownership override state
- working set

It does not persist:

- instructions
- context summary
- behavior summary
- behavioral contract
- capabilities

That is not necessarily wrong. Those fields are role-owned and remain available
through the resolved `Member`. But it means compaction fidelity depends on what
the reinjection composer chooses to pull from `Member`, not from the snapshot.

### What compaction reinjection actually carries

The compaction paths are:

- Codex:
  [compaction_processor.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/compaction_processor.rs)
- Claude:
  [claude_hooks.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/claude_hooks.rs)

Both paths call the same composer in
[reinjection.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/reinjection.rs):

- `CompactionReinjectionService::compose(...)`

That composer currently retains only:

- role:
  - `role_id`
  - `role_name`
  - `focus_area`
  - `behavior_summary`
- task:
  - `id`
  - `subject`
  - `execution_mode`
  - `validation_expectation`
- boundaries:
  - `file_ownership_boundary`
  - `adjacent_fix_policy`
  - `override_allowed`
  - `active_override_reason`
- working set:
  - `project_path`
  - `focal_files`

It drops or ignores:

- `instructions`
- `context_summary`
- `behavioral_contract`
- `capabilities`
- assignment `response_expectation`

The `response_expectation` omission matters because it is already present in the
operational snapshot, but compaction rendering does not surface it.

### Codex and Claude share the same narrow semantics

This is not a Codex-only issue.

The Codex inbox text and the Claude `additionalContext` JSON are both rendered
from the same reduced `OperationalReinjectionCard`. The only difference is the
presentation format.

## 2. Gap Analysis Against Representative Role Templates

I compared the current reinjection contract against these current Taurhaus role
templates:

- [taurhaus-lead-codex.yaml](/home/user/projects/taurhaus/src-tauri/resources/templates/roles/taurhaus-lead-codex.yaml)
- [taurhaus-developer.yaml](/home/user/projects/taurhaus/src-tauri/resources/templates/roles/taurhaus-developer.yaml)
- [taurhaus-architect.yaml](/home/user/projects/taurhaus/src-tauri/resources/templates/roles/taurhaus-architect.yaml)

### What survives today

All three templates currently survive compaction only through:

- role identity
- one focus sentence
- one behavior sentence
- the generic operational task/footer/working-set fields

That means the templates do still produce different compaction cards, but only
weakly. The differences are mostly cosmetic framing, not contract-level
behavior.

### What is lost

Across the three representative templates, the following important contract
content is lost:

- full instructions block
- context summary
- communication rules
- execution rules
- escalation rules
- anti-pattern guidance
- workflow sequencing encoded in instructions
- capabilities
- explicit response-expectation reminders from the assignment footer

### Template-specific impact

#### Taurhaus Team Lead (Codex)

Lost behavior includes:

- imperative-first assignment style
- no acknowledgment-only traffic
- do not drift into implementation
- task-graph-as-source-of-truth behavior
- explicit footer standard enforcement
- idle/escalation protocol

What remains in reinjection is mostly:

- procedural orchestration focus
- a short behavior summary
- current task metadata

That is not enough to preserve the lead's routing and anti-drift contract.

#### Taurhaus Developer

Lost behavior includes:

- regression-first rule
- exact validation-lane discipline
- narrow override threshold
- explicit escalation triggers for overlap/architecture ambiguity
- Tauri runtime verification caveat for plugin/capability paths

What remains in reinjection is mostly:

- scoped implementation framing
- one behavior summary sentence
- current task/footer context

That is not enough to preserve the detailed execution and escalation rules.

#### Taurhaus Architect

Lost behavior includes:

- review-vs-recommend-vs-implement distinction
- cross-layer tracing expectations
- explicit escalation rules for ownership ambiguity
- anti-drift guidance against generic implementation work

What remains in reinjection is mostly:

- cross-layer diagnosis framing
- one behavior summary sentence
- current task/footer context

That is not enough to preserve the role's boundary discipline.

### Quantified gap

Using the current three representative Taurhaus role templates:

| Role | Words retained now | Words available in role-level summary inputs |
| --- | ---: | ---: |
| `taurhaus-lead-codex` | 29 | 435 |
| `taurhaus-developer` | 30 | 444 |
| `taurhaus-architect` | 29 | 392 |

The retained portion is about 7-8% of the available role-level contract text.

That number should not be read as "we should send all 400+ words." It does show
that the current compaction card is far below the threshold where a reasonable
user could assume the role contract is still faithfully present.

## 3. Context-Cost Tradeoff of Richer Reinjection

There are three realistic choices.

### A. Status quo

Pros:

- cheap
- low context cost
- operationally simple

Cons:

- too lossy
- different role templates do not remain meaningfully distinct
- users cannot reasonably assume the active role contract survived compaction

### B. Full raw role contract in reinjection

Pros:

- highest fidelity
- lowest inference risk

Cons:

- too expensive in context
- duplicates onboarding payloads
- high risk of burying the actual task/working-set context

This is not the right target.

### C. Bounded richer role summary

Pros:

- preserves the parts of the role contract that actually steer behavior
- keeps compaction payloads short enough to remain useful
- makes two different roles visibly and operationally different after compaction

Cons:

- requires schema and rendering work
- introduces a new summary-maintenance surface

This is the right tradeoff.

### Practical budget

The current role contribution to reinjection is about 29-30 words.

A useful bounded richer summary can likely stay in the rough range of:

- 90-160 words for role-level compact guidance

That is materially larger than today, but still far smaller than replaying the
full instructions plus full behavioral contract.

So the context-cost argument against improvement is real but not decisive. The
correct question is not "full contract or nothing." The correct question is
"what bounded role summary preserves behavior without burying the current task."

## 4. `runtime_compact_summary` Evaluation and Recommended Approach

The 2ksim reference document proposes a `runtime_compact_summary` direction.
That direction is correct, but it should not be implemented as a single opaque
free-text blob.

### Why a single free-text field is not enough

Pros:

- simple to add
- easy for template authors to understand
- can capture ordered workflow and anti-patterns

Cons:

- high duplication with existing role fields
- easy to let drift from the real role contract
- hard to validate structurally
- hard to render differently for Codex vs Claude without parsing prose

### Why pure deterministic generation is also insufficient

Pros:

- no duplicated authoring surface
- easier to keep aligned with the template schema

Cons:

- current fields are not rich enough to deterministically recreate ordered
  workflow or priority
- important distinctions are often in `instructions`, which is free text
- heuristics for extracting "must keep doing" versus "must avoid" will be noisy

### Recommended approach

Add a structured compact-summary layer to role templates and persist it with the
member configuration.

Use the concept name `runtime_compact_summary`, but model it as structured data,
not a single prose blob.

Recommended shape:

- `role_purpose`
- `keep_doing`
- `workflow_sequence`
- `avoid`
- `escalate_when`

Bound each section:

- short sentence for `role_purpose`
- small bullet limits for the lists
- validation on total size

Then compaction reinjection should carry:

- existing operational context:
  - task
  - execution mode
  - validation expectation
  - response expectation
  - ownership boundary
  - working set
- plus the bounded role compact summary

### Why this is the right compromise

- It preserves meaningful role differences.
- It keeps compaction output bounded.
- It is maintainable and testable.
- It avoids trying to regenerate precise workflow semantics from arbitrary prose
  at runtime.

## 5. Pros, Cons, Risks, and Concrete Implementation Steps

### Pros of fixing this

- Post-compaction behavior remains role-shaped instead of generic.
- Different roles produce meaningfully different reinjections.
- Less dependence on repo-local process docs to restate framework behavior.
- Better debuggability when behavior diverges from the intended role contract.

### Cons of fixing this

- Adds schema and template-authoring surface area.
- Requires migration of built-in roles.
- Requires tests to keep the summary aligned with the underlying role contract.

### Risk of not fixing it

- Leads can drift into implementation after compaction because the routing
  contract is not sufficiently restored.
- Developers can lose precise override/escalation discipline after compaction.
- Architects can lose the review-vs-implement boundary and drift into generic
  problem solving.
- Users will continue to assume role fidelity that the runtime does not
  actually provide.
- Local repo docs will keep absorbing role/runtime debt that should live in the
  framework.

### Recommended implementation steps

1. Extend the role template schema in
   [types.rs](/home/user/projects/taurhaus/src-tauri/src/templates/types.rs)
   with a structured `runtime_compact_summary` field and validation limits.

2. Propagate that field through template composition, member/config storage, and
   coordination domain models so the resolved `Member` carries it at compaction
   time.

3. Expand
   [reinjection.rs](/home/user/projects/taurhaus/src-tauri/src/coordination/reinjection.rs)
   so compaction cards include:
   - bounded role compact summary sections
   - `context_summary`
   - `response_expectation`

4. Keep Codex and Claude on the same semantic card, but render it in
   tool-appropriate form:
   - concise imperative text for Codex
   - structured JSON `additionalContext` for Claude

5. Add regression coverage proving that representative roles produce materially
   different compact reinjections and that critical contract sections survive
   compaction.

## Final Recommendation

The current compaction reinjection path is too lossy to claim fidelity to the
active role template contract.

Do not expand it to full template replay.
Do not leave it as a generic resume prompt.

Add a bounded structured `runtime_compact_summary` layer and merge that with
the existing operational task/footer/working-set context. That is the smallest
maintainable change that materially improves post-compaction role fidelity.
