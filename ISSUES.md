# taurhaus — Issue Tracker

Active issues discovered during testing. Ordered by priority.

## Open

### I-01: Markdown rendering not implemented
**Priority**: High
**Status**: In progress
**Description**: README and session content displays as plain text in a `<pre>` tag. Need markdown parser + Shiki syntax highlighting as specified in ADR-012. Typography must match phase-3f-visual.md specs (15px body, 720px max-width, reading-optimized scale).

### I-02: Checkbox visuals broken in First-Run wizard
**Priority**: High
**Status**: Fixed (not committed)
**Description**: Selecting projects in wizard step 2 updates the count but checkboxes don't show checkmark or brand-600 fill. Root cause: Tailwind v4 class conflict — `checkBg` classes override the selected state classes. Fix: conditionally apply `checkBg` only when NOT selected.

### I-03: Drag handle missing on First-Run wizard screen
**Priority**: Medium
**Status**: Fixed (not committed)
**Description**: Window cannot be dragged during the First-Run wizard. The wizard outer div was missing `data-tauri-drag-region`. Fix: added attribute to wizard container.

### I-04: Tilde expansion not working in scanner
**Priority**: High
**Status**: Fixed (not committed)
**Description**: Scanning `~/projects` fails with "Not a directory" because `~` isn't expanded to the home directory. Fix: added `expand_tilde()` helper to scan_directory, register_project, and register_projects_batch commands.

### I-05: Visual accent markers may not match design spec
**Priority**: Medium
**Status**: Needs audit
**Description**: Session card left border and other accent markers should use brand-500 (light) / brand-400 (dark) per prototype. Need to audit all accent colors in Shell.svelte against prototype source of truth. Specific concern: some markers may have shifted to grey/neutral instead of brand teal.

### I-06: Syntax highlighting not implemented
**Priority**: Medium
**Status**: Blocked by I-01
**Description**: Code blocks in markdown and the Files tab code viewer need Shiki syntax highlighting with VS Code TextMate grammars. Light/dark theme support required. Part of ADR-012.

## Resolved

(None yet)
