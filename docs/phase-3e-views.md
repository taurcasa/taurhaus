# Phase 3E: View Design

> Per-view specifications for each view in the [Information Architecture](phase-3d-architecture.md). Designed in journey-priority order. Each spec is implementation-ready: a developer reading it should be able to build the view without further design input.

**Responsive context**: taurhaus is a desktop Tauri app. Minimum viewport: 1280x1440 (ultrawide side panel). Maximum viewport: 2560x1440 (center zone). No mobile or tablet breakpoints needed.

---

## V-01: Project List (Sidebar)

### Purpose

This view lets the user browse all registered projects and select one for focus by showing project names with activity state indicators, working tree status, and recency signals in a compact, scannable list.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| Application launch | App opens | None — loads all projects, restores last selection |
| Any view | Always visible | Sidebar persists across all view states |

The sidebar is always visible. There is no "entry point" in the traditional sense — it's a persistent fixture. On launch, restore the last selected project (if any) from local storage.

### Layout Structure

```
┌────────────────────────┐
│ ┌────────────────────┐ │ ← Filter input (fixed)
│ │ 🔍 Filter...       │ │
│ └────────────────────┘ │
│ ┌────────────────────┐ │ ← Sort control (fixed)
│ │ Sort: Activity ▾   │ │
│ └────────────────────┘ │
│ ── Active ──────────── │ ← Group header (scrolls with list)
│ ┌────────────────────┐ │
│ │ ● taurhaus    main │ │ ← Project item
│ ├────────────────────┤ │
│ │ ● taurui     main  │ │
│ ├────────────────────┤ │
│ │ ◐ MIR      feat/x  │ │ ← dirty indicator
│ └────────────────────┘ │
│ ── Recent ──────────── │
│ ┌────────────────────┐ │
│ │ ○ taursec    main  │ │
│ └────────────────────┘ │
│ ── Stale ───────────── │
│ ┌────────────────────┐ │
│ │ ◌ ledger     main  │ │
│ └────────────────────┘ │
│                        │
│ ┌──────┐ ┌──────────┐ │ ← Footer actions (fixed)
│ │ + Add│ │ ⚙ Settings│ │
│ └──────┘ └──────────┘ │
└────────────────────────┘
```

- **Width**: 240px fixed at ≤1920px viewport. 280px at >1920px.
- **Regions**: Filter input (fixed top), sort control (fixed), project list (scrollable), footer actions (fixed bottom).
- **Scroll**: Project list scrolls independently. Filter, sort, and footer remain fixed.
- **Grouping**: Projects grouped by activity state (Active → Recent → Stale → Dormant). Group headers scroll with the list. Groups can be collapsed.

### Information Hierarchy

**1. Primary — seen immediately**

| Element | Content | Why primary |
|---------|---------|-------------|
| Project name | Directory name or user-set name | Primary identifier — distinguishes projects. Answers "which project is this?" |
| Activity state indicator | Colored dot: ● active, ○ recent, ◌ stale, ◌ dormant | Answers "is this project alive?" Drives the core J-01 decision. |
| Working tree status | Clean/dirty indicator (dot fill or icon) | Answers "is there uncommitted work?" Urgent signal. |

**2. Secondary — visible on focused scanning**

| Element | Content | Why secondary |
|---------|---------|---------------|
| Current branch | Branch name, truncated | Contextual — "where is development happening?" Not needed for every scan. |
| Group headers | Activity state labels | Structural — organizes the scan but not individually important. |

**3. Tertiary — revealed on interaction**

| Element | Content | Revealed by | Why tertiary |
|---------|---------|-------------|--------------|
| Last activity date | Relative time ("2h ago") | Hover on project item | Useful but the group membership already conveys recency. |
| Tags | Project type badges | Hover or expand | Low frequency need — user usually knows project types. |
| Path | Filesystem path | Hover tooltip | Only needed for disambiguation. |

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Select project | Click item / Arrow keys + Enter | Main area shows selected project's Overview (V-02) | Item highlights with accent background. Previous selection unhighlights. | Arrow Up/Down moves focus through list. Enter selects focused item. |
| 2 | Filter projects | Type in filter input | List updates to show matching projects | Non-matching items hidden. Result count shown. Filter input has clear button. | Filters on project name, description, tags. Instant (no debounce needed — client-side). |
| 3 | Sort projects | Click sort control, select option | List reorders | Sort control shows current sort. List animates reorder. | Options: Activity (default), Name (A-Z), Status. |
| 4 | Collapse/expand group | Click group header | Group items hide/show | Chevron rotates. Smooth height animation. | Collapsed state persisted per session. |
| 5 | Add project | Click "+" button | Registration modal opens | Modal overlay appears | See Project Registration (modal spec below). |
| 6 | Open settings | Click settings icon | Main area switches to V-05 | Settings icon highlights. Main area content transitions. | Project selection is preserved for return. |
| 7 | Keyboard: focus filter | Cmd+F or "/" when sidebar focused | Filter input receives focus | Cursor appears in filter input | Standard search-in-list shortcut. |
| 8 | Keyboard: navigate list | Arrow Up/Down | Focus moves between items | Focus ring on item | Wraps: bottom → top. |

### State Variations

**Empty — no projects registered**
- Visual: Sidebar shows filter input (disabled), empty list area with centered message: "No projects yet" and "Scan for projects" button.
- Actions: "Scan for projects" opens V-06 (First-Run Setup) flow. "Add project" button also visible.
- This state triggers V-06 to take over the main area automatically.

**Loading — initial project list loading**
- Visual: Skeleton items (3-5 animated placeholder rows) in the list area. Filter and sort controls visible but disabled.
- Duration: <50ms expected (local data). Skeleton appears only if loading takes >200ms (threshold prevents flicker).

**Error — project list failed to load**
- Visual: Error message in list area: "Could not load projects" with "Retry" action. Sidebar structure (filter, sort, footer) remains visible.
- Recovery: Retry button re-fetches project list. If persistent, suggest checking filesystem permissions.

**Populated — minimal (1-3 projects)**
- Visual: List renders normally. No groups shown (all projects likely in same activity state). The list doesn't look broken — just short.
- Notes: "Add project" affordance is more prominent in this state.

**Populated — typical (30-50 projects)**
- Visual: Full list with activity state groups. Scrollable. Filter and sort are useful but not critical.

**Populated — maximum (100+ projects)**
- Visual: Long scrollable list. Groups help chunking. Filter becomes the primary navigation mechanism — user types to narrow rather than scrolling.
- Performance: Virtual scrolling if list renders more than ~100 visible items.

**Filtered — user has typed in filter**
- Visual: Only matching projects visible. Filter input shows typed text with clear ("×") button. Non-matching groups may be empty (hidden entirely). Result count: "12 of 47 projects".
- No results: "No projects match '[query]'" with clear filter action.

**Selected — a project is highlighted**
- Visual: Selected project item has accent background and left border. Visually distinct from hover state.
- Keyboard: Focus ring appears on keyboard navigation. Selected state persists even when sidebar loses focus.

---

## V-02: Project Overview (Main Area — Default Tab)

### Purpose

This view lets the user assess a project's current state and resume working context by showing the most recent session handoff, recent git activity, project metadata, and relationships in a single scrollable overview.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| V-01: Project List | Click project in sidebar | Project ID — view loads this project's data |
| V-03: Document Viewer | Click "Overview" tab | Project ID — same project, tab switch. Scroll position restored. |
| V-04: Search Overlay | Click project/session/commit result | Project ID + optional session/commit ID to scroll to |
| App launch | Automatic | Last selected project ID from local storage |

Most common entry: click project in sidebar (~80%). Default state: most recent session prominently displayed, Overview tab active.

### Layout Structure

```
┌──────────────────────────────────────────────────┐
│ ┌─────────────┬──────────┐                       │ ← Tab bar (fixed)
│ │ ▸ Overview  │  Files   │                       │
│ └─────────────┴──────────┘                       │
├──────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────┐ │ ← Project header (fixed)
│ │ taurhaus                         main  ●     │ │
│ │ Desktop tool for AI project management       │ │
│ └──────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────┤ ← Scrollable content below
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ LATEST SESSION                    2026-02-16 │ │ ← Session card (expanded)
│ │                                              │ │
│ │ Summary: Completed Phase 3D information...   │ │
│ │                                              │ │
│ │ Next Steps:                                  │ │
│ │ • Write Phase 3E view specs                  │ │
│ │ • Design each view in priority order         │ │
│ │                                              │ │
│ │ Open Questions:                              │ │
│ │ • Tab bar vs. segmented control for 2 tabs   │ │
│ │                                              │ │
│ │ [Add notes]              [View full session] │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ RECENT ACTIVITY                              │ │ ← Git commits section
│ │                                              │ │
│ │ a1b2c3  Add phase-3d-architecture.md   2h   │ │
│ │ d4e5f6  Complete Phase 3C journeys     1d   │ │
│ │ g7h8i9  Add phase-3b-domain.md         2d   │ │
│ │ ...                                          │ │
│ │                        [View all commits →]  │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ RELATIONSHIPS                                │ │ ← Relationships section
│ │                                              │ │
│ │ → taurui (provides design to)                │ │
│ │ ← taursec (audited by)                       │ │
│ │                                              │ │
│ │ [+ Add relationship]                         │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ SESSION HISTORY                              │ │ ← Older sessions
│ │                                              │ │
│ │ 2026-02-15  Completed Phase 3C user...       │ │
│ │ 2026-02-14  Started Phase 3B domain...       │ │
│ │ 2026-02-13  Wrote design brief...            │ │
│ │                                              │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ PROJECT INFO                                 │ │ ← Metadata section
│ │ Path: ~/projects/taurhaus                    │ │
│ │ Tags: tauri-app, design                      │ │
│ │ [Edit metadata]  [Remove project]            │ │
│ └──────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
```

- **Width**: Fills remaining space after sidebar. ~1000px at 1280px viewport. ~2280px at 2560px (wider sections, more horizontal breathing room, possible multi-column for relationships + git activity side by side).
- **Regions**: Tab bar (fixed top of main area), project header (fixed below tabs), scrollable content (session → git → relationships → session history → metadata).
- **Scroll**: Content scrolls vertically. Tab bar and project header remain fixed.
- **At 2560px**: Consider two-column layout for the scrollable content — latest session in left column (wider), git activity + relationships in right column. This uses the extra width productively.

### Information Hierarchy

**1. Primary — seen immediately**

| Element | Content | Why primary |
|---------|---------|-------------|
| Project name + branch + status | Header row: "taurhaus — main — ● Active" | Confirms identity. Answers "am I looking at the right project?" |
| Latest session summary | What happened last time | Highest-value content. Answers "what did we do?" — the core J-02 question. |
| Latest session next steps | What to do next | Directly actionable. Answers "where do we pick up?" |

**2. Secondary — visible on focused scanning**

| Element | Content | Why secondary |
|---------|---------|---------------|
| Open questions | Unresolved items from session | Important but not blocking — user may decide they're resolved. |
| Recent commits | Last 10-20 commit messages + dates. When commits exist after the latest session, the section header shows "N new since session" count and a subtle divider separates post-session from pre-session commits. | Answers "has anything changed since the session?" The divergence count is the key J-02 signal for plan validity. |
| Relationships | Linked projects with types | Contextual — "what is this project connected to?" |

**3. Tertiary — revealed on interaction**

| Element | Content | Revealed by | Why tertiary |
|---------|---------|-------------|--------------|
| Session history | Older sessions list | Scroll down | Usually the latest session is sufficient. Historical is for reference. |
| Full session detail | User notes, decisions, session ID | Click "View full session" | Most sessions are consumed via the summary. Full detail is occasional. |
| Full git history | All commits | Click "View all commits" | Recent commits suffice for the "plan still valid?" check. |
| Project metadata | Path, tags, edit/remove | Scroll to bottom | Set once, rarely revisited. |
| Commit diffs | File changes per commit | Click commit entry | Detailed investigation, not routine. |

> **Design note — Session decisions hierarchy**: Decisions made during a session are intentionally tertiary (behind "View full session" expand). The summary, next steps, and open questions visible in the default card view are sufficient for the J-02 "resume context" flow in ~90% of cases. Decisions are reference material consulted occasionally, not the primary action items.

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Switch to Files tab | Click "Files" tab | Main area shows V-03 Document Viewer | Tab indicator moves. Overview scroll position preserved for return. | Keyboard: specific shortcut TBD (e.g., Ctrl+2). |
| 2 | View full session | Click session card or "View full session" | Session card expands in-place to show full content (decisions, notes, session ID) | Card expands with smooth animation. "Collapse" action appears. | |
| 3 | Add session notes | Click "Add notes" on session card | Inline text editor appears below session content | Editor area appears. Save/cancel actions visible. | Rich text or markdown input. Images via paste or file picker. |
| 4 | Expand session history item | Click historical session entry | Expands in-place to show full session content | Entry expands. Others remain collapsed. | |
| 5 | View all commits | Click "View all commits" | Commit list expands to show full history | Section grows. Virtual scrolling for large histories. | |
| 6 | Click commit entry | Click a commit | Expands to show changed files summary | Entry expands in-place. | |
| 7 | Add relationship | Click "+ Add relationship" | Modal opens with target project selector and type picker | Modal overlay. Project list searchable/filterable. | |
| 8 | Edit relationship | Click relationship entry's edit action | Inline edit or modal for type/description change | Fields become editable. Save/cancel. | |
| 9 | Remove relationship | Click relationship entry's remove action | Confirmation prompt → relationship removed | "Remove this link?" confirmation. Entry fades out on confirm. | Low stakes but still confirm to prevent accidental removal. |
| 10 | Edit metadata | Click "Edit metadata" in project info | Inline form for name, description, tags | Fields become editable. Save/cancel buttons appear. | |
| 11 | Remove project | Click "Remove project" | Confirmation dialog: "Unregister [project]? This does not delete files." | Dialog with confirm/cancel. On confirm, sidebar updates, main area shows empty or next project. | |
| 12 | Navigate to related project | Click project name in relationship | Sidebar selection changes to target project. V-02 loads. | Sidebar highlight moves. Main area transitions. | |
| 13 | Keyboard: scroll sections | Arrow Up/Down when content focused, or J/K | Scroll through content sections | Subtle section focus indicator. | |
| 14 | Auto-detect new session | File watcher event | New session appears at top of latest session area | Highlight animation on new session card. Optional notification badge in sidebar. | Automatic, no user trigger. |

### State Variations

**Empty — no project selected**
- Visual: Main area shows centered message: "Select a project from the sidebar to view its details." Subtle icon or illustration.
- Actions: None — user must select from sidebar.

**Empty — project has no sessions**
- Visual: Latest Session area shows: "No sessions yet. Sessions are created via the Claude Code handoff skill." with brief explanation of the workflow.
- Other sections (git activity, metadata) still display normally.

**Loading — project data loading**
- Visual: Project header renders immediately (name from sidebar selection). Content area shows skeleton: session card skeleton, commit list skeleton, relationship placeholders.
- Duration: <100ms expected. Skeleton threshold: 200ms.

**Error — project data failed to load**
- Visual: Header shows project name. Content area shows: "Could not load project data" with specific error (path not found, git error, etc.) and suggested action.
- Recovery: "Retry" button. "Check path" link for filesystem errors. For git errors, gracefully show what IS available (metadata without git data).

**Populated — minimal (new project, no sessions, few commits)**
- Visual: Latest Session shows empty state. Git activity shows the few commits that exist. Relationships section shows "No relationships" with add button. Layout doesn't feel broken — sections have consistent vertical rhythm.

**Populated — typical**
- Visual: Full layout as diagrammed. Latest session expanded, 10-20 commits visible, 2-5 relationships, 5-10 historical sessions.

**Populated — maximum (hundreds of sessions, thousands of commits)**
- Visual: Session history section uses virtual scrolling or "load more" pagination. Git commits show 20 recent with "View all" expansion. Performance stays acceptable.

**Selected — session expanded**
- Visual: Expanded session card has distinct background or border to show it's the active/expanded content. Collapse action visible.

---

## V-03: Document Viewer (Main Area — Files Tab)

### Purpose

This view lets the user find and read project files by showing a navigable file tree alongside rendered document content, supporting both known-location navigation and exploratory browsing.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| V-02: Project Overview | Click "Files" tab | Project ID. If a file was previously selected in this project, restore it (context preservation). |
| V-04: Search Overlay | Click document search result | Project ID + file path → auto-select file in tree, render content. |
| V-01: Project List | Select project when Files tab was last active | Project ID + preserved file tree state + selected file. |

Most common entry: tab switch from V-02 (~60%) or direct navigation from sidebar with preserved state (~30%). Context preservation is critical — the user switches to Files, reads a doc, switches back to Overview, and expects Files to show the same doc when they return.

### Layout Structure

```
┌─────────────┬───────────────────────────────────┐
│ ▸ Overview  │▸ Files                             │ ← Tab bar (fixed)
├─────────────┴───────────────────────────────────┤
│ ┌──────────┬────────────────────────────────────┐│
│ │File Tree │ File Content                       ││
│ │          │                                    ││
│ │ ▾ docs/  │ ┌────────────────────────────────┐ ││
│ │   brief..│ │ phase-3d-architecture.md       │ ││ ← File header (fixed)
│ │   3b-do..│ │ Last modified: 2h ago · 14 KB  │ ││
│ │   3c-jo..│ ├────────────────────────────────┤ ││
│ │  ▸3d-ar..│ │                                │ ││ ← Content (scrolls)
│ │   3e-vi..│ │ # Phase 3D: Information        │ ││
│ │ ▾ src/   │ │ Architecture                   │ ││
│ │   main.rs│ │                                │ ││
│ │   lib.rs │ │ > Defines the structural...    │ ││
│ │ CLAUDE.md│ │                                │ ││
│ │ README.md│ │ ## Step 1: Entity Inventory    │ ││
│ │          │ │ ...                            │ ││
│ │          │ │                                │ ││
│ └──────────┴─┴────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

- **Width**: Main area minus sidebar = ~1000px at 1280px, ~2280px at 2560px.
- **File tree width**: 200px fixed. Sufficient for filenames with indentation. No resize handle needed — filenames that overflow use truncation with full-path tooltip.
- **Content area**: Fills remaining space (~800px at 1280px, ~2080px at 2560px). Readable for markdown and source code.
- **Scroll**: File tree and file content scroll independently. Tab bar stays fixed. File header (name, metadata) stays fixed within the content area.

### Information Hierarchy

**1. Primary — seen immediately**

| Element | Content | Why primary |
|---------|---------|-------------|
| File tree | Directory/file hierarchy | Navigation mechanism — the user's first interaction to find a file. |
| File content (rendered) | Markdown rendered, source highlighted, images displayed | The payload — what the user came to see. |
| Selected file name | Name + path in content header | Confirms "am I reading the right file?" |

**2. Secondary — visible on focused scanning**

| Element | Content | Why secondary |
|---------|---------|---------------|
| File type icons | In tree — markdown, code, image, config icons | Aids scanning but not critical for identification. |
| Directory expand/collapse state | Open/closed folders | Structural — supports navigation but is secondary to file content. |

**3. Tertiary — revealed on interaction**

| Element | Content | Revealed by | Why tertiary |
|---------|---------|-------------|--------------|
| Last modified date | Relative date per file | Shown in content header | Rarely needed for the "find and read" task. |
| File size | KB/MB | Shown in content header | Rarely needed. |
| Full file path | Absolute path | Hover on file name | For disambiguation only. |

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Select file | Click file in tree | Content area renders the file | Tree item highlights. Content area transitions (brief crossfade). | Content scroll resets to top for new file. |
| 2 | Expand/collapse directory | Click directory in tree or arrow keys | Children show/hide | Chevron rotates. Smooth height animation. | Right arrow: expand or descend. Left arrow: collapse or ascend to parent. |
| 3 | Switch to Overview tab | Click "Overview" tab | Main area shows V-02 | Tab indicator moves. File tree state and selected file preserved. | |
| 4 | Keyboard: navigate tree | Arrow Up/Down | Move focus between tree items | Focus ring on tree items | Standard tree keyboard pattern. See `patterns/interaction/keyboard-navigation.md`. |
| 5 | Keyboard: quick search | Type characters when tree focused | Type-ahead: jump to first matching file | Brief highlight on matched item | Standard tree type-ahead behavior. |
| 6 | Keyboard: focus content | Tab (when tree focused) | Focus moves to content area | Content area gets focus ring | Allows keyboard scrolling of content. |
| 7 | Click link in rendered markdown | Click | Navigate to linked file if internal, or open in browser if external | Internal: auto-select in tree, render content. External: system browser opens. | Internal links resolved relative to project root. |

### State Variations

**Empty — no file selected**
- Visual: File tree shows, content area shows: "Select a file from the tree to view its contents." Centered, muted text.
- Auto-select: Consider auto-selecting README.md or CLAUDE.md if present. Design decision for implementation.

**Empty — project has no viewable files**
- Visual: File tree shows: "No viewable files. Check ignore patterns in Settings." Content area shows same message.

**Loading — file tree loading**
- Visual: Tree area shows skeleton (indented lines). Content area shows empty-selection message.
- Duration: <100ms expected for tree. Skeleton threshold: 200ms.

**Loading — file content loading**
- Visual: Tree shows normally, selected file highlighted. Content area shows skeleton matching expected content shape (heading block, paragraph blocks).
- Duration: <50ms for most files. Large files (>1MB source) may take 50-200ms — skeleton appropriate.

**Error — file not found**
- Visual: Content area shows: "File not found: [path]. It may have been deleted or moved." Tree may show the file grayed out or removed after refresh.

**Error — file cannot be rendered**
- Visual: Content area shows raw content with warning: "This file type cannot be rendered. Showing raw content."
- Binary files: "Binary file ([type], [size]). Cannot display content."

**Populated — typical**
- Visual: Tree shows project files. Selected file rendered in content area. Standard layout.

**Populated — maximum (large codebase, thousands of files)**
- Visual: Tree with many directories. Collapsed by default — only top-level items visible. User expands as needed. Deep nesting (>4 levels) truncates paths. Search (V-04) becomes the primary way to find specific files.

**Filtered — N/A**
- The file tree is not filterable in V-03. Global search (V-04) handles file finding. If implementation reveals a need for tree filtering, revisit.

---

## V-04: Search Overlay (Command Palette)

### Purpose

This view lets the user find any information across all projects by showing a focused search input with real-time results grouped by type and project, accessible from any view via keyboard shortcut.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| Any view | Cmd+K keyboard shortcut | None — fresh search input. Underlying view remains visible (dimmed). |
| Any view | Click search icon (if shown in sidebar header) | None — same as Cmd+K. |

100% keyboard-triggered entry in practice. The overlay appears centered in the viewport, over the current view.

### Layout Structure

```
┌─────────────────────────────────────────────────┐
│                                                 │
│         ┌───────────────────────────┐           │
│         │ 🔍 Search taurhaus...     │           │ ← Input (auto-focused)
│         ├───────────────────────────┤           │
│         │                           │           │
│         │ DOCUMENTS                 │           │ ← Results by type
│         │ ┌───────────────────────┐ │           │
│         │ │ taurhaus › docs/      │ │           │
│         │ │ phase-3d-archite...   │ │           │
│         │ │ ...entity inventory...│ │           │ ← Snippet with highlight
│         │ ├───────────────────────┤ │           │
│         │ │ taurui › patterns/    │ │           │
│         │ │ master-detail.md      │ │           │
│         │ │ ...split the viewp... │ │           │
│         │ └───────────────────────┘ │           │
│         │                           │           │
│         │ SESSIONS                  │           │
│         │ ┌───────────────────────┐ │           │
│         │ │ taurhaus  2026-02-16  │ │           │
│         │ │ ...Phase 3D info...   │ │           │
│         │ └───────────────────────┘ │           │
│         │                           │           │
│         │ COMMITS                   │           │
│         │ ┌───────────────────────┐ │           │
│         │ │ taurhaus  a1b2c3      │ │           │
│         │ │ Add phase-3d-arch...  │ │           │
│         │ └───────────────────────┘ │           │
│         │                           │           │
│         │ ↑↓ Navigate  ↵ Open  esc │ │          │ ← Hints (fixed bottom)
│         └───────────────────────────┘           │
│                                                 │
│            (dimmed background)                  │
└─────────────────────────────────────────────────┘
```

- **Overlay width**: 600px centered. Fixed width regardless of viewport.
- **Overlay height**: Dynamic — grows with results, max ~60% of viewport height. Scrolls internally if results exceed.
- **Background**: Dimmed overlay behind the palette. Click outside dismisses.

### Information Hierarchy

**1. Primary — seen immediately**

| Element | Content | Why primary |
|---------|---------|-------------|
| Search input | Text input, auto-focused, placeholder | The entry point — user's first interaction. |
| Result items | Per result: project + path + snippet | The payload — answers "where is the thing I'm looking for?" |

**2. Secondary — visible on focused scanning**

| Element | Content | Why secondary |
|---------|---------|---------------|
| Type group headers | "Documents", "Sessions", "Commits" | Structural — helps user scan by category. |
| Project name per result | Which project this result belongs to | Context for the result — needed to judge relevance. |
| Matching snippet | Content excerpt with highlighted terms | Preview — helps confirm this is the right result without clicking. |

**3. Tertiary — revealed on interaction**

| Element | Content | Revealed by | Why tertiary |
|---------|---------|-------------|--------------|
| Result count | "X results" | Visible at bottom | Informational, not actionable. |
| Keyboard hints | Navigation shortcuts | Visible at bottom | Only for first-time users; regulars know the shortcuts. |

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Open search | Cmd+K | Overlay appears, input focused | Overlay slides in or fades in (fast, ~150ms). Background dims. | Also works as toggle: Cmd+K while open dismisses. |
| 2 | Type query | Type in input | Results update in real-time (debounced ~100ms) | Results stream in below input. Previous results replaced. | Searches docs, sessions, commits, code across all projects. |
| 3 | Navigate results | Arrow Up/Down | Highlight moves between results | Highlighted result has accent background. | Arrow Down from input moves to first result. Arrow Up from first result moves to input. |
| 4 | Select result | Enter on highlighted result / Click result | Navigate to target: document → V-03, session → V-02 (session expanded), commit → V-02 (commit section). Overlay dismisses. | Overlay fades out. Target view loads with result visible/highlighted. | If result is in a different project, sidebar selection changes too. |
| 5 | Dismiss search | Escape / Click outside overlay | Overlay dismisses. Previous view state restored. | Overlay fades out. Background undims. | No state change — the search was cancelled. |
| 6 | Clear input | Click × button in input / Ctrl+A + Backspace | Input clears, results clear | Input empty, ready for new query. | |
| 7 | Keyboard: jump to type | Ctrl+D / Ctrl+S / Ctrl+C (TBD) | Filter results to Documents / Sessions / Commits only | Type header highlights. Other types hidden. | Optional — implement if result volume justifies filtering. |

### State Variations

**Fresh open (no previous query)**
- Visual: Input focused with placeholder "Search across all projects...". No results section. Optionally show recent searches or "frequently accessed" items.

**Reopened (previous query exists)**
- Visual: Input shows previous query text, fully selected (typing replaces it). Previous results are visible immediately. User can Arrow Down to try another result, or start typing to begin a fresh search. Pressing Escape clears the query first, then dismisses on second press.

**Loading — query entered, results loading**
- Visual: Input shows query text. Below input: subtle inline spinner or "Searching..." indicator. No skeleton — results should arrive in <200ms.
- Duration: <200ms expected (local Rust-powered index). If exceeds 500ms, show explicit loading indicator.

**Error — search index unavailable**
- Visual: Below input: "Search index is rebuilding. Please try again in a moment." or "Search unavailable — [error]" with retry hint.
- Recovery: Automatic — index rebuilds in background.

**Populated — typical results**
- Visual: Results grouped into 3 categories: Documents (all file types — markdown, source code, config), Sessions, and Commits. 5-20 results shown. Each with project name, path/context, snippet.

**Populated — many results**
- Visual: Results list scrolls within overlay. Max ~15-20 visible, rest scrollable. Results ranked by relevance — most relevant at top.
- Consideration: "Show all X results" link at bottom if results are truncated.

**No results**
- Visual: Below input: "No results for '[query]'" in muted text. Optionally suggest: "Try different keywords or check spelling."
- Distinct from empty: the input has text but nothing matched.

---

## V-06: First-Run Setup

### Purpose

This view lets the user onboard and populate taurhaus on first launch by scanning a directory for git repositories, selecting which to register, and building the initial search index.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| Application launch | Automatic — no projects in registry | None |

Single entry point. Shown once, ever. Takes over the entire viewport — sidebar and main area are replaced by the setup flow.

### Layout Structure

Step-by-step flow. Each step replaces the previous.

**Step 1: Welcome**
```
┌──────────────────────────────────────────────────┐
│                                                  │
│                  taurhaus                        │
│        The house where all your projects live.   │
│                                                  │
│    taurhaus gives you a single view into all     │
│    your AI-driven projects — their code, docs,   │
│    progress, and session history.                │
│                                                  │
│    Let's find your projects.                     │
│                                                  │
│         [ Scan ~/projects/ ]                     │ ← Primary action
│                                                  │
│    Or add a project manually                     │ ← Secondary link
│                                                  │
└──────────────────────────────────────────────────┘
```

**Step 2: Scan results / project selection**
```
┌──────────────────────────────────────────────────┐
│                                                  │
│  Found 23 git repositories in ~/projects/        │
│                                                  │
│  ┌──────────────────────────────────────────┐    │
│  │ ☑ taurhaus     ~/projects/taurhaus       │    │
│  │ ☑ taurui       ~/projects/taurui         │    │
│  │ ☑ taursec      ~/projects/taursec        │    │
│  │ ☑ MIR          ~/projects/missing_inv... │    │
│  │ ☑ taursult     ~/projects/taursult       │    │
│  │ ☐ old-project  ~/projects/old-project    │    │ ← User deselected
│  │ ☑ ...                                    │    │
│  └──────────────────────────────────────────┘    │
│                                                  │
│  [Select all]  [Deselect all]   22 selected      │
│                                                  │
│         [ Register selected ]                    │
│                                                  │
│  ← Change scan directory                         │
│                                                  │
└──────────────────────────────────────────────────┘
```

**Step 3: Indexing progress**
```
┌──────────────────────────────────────────────────┐
│                                                  │
│  Setting up taurhaus...                          │
│                                                  │
│  ████████████░░░░░░░░  14 / 22 projects          │
│                                                  │
│  Indexing: taursult                              │
│                                                  │
└──────────────────────────────────────────────────┘
```

**Step 4: Complete → transitions to V-01 + V-02**

- **Width**: Centered content, max 600px wide. Generous whitespace.
- **Scroll**: Step 2 (project list) scrolls if many projects found.
- **Progression**: Steps advance forward only. User can go back to change scan directory.

### Information Hierarchy

Step 1:
- Primary: Application tagline + "Scan" action.
- Secondary: Brief description of what taurhaus does.
- Tertiary: "Add manually" alternative.

Step 2:
- Primary: Project list with checkboxes + "Register" action.
- Secondary: Project count, select/deselect all.
- Tertiary: Change scan directory link.

Step 3:
- Primary: Progress bar + current project name.
- Secondary: Progress count (X / Y).

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Start scan | Click "Scan ~/projects/" | Scan begins, transitions to step 2 when done | Brief scanning indicator if >1s. Step 2 renders when scan completes. | Default path: ~/projects/. User can change before scanning. |
| 2 | Change scan directory | Click "Change scan directory" | Path input appears or system directory picker | Input field replaces default path. | |
| 3 | Toggle project | Click checkbox | Project selected/deselected | Checkbox toggles. Selected count updates. | |
| 4 | Select all / Deselect all | Click action | All checkboxes toggled | All items update. Count updates. | |
| 5 | Register selected | Click "Register selected" | Indexing begins (step 3) | Transition to progress view. Button disabled during indexing. | |
| 6 | Skip / cancel | Close window or Escape | Setup cancelled. Next launch shows setup again. | No confirmation needed — nothing was committed yet. | |

### State Variations

**Empty — scan finds no repositories**
- Visual: "No git repositories found in ~/projects/." Suggestions: "Check the scan directory" or "Add a project manually."

**Loading — scanning directory**
- Visual: "Scanning ~/projects/..." with spinner. Should complete in 1-5 seconds.

**Loading — indexing projects**
- Visual: Progress bar + current project name + count. 2-30 seconds depending on project count/size.

**Error — scan failed**
- Visual: "Could not scan ~/projects/: [permission denied / path not found]." Suggested action: "Check the path and try again."

**Error — indexing partially failed**
- Visual: Progress bar completes. Summary: "21 of 22 projects registered. 1 failed: [project — reason]." "Continue to dashboard" button available — don't block on partial failure.

---

## V-05: Settings

### Purpose

This view lets the user configure taurhaus preferences by showing organized sections for project scanning, display options, and index management.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| V-01: Project List | Click settings icon in sidebar footer | None — settings are global, not project-specific |

Single entry point. Low frequency (Tier 3).

### Layout Structure

```
┌──────────────────────────────────────────────────┐
│ ← Back to projects              Settings         │ ← Header (fixed)
├──────────────────────────────────────────────────┤
│                                                  │
│ PROJECT SCANNING                                 │ ← Section
│ ┌──────────────────────────────────────────────┐ │
│ │ Scan directories                             │ │
│ │ ~/projects/                          [Edit]  │ │
│ │                                              │ │
│ │ Global ignore patterns                       │ │
│ │ node_modules, .git, target, dist     [Edit]  │ │
│ │                                              │ │
│ │ [Rescan all projects]                        │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ DISPLAY                                          │ ← Section
│ ┌──────────────────────────────────────────────┐ │
│ │ Activity thresholds                          │ │
│ │ Active: < 7 days    Recent: < 30 days        │ │
│ │ Stale: < 90 days    Dormant: 90+ days        │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ INDEX                                            │ ← Section
│ ┌──────────────────────────────────────────────┐ │
│ │ Status: Healthy · 47 projects · 12,403 files │ │
│ │ Last rebuilt: 2 hours ago                    │ │
│ │                                              │ │
│ │ [Rebuild index]                              │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
└──────────────────────────────────────────────────┘
```

- **Width**: Full main area. Content centered, max 640px wide for readability.
- **Scroll**: Scrollable if sections exceed viewport. Header fixed.
- **Sidebar**: Sidebar remains visible. Project selection preserved for easy return.

### Information Hierarchy

**1. Primary — seen immediately**

| Element | Content | Why primary |
|---------|---------|-------------|
| Section headers | "Project Scanning", "Display", "Index" | Orientation — user finds the section they came to change. |
| Current values | Scan dirs, ignore patterns, thresholds | User needs to see current state before editing. |

**2. Secondary — visible on focused scanning**

| Element | Content | Why secondary |
|---------|---------|---------------|
| Edit actions | Per-field edit buttons | Available but not dominant — most visits are read-only checks. |
| Index status | Health, counts, last rebuild | Informational — user glances but rarely acts. |

**3. Tertiary — revealed on interaction**

| Element | Content | Revealed by | Why tertiary |
|---------|---------|-------------|--------------|
| Rescan action | Trigger full project rescan | Scroll to scanning section | Rare action. |
| Rebuild index | Trigger full index rebuild | Scroll to index section | Rare action. |

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Return to projects | Click "← Back to projects" or click project in sidebar | Main area returns to V-02 for last-selected project | Tab bar reappears. Settings close. | Project selection preserved. |
| 2 | Edit scan directories | Click edit on scan dirs | Inline editor for directory list | Fields become editable. Save/cancel. | |
| 3 | Edit ignore patterns | Click edit on patterns | Inline editor for pattern list | Fields become editable. Save/cancel. | |
| 4 | Rescan projects | Click "Rescan all projects" | Background scan begins | Button shows spinner. "Scanning..." status. When done: "Scan complete. X new projects found." | Non-blocking — user can navigate away. |
| 5 | Rebuild index | Click "Rebuild index" | Background rebuild begins | Button shows spinner. "Rebuilding..." status with progress. | Non-blocking. |
| 6 | Edit thresholds | Click/edit threshold values | Values update | Inline editing with number inputs. Changes apply immediately. | |

### State Variations

**Loading**: Settings load from local config — effectively instant (<10ms). No loading state needed.

**Error**: Settings file corrupt or unreadable — show defaults with warning: "Could not load settings. Using defaults." Save action available to write fresh config.

**Normal**: All settings displayed with current values. Standard layout.

**Rescan in progress**: "Rescan" button shows spinner. Status text updates: "Scanning... found 3 new projects."

**Rebuild in progress**: "Rebuild" button shows spinner. Progress indicator: "Rebuilding index: 23 / 47 projects."

---

## Project Registration Modal

### Purpose

This modal lets the user add a new project to taurhaus by specifying a path and confirming auto-detected metadata.

### Entry Points

| From | Trigger | Context Carried |
|------|---------|-----------------|
| V-01: Project List | Click "+" button | None |
| V-06: First-Run | "Add manually" link | None |

### Layout Structure

```
┌────────────────────────────────────────┐
│ Register Project                    ✕  │ ← Modal header
├────────────────────────────────────────┤
│                                        │
│ Project path                           │
│ ┌────────────────────────────────┐     │
│ │ ~/projects/                    │ [📁]│ ← Path input + browse
│ └────────────────────────────────┘     │
│                                        │
│ ── Auto-detected ──────────────────    │
│                                        │
│ Name:  taurmolt                        │ ← Editable
│ Description: (from README or empty)    │ ← Editable
│ Tags:  [ + Add tag ]                   │ ← Editable
│                                        │
│               [Cancel]  [Register]     │ ← Actions
│                                        │
└────────────────────────────────────────┘
```

- **Width**: 480px centered overlay.
- **Behavior**: Modal overlay with dimmed background. Escape or ✕ to dismiss.

### Interaction Inventory

| # | Action | Trigger | Result | Feedback | Notes |
|---|--------|---------|--------|----------|-------|
| 1 | Enter path | Type or paste in path input | Auto-detection triggers when path resolves to a git repo | Name, description auto-populate. "Not a git repo" error if invalid. | Autocomplete on ~/projects/ paths. |
| 2 | Browse filesystem | Click browse button (📁) | System file picker opens | Selected path fills input. Auto-detection triggers. | |
| 3 | Edit metadata | Modify name, description, tags | Fields update | Inline editing. | |
| 4 | Register | Click "Register" | Project added to registry and sidebar | Modal closes. Sidebar updates with new project. Indexing begins in background. | Button disabled until path is valid. |
| 5 | Cancel | Click "Cancel" or Escape or ✕ | Modal closes, no changes | Modal dismisses. | |

### State Variations

**Empty**: Path input empty. Auto-detected fields hidden. Register button disabled.
**Path entered — valid**: Auto-detected fields shown. Register button enabled.
**Path entered — invalid**: Error below input: "Path not found" or "Not a git repository." Register button disabled.
**Already registered**: Warning: "This project is already registered." Register button disabled.

---

## Cross-View Patterns

### Context Preservation

All view states are preserved in memory during a session:
- **V-01**: Selected project, filter text, sort order, collapsed groups.
- **V-02**: Scroll position, expanded sessions, expanded commits.
- **V-03**: File tree expand/collapse state, selected file, content scroll position.
- **V-04**: Last query preserved in-memory for the session. Reopening shows previous query (pre-selected, so typing replaces it) with results. Resets on app restart.
- **V-05**: No state to preserve (values are persisted settings).

On app restart, restore: last selected project, last active tab (Overview/Files).

### Keyboard Shortcuts Summary

| Shortcut | Action | Available from |
|----------|--------|---------------|
| Cmd+K | Open search (V-04) | Anywhere |
| Escape | Dismiss overlay / deselect | V-04, modals |
| Arrow Up/Down | Navigate list items | V-01 (projects), V-03 (file tree), V-04 (results) |
| Enter | Select/open focused item | V-01, V-03, V-04 |
| Tab | Move focus between regions | All views |
| / or Cmd+F | Focus sidebar filter | V-01 |
| J/K | Next/previous item | V-02 (sections), V-04 (results) — optional |

### Shared Component Usage Map

| Component | V-01 | V-02 | V-03 | V-04 | V-05 | V-06 |
|-----------|:----:|:----:|:----:|:----:|:----:|:----:|
| SC-01: Project List Item | ✓ | | | ✓ | | ✓ |
| SC-02: Session Card | | ✓ | | ✓ | | |
| SC-03: Commit Entry | | ✓ | | ✓ | | |
| SC-04: File Tree | | | ✓ | | | |
| SC-05: Document Renderer | | | ✓ | | | |
| SC-06: Search Input | ✓ | | | ✓ | | |
| SC-07: Activity State Badge | ✓ | ✓ | | | | |
| SC-08: Empty State | ✓ | ✓ | ✓ | ✓ | | ✓ |
| SC-09: Relationship Entry | | ✓ | | | | |

### Deferred: Cross-Project Chronological View

J-01 (Orient) mentions a need to scan "what happened recently across everything?" V-01's sort-by-activity partially addresses this by surfacing recently-active projects at the top of the list. A dedicated cross-project activity feed (interleaving commits and sessions across all projects in reverse chronological order) was considered but deferred to v1.1. The current sort-by-activity + per-project drill-down covers the v1 orient flow; a chronological feed would be additive, not blocking.

---

## Handoff to Phase 3F

This document provides the inputs for Visual System design:

- **6 view specs + 1 modal** with complete layout structures → determines the component inventory that needs styling
- **Information hierarchy per view** → drives type scale decisions (primary = larger/bolder, secondary = smaller/lighter, tertiary = muted)
- **Shared components** (SC-01 through SC-09) → each needs visual definition (colors, spacing, borders, typography)
- **State variations per view** → defines the full range of visual states that tokens must support (empty, loading, error, selected, hover, focus)
- **Layout proportions** (240px sidebar, 200px file tree, etc.) → spacing system must accommodate these structural dimensions
- **Activity state indicators** (active/recent/stale/dormant) → color system must encode these states
- **Keyboard shortcuts** → focus ring styling, active state styling

**Key visual decisions for 3F:**
1. **Activity state colors**: 4 distinct colors for Active/Recent/Stale/Dormant. Must be distinguishable by shape/icon too, not color alone.
2. **Sidebar density**: At 240px width and 30-50 projects, each list item needs to be ~40-48px tall. Typography must be legible at compact sizes.
3. **Session card prominence**: The latest session must visually dominate V-02's scrollable content. Needs distinct surface treatment (background, border, or card elevation).
4. **Tab bar or segmented control**: Only 2 tabs (Overview, Files). Full tab bar may be over-engineered — consider a segmented control or simple toggle.
5. **Search overlay styling**: Must feel fast and lightweight. Minimal chrome, fast transitions.
