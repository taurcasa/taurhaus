# Project Branch Presentation Implementation

## Task

Implement approved proposal B from `project-branch-presentation-design-options-2026-03-13.md` in the sidebar project list.

## What changed

Updated `src/lib/SidebarProjectList.svelte` so branch display now follows the approved two-line rule:

- default branches are hidden
  - `main`
  - `master`
  - `develop`
- non-default branches render on a dedicated second line below the project name
- the first line keeps the project name, session indicators, and dirty marker
- feature-branch rows expand to `h-[50px]`
- default-branch rows stay at `h-[36px]`

The second-line branch treatment uses:

- `text-[10px]`
- `font-mono`
- subdued contrast
- branch glyph prefix `⑂`
- full-width truncation on the secondary line instead of a competing right-side pill

## Why this matches proposal B

Proposal B was chosen because it preserves the project name as the primary identifier and moves non-default branch context into a secondary lane rather than letting a long branch pill crush the first-line layout.

This implementation keeps that hierarchy explicit:

- line 1 answers: which project is this?
- line 2 answers: which non-default branch is it on?

That means long feature branch names no longer steal width from the project name or from session/dirt indicators.

## Regression coverage

Added a component test in `src/lib/Sidebar.component.test.js` that verifies:

- a feature branch renders on the second line
- a default branch does not render a branch line
- the feature-branch row uses the taller layout
- the default-branch row stays compact

## Verification

- `bunx vitest run src/lib/Sidebar.component.test.js`
- `bunx vitest run src/lib/sidebar.test.js`

Both passed.

## Notes

This task intentionally stayed narrow:

- no hover-card changes
- no branch icon-only fallback
- no changes to project sorting, filtering, or activity grouping
- no changes to branch handling outside the sidebar list
