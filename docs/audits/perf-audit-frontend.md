## Bundle Size — Shiki/Large Dependency Footprint
**Current**: `npm run build` produces a main chunk of ~753.5 kB minified (`index-*.js`, ~237.1 kB gzip) and multiple very large lazy chunks (`emacs-lisp` ~779.9 kB, `cpp` ~626.1 kB, `wasm` ~622.3 kB, `mermaid.core` ~475.7 kB, `cytoscape.esm` ~442.4 kB). Vite reports multiple `>500 kB` warnings.
**Issue**: `src/lib/markdown.js` uses Shiki highlighter setup that progressively loads many languages/themes, and markdown/diagram stack (Shiki + Mermaid + Cytoscape/Katex paths) dominates JS output. This materially increases cold-start parse/compile cost.
**Impact**: High — startup responsiveness and memory pressure are directly affected by multi-hundred-kB chunks.
**Fix**: Restrict default-highlighted language/theme set and lazy-load non-default packs behind user actions; split heavy diagram features to route/component-level dynamic imports with manual chunking. — effort: M

## IPC Call Patterns — Template Catalog N+1 + Duplicate Fetches
**Current**: Opening template catalog triggers `loadCatalog()` in `TemplateBrowserPanel` (`Promise.all([refreshRoles(), refreshPresets()])`). `refreshRoles()` calls `listRoleTemplates()`; `refreshPresets()` calls `listTeamPresets()`, which internally calls `listRoleTemplates()` again.
**Issue**: `src/lib/ipc.js` performs N+1 enrichment: `templates_list_roles` + per-role `templates_get_role`, and `templates_list_presets` + per-preset `templates_get_preset`. Because presets path also re-fetches roles, one catalog load duplicates role list/detail calls.
**Impact**: High — extra IPC round-trips increase latency and backend load; impact scales with number of roles/presets.
**Fix**: Add backend summary endpoints that already include required fields (or batched detail fetch), and reuse one role snapshot for both roles/presets in the same load cycle. — effort: M

## Reactivity Chains — Async Stale-Result Races
**Current**: Async rendering/search paths in `MarkdownRenderer.svelte`, `CodeViewer.svelte`, and `SearchOverlay.svelte` set component state from promise completions without sequence/token checks.
**Issue**: Rapid input/theme/source changes can cause older async responses to win race and overwrite newer state (`renderMarkdown(...)`, `highlightCode(...)`, `search(...)`).
**Impact**: Medium — leads to UI flicker/stale content and avoidable extra work under fast typing/navigation.
**Fix**: Add monotonic request IDs (or `AbortController` where possible) and commit results only when request is current. — effort: S

## List Rendering — No Virtualization on Large Lists/Trees
**Current**: File tree, sidebar project list, task board columns, and session history render full item sets using nested `#each` blocks; only Git commit list has incremental loading via `IntersectionObserver` (`loadMore`).
**Issue**: Large repositories/project sets can produce significant DOM size and repeated per-item computation without viewport windowing.
**Impact**: Medium — scrolling and interaction degrade with 100+ items, especially in recursive file tree and grouped sidebar/task lists.
**Fix**: Introduce virtualization/windowing for file tree/sidebar/task cards (start with largest surfaces), keeping GitTab pattern as baseline for incremental loading. — effort: M

## Memory — Asset Cache Has No Size/TTL Bound
**Current**: `src/lib/assetCache.js` stores data URIs in a process-lifetime `Map`; invalidation is event-driven only.
**Issue**: Large or numerous opened images can accumulate indefinitely in-memory if watcher invalidation is not triggered for those keys.
**Impact**: Medium — memory growth risk over long sessions and large docs/images.
**Fix**: Add LRU eviction with max entries/bytes and optional TTL; keep current explicit invalidation as a fast path. — effort: M

## Component Rendering — Heavy Components Mostly Controlled, but Hot Paths Remain
**Current**: Heavy components (`MeshTab`, `Shell`, `TemplateBrowserPanel`, `MeshCanvas`) use runes with generally bounded effects; `Shell.selectProject()` already parallelizes major IPC calls and applies stale-generation guard.
**Issue**: Some templates still do repeated per-render work in loops (for example grouped filtering/per-item session indicator calculations in sidebar/task views) without memoization boundaries.
**Impact**: Medium — avoidable CPU cost when list sizes grow or frequent updates occur.
**Fix**: Hoist repeated group computations into stable `$derived.by` maps keyed by source arrays; compute per-item display metadata once per update cycle. — effort: S

## Animation Performance — One Layout-Triggering Keyframe
**Current**: Most global animations use opacity/transform and are GPU-friendly; reduced-motion fallbacks exist in `app.css`.
**Issue**: `src/lib/components/MeshTab.svelte` uses `@keyframes shrink` animating `width` from `100%` to `0%` on status bars, triggering layout/repaint.
**Impact**: Low — localized animation, but still unnecessary layout work.
**Fix**: Replace width animation with transform-based progress (`scaleX`) on a fixed-width element with `transform-origin: left`. — effort: S

## Memory Leaks — Minor Timer Cleanup Gaps
**Current**: Major listeners/observers are generally cleaned up (for example in `Shell`, `GitTab`, `TaskBoard`, `SessionHistory`, `SlideOver`, `MeshConnection`, `sessionStore`).
**Issue**: Some timeout handles (notably in `Sidebar`, `AddProjectModal`, and `TeamCustomizerPanel`) are not universally cleared by unmount cleanup effects.
**Impact**: Low — short-lived timers, but can cause post-unmount state writes/noise.
**Fix**: Add component-level cleanup effect returning `clearTimeout(...)` for all outstanding timer handles. — effort: S
