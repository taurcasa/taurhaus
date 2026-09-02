# Team Templates Guide

User guide for the Mesh template system, with the correct mental model for role design.

## Why Roles Matter

Roles are not mainly about listing what a model can do. They exist to define a **context domain**.

When a role owns a lane such as architecture, UI, or review, that agent keeps absorbing the same kind of work over time. As work accumulates, the agent builds domain-specific context. That matters even more after handoff or compaction, because the role continues steering future work back into the same lane instead of resetting the agent into a generic capability bucket.

A good role template therefore answers:

- what work this agent should keep absorbing
- what decisions it should handle without escalation
- what it should escalate immediately

The best templates create **lane ownership** and **behavioral boundaries**. They help the lead route work cleanly and help the agent keep the right context alive over time.

## Template System Overview

Team templates let you define reusable team structure before launch:

- **Role template**: one reusable lead or agent definition
- **Team preset**: a lead role plus agent slot counts and overrides
- **Composition**: the resolved roster generated from the selected lead and agent slots

Lead roles are no longer Claude-only. Built-in and user presets can now target:

- `claude` lead roles
- `codex` lead roles
- `agy` lead roles

The current setup flow is:

`MeshSetupView` -> `MeshTeamBuilder` (primary surface) -> `coordination_initialize_team`

Advanced catalog/history/edit flows still exist through:

`TemplateBrowserPanel` -> `TeamCustomizerPanel` -> `MeshSetupView`

Manual setup is the builder's own empty roster — drag roles in, or start from a quick preset and edit.

## The Mental Model

Treat each role as a durable lane definition, not a capability tag list.

When you author a role, write down three things first:

- `focus_area`: the main lane this agent should own
- `context_summary`: the long-lived context it should keep accumulating
- `behavior_summary`: the boundary for independent action vs escalation

Those labels are not just the right authoring model. They are first-class persisted fields in the current role schema and now flow through composition, mesh runtime snapshots, hover/detail UI, and role import/export adapters. Instructions, behavioral contract, and constraints still matter, but they now support these context-steering fields instead of replacing them.

Once those three are clear, add the workflow fields that tell the role how to operate inside that lane:

- `communication_style`: how the role should sound in updates, reviews, and handoffs
- `quality_gates`: checks that must pass before the role can claim success
- `definition_of_done`: what "finished" means for this lane
- `phase_scope`: which delivery phases the role is meant to work in
- `mode`: the default operating mode, such as `research`, `execution`, or `review`
- `inherits_from`: the parent role when this role is a specialization or variant
- `required_artifacts`: what outputs the role is expected to produce

## Work Kinds Scale The Ceremony

Every role names the work kinds it primarily performs and links the shared
[team delivery standard](team-delivery-standard.md). The five work kinds are:

- `measure`: the measurement is the artifact; no commit, screenshot, or quoted-copy ceremony by default
- `diagnose`: produce a reproducible cause statement and focused evidence; do not change production behavior without authority
- `implement`: commit the behavior and its focused proof; use red-first for behavior and regression risk
- `review`: return numbered, standalone findings followed by the assigned score table
- `spec-delta`: make a small committed packet or specification correction, accepted by the named acceptance owner

Assignments select the work kind and use the standard's five-line contract:
objective, deliverable, first action, completion signal, and review route. The
committed packet remains the specification, so role templates should keep their
focus and behavioral identity without copying shared per-task ceremony into
their instructions.

Review depth follows the surface, not the role name. A declared hero surface
gets two independent reviews; other surfaces get one, and a spec-delta needs no
review beyond its acceptance owner. Deadlines and model effort remain optional
overrides; see [Optional deadline and effort overrides](team-delivery-standard.md#optional-deadline-and-effort-overrides)
for what setting a deadline actually buys.

Each surface also names one accountable implementer and one acceptance owner.
State cross-surface seams and commit authority in the review route so handoffs
are decided before work starts rather than discovered in the shared tree.

## What A Good Role Definition Looks Like

A strong role definition is concise and lane-specific:

- **Focus area**: one durable ownership domain
- **Context summary**: the background this agent should keep carrying forward
- **Behavioral boundary**: what it can decide alone and what must go back to the lead

Good role definitions are usually about:

- one domain, not many unrelated tasks
- stable ownership, not ad hoc assignment
- clear escalation rules, not vague “help where needed” language

Weak role definitions usually look like:

- broad capability lists without lane ownership
- instructions that overlap heavily with other roles
- no clear escalation boundary

## Authoring Prompts

Before saving a role, answer these questions:

- What kind of work should this agent keep absorbing?
- What should this agent handle without escalation?
- What should this agent escalate immediately?

If those answers are unclear, the role is still too generic.

## Example Role Definition

Use this as the conceptual template for a role:

```yaml
defaults:
  cli_tool: codex
  model: gpt-5.6-sol      # bare slug
  reasoning_effort: high  # separate field; null when a model has no separate effort
  default_name_pattern: architect-{project}

focus_area: "Architecture decisions and structural review"
context_summary: "Carries long-lived context around module boundaries, design tradeoffs, and review history."
behavior_summary: "Handles pattern choices independently; escalates direction changes immediately."
communication_style: "Short, decisive check-ins with explicit tradeoffs."
quality_gates:
  - "Validate the proposed shape against the touched modules."
  - "Call out migration or regression risk explicitly."
definition_of_done:
  - "The recommended structure is clear enough to implement."
  - "Open risks and follow-up decisions are documented."
phase_scope:
  - "planning"
  - "implementation"
mode: "review"
inherits_from: "taurhaus-base-reviewer"
required_artifacts:
  - "decision summary"
  - "risk list"
```

Then support it with:

- role instructions
- behavioral contract
- constraints and defaults where needed

## Current Structural Reference

The current template system remains structurally the same, but the role schema now explicitly carries both context-steering and workflow fields:

- **Role template**: `defaults` (`cli_tool`, `model`, `reasoning_effort`, `default_name_pattern`), `focus_area`, `context_summary`, `behavior_summary`, `communication_style`, `quality_gates`, `definition_of_done`, `handoff_expectations`, `phase_scope`, `mode`, `inherits_from`, `required_artifacts`, `runtime_compact_summary`, instructions, behavioral contract, capabilities (optional — empty is allowed), constraints, and optional import provenance
- **Team preset**: one lead role plus agent slots and preset-specific overrides. A preset can also pin the lead itself with `lead_overrides` (`model`, `reasoning_effort`); composition applies it on top of the lead role's defaults, and the advanced preset editor writes it when you edit the lead card.
- **Composition**: resolved roster produced from the selected lead and slots

### Model and reasoning effort

The canonical on-disk form is a bare model slug plus a separate `reasoning_effort` under `defaults:` — for example, the Codex implementation lane uses `model: gpt-5.6-sol` / `reasoning_effort: medium`, while a harness without a separate effort sets `null`. Legacy spellings such as `"gpt-5.4 high"` and `"gpt-5.4-high"` still load through `ModelSpec::parse_legacy` in composition and request normalization, and `SlotOverrides` — agent slots and `lead_overrides` — take `model` and `reasoning_effort` as separate fields too. Saving from the editor always writes the canonical form.

That means this guide changes both **how to think about roles** and what concrete fields you should author.

## What Each New Field Does

Use the new fields intentionally; they are not just extra metadata.

- `communication_style`
  - Use when the role should report in a specific way.
  - Example: `"Concise updates with exact blockers and file references."`
- `quality_gates`
  - Use for checks the role should satisfy before calling the task done.
  - Example: `"Run the scoped verification lane."`
- `definition_of_done`
  - Use to state what a successful outcome looks like for that lane.
  - Example: `"Residual risk is documented in the handoff."`
- `phase_scope`
  - Use to show where the role belongs in the delivery flow.
  - Example: `["planning", "review"]`
- `mode`
  - Use when a role has a normal operating mode that should be visible at a glance.
  - Example: `"research"`
- `inherits_from`
  - Use when the role is a specialization of an existing base role.
  - Example: `"taurhaus-base-worker"`
- `required_artifacts`
  - Use when the role should consistently emit concrete outputs.
  - Example: `["verification summary", "risk list"]`

## Authoring Guidance By Field

### `communication_style`

Keep this about delivery style, not responsibilities.

- Good: `"Calm, specific updates with exact file references."`
- Weak: `"Knows Rust and reviews code carefully."`

### `quality_gates`

Write these as verifiable expectations, not aspirations.

- Good: `"Run the named Rust test module before reporting."`
- Weak: `"Care about quality."`

### `definition_of_done`

Describe user-visible or operator-visible completion.

- Good: `"The bug no longer reproduces and the regression test stays in place."`
- Weak: `"Work is complete."`

### `phase_scope` and `mode`

Use `phase_scope` for where the role belongs in a workflow and `mode` for how it usually operates.

- `phase_scope`: `["planning", "review"]`
- `mode`: `"review"`

### `inherits_from`

Reach for this when a new role mostly reuses an existing lane but changes tone or strictness.

- Good candidate: `adversarial-reviewer` inherits from a base `reviewer`
- Bad candidate: two unrelated roles forced into a fake parent/child link

### `required_artifacts`

Prefer outputs another human or agent can immediately use.

- Good: `"decision summary"`, `"release checklist"`, `"follow-up list"`
- Weak: `"do good work"`

## Using Templates In Setup

`MeshSetupView` hosts `MeshTeamBuilder`, the single setup surface. It combines:

- quick presets, applied in one click
- a searchable role catalog with `tool`, `kind`, and `mode` filters, plus `Import YAML`, `New role`, and `Focus search` buttons
- drag-and-drop lead / agent composition
- inline roster editing for names, tools, models, project binding, and descriptions, with inline validation

Model editing uses the shared effort-aware `ModelSelect` (model plus reasoning effort). It keeps unknown YAML models as custom entries, shows deprecation hints, and displays an inherited role effort as the effective value. Its list comes from `ModelCatalog` on the terminal contract.

The builder's catalog hand-off (`onBrowseCatalog`) opens `TemplateBrowserPanel` for advanced import/export/history work, and preset editing/saving can flow through the advanced customizer panel when needed.

When more than one **Claude** account is registered, the builder notes that team members run on the team's config dir — per-team account selection is a follow-up. The note is scoped to Claude: it reads the first tool declaring `teamConfigNamespace` (`teamAccountNote`, `MeshTeamBuilder.svelte:83-91`), and Claude is the only harness that does (`cli_tool.rs:299` — every other entry declares `false`), so extra Codex or Grok accounts produce no note. A launch that names an account anyway is dropped and logged once as `launch.account.ignored_for_team`.

## Composition And Validation

The setup flow runs live checks while composing the roster:

- single-lead validation
- name-collision detection
- tool availability warnings
- composition warnings and errors from the backend

Apply sends the final roster in the same initialize shape used by manual setup.

Current lead-mode rule:

- **Claude leads** may use the existing attach-existing flow.
- **Codex, Antigravity and Grok leads** are currently `launch_new` only. The check is capability-driven — `should_use_mesh_sidecar`, i.e. any harness without `native_inbox_poller` — so it covers every future non-Claude harness too. If a preset or request tries `attach_existing`, backend validation rejects it with a clear error instead of silently falling back.

## Import, Export, and Provenance

Roles can now move across external agent-file formats through the adapter layer:

`RoleExportFormat` is `Yaml | ClaudeAgent | CopilotAgent | AgentsMd | GeminiMd`.

- **Export**:
  - canonical role YAML
  - Claude agent files
  - Copilot agent files
  - instruction-only formats such as `AGENTS.md` and `GEMINI.md`
- **Import**:
  - role YAML (`Import YAML` in the roster builder)
  - Claude custom-agent Markdown
  - Copilot custom-agent Markdown

Imported roles persist provenance metadata:

- source format
- source path
- import timestamp
- `non_roundtrippable_fields` for lossy conversions

For Taurhaus-authored Claude and Copilot exports, the adapter round-trips the extended role fields through compiled Markdown sections. That means `communication_style`, `quality_gates`, `definition_of_done`, `handoff_expectations`, `phase_scope`, `mode`, `inherits_from`, and `required_artifacts` survive export/import when the file came from Taurhaus. Instruction-only exports such as `AGENTS.md` and `GEMINI.md` remain intentionally lossy and record that downgrade in provenance.

One field never survives a non-YAML export: `defaults.reasoning_effort` has no representation in Claude or Copilot frontmatter, so it is recorded as lossy for every format except YAML.

The catalog UI surfaces that provenance so imported roles are visibly different from native Taurhaus roles, including in the filtered role catalog shown by `MeshTeamBuilder`.

## Agent Definitions For Workflows

Claude Code resolves a custom subagent from `<project>/.claude/agents/<name>.md`,
and the Workflow API's `agentType` reads the same registry. Taurhaus generates
those files from the role catalog, so a workflow stage, a mesh member, and a
taureval run are all steered by one text.

Run it from the template browser action **Export as Claude Code agents** — open
the browser with **Browse templates** in the mesh team builder — from
`just export-agents <project>`, or through the `export_agent_definitions` IPC
command (`exportAgentDefinitions(projectId)` on the frontend). Each returns
`{ written, removed, skipped }`, which the browser reports as *Exported 3 ·
removed 1 obsolete · 2 hand-written agents left untouched*. The recipe resolves
a relative `<project>` against the directory the command was typed in, and every
path refuses a project root that is not already a directory.

What lands in the file:

```text
---
name: "<role_id>"
description: "<focus_area, or the role name when it has none>"
model: "<defaults.model>"
effort: "<defaults.reasoning_effort>"     # omitted when the role sets none
---

# generated by taurhaus — edit the role template instead

Role: <role_id>

Communication Style:
...
```

The body is exactly the steering text an onboarding contract carries — role id,
communication style, instructions, behavioral contract, quality gates, handoff
expectations, definition of done, capabilities — rendered by the same code, so
the subagent and the mesh member can never drift apart.

Rules worth knowing:

- **Only harnesses that read agent definitions are exported.** That is a registry
  capability (`agent_definitions`), and today only Claude Code declares it, so a
  Codex, Antigravity, or Grok role is skipped without a file.
- **Generated files are owned by taurhaus.** The marker line is the contract,
  and only in the header position above — on its own line, right after the
  frontmatter block. A file in that shape is rewritten on every export, and
  anything else at that name is left exactly as it is and reported in `skipped`
  as `user_authored`, including a hand-written agent that quotes the marker
  sentence somewhere in its body. To change a generated agent, edit the role
  template and export again.
- **An export reconciles the directory.** A generated file whose role left the
  catalog — renamed, deleted, or moved to a harness that does not read agent
  definitions — is deleted and reported in `removed`, because Claude Code and a
  workflow's `agentType` would otherwise keep resolving instructions nobody can
  edit any more. Only `.md` files that carry the generated header directly in
  `.claude/agents` are ever removed; a hand-written agent is never one of them.
- **An export never leaves the project.** `.claude` and `agents` are resolved
  before anything is written: a linked component that stays inside the project
  (a shared agents directory of its own) is followed, one that points outside it
  — another checkout, a Windows junction, a link to nowhere — refuses the whole
  export rather than writing and deleting files nobody selected. A link at an
  agent's own file name is treated as hand-written: it is reported in `skipped`
  and neither followed nor replaced.
- **A role id has to be an agent name.** Claude Code resolves a subagent by a
  lowercase, hyphen-separated name, and that same id is what a workflow's
  `agentType` asks for. A role id such as `QA_reviewer` is reported in `skipped`
  as `unsupported_agent_name` instead of being written as a file that would
  never register; rename the role to export it.
- **Export is a deliberate act.** Saving a role does not re-export; run the
  action again after the role changes.
- **Project scope only.** User-scope agents (`~/.claude/agents`) are never
  touched.

This is a different thing from the single-role **Export → Claude Code agent** in
the role catalog. That one asks for a path and writes one portable, importable
file. This one regenerates a project directory that Claude Code reads by name.

## Template Sources

Templates come from two sources:

- **Built-in**: shipped with the app, read-only
- **User**: created or updated in app data, writable

On mutation, taurhaus initializes a git repository under the template store and commits managed template files.

Managed layout:

```text
<app_data_dir>/templates/
  roles/
    <role-id>.yaml
  presets/
    <preset-id>.yaml
  _meta/
    state.json
```

For isolated test runs, the app data root can be overridden with `TAURHAUS_DATA_DIR`, so template storage resolves under `<TAURHAUS_DATA_DIR>/templates/`.

## Built-In Catalog

Current built-ins ship from `src-tauri/resources/templates/`:

- **Roles (16)**:
  - orchestration: `v3-lead-claude` (Fable 5.1), `codex-orchestrator` (GPT-5.6 Sol), and `antigravity-orchestrator` (the Antigravity/agy alternative)
  - implementation: `v4-developer-claude`, `v4-developer-codex`, `v4-developer-agy`, `v4-developer-grok`, `quick-dev-codex`, and `frontend-design-skill-developer`
  - review and decision support: `v3-architect-codex`, `adversarial-reviewer-claude`, `claude-product-checker`, `claude-design-lead`, `claude-researcher`, `docs-verifier-codex`, and `codex-qa`
- **Presets (5)**:
  - `pair` — `v3-lead-claude` plus `quick-dev-codex`
  - `dev-team` — `v3-lead-claude` plus two `v4-developer-codex`
  - `full-team` — `v3-lead-claude` plus `v3-architect-codex` and two `v4-developer-codex`
  - `research-team` — `v3-lead-claude` plus `claude-researcher` and one `v4-developer-codex`
  - `grok-pair` — `v3-lead-claude` plus one `v4-developer-grok`

Every preset names its lead explicitly, references only canonical role ids, and
inherits model and effort from each role instead of pinning slot overrides. The
historical `v3-lead-claude` and `v3-architect-codex` ids remain because presets
already reference them; their bodies and versions carry the current playbook.
Those frozen compatibility ids no longer indicate which harness runs the role.

The architect and researcher are open model slots. Architect defaults to Fable 5.1
with GPT-5.6 Sol named as the fallback; researcher defaults to session-proven Sol
with Opus 5 High named as the alternative. The adversarial reviewer defaults to
Opus 5 and documents the candidate Sol-recall-then-Opus-verification variant.
Switching one of these experiments is a field edit to the role's `defaults`, not
a new role file.

Design is deliberately split: `claude-design-lead` owns creative direction
(Fable 5.1 preferred, Gemini via Antigravity as the alternative), while
`frontend-design-skill-developer` owns UI implementation (Sol preferred, Opus 5
as the alternative). Both roles treat automated evidence as a pre-filter and
state that UX conclusions require human validation.

These built-ins are most useful when you read them as lane definitions:

- orchestrator owns routing and unblock decisions
- architect owns structure and review boundaries
- developer owns implementation lanes
- creative-direction lead owns intent, critique, and final visual judgment
- UI implementation specialist turns approved direction into production UI
- product, adversarial, docs, and QA lanes own distinct verification questions

## History, Diff, And Revert

`TemplateHistoryPanel` supports:

- **Global history**: commits across all managed template files
- **Selected template history**: commits touching a selected role or preset path
- **Commit details**: message, author, timestamp, changed files
- **Diff view**: per-file hunks
- **Revert**: restore a template ID to a selected commit by creating a new forward commit

Revert is template-ID scoped and uses the backend `templates_revert` command.

## Storage Status And Pending Actions

History UI exposes template storage status:

- repo mode (`git` or fallback filesystem)
- dirty state
- pending action count from `_meta/state.json`

Manual flush (`templates_flush_pending`) force-commits pending template mutations when needed.

## IPC Surface (Frontend Names)

- `listRoleTemplates` / `getRoleTemplate`
- `exportAgentDefinitions`
- `listTeamPresets` / `getTeamPreset`
- `composeTeam`
- `getTemplateStorageStatus`
- `getTemplateHistory`
- `getTemplateDiff`
- `revertTemplateVersion`

These map to backend `templates_*` commands.
