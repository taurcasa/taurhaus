# Role Management Review Against The Context-Steering Model

## Executive Summary

The current role system is **partially aligned** with the context-steering mental model, but not consistently.

What is already strong:

- built-in role **instructions** mostly define operating lanes, not raw abilities
- **behavioral contracts** encode communication, execution, and escalation boundaries well
- the **composition engine** already treats instructions and behavioral contract as the main resolved payload
- preset overrides rely heavily on `instructions_append`, which is compatible with context steering

What is misaligned:

- the schema still treats `capabilities` as a required first-class concept
- the UI surfaces `capabilities` prominently in authoring and browsing flows
- frontend payload normalization invents default capabilities like `implementation` and `orchestration`
- docs still explain role templates structurally, but not philosophically, so users are not guided toward defining a context domain

The result is a split-brain system:

- the best parts of the implementation think in terms of **instructions + behavioral boundaries**
- the visible schema and UX still suggest roles are about **capability tags**

That should be corrected.

## Review Lens

Under the correct mental model, a role is valuable because it:

- steers what work gets assigned to an agent
- causes that agent to accumulate domain-specific context over time
- preserves a specialization lane through compaction and handoff
- communicates behavioral boundaries to the team lead

A role is **not** primarily valuable because it declares what the model can do.

## 1. Role Schema

### Assessment

The schema is mixed.

Aligned fields:

- `instructions`
- `behavioral_contract`
- `constraints.allowed_project_binding`
- `constraints.requires_lead_tool`
- `defaults.default_name_pattern`

These all help define context domain, collaboration boundaries, and operating lane.

Misaligned fields:

- `capabilities` on `RoleTemplate` is required and validated as non-empty
- `capabilities_add` / `capabilities_remove` on `SlotOverrides`
- `capabilities` on `ResolvedMember`

These fields over-index on taxonomy rather than context steering.

### Specific mismatches

In [types.rs](/home/mstie/projects/taurhaus/src-tauri/src/templates/types.rs):

- `RoleTemplate` requires `capabilities: Vec<String>`
- validation fails when capabilities are empty
- slot overrides allow mutating capabilities instead of context summaries

In [composition.rs](/home/mstie/projects/taurhaus/src-tauri/src/templates/composition.rs):

- `ResolvedFields` and `ResolvedMember` carry `capabilities`
- override resolution mutates capabilities via add/remove logic

This makes capabilities look like a real semantic layer, even though the actual steering value is carried by instructions and behavioral contract.

### Conclusion

The schema is **structurally close** to the right model, but the mandatory capability field is the main conceptual mismatch.

## 2. Built-In Role Templates

### Assessment

The built-in templates are mostly good.

Their strongest parts are the instructions and behavioral contracts. Those already describe:

- what lane the role should stay in
- what kind of work it should absorb
- what it should escalate
- how it should collaborate with the lead and other agents

That is exactly what context steering needs.

### What is already aligned

Examples:

- [claude-orchestrator.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/claude-orchestrator.yaml)
  Strong context-steering language: delegation, routing, unblock behavior, direction-vs-implementation boundary.

- [codex-architect.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/codex-architect.yaml)
  Strong lane definition: structural decisions, design review, documentation, escalation boundary.

- [claude-reviewer.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/claude-reviewer.yaml)
  Strong review-focused operating context and review closure rules.

- [gemini-ui-specialist.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/gemini-ui-specialist.yaml)
  Strong domain ownership and scope limits, though verbose.

The preset layer is also generally aligned because it mostly appends lane-specific instructions rather than toggling abstract traits.

Examples:

- [standard-team.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/presets/standard-team.yaml)
- [research-dev.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/presets/research-dev.yaml)
- [review-team.yaml](/home/mstie/projects/taurhaus/src-tauri/resources/templates/presets/review-team.yaml)

### What is misaligned

Every built-in role still includes a `capabilities:` list.

Examples:

- `planning`, `coordination`, `review`, `triage`
- `architecture`, `review`, `documentation`, `planning`, `refactoring`
- `implementation`, `refactoring`, `testing`, `debugging`

These are not harmful in isolation, but they reinforce the wrong explanation for why the role exists.

The instructions are doing the real work; the capabilities lists are mostly decorative taxonomy.

### Conclusion

Built-in templates are **substantively aligned** but **semantically mislabeled** by the capabilities field.

## 3. Template Browser / Customizer UX

### Assessment

The UX is the most visibly misaligned part of the system.

It teaches users to think in capability terms even though the underlying role value comes from context steering.

### Specific mismatches

In [RoleCatalog.svelte](/home/mstie/projects/taurhaus/src/lib/components/RoleCatalog.svelte):

- capability chips are rendered directly on role cards
- capability chip helpers are core presentation utilities
- role detail primarily shows raw instructions, but summary metadata is still tool/model/capabilities based

In [templateBrowserUtils.js](/home/mstie/projects/taurhaus/src/lib/components/templateBrowserUtils.js):

- normalization exposes `capabilities` as a standard role field
- there is no `focus area`, `context summary`, or `behavior summary` field
- helper naming such as `capabilityChipTone` and `capabilityTestId` bakes the concept into the UI vocabulary

In [RoleEditor.svelte](/home/mstie/projects/taurhaus/src/lib/components/RoleEditor.svelte):

- there is a full dedicated `Capabilities` section
- prompt text is `Add capability tag...`
- empty state says `No custom capabilities defined.`
- behavioral contract is edited as a loose checklist, but there is no dedicated field for context domain or focus area

In [templates.js](/home/mstie/projects/taurhaus/src/lib/ipc/templates.js) and [templatePayloads.js](/home/mstie/projects/taurhaus/src/lib/ipc/templatePayloads.js):

- normalized roles default missing capabilities to `orchestration` or `implementation`
- mock composition payloads return capabilities as if they are the core resolved meaning

In [TeamCustomizerPanel.svelte](/home/mstie/projects/taurhaus/src/lib/components/TeamCustomizerPanel.svelte):

- the save-as-preset flow maps chosen members back to role IDs but does not help the user articulate context domain
- the flow is structurally sound, but not pedagogically aligned

### Conclusion

The authoring and browsing UX is **the highest-misalignment area**. It currently trains users to define roles as bags of tags.

## 4. Role Composition Engine

### Assessment

The composition engine is mostly compatible with context steering.

Why:

- it resolves full instructions per role
- it resolves and appends behavioral contract content
- it enforces project-binding and lead-tool constraints
- it supports preset-specific instruction appends, which are a good mechanism for sharpening a role’s lane in a given preset

### What is aligned

In [composition.rs](/home/mstie/projects/taurhaus/src-tauri/src/templates/composition.rs):

- `instructions_replace` / `instructions_append` are strong context-steering primitives
- `behavioral_contract_append` is also aligned
- project-binding validation fits the mental model because project location shapes context accumulation

### What is misaligned

- the engine still resolves and mutates capabilities via `capabilities_add` / `capabilities_remove`
- the resolved output carries capabilities to the frontend as if they belong in runtime meaning

This is unnecessary conceptual baggage.

### Conclusion

The composition engine needs **schema cleanup**, not a redesign.

## 5. Documentation

### Assessment

Documentation is incomplete on the “why” of roles.

[team-templates.md](/home/mstie/projects/taurhaus/docs/team-templates.md) explains:

- what a role template is structurally
- what a team preset is
- how templates are stored and composed

But it does **not** explain the mental model:

- why roles exist
- how they preserve specialization through context accumulation
- why users should describe domains and boundaries rather than abstract capabilities

### Specific mismatches

In [team-templates.md](/home/mstie/projects/taurhaus/docs/team-templates.md):

- `Role template` is described as `tool, model, instructions, constraints`
- no section explains context steering or domain memory
- no guidance tells users what a good role definition looks like under this model

In frontend mocks/docs:

- role examples and summaries still include capability arrays in a first-class way
- the role system is not documented as a memory and routing aid

### Conclusion

Documentation is **structurally accurate but conceptually under-explained**.

## Mismatches Found

### High-severity conceptual mismatches

- Mandatory `capabilities` in the role schema
- `capabilities_add` / `capabilities_remove` in slot overrides
- Role editor’s dedicated capabilities authoring section
- Role catalog’s capability-chip-heavy presentation
- Frontend normalization that invents default capabilities

### Medium-severity mismatches

- Built-in templates still carry capability lists even though instructions and contracts already define the role meaning
- Team template docs explain structure but not context-steering purpose
- Mock payloads/tests reinforce the wrong vocabulary

### Low-severity mismatches

- Some preset descriptions still emphasize separation of tasks more than accumulation of domain context
- Role detail surfaces show raw instructions but not concise focus/context summaries

## Recommendations

### 1. Deprecate capability-first semantics

Replace `capabilities` as the primary conceptual field with context-oriented fields such as:

- `focus_area`
- `context_summary`
- `behavior_summary`

Possible future schema:

```yaml
focus_area: "Architecture decisions and structural review"
context_summary: "Carries long-lived context around module boundaries, design tradeoffs, and review history."
behavior_summary: "Handles pattern choices independently; escalates direction changes."
```

`capabilities` can be:

- removed entirely
- or kept as optional internal tags for search/filtering only, not as user-facing meaning

### 2. Make schema evolution explicit

Recommended migration path:

- Phase 1: make `capabilities` optional, stop requiring non-empty values
- Phase 2: add explicit context-steering summary fields
- Phase 3: deprecate `capabilities_add` / `capabilities_remove` from slot overrides

### 3. Rework role authoring UX

In `RoleEditor`:

- replace `Capabilities` with `Focus Area` and `Context Summary`
- keep behavioral contract, but present it as operational boundaries rather than generic checklists
- guide the author with prompts like:
  - `What kind of work should this agent keep absorbing?`
  - `What should this agent handle without escalation?`
  - `What should this agent escalate immediately?`

### 4. Rework role browsing UX

In `RoleCatalog` and role detail:

- remove capability chips from the primary card surface
- show:
  - role name
  - tool/model
  - focus area
  - one-line behavioral boundary
- use raw instructions only in deeper inspection

### 5. Keep composition mechanics, drop capability mechanics

The composition engine should continue to resolve:

- instructions
- behavioral contract
- project binding
- model/tool defaults

It should stop treating capability mutation as a core preset override behavior.

### 6. Rewrite template docs around context steering

Update `docs/team-templates.md` to explain:

- roles define context domains
- role quality matters because it shapes future work after context builds up
- the best role templates are about lane ownership and behavioral boundaries, not lists of abilities

### 7. Update built-in roles lightly, not radically

Most built-in instructions are already good. The main change is:

- remove or demote capabilities lists
- add concise context-domain summaries where useful

## Priority Ranking

### P0

- Stop teaching users that capabilities are the meaning of roles
- Update docs and UI copy to explain context steering
- Remove capability chips from primary role-browsing surfaces

### P1

- Make `capabilities` optional in schema/validation
- Add explicit context-oriented summary fields
- Replace capabilities authoring UI with focus/context/boundary fields
- Stop generating default capabilities in frontend normalization

### P2

- Remove capabilities add/remove override mechanics
- Update mocks/tests/examples to the new vocabulary
- Optionally add richer runtime signals such as recent task themes

## Final Assessment By Area

- **Role schema**: partially aligned, but polluted by mandatory capabilities
- **Built-in role templates**: mostly aligned in substance, mislabeled by capabilities
- **Template browser / customizer UX**: materially misaligned and needs redesign
- **Role composition engine**: mostly sound, needs cleanup rather than replacement
- **Documentation**: structurally accurate, conceptually incomplete

## Final Call

The role system should be treated as a **context-steering system with leftover capability-era vocabulary**.

The good news is that the deepest parts are already pointing in the right direction. The fix is not to rebuild roles from scratch. The fix is to:

1. remove the misleading capability framing
2. make context domain explicit in schema and UI
3. preserve the existing strengths around instructions, behavioral boundaries, and preset-specific steering
