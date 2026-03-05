# Team Templates Guide

User guide for the template system used by Mesh team setup.

## Overview

Team templates let you define reusable team structure before launch:

- **Role template**: one reusable lead/agent role definition (tool, model, instructions, constraints)
- **Team preset**: a lead role plus agent slot counts
- **Composition**: resolved roster generated from selected lead + agent slots

The template flow is:

`TemplateBrowserPanel` -> `TeamCustomizerPanel` -> `MeshSetupForm` -> `coordination_initialize_team`

Manual setup remains available via **Blank slate**.

## Template Sources

Templates come from two sources:

- **Built-in**: shipped with the app, read-only
- **User**: created/updated in app data, writable

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

## Built-in Catalog (v0.5.0)

Current built-ins ship from `src-tauri/resources/templates/`:

- **Roles (7)**: `claude-orchestrator`, `claude-researcher`, `claude-reviewer`, `codex-architect`, `codex-developer`, `codex-qa`, `gemini-ui-specialist`
- **Presets (4)**: `standard-team`, `fullstack-dev`, `research-dev`, `review-team`

Notable updates:
- `codex-architect` is a first-class built-in role template for architecture/review ownership.
- `standard-team` is the default "full team" preset (lead orchestrator + architect + developers + UI specialist).

## Using Templates in Setup

In Mesh setup, use **Start from template**:

1. **Quick preset** to apply a known preset immediately
2. **Browse catalog** to inspect role/preset metadata and open composition
3. **Build custom team** to compose from role templates
4. **Blank slate** to fall back to manual roster editing

After composition, the roster is still editable in `MeshSetupForm` (names, tools, models, project binding, descriptions).

## Composition and Validation

`TeamCustomizerPanel` resolves roster members and runs live checks:

- single-lead validation
- name-collision detection
- tool availability warnings
- composition warnings/errors from backend

Apply sends the final roster in standard initialize shape (same shape as manual setup).

## History, Diff, and Revert

`TemplateHistoryPanel` supports:

- **Global history**: commits across all managed template files
- **Selected template history**: commits touching a selected role/preset path
- **Commit details**: message, author, timestamp, changed files
- **Diff view**: per-file hunks (+/- lines)
- **Revert**: restore a template ID to a selected commit (creates a new forward commit)

Revert is template-ID scoped and uses the backend `templates_revert` command.

## Storage Status and Pending Actions

History UI exposes template storage status:

- repo mode (`git` or fallback filesystem)
- dirty state (uncommitted managed changes exist)
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
