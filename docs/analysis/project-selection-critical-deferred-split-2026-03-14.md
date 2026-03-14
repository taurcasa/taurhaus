# Project Selection Critical/Deferred Split

Task: `#1296`  
Date: `2026-03-14`

## Problem

`selectProject()` in `src/Shell.svelte` previously waited for the full six-section selection batch before applying the next project. That made restore-heavy switches feel stalled because the shell would not update the selected project, restored tab, or saved navigation targets until all IPC-backed sections completed.

## Critical vs deferred split

The new split keeps only local restore state on the critical path:

- selected project shell identity
- restored tab
- restored file target
- restored git target
- restored task target
- visited-tab state

The following sections are explicitly deferred:

- project details
- recent commits
- latest session
- session history
- README
- relationships

This means the shell switches immediately using the sidebar snapshot plus saved position, while the content sections catch up after the switch.

## Implementation

- `src/lib/shell/navigation.svelte.js`
  - added `buildCriticalProjectSelectionState(...)` for the immediate restore-first shell state
  - kept `buildProjectSelectionState(...)` as the fully hydrated variant layered on top of the critical state
- `src/lib/projectSelection.js`
  - added `loadDeferredProjectSelectionData(...)`
  - deferred selection loads now reuse the existing batching/prefetch machinery, but are tagged as `batchKind: 'deferred'`
- `src/Shell.svelte`
  - `selectProject(...)` now applies the critical state before awaiting the deferred batch
  - deferred results merge back into the selected project and secondary sections after the shell has already switched
  - project-selection lifecycle logs now correctly mark this path as `blocking: false` / `deferred: true`

## Restore catch-up behavior

When a project has saved position state, the shell now restores the target tab and navigation immediately, even while the deferred content sections are still loading. Secondary panels clear their old project data during the switch and then refill from the deferred batch, which avoids showing stale commits/sessions/relationships from the previous project.

## Regression coverage

Added or updated coverage for:

- deferred batch reuse through `loadDeferredProjectSelectionData(...)`
- restore-first selection state in shell navigation helpers
- startup selection using one deferred batch without a delayed duplicate load
- hover prefetch staying non-visible while priming the deferred batch
- deferred reconnect retry during project switching
- mesh tab remounting correctly across immediate shell switches

## Verification

- `bunx vitest run src/lib/projectSelection.test.js src/lib/shell.test.js src/Shell.meshFocus.test.js`
- `bun run typecheck`
