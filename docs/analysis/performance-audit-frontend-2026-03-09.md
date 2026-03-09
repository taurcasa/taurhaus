# Frontend Performance Audit — 2026-03-09

## Scope

Audit the taurhaus frontend performance posture for the Svelte/Tauri WebView layer, with emphasis on startup cost, project-switch cost, large-list behavior, and heavy-content rendering.

Method:

- static review of the current Svelte frontend paths
- one production frontend build via `bun run build`
- no code changes in this task

## Executive Summary

The dominant frontend risk is the markdown/code-rendering stack. It currently pulls in a very large Shiki and Mermaid surface area, then compounds that bundle weight with expensive full-document render passes and sequential language loading. The next biggest issue is project selection: the initial bootstrap path intentionally re-enters full project loading after 1.5 seconds, and every project switch still fans out six IPC calls before mutating broad shell-level state.

Virtualization is already present in the sidebar and file tree, so this is not a general “everything is naive” codebase. The main gaps are concentrated in a few hot paths:

- markdown/code rendering
- project selection and shell-level state updates
- unvirtualized git history growth
- production console-to-IPC logging overhead

## Build Evidence

`bun run build` completed in 11.45s and emitted multiple very large chunks plus Vite chunk-size warnings. The largest current artifacts were:

- `dist/assets/emacs-lisp-C9XAeP06.js` — 779.85 kB / gzip 196.03 kB
- `dist/assets/index-BUrrhqcF.js` — 633.57 kB / gzip 180.76 kB
- `dist/assets/cpp-CofmeUqb.js` — 626.08 kB / gzip 44.82 kB
- `dist/assets/wasm-CG6Dc4jp.js` — 622.34 kB / gzip 230.29 kB
- `dist/assets/mermaid.core-e_tVr9dK.js` — 475.88 kB / gzip 131.29 kB
- `dist/assets/treemap-GDKQZRPO-CXb7tr0t.js` — 454.83 kB / gzip 107.87 kB
- `dist/assets/cytoscape.esm-BQaXIfA_.js` — 442.44 kB / gzip 141.91 kB
- `dist/assets/vendor-markdown-1LtA3jLe.js` — 103.98 kB / gzip 46.56 kB

That directly contradicts the current comment in `src/lib/markdown.js:12-14`, which says bundle size is irrelevant for this path.

## Findings

### Critical

#### 1. Markdown and code rendering carry the largest startup and interaction cost

Refs:

- `src/lib/markdown.js:9-18`
- `src/lib/markdown.js:54-83`
- `src/lib/markdown.js:161-169`
- `src/lib/markdown.js:208-237`
- `src/lib/markdown.js:249-275`
- `src/lib/MarkdownRenderer.svelte:34-56`
- `src/lib/MarkdownRenderer.svelte:58-160`
- `src/lib/CodeViewer.svelte:24-51`

Why this is a problem:

- The code explicitly opts into the full Shiki bundle and treats bundle size as irrelevant.
- Markdown rendering preloads fenced languages before render, and `preloadFencedLanguages()` awaits `highlighter.loadLanguage(...)` one language at a time.
- `MarkdownRenderer` does multiple post-render passes over the DOM: relative image resolution, Mermaid block discovery and SVG replacement, and anchor scrolling.
- `CodeViewer` reruns full-file highlighting whenever code, language, or theme changes.

Impact:

- Large startup parse/compile cost in the WebView from the markdown stack and its chunk graph.
- Large README and source-file opens pay full-document work instead of size-aware degradation.
- Theme changes and markdown changes re-trigger expensive end-to-end work.

#### 2. Mermaid pulls in a very heavy dependency chain for a feature that is often incidental

Refs:

- `src/lib/MarkdownRenderer.svelte:94-160`

Why this is a problem:

- Mermaid is dynamically imported only when needed, which is correct, but the loaded dependency graph is still very heavy.
- The build output shows `mermaid.core`, `cytoscape`, and `treemap` chunks all landing in the 440-476 kB range.
- Markdown rendering does full Mermaid discovery by querying the rendered DOM for every render pass.

Impact:

- Opening a README with Mermaid can trigger a very large one-time code load and a costly client-side SVG render path.
- This is especially visible in a desktop WebView where CPU spikes are easy to feel.

### High

#### 3. Initial project bootstrap intentionally schedules a second full project load after 1.5 seconds

Refs:

- `src/Shell.svelte:580-593`
- `src/Shell.svelte:620-628`

Why this is a problem:

- `loadProjects()` can call `bootstrapInitialProject(firstProject)`.
- `bootstrapInitialProject()` immediately sets `selectedProject`, then schedules `selectProject(project)` 1500 ms later.
- That creates a guaranteed delayed re-entry into the full project selection path for the initially selected project.

Impact:

- Extra startup work and avoidable post-launch churn.
- The delay also makes first-load behavior less deterministic because the app can look settled, then do another large project data fetch burst.

#### 4. Every project switch still fans out six IPC calls, then mutates broad shell state in one hot path

Refs:

- `src/Shell.svelte:630-688`
- `src/lib/projectSelection.js:3-5`
- `src/lib/projectSelection.js:63-118`

Why this is a problem:

- `selectProject()` waits on project details, commits, latest session, session history, README, and relationships.
- `loadProjectSelectionData()` adds a 25 ms debounce and resolves the six requests together.
- Once loaded, `Shell.svelte` updates a long list of root-level state fields in sequence.

Impact:

- Project switches produce a burst of IPC, then a broad rerender wave through the shell.
- The pattern is resilient but not cheap, and it scales poorly if more per-project sections are added.

#### 5. Git history uses incremental fetch but not DOM virtualization

Refs:

- `src/lib/GitTab.svelte:267-298`
- `src/lib/GitTab.svelte:569-655`

Why this is a problem:

- Infinite scroll correctly pages commit data in 50-row batches.
- `loadMore()` appends batches with `commits = [...commits, ...batch]`.
- The rendered commit list keeps every loaded row mounted.

Impact:

- Deep scroll sessions will steadily increase DOM size and diff cost.
- The fetch strategy is good; the rendering strategy is what falls behind.

### Medium

#### 6. File-tree virtualization exists, but visible-row flattening still recomputes the whole expanded tree

Refs:

- `src/lib/FilesTab.svelte:67-97`
- `src/lib/FilesTab.svelte:151-193`
- `src/lib/FilesTab.svelte:301-320`

Why this is a problem:

- Virtualization only reduces rendered rows.
- `treeRows` is still built by recursively flattening the entire visible tree whenever the tree or expansion state changes.
- File-tree refresh also replaces the full tree on refresh.

Impact:

- Good enough for medium projects, but large repos still pay whole-tree traversal cost on expansion and refresh.
- This is not the worst hotspot today, but it can become noticeable in very large monorepos.

#### 7. Opening files still does full-content reads and full highlight/render passes

Refs:

- `src/lib/FilesTab.svelte:157-164`
- `src/lib/FilesTab.svelte:235-278`
- `src/lib/CodeViewer.svelte:24-51`
- `src/lib/MarkdownRenderer.svelte:34-56`

Why this is a problem:

- `FilesTab` reads full file contents for text and markdown views.
- `CodeViewer` highlights the full file, not just the visible window.
- `MarkdownRenderer` fully rerenders the source and then performs additional DOM work.

Impact:

- Large README and source-file opens can be expensive even though the tree itself is virtualized.
- The current path lacks a size threshold where the UI degrades gracefully to plain text or partial rendering.

#### 8. The frontend logger forwards production console traffic over IPC

Refs:

- `src/lib/logger.js:16-23`
- `src/lib/logger.js:34-47`
- `src/lib/logger.js:140-208`
- `src/lib/logger.js:210-211`

Why this is a problem:

- Every forwarded console call serializes arguments, extracts structured context, and sends an IPC payload with `invoke('frontend_log', ...)`.
- Warnings and errors always forward.
- Even the bridge self-check log is forwarded at startup.

Impact:

- Logging overhead lands on interaction paths that should stay as close to zero-cost as possible.
- The rate limit helps, but production cost is still non-trivial in noisy paths.

#### 9. Mesh canvas keeps anchor placement accurate with repeated DOM reads plus global listeners

Refs:

- `src/lib/components/MeshCanvas.svelte:47-108`
- `src/lib/components/MeshCanvas.svelte:383-421`

Why this is a problem:

- Floating-card anchors are derived from `querySelectorAll('[data-node-id]')`, `getBoundingClientRect()`, and canvas measurements.
- When detail or hover cards are active, the component installs `ResizeObserver`, `window.resize`, and capture-phase `window.scroll` listeners that schedule animation-frame refreshes.

Impact:

- Probably acceptable at current mesh sizes, but this is a layout-read-heavy path and will not age well if the runtime canvas gets more dynamic.

### Low

#### 10. Sidebar virtualization is solid, but per-row session derivation still runs during render

Refs:

- `src/lib/Sidebar.svelte:30-43`
- `src/lib/SidebarProjectList.svelte:93-120`

Why this is a problem:

- The sidebar already virtualizes above a reasonable threshold.
- Each rendered project row still calls `getSessionsForProject(project.path)`, `toolIndicators(projectSessions)`, and `rowTintForSessions(projectSessions)` inline during rendering.

Impact:

- This is not a primary bottleneck now.
- It is still work that can multiply under frequent session-store churn.

## Ordered Recommendations

1. Reduce markdown-path weight first.
   - Stop treating “full Shiki bundle” as acceptable by default.
   - Shrink the language/theme set to the real project mix.
   - Parallelize fenced-language loads instead of awaiting them serially.
   - Add a hard size threshold where large files fall back to plain text or reduced highlighting.

2. Make Mermaid more selective.
   - Consider explicit user opt-in for diagram rendering, or render only once blocks are visible.
   - Avoid paying the full Mermaid dependency cost for markdown documents where diagrams are non-essential.

3. Remove the delayed second bootstrap load.
   - Replace `bootstrapInitialProject()` with a single deterministic first-load path.
   - If staged loading is still desired, make it explicit and incremental rather than “load again in 1.5s”.

4. Narrow the project-switch hot path.
   - Prefer one backend aggregation IPC for initial project selection, or load less critical sections lazily per tab.
   - Reduce shell-level root state churn by localizing state closer to the tab that owns it.

5. Virtualize the Git commit list.
   - Keep paged fetching, but stop mounting every loaded commit row forever.

6. Add large-file safeguards in the Files tab.
   - Introduce thresholds for syntax highlighting, markdown rendering, and image/diagram post-processing.

7. Tighten production logging cost.
   - Forward fewer info/debug logs in production, batch when possible, and avoid bridge self-logging on startup.

8. Cache or simplify mesh anchor measurement.
   - Prefer direct node-element maps over repeated broad queries.
   - Scope global listeners to the minimum necessary period and surface area.

## Positive Notes

- Sidebar virtualization is already in place and reasonably well-scoped.
- File-tree virtualization is already in place.
- Git history fetches are paged instead of preloading the full history.
- Async guard usage in the shell, files tab, and git tab is generally sound and avoids obvious stale-write races.

## Conclusion

The frontend does not need a broad rewrite. It needs targeted cleanup in a few heavy paths. The first payoff will come from reducing markdown-path weight and removing redundant project-bootstrap work; those two changes should materially improve both startup feel and first-interaction responsiveness in the Tauri WebView.
