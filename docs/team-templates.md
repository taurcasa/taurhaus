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
- `gemini` lead roles

The current setup flow is:

`MeshSetupView` -> `MeshTeamBuilder` (primary surface) -> `coordination_initialize_team`

Advanced catalog/history/edit flows still exist through:

`TemplateBrowserPanel` -> `TeamCustomizerPanel` -> `MeshSetupView`

Manual setup remains available via **Blank slate**.

## The Mental Model

Treat each role as a durable lane definition, not a capability tag list.

When you author a role, write down three things first:

- `focus_area`: the main lane this agent should own
- `context_summary`: the long-lived context it should keep accumulating
- `behavior_summary`: the boundary for independent action vs escalation

Those labels are not just the right authoring model. They are first-class persisted fields in the current role schema and now flow through composition, mesh runtime snapshots, hover/detail UI, and role import/export adapters. Instructions, behavioral contract, and constraints still matter, but they now support these context-steering fields instead of replacing them.

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
focus_area: "Architecture decisions and structural review"
context_summary: "Carries long-lived context around module boundaries, design tradeoffs, and review history."
behavior_summary: "Handles pattern choices independently; escalates direction changes immediately."
```

Then support it with:

- role instructions
- behavioral contract
- constraints and defaults where needed

## Current Structural Reference

The current template system remains structurally the same, but the role schema now explicitly carries the context-steering fields:

- **Role template**: tool, model defaults, `focus_area`, `context_summary`, `behavior_summary`, instructions, behavioral contract, constraints, and optional import provenance
- **Team preset**: one lead role plus agent slots and preset-specific overrides
- **Composition**: resolved roster produced from the selected lead and slots

That means this guide changes both **how to think about roles** and what concrete fields you should author.

## Using Templates In Setup

In Mesh setup, use **Start from template**:

1. **Quick preset** to apply a known preset immediately
2. **Browse catalog** to inspect role and preset metadata
3. **Build custom team** to compose from role templates
4. **Blank slate** to fall back to manual roster editing

`MeshTeamBuilder` is now the default setup surface. It combines:

- quick presets
- searchable role catalog filters (`tool` + `kind`)
- drag-and-drop lead / agent composition
- inline roster editing for names, tools, models, project binding, and descriptions

`Browse catalog` still opens the advanced template catalog for import/export/history work, and preset editing/saving can still flow through the advanced customizer panel when needed.

## Composition And Validation

The setup flow runs live checks while composing the roster:

- single-lead validation
- name-collision detection
- tool availability warnings
- composition warnings and errors from the backend

Apply sends the final roster in the same initialize shape used by manual setup.

Current lead-mode rule:

- **Claude leads** may use the existing attach-existing flow.
- **Codex and Gemini leads** are currently `launch_new` only. If a preset or request tries `attach_existing`, backend validation rejects it with a clear error instead of silently falling back.

## Import, Export, and Provenance

Roles can now move across external agent-file formats through the adapter layer:

- **Export**:
  - Claude agent files
  - Copilot agent files
  - instruction-only formats such as `AGENTS.md` and `GEMINI.md`
- **Import**:
  - Claude custom-agent Markdown
  - Copilot custom-agent Markdown

Imported roles persist provenance metadata:

- source format
- source path
- import timestamp
- `non_roundtrippable_fields` for lossy conversions

The catalog UI surfaces that provenance so imported roles are visibly different from native Taurhaus roles, including in the filtered role catalog shown by `MeshTeamBuilder`.

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

- **Roles (34)**:
  - legacy and imported-compatible roles such as `claude-orchestrator`, `claude-researcher`, `claude-reviewer`, `codex-developer`, and `gemini-ui-specialist`
  - taurhaus-specific roles such as `taurhaus-lead-claude`, `taurhaus-lead-codex`, `taurhaus-architect`, `taurhaus-developer`, and `taurhaus-designer`
  - v2 and v3 role families such as `v2-lead-claude`, `v2-developer-codex`, `v3-lead-claude`, `v3-architect-codex`, and `v3-product-checker-claude`
- **Presets (4)**:
  - `pair`
  - `dev-team`
  - `full-team`
  - `research-team`

These built-ins are most useful when you read them as lane definitions:

- orchestrator owns routing and unblock decisions
- architect owns structure and review boundaries
- developer owns implementation lanes
- UI specialist owns frontend presentation and polish
- reviewer owns verification and risk spotting

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
- `listTeamPresets` / `getTeamPreset`
- `composeTeam`
- `getTemplateStorageStatus`
- `getTemplateHistory`
- `getTemplateDiff`
- `revertTemplateVersion`

These map to backend `templates_*` commands.
