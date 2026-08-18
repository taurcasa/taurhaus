# Shell.svelte decomposition assessment

Date: 2026-03-07
Task: #574
Finding: Q-AI-02

## Current state

- [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte) is 1302 lines.
- It already delegates the main feature tabs to child components (`Sidebar`, `OverviewTab`, `FilesTab`, `TaskBoard`, `MeshTab`, `GitTab`).
- Some supporting logic has already been extracted:
  - project selection hydrate helper: [projectSelection.js](/home/user/projects/taurhaus/src/lib/projectSelection.js)
  - theme preference loader: [themePreferences.js](/home/user/projects/taurhaus/src/lib/shell/themePreferences.js)
  - shared contexts: [ProjectContext.js](/home/user/projects/taurhaus/src/lib/context/ProjectContext.js), [SessionContext.js](/home/user/projects/taurhaus/src/lib/context/SessionContext.js)

## What is actually hurting

This is a real maintainability problem, but not because the file is simply "too many lines."

The real issues are:

1. It is a hot integration file.
   - It has taken repeated fixes this week around Mesh switching, focus navigation, polling fallback, degraded project loads, and startup behavior.
   - Recent churn is concentrated here, which makes it a likely conflict and regression surface even if merge conflicts are not yet the primary failure mode.

2. Multiple unrelated responsibilities still live in one script block.
   - startup / wizard / viewport bootstrap: [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:237)
   - daemon banner / update state: [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:304)
   - polling and Tauri event bridge wiring: [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:365)
   - project selection / hydrate / restore state: [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:510)
   - navigation and markdown routing: [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:763), [Shell.svelte](/home/user/projects/taurhaus/src/Shell.svelte:849)

3. Testability is weaker than it should be.
   - [shell.test.js](/home/user/projects/taurhaus/src/lib/shell.test.js) is 1303 lines and many tests re-implement Shell logic instead of importing extracted helpers or exercising the component boundary.
   - Example: the `applyNavEntry` tests reconstruct the function locally instead of testing a shared module: [shell.test.js](/home/user/projects/taurhaus/src/lib/shell.test.js:768)
   - Example: one test verifies string presence in the source file rather than behavior: [shell.test.js](/home/user/projects/taurhaus/src/lib/shell.test.js:1292)

## What is not hurting enough to justify a big rewrite

- The markup itself is not the main problem.
- The tab-level UI is already componentized.
- A large component split right now would create a lot of prop wiring without removing the risky state/effect coupling.

So this should not become a broad "break Shell into many Svelte components" effort.

## Recommendation

Do a narrow phase-1 decomposition now.

Worth doing now:

1. Extract pure navigation helpers.
   - candidate module: `src/lib/shell/navigation.js`
   - move:
     - `applyNavEntry`
     - `normalizeMarkdownTarget`
     - `buildPlatformRouteUrl`
     - small route-construction helpers
   - benefit:
     - direct unit tests against real exported functions
     - removes duplicated logic from `shell.test.js`

2. Extract runtime/event wiring behind a focused shell runtime helper.
   - candidate module: `src/lib/shell/runtime.svelte.js`
   - move:
     - session polling fallback effect
     - Tauri listener registration and cleanup
     - daemon bridge hydration/setup
   - keep state ownership in Shell if needed, but move listener orchestration out of the main script body

3. Keep project selection orchestration in Shell for now.
   - `loadProjectSelectionData()` is already extracted.
   - the remaining `selectProject()` flow still coordinates local state, navigation restore, and tab payload setup.
   - extracting it further right now would be higher risk than value.

Do not do yet:

- split titlebar/body into more components
- move all state into one giant controller object
- rewrite Shell around a new abstraction layer

## Proposed boundary shape

Phase 1:

- `src/lib/shell/navigation.js`
  - pure helpers only
- `src/lib/shell/runtime.svelte.js`
  - listener/polling registration and teardown
- `src/Shell.svelte`
  - remains the composition root and source of truth for high-level state

Possible later phase:

- if project-selection logic keeps growing, extract a focused `shellProjectState.svelte.js`
- only do this after phase 1 proves the seams are stable

## TDD approach

1. Add helper tests against exported navigation functions first.
2. Replace current "recreated logic" tests with imports from the extracted helpers.
3. Add one integration test for the runtime helper around event registration / cleanup behavior.
4. Move code without changing user-visible behavior.

## Effort estimate

- Phase-1 extraction: medium, about one day
- Large multi-component Shell rewrite: not justified now

## Bottom line

This is not audit noise, but it also does not justify a sweeping rewrite.

The right move is a small script-level decomposition that improves testability and reduces hot-file coupling while keeping `Shell.svelte` as the app composition root.
