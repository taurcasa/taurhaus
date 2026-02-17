# Phase 3D: Information Architecture

> Defines the structural skeleton of the taurhaus interface: what views exist, how the user moves between them, and what information belongs where. Built from the [Journey Maps](phase-3c-journeys.md) and [Domain Understanding](phase-3b-domain.md).

---

## Step 1: Entity Inventory (Cross-Referenced with Journeys)

Entity profiles from Phase 3B, now annotated with journey references from Phase 3C.

### E-01: Project

**Display name**: Project name (directory name by default, user-overridable).
**Key properties**: Name, working tree status, current branch, last activity date, description, tags, path, relationships.
**States**: Active, Recent, Stale, Dormant (activity-derived).
**Volume**: 30-50 typical, 100+ max.
**Journeys**: J-01 (orient — primary entity), J-02 (resume — container for sessions/docs), J-03 (reference — selected project context), J-04 (search — result grouping), J-05 (end session — target for handoff), J-06 (register — created), J-07 (relationships — linked), J-09 (first-run — batch registered).

### E-02: Session / Handoff

**Display name**: Date + project name.
**Key properties**: Summary, next steps, open questions, date, decisions, user notes, session ID.
**States**: Created, Imported, Current, Historical.
**Volume**: 5-20 per active project, hundreds historical.
**Journeys**: J-02 (resume — read latest session), J-04 (search — searchable content), J-05 (end session — created and verified).

### E-03: Document

**Display name**: File name with optional relative path.
**Key properties**: File name, file type, relative path, last modified, size, content.
**States**: Present, Modified (transient), Deleted.
**Volume**: 20 (small KB) to thousands (large codebase).
**Journeys**: J-02 (resume — review key docs), J-03 (reference — browse and read), J-04 (search — searchable content).

### E-04: Relationship

**Display name**: "Source → Target (type)".
**Key properties**: Type, source project, target project, description.
**States**: Active, Removed.
**Volume**: 2-5 per project, 20-80 total.
**Journeys**: J-01 (orient — relationship indicators), J-07 (manage — CRUD).

### Implicit Entities

- **Git Commit**: Displayed within project detail. Journeys: J-01 (activity signal), J-02 (recent changes), J-04 (searchable).
- **Git Branch**: Property of Project. Journey: J-01 (dashboard indicator), J-02 (context).
- **Settings**: Journey: J-08. Not an entity in the traditional sense — global preferences.

**No orphaned entities.** Every entity appears in at least one journey. No phantom entities identified.

---

## Step 2: View Inventory

Views derived by walking every journey step and asking: "What is the user looking at?"

### V-01: Project List (Sidebar)

**Purpose**: This view lets the user browse all registered projects and select one for focus by showing project names with activity states, working tree status, and recency indicators in a scannable list.

**Journeys served**: J-01 (orient — scan projects), J-06 (register — add project), J-09 (first-run — populated after scan).
**Primary entities**: E-01: Project.
**User arrives from**: Application launch (always visible).
**User navigates to**: V-02 (select project), V-03 (switch to Files tab), V-06 (click settings), V-05 (Cmd+K search).

**Notes**: Always visible as the left sidebar. Independent scroll. Contains list search/filter and sort controls. "Add project" action accessible here. At 1280px total width, the sidebar is ~240-280px; at 2560px, it can be slightly wider or stay fixed.

### V-02: Project Overview (Main Area — Default Tab)

**Purpose**: This view lets the user assess a project's current state and resume context by showing the most recent session handoff, recent git activity, project metadata, and relationships.

**Journeys served**: J-01 (orient — drill into project), J-02 (resume — read session, check commits, review docs), J-05 (end session — verify handoff imported, add notes), J-07 (manage relationships).
**Primary entities**: E-01: Project, E-02: Session (latest), Git Commit (recent).
**User arrives from**: V-01 (select project), V-05 (search result click), V-03 (switch tab back).
**User navigates to**: V-03 (switch to Files tab), V-05 (Cmd+K search), session detail (expand session in-place), full git history (expand in-place).

**Notes**: Default content when a project is selected. The most recent session is the highest-value content — displayed prominently, not buried. Session detail (full session content, notes, enrichment) is an expanded state within this view, not a separate navigation target. Full git history expands or scrolls within this view.

### V-03: Document Viewer (Main Area — Files Tab)

**Purpose**: This view lets the user find and read project files by showing a navigable file tree alongside rendered document content.

**Journeys served**: J-02 (resume — review project docs), J-03 (reference — browse and read files).
**Primary entities**: E-03: Document.
**User arrives from**: V-02 (switch to Files tab), V-05 (search result click on a document), V-01 (direct navigation if last tab was Files).
**User navigates to**: V-02 (switch to Overview tab), V-05 (Cmd+K search), V-01 (select different project).

**Notes**: Split layout within the main area: file tree on the left (~200-240px), rendered content on the right. Markdown is rendered, source is syntax-highlighted, images are displayed. Context-preserving: switching to V-02 and back restores file tree position and selected file.

### V-04: Search Overlay (Command Palette)

**Purpose**: This view lets the user find any information across all projects by showing unified search results with project context, entity type, and matching snippets.

**Journeys served**: J-03 (reference — search for a file), J-04 (search — primary view for cross-project search).
**Primary entities**: E-01: Project (result grouping), E-02: Session (searchable), E-03: Document (searchable), Git Commit (searchable).
**User arrives from**: Any view via Cmd+K keyboard shortcut.
**User navigates to**: V-02 (click project/session result), V-03 (click document result), dismissed (Escape key).

**Notes**: Overlay, not a full view replacement. The underlying view remains visible (dimmed). Results appear as the user types (debounced, <200ms). Each result shows: project name, entity type badge, file path or entity context, matching snippet with highlighted terms. Clicking a result dismisses the overlay and navigates to the appropriate view with the match visible.

### V-05: Settings

**Purpose**: This view lets the user configure taurhaus preferences by showing organized sections for scan directories, ignore patterns, and display options.

**Journeys served**: J-08 (configure settings).
**Primary entities**: Settings (implicit entity).
**User arrives from**: V-01 (settings link in sidebar, bottom).
**User navigates to**: V-01 (back to project list — click any project or back action).

**Notes**: Replaces the main area content. Project list sidebar may remain visible or be replaced — design decision for 3E. Sectioned form layout. Rarely visited (Tier 3).

### V-06: First-Run Setup

**Purpose**: This view lets the user onboard by scanning a directory for git repositories, selecting which to register, and building the initial index.

**Journeys served**: J-09 (first-run setup).
**Primary entities**: E-01: Project (being discovered and registered).
**User arrives from**: Application launch (when no projects registered).
**User navigates to**: V-01 + V-02 (dashboard, after setup completes).

**Notes**: One-time experience. Replaces the entire UI until complete. Steps: welcome → scan directory → review discovered projects → index → done. Progress indicator during indexing (2-10s). After completion, transitions to the populated dashboard and never shows again.

### View Necessity Validation

| View | In a journey? | Distinct purpose? | State of another view? | Navigation target? | Verdict |
|------|:---:|:---:|:---:|:---:|---------|
| V-01: Project List | Yes (J-01, J-06, J-09) | Yes (browse/select) | No | Yes (always visible) | Keep |
| V-02: Project Overview | Yes (J-01, J-02, J-05, J-07) | Yes (assess/resume) | No | Yes (click project) | Keep |
| V-03: Document Viewer | Yes (J-02, J-03) | Yes (find/read files) | No — different content from V-02 | Yes (tab switch) | Keep |
| V-04: Search Overlay | Yes (J-03, J-04) | Yes (find across projects) | No | Yes (Cmd+K) | Keep |
| V-05: Settings | Yes (J-08) | Yes (configure) | No | Yes (sidebar link) | Keep |
| V-06: First-Run | Yes (J-09) | Yes (onboard) | No | Conditional (no projects) | Keep |

**Considered and rejected:**
- **Separate Session View**: Full session detail is an expanded state within V-02, not a separate view. The user always arrives via V-02 and returns to V-02. Session history is a scrollable/expandable section.
- **Separate Git History View**: Git commits are shown within V-02 (recent) with expand-to-full. A separate tab for git history would not be served by any distinct journey — it's always in service of J-02 (resume context).
- **Relationship Graph View**: At 20-80 total relationships with sparse connectivity, a standalone graph view is premature. Relationships are displayed as a section in V-02. Can be reconsidered if volume grows.

---

## Step 3: Navigation Model

### 3a: Navigation Tiers

**Tier 1 — Always visible / always accessible:**
- **V-01: Project List** — sidebar, visible from every state. 100% of sessions use it. Primary journey J-01 lives here.
- **V-04: Search Overlay** — Cmd+K from any view. Used in primary (J-03) and secondary (J-04) journeys. High frequency.

**Tier 2 — One action from Tier 1:**
- **V-02: Project Overview** — click a project in V-01. Default content when a project is selected. Serves 4 journeys including the top 2 primary journeys (J-01, J-02).
- **V-03: Document Viewer** — tab switch from V-02 or search result click. Serves the highest-frequency journey (J-03).

**Tier 3 — Contextual access:**
- **V-05: Settings** — sidebar link (bottom). Rarely visited.
- **V-06: First-Run Setup** — automatic, one-time.
- **Project Registration (modal)** — action button on V-01. Monthly frequency.
- **Session Detail (expanded)** — click session within V-02. Part of J-02 flow.

### 3b: Navigation Paths (Mapped to Journeys)

**J-01 (Orient Across Projects):**
```
V-01 (scan list) → V-01 (filter/sort) → V-02 (select project)
Steps: 1 action to reach project detail.
```

**J-02 (Resume Project Context):**
```
V-01 → V-02 (select project, read session) → V-02 (expand session, scroll commits) → optionally V-03 (check docs)
Steps: 1 action to project, 1 more to files. Max 2 actions.
```

**J-03 (Reference Docs Mid-Session):**
```
V-03 (already on Files tab from last use, context preserved) → navigate tree → read file
OR: V-04 (Cmd+K) → type query → click result → V-03
Steps: 0-1 actions (context preserved or search shortcut).
```

**J-04 (Search Across Projects):**
```
Any view → V-04 (Cmd+K) → type query → click result → V-02 or V-03
Steps: 1 action to search, 1 to navigate result. Max 2 actions.
```

**J-05 (End Session):**
```
V-02 (already viewing project) → session auto-appears via file watcher → optionally expand to add notes
Steps: 0 actions (automatic detection).
```

**J-06 (Register New Project):**
```
V-01 → registration modal (click add button) → fill form → confirm
Steps: 1 action to start.
```

**J-07 (Manage Relationships):**
```
V-01 → V-02 (select project) → relationships section → create/edit/remove
Steps: 1-2 actions.
```

**J-08 (Configure Settings):**
```
V-01 → V-05 (click settings link)
Steps: 1 action.
```

**J-09 (First-Run Setup):**
```
V-06 (automatic on first launch) → scan → review → index → V-01 + V-02
Steps: 0 actions to start (automatic).
```

**Primary navigation path** (highest frequency): V-01 → V-02 → V-03. This is the orient → resume → reference flow that happens multiple times daily. Must be frictionless.

**Secondary paths**: V-04 (search) → V-02/V-03 (result navigation). V-02 ↔ V-03 (tab switching).

### 3c: Navigation Pattern Selection

| Condition | Applies? | Pattern |
|-----------|:---:|---------|
| 3-7 Tier 1 views, equal importance | No (2 Tier 1 items) | — |
| Deep entity hierarchies | No (Project → Sessions/Docs is only 2 levels) | — |
| Users frequently return to previous context | **Yes** | Context Preservation |
| 1-2 Tier 1 views with Tier 2 sub-views | **Yes** | Hub and spoke |
| Power users with keyboard-centric workflows | **Yes** | Command palette + shortcuts |

**Selected pattern: Sidebar + Content Tabs + Command Palette**

This is a combined pattern:

1. **Sidebar** (V-01): Narrow, persistent project list on the left. Functions as the "hub" of the hub-and-spoke model. Always visible, keyboard-navigable (arrow keys to move between projects, Enter to select).

2. **Content Tabs** (V-02/V-03): Horizontal tabs at the top of the main area — "Overview" and "Files" — for switching between content modes within the selected project. Tabs are persistent and stable (same order, same items for every project). Matches Tower's tab pattern.

3. **Command Palette** (V-04): Cmd+K overlay for global search. The primary cross-project navigation mechanism. Matches VS Code, Linear, Raycast pattern.

4. **Context Preservation**: Switching between projects preserves each project's tab selection, scroll position, selected file, and expanded states. Switching tabs within a project preserves the other tab's state. Critical for J-03 (reference docs) where the round trip must be fast.

**Reference apps for this pattern combination:**
- **Tower** (git client): Sidebar repo list + content tabs (Working Copy, History, Stashes) + search. Closest structural analog to taurhaus.
- **VS Code**: Activity bar + sidebar + editor area. File tree + editor split mirrors V-03.
- **Linear**: Sidebar project/team nav + main area with views. Command palette (Cmd+K) for quick navigation.
- **Obsidian**: File tree sidebar + rendered document viewer. Direct precedent for V-03.

### 3d: Navigation Validation

| Journey | Views traversed | Max actions to reach any view | Dead ends? |
|---------|----------------|:---:|:---:|
| J-01 Orient | V-01 → V-02 | 1 | No — sidebar always visible |
| J-02 Resume | V-01 → V-02 → V-03 | 2 | No — tabs always available |
| J-03 Reference | V-03 (preserved) or V-04 → V-03 | 0-2 | No — sidebar + tabs always available |
| J-04 Search | V-04 → V-02/V-03 | 2 | No — Cmd+K always available, Escape dismisses |
| J-05 End Session | V-02 (automatic) | 0 | No |
| J-06 Register | V-01 → modal | 1 | No — modal has cancel/close |
| J-07 Relationships | V-01 → V-02 → section | 2 | No |
| J-08 Settings | V-01 → V-05 | 1 | No — sidebar visible for return |
| J-09 First-Run | V-06 → V-01 + V-02 | 0 (automatic) | No — forward-only flow to dashboard |

**All primary journeys reach their target views in 1-2 actions.** No view requires more than 2 actions from any starting point. No dead ends — the sidebar is always visible, providing a persistent escape route. The command palette is always one shortcut away.

---

## Step 4: Information Grouping Within Views

### V-01: Project List — Information Groups

**Group 1: Project Identity (Primary)**
- Project name — primary identifier, scanned first
- Activity state indicator — Active/Recent/Stale/Dormant (color or icon, not text)
- Working tree status — clean/dirty (dot or icon)

**Group 2: Project Context (Secondary — visible but not prominent)**
- Current branch name — truncated if long
- Last activity date — relative ("2h ago", "3d ago"), not absolute
- Tags — compact badges if space allows

**Group 3: List Controls (Primary — always accessible)**
- Search/filter input — top of sidebar, filters project list as you type
- Sort toggle — name / last activity / status
- Add project button — "+" icon or action bar

**Volume handling**: 30-50 projects fits in a scrollable list without pagination. At 100+, the search filter becomes the primary navigation within the list. Virtual scrolling if performance degrades. Groups within the list (by activity state: Active → Recent → Stale → Dormant) aid scanning at high volume.

---

### V-02: Project Overview — Information Groups

**Group 1: Project Header (Primary)**
- Project name — large, immediately visible
- Current branch — inline with name or immediately below
- Working tree status — clean/dirty indicator
- Activity state — badge or indicator
- Description — one-liner, below name

**Group 2: Latest Session / Handoff (Primary — highest-value content)**
- Session date — when this session occurred
- Summary — what happened (the key content)
- Next steps — what to do next (actionable, prominent)
- Open questions — unresolved items needing attention
- "View full session" / expand — access decisions, notes, full content
- "Add notes" affordance — for enriching the session (J-05)

**Group 3: Recent Git Activity (Secondary)**
- Last 10-20 commits — message, relative date, abbreviated hash
- "View all commits" expansion — loads full history inline
- Visual indicator if commits exist after latest session (signals divergence)

**Group 4: Relationships (Secondary)**
- Linked projects — name, type, direction arrow
- "Add relationship" action — small, not prominent (low frequency)

**Group 5: Session History (Tertiary — scroll or expand)**
- Older sessions — date and summary snippet per entry
- Click to expand full session content in-place
- Search within sessions (optional, if volume justifies)

**Group 6: Project Metadata (Tertiary — bottom or expandable)**
- Path — filesystem location
- Tags — editable
- "Edit metadata" action
- "Remove project" action (with confirmation)

**Decision support**: J-02's key decision ("Is the plan still valid?") requires Group 2 (latest session) + Group 3 (recent commits) visible together. Both must be primary or secondary — never buried.

---

### V-03: Document Viewer — Information Groups

**Group 1: File Tree (Primary — left sub-panel)**
- Directory structure — expandable/collapsible, indented
- File names with type icons — distinguish markdown, source, image, config
- Selected file indicator — highlighted background
- Scroll position preserved independently

**Group 2: File Content (Primary — right sub-panel)**
- Rendered content — markdown rendered, source syntax-highlighted, images displayed
- File name and relative path — header above content
- Scroll position preserved per file

**Group 3: File Metadata (Tertiary — subtle, inline with header)**
- Last modified date — relative
- File size — for context

**Layout**: Split within the main area. File tree (~200-240px) on the left, rendered content fills the rest. At 1280px total viewport with ~240px sidebar, the main area is ~1040px, leaving ~800px for the content pane — sufficient for readable markdown and source code.

---

### V-04: Search Overlay — Information Groups

**Group 1: Search Input (Primary)**
- Text input — auto-focused when overlay opens
- Placeholder text — "Search across all projects..."
- Clear/dismiss — Escape key or X button

**Group 2: Results List (Primary)**
- Per result: project name, entity type badge (Document, Session, Commit), file path or context, matching snippet with highlighted terms
- Results ranked by relevance
- Keyboard navigation — arrow keys to move, Enter to select

**Group 3: Result Metadata (Secondary — inline per result)**
- Result count — "X results"
- Type grouping or filtering — optional tabs or badges for filtering by type
- Keyboard shortcut hints — at bottom of overlay

**Density**: Results must be scannable. One result ≈ 2-3 lines: title/path on line 1, snippet on line 2, project + type badge inline. At <200ms response time, results stream in as the user types.

---

### V-05: Settings — Information Groups

**Group 1: Project Scanning (Primary)**
- Scan directories — list of watched directories, editable
- Ignore patterns — global patterns (.gitignore supplement)
- "Rescan" action

**Group 2: Display Preferences (Secondary)**
- Theme — light/dark (if supported in v1)
- Activity state thresholds — Active/Recent/Stale/Dormant day boundaries (optional)

**Group 3: Index Management (Tertiary)**
- Index status — last rebuilt, entry count
- "Rebuild index" action — triggers background rebuild

---

### V-06: First-Run Setup — Information Groups

**Group 1: Welcome (Primary — step 1)**
- Application name and tagline
- Brief explanation of what taurhaus does
- Primary action: "Scan for projects"

**Group 2: Scan Configuration (Primary — step 2)**
- Directory input — default ~/projects/, editable
- "Scan" action

**Group 3: Project Selection (Primary — step 3)**
- Discovered projects list — name, path, checkbox for include/exclude
- Select all / deselect all
- Project count — "Found X git repositories"
- "Register selected" action

**Group 4: Indexing Progress (Primary — step 4)**
- Progress indicator — per project or overall percentage
- Current project name being indexed
- Estimated time (if calculable)

**Group 5: Completion (Primary — step 5)**
- Success message — "X projects registered"
- "Go to dashboard" action — transitions to V-01 + V-02

---

## Step 5: Shared Components

Components that appear across multiple views, identified from information grouping.

### SC-01: Project List Item

**Used in**: V-01 (sidebar list), V-04 (search result with project context).

**Always shows**: Project name, activity state indicator, working tree status indicator.

**Variants**:
- **Compact (sidebar)**: Single line or tight two-line. Name + indicators. ~40-48px height. Used in V-01 where 30-50 items must be scannable.
- **Selected (sidebar)**: Same as compact with highlighted background and accent border. Shows the currently active project.
- **Search result context**: Project name as a label/badge on a search result row, not a standalone component.

---

### SC-02: Session Card

**Used in**: V-02 (latest session — expanded), V-02 (session history — compact).

**Always shows**: Date, summary snippet.

**Variants**:
- **Current (expanded)**: Full display of summary, next steps, open questions. "Add notes" affordance. Prominent visual treatment — this is the highest-value content on V-02.
- **Historical (compact)**: Date + one-line summary. Click to expand in-place to full content.

---

### SC-03: Commit Entry

**Used in**: V-02 (recent git activity), V-04 (search results for commits).

**Always shows**: Commit message (first line), relative date.

**Variants**:
- **Compact (list)**: One line — message + date + abbreviated hash. ~32-36px height.
- **Expanded (search result)**: Message + date + hash + file change summary. Used when a commit is a search result with matching snippet.

---

### SC-04: File Tree

**Used in**: V-03 (primary navigation within document viewer).

**Always shows**: Directory/file hierarchy, type icons, expand/collapse controls.

**Behavior**: Independent scroll. Expand/collapse preserves state across tab switches (context preservation). Selected file highlighted.

---

### SC-05: Document Renderer

**Used in**: V-03 (primary content display), V-04 (potential result preview).

**Renders**: Markdown → formatted HTML. Source code → syntax-highlighted. Images → displayed. Binary/unknown → type info + size.

**Behavior**: Scrollable. Independent scroll position per file. Must be readable at ~800px width (1280px viewport minus sidebar minus file tree).

---

### SC-06: Search Input

**Used in**: V-01 (project list filter), V-04 (global search).

**Always shows**: Text input, placeholder, clear action.

**Variants**:
- **Inline filter (V-01)**: Compact, filters the adjacent list. No results display — the list IS the results.
- **Command palette (V-04)**: Centered overlay, auto-focused, results displayed below. Keyboard navigable. Dismiss on Escape.

---

### SC-07: Activity State Badge

**Used in**: V-01 (per project in list), V-02 (project header).

**Shows**: Active / Recent / Stale / Dormant state. Color-coded (design decision for 3F: green → yellow → orange → gray or similar).

**Variants**:
- **Dot (V-01 compact)**: Color dot only, no text. Space-efficient for sidebar.
- **Label (V-02 header)**: Color + text label. More explicit in the detail view.

---

### SC-08: Empty State

**Used in**: V-01 (no projects — redirects to V-06), V-02 (no sessions for project), V-03 (no docs or no file selected), V-04 (no search results).

**Always shows**: Descriptive message, suggested action.

**Variants context-specific**:
- V-01 empty: "No projects registered" → action: "Scan for projects" or "Add a project"
- V-02 no sessions: "No sessions yet" → explanation of how sessions are created
- V-03 no selection: "Select a file from the tree" → or auto-select first file
- V-04 no results: "No results for [query]" → suggestion to broaden search

---

### SC-09: Relationship Entry

**Used in**: V-02 (relationships section).

**Shows**: Direction arrow, target project name (clickable → navigates to that project), relationship type label, optional description.

**Actions**: Edit (change type/description), Remove (with confirmation).

---

## Step 6: Validation

### 1. Journey Coverage

| Journey | Views traversed | All steps covered? |
|---------|----------------|--------------------|
| J-01 Orient | V-01 → V-02 | Yes — V-01 provides browse/scan, V-02 provides detail |
| J-02 Resume | V-01 → V-02 → (V-03) | Yes — V-02 has session + commits, V-03 has docs |
| J-03 Reference | V-03 (or V-04 → V-03) | Yes — V-03 has file tree + renderer, V-04 provides search |
| J-04 Search | V-04 → V-02/V-03 | Yes — V-04 has input + results, navigation to target views |
| J-05 End Session | V-02 | Yes — session auto-appears, add notes available |
| J-06 Register | V-01 → modal | Yes — V-01 has add button, modal has registration form |
| J-07 Relationships | V-02 | Yes — relationships section with CRUD |
| J-08 Settings | V-05 | Yes — settings form with all sections |
| J-09 First-Run | V-06 → V-01 + V-02 | Yes — V-06 covers scan/select/index/transition |

No journey step lacks a corresponding view.

### 2. Entity Coverage

| Entity | Views where it appears |
|--------|----------------------|
| E-01: Project | V-01 (list), V-02 (header), V-04 (result grouping), V-05 (scan config), V-06 (discovery) |
| E-02: Session | V-02 (latest + history), V-04 (search results) |
| E-03: Document | V-03 (tree + content), V-04 (search results) |
| E-04: Relationship | V-02 (relationships section) |
| Git Commit | V-02 (recent activity), V-04 (search results) |
| Settings | V-05 |

No orphaned entities.

### 3. Navigation Completeness

All view-to-view transitions required by journeys are supported:
- V-01 → V-02: Click project in sidebar. **Supported.**
- V-02 → V-03: Tab switch (Overview → Files). **Supported.**
- V-03 → V-02: Tab switch (Files → Overview). **Supported.**
- Any → V-04: Cmd+K. **Supported.**
- V-04 → V-02/V-03: Click search result. **Supported.**
- V-01 → V-05: Click settings link. **Supported.**
- V-05 → V-01: Click project or back. **Supported.**
- V-06 → V-01 + V-02: Setup completion. **Supported.**

### 4. Information Sufficiency

| Decision point (from journeys) | Required info | View | Priority level | Sufficient? |
|-------------------------------|---------------|------|:-:|:-:|
| J-01: "Where should I focus?" | Activity state, recency, status | V-01 | Primary | Yes |
| J-02: "Is the plan still valid?" | Latest session + recent commits | V-02 | Both Primary | Yes |
| J-02: "What doc should I check?" | File tree access | V-03 (one tab switch) | Primary | Yes |
| J-03: "Navigate or search?" | File tree visible + search accessible | V-03 + V-04 | Both Primary | Yes |
| J-04: "Which result is correct?" | Project, type, path, snippet per result | V-04 | Primary | Yes |

All decision-critical information is at Primary or Secondary priority in the relevant view.

### 5. Volume Viability

| View | Entity | Volume range | Handling |
|------|--------|-------------|----------|
| V-01 | Projects | 30-100+ | Scrollable list with search filter. Virtual scrolling at 100+. Groups by activity state. |
| V-02 | Sessions | 5-20 active, hundreds historical | Latest shown expanded. History is scrollable list, older sessions lazy-loaded. |
| V-02 | Commits | ~50 recent, thousands historical | Recent 10-20 shown. "View all" expands with virtual scrolling. |
| V-03 | Documents | 20-thousands | File tree with expand/collapse. .gitignore + .taurhausignore filtering. Search as fallback for large trees. |
| V-04 | Search results | 0-hundreds | Relevance-ranked. Virtual scrolling for large result sets. Type filtering to narrow. |

No volume range exceeds what the view can handle with the described patterns.

### 6. No Dead Ends

Every view has at least one exit:
- V-01: Always visible (it IS the persistent navigation)
- V-02: Sidebar (V-01), tabs (V-03), search (V-04)
- V-03: Sidebar (V-01), tabs (V-02), search (V-04)
- V-04: Escape to dismiss, click result to navigate, underlying view still visible
- V-05: Sidebar (V-01) for return
- V-06: Forward-only flow ending at V-01 + V-02

No dead ends.

### Deferred: Cross-Project Chronological View

J-01 (Orient) includes the need to answer "what happened recently across everything?" V-01's sort-by-activity mode partially serves this by surfacing the most recently active projects. A dedicated cross-project activity feed (interleaving commits and sessions across all projects in reverse chronological order) was evaluated and deferred to v1.1. The current sort-by-activity + per-project drill-down covers the v1 orient flow; the activity feed would be additive.

---

## Role-Based IA

v1 has a single role (Developer). Skipping role-based view differentiation per the IA process guide.

The permission check pattern in 3B's role-permission matrix (Developer / Viewer / Admin) should be architecturally supported but does not affect the v1 IA. Future roles would use **Strategy A** (conditional elements within shared views) — the Viewer sees the same views with edit/create actions hidden, the Admin sees the same views with additional management actions.

---

## Architecture Summary

### Application Layout

```
┌─────────────────────────────────────────────────┐
│  ┌──────────┐  ┌──────────────────────────────┐  │
│  │  V-01    │  │  Main Area                   │  │
│  │  Project │  │  ┌─────────┬────────┐        │  │
│  │  List    │  │  │Overview │ Files  │ (tabs)  │  │
│  │  (sidebar)│  │  ├─────────┴────────┤        │  │
│  │          │  │  │                   │        │  │
│  │ [filter] │  │  │  V-02 or V-03    │        │  │
│  │          │  │  │  (active tab      │        │  │
│  │ project 1│  │  │   content)        │        │  │
│  │▸project 2│  │  │                   │        │  │
│  │ project 3│  │  │                   │        │  │
│  │ project 4│  │  │                   │        │  │
│  │ ...      │  │  │                   │        │  │
│  │          │  │  │                   │        │  │
│  │ [+add]   │  │  │                   │        │  │
│  │ [⚙ set]  │  │  └───────────────────┘        │  │
│  └──────────┘  └──────────────────────────────┘  │
└─────────────────────────────────────────────────┘

V-04 Search Overlay: Cmd+K from anywhere
┌─────────────────────────────────────────┐
│         ┌───────────────────┐           │
│         │ Search...         │           │
│         ├───────────────────┤           │
│         │ result 1          │           │
│         │ result 2          │           │
│         │ result 3          │           │
│         └───────────────────┘           │
│              (dimmed background)         │
└─────────────────────────────────────────┘
```

### Navigation Model

```
                    ┌──────────┐
            Cmd+K → │ V-04     │ ← Cmd+K
           ┌───────→│ Search   │←───────┐
           │        └────┬─────┘        │
           │             │ click        │
           │             │ result       │
    ┌──────┴──┐    ┌─────▼─────┐  ┌────┴────┐
    │ V-01    │───→│ V-02      │←→│ V-03    │
    │ Project │    │ Overview  │  │ Files   │
    │ List    │───→│ (default) │  │         │
    └──┬──────┘    └───────────┘  └─────────┘
       │                tab switch ↕
       │           ┌───────────┐
       └──────────→│ V-05      │
                   │ Settings  │
```

### Tier Summary

| Tier | Views | Access method |
|------|-------|---------------|
| 1 | V-01 Project List, V-04 Search | Always visible / Cmd+K |
| 2 | V-02 Project Overview, V-03 Document Viewer | Click project / tab switch |
| 3 | V-05 Settings, V-06 First-Run, Registration modal | Sidebar link / conditional / action button |

---

## Handoff to Phase 3E

This document provides the structural inputs for View Design:

- **6 views** (V-01 through V-06) with purpose statements → one view spec per view in 3E
- **Navigation model** with pattern selection (sidebar + tabs + command palette) → layout framework for all views
- **Information grouping** per view with primary/secondary/tertiary → hierarchy decisions for 3E layout
- **9 shared components** (SC-01 through SC-09) → component design in 3E/3F
- **Volume handling** notes per view → informs scrolling, virtualization, and filtering decisions
- **Context preservation** requirement → state management specification for 3E

**Key design tensions for 3E to resolve:**
1. **Sidebar width at 1280px**: ~240px sidebar + ~1040px main area. Tight but workable. The sidebar project list item must be very compact.
2. **V-03 sub-split**: File tree (~200px) + content (~800px) within the main area. At 1280px total, this is three columns. Feasible but demands careful spacing.
3. **Tab bar vs. icon toggle**: Overview and Files are only 2 tabs. A full tab bar may feel over-engineered. Consider a segmented control or icon toggle.
4. **Session prominence**: The latest session is the highest-value content on V-02 but competes with project header, git activity, and relationships. Layout must give it visual dominance without burying the rest.
