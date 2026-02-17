# taurhaus — Issue Tracker

Active issues discovered during testing. Ordered by priority.

## Open

### I-05: Visual accent markers may not match design spec
**Priority**: Medium
**Status**: Needs audit
**Description**: Session card left border and other accent markers should use brand-500 (light) / brand-400 (dark) per prototype. Need to audit all accent colors in Shell.svelte against prototype source of truth. Specific concern: some markers may have shifted to grey/neutral instead of brand teal.

## Resolved

### I-01: Markdown rendering not implemented
**Priority**: High
**Status**: Fixed
**Resolution**: Created MarkdownRenderer.svelte using markdown-it + @shikijs/markdown-it + DOMPurify. Typography matches phase-3f spec (15px body, 720px max-width, reading-optimized scale). Light/dark theme support. 15 unit tests.

### I-02: Checkbox visuals broken in First-Run wizard
**Priority**: High
**Status**: Fixed
**Resolution**: Conditionally apply `checkBg` only when NOT selected — eliminates Tailwind v4 class conflict.

### I-03: Drag handle missing on First-Run wizard screen
**Priority**: Medium
**Status**: Fixed
**Resolution**: Added `data-tauri-drag-region` to wizard container and titlebar elements.

### I-04: Tilde expansion not working in scanner
**Priority**: High
**Status**: Fixed
**Resolution**: Added `expand_tilde()` helper using `dirs::home_dir()` to scan_directory, register_project, and register_projects_batch commands.

### I-06: Syntax highlighting not implemented
**Priority**: Medium
**Status**: Fixed
**Resolution**: Created CodeViewer.svelte using Shiki highlighter with CSS counter line numbers. Replaces plain text in Files tab. 16 languages loaded. Light/dark theme support.
