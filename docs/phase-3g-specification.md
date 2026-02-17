# Phase 3G: Implementation Specification

> Implementation-ready view specifications. Every element has concrete token values from the [Visual System](phase-3f-visual.md). A developer reading this makes zero design decisions.

> **Alignment Note (Phase 5):** This spec was written before the Proposal D "Hybrid" prototype was built. The prototype (`prototype/src/Shell.svelte` + `prototype/src/app.css`) is the **source of truth** for all visual decisions. Where this spec and the prototype conflict, the prototype wins. See the [Prototype–Spec Alignment Table](#prototype-spec-alignment) at the bottom of this document for all resolved discrepancies.

---

## Application Shell

The shell is the persistent frame that contains all views.

### Design: Floating Panel Layout

The entire window has a dark teal frame (`bg-brand-950`). Sidebar and main content are separate panels "floating" inside this frame, separated by a visible gap. The titlebar is part of the frame (custom, no OS decorations).

### Grid

```
┌─────────────────────────────────────────────────────────┐
│ bg-brand-950 frame (p-1.5 = 6px)                       │
│ ┌──[46px titlebar]─────────────────────────────────────┐│
│ │ [Logo 252px]  [Tab pill 36px]  [drag region] [ctrls]││
│ └──────────────────────────────────────────────────────┘│
│ ┌─[252px]──┐ 6px gap ┌─[1fr fluid]───────────────────┐│
│ │ Sidebar  │         │ Main Panel                     ││
│ │ brand-950│         │ (rounded-b-lg rounded-tr-lg)   ││
│ └──────────┘         └────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

- **Frame**: `bg-brand-950`, `p-1.5` (6px) padding around all panels.
- **Titlebar**: 46px tall. Logo area 252px wide (matches sidebar). Tab pill + drag region + controls.
- **Sidebar**: 252px fixed. `bg-brand-950` with `border border-white/[0.06]`.
- **Main panel**: `1fr`, `bg-white` (light) / `bg-zinc-950` (dark). Rounded corners.
- **Gap**: 6px (`gap-1.5`) between sidebar and main panel.
- **Tab pill**: 36px tall, `rounded-t-lg`, shares bg with main panel (Manila Folder pattern).
- **Inverse scoop**: concave corner where tab pill meets dark frame on the right side.

### Responsive Rule

taurhaus is a desktop Tauri app. Minimum window: 1280×800. No tablet/mobile breakpoints.

---

## V-01: Project List (Sidebar)

### Purpose

This view lets the user browse all registered projects and select one for focus by showing project names with activity state indicators and working tree status.

### Layout

```
┌─────────────────────────────────┐
│ Filter Input          (fixed)   │ h: 36px, p: space-2 space-3
│                                 │ bg: white/[0.05], text: white/80
├─────────────────────────────────┤
│ Sort: Activity ▾      (fixed)   │ h: 28px, p: space-1 space-3
│                                 │ text-caption, white/50
├─────────────────────────────────┤ gap: space-1
│ ACTIVE              (scrolls)   │ Uppercase label, 10px, white/40
│ ┌─────────────────────────────┐ │
│ │ ● taurhaus        main     │ │ C-02 Sidebar Item, h: 34px
│ │ ◐ MIR           feat/auth  │ │ dirty indicator: warning-300
│ │ ● taurui          main     │ │
│ └─────────────────────────────┘ │
│ RECENT                          │
│ ┌─────────────────────────────┐ │
│ │ ○ taursec         main     │ │
│ │ ○ taursult        main     │ │
│ └─────────────────────────────┘ │
│ STALE                           │
│ ...                             │
├─────────────────────────────────┤
│ [+] [⚙]              (fixed)   │ h: 44px, p: space-2 space-3
│                                 │ Icon buttons: C-01 Button/icon-only
└─────────────────────────────────┘
```

**Width**: 252px fixed.
**Background**: `bg-brand-950` (same as frame — dark teal in both modes).
**Border**: `border border-white/[0.06]`, `rounded-lg`.
**Scroll**: Project list scrolls independently. Filter, footer fixed.

### Component Instances

| Element | Component | Tokens |
|---------|-----------|--------|
| Filter input | C-13 Input/search | h: 36px, bg: `white/[0.05]`, text: `white/80`, placeholder: `white/40`, focus: `ring-white/20` |
| Sort control | C-01 Button/ghost/small | text: `text-caption`, color: `white/50`, hover: `white/70` |
| Group header | Uppercase label | text: 10px uppercase tracking-wider, color: `white/40`, no chevron |
| Project item | C-02 Sidebar Item | h: 34px, p: `py-1.5 px-3` |
| → name | — | `text-[13px]`, `white/80` (default) / `white` (selected) |
| → activity dot | C-12 Badge (dot variant) | 8px circle: `success-300` (active), `info-300` (recent), `warning-300` (stale), `white/30` (dormant) |
| → branch | — | `text-mono-small`, `white/50`, right-aligned, truncated |
| → dirty indicator | — | Half-filled dot: `warning-300`. Replaces activity dot fill. |
| Add button | C-01 Button/icon-only | icon: "+", 24×24px, `white/50`, hover: `white/70` |
| Settings button | C-01 Button/icon-only | icon: "⚙", 24×24px, `white/50`, hover: `white/70` |

### Interaction Feedback (Tokenized)

| # | Action | Trigger | Feedback |
|---|--------|---------|----------|
| 1 | Select project | Click / Enter | bg → `white/[0.08]` (`motion-instant`). Left border 3px `brand-400`. Detail panel crossfades (`motion-normal`). |
| 2 | Hover project | Mouse enter | bg → `white/[0.04]` (`motion-fast`). Cursor: pointer. |
| 3 | Filter projects | Type in input | List filters instantly. Non-matching hidden. Count: `text-caption` `white/40` "12 of 47". |
| 4 | Sort change | Click sort | List reorders. Sort label updates. |
| 5 | Collapse group | Click header | Deferred to Phase 5G. Groups always expanded initially. |
| 6 | Keyboard navigate | Arrow Up/Down | Focus ring `ring-white/20` on item. Wraps bottom→top. |

### State Specifications

**Empty (no projects):**
- C-11 Empty State centered in list area.
- Icon: folder outline, 48px, `neutral-300`.
- Text: "No projects yet" `text-body` `neutral-500`.
- Action: "Scan for projects" C-01 Button/primary/medium.
- Filter input: disabled, `neutral-100` bg.

**Loading:**
- C-16 Skeleton: 5 rectangular bars (240×40px), `neutral-100` with shimmer.
- Threshold: 200ms before showing.

**Error:**
- Text: "Could not load projects" `text-body` `danger-600`.
- Action: "Retry" C-01 Button/ghost/small.

**Filtered — no results:**
- Text: "No projects match '[query]'" `text-small` `neutral-500`.
- Action: "Clear filter" ghost link, `brand-600`.

### Focus Order

Tab: Filter input → Sort control → First project item → ... → Add button → Settings button.
Arrow Up/Down: Navigate project items when list is focused.

### Accessibility

- Role: `nav` landmark for sidebar.
- Filter: `role="searchbox"`, `aria-label="Filter projects"`.
- Project list: `role="listbox"`, items `role="option"`, `aria-selected` on active.
- Group headers: `role="group"`, `aria-label="Active projects"`.
- Activity dots: `aria-label` with state text ("Active", "Recent", etc.).

---

## V-02: Project Overview (Main Area — Default Tab)

### Purpose

This view lets the user assess a project's current state and resume context by showing the most recent session handoff, recent git activity, and relationships.

### Layout

```
┌────────────────────────────────────────────────────────┐
│ ┌──────────┬─────────┐                                 │ Tab bar (fixed)
│ │▸Overview │ Files   │                     (C-08)      │ h: 40px, bg: neutral-0
│ └──────────┴─────────┘                                 │ border-bottom: neutral-200
├────────────────────────────────────────────────────────┤
│ taurhaus                    main  ● Active    (fixed)  │ Project header
│ Desktop tool for AI project management                 │ h: ~72px, p: space-4 space-6
│                                                  bg: neutral-0
├────────────────────────────────────────────────────────┤
│                                             (scrolls)  │
│ ┌────────────────────────────────────────────────────┐ │
│ │ LATEST SESSION                        2026-02-16   │ │ C-04 Session Card/current
│ │                                                    │ │ bg: neutral-0, shadow-md
│ │ Summary text here spanning full width...           │ │ border: neutral-200, radius-lg
│ │                                                    │ │ p: space-4
│ │ Next Steps:                                        │ │
│ │ • Step one description here                        │ │
│ │ • Step two description                             │ │
│ │                                                    │ │
│ │ Open Questions:                                    │ │
│ │ • Question about approach                          │ │
│ │                                                    │ │
│ │ [Add notes]                  [View full session]   │ │ C-01 Button/ghost/small
│ └────────────────────────────────────────────────────┘ │
│                                          gap: space-6  │
│ ┌────────────────────────────────────────────────────┐ │
│ │ RECENT ACTIVITY                          (C-09)    │ │ Section header
│ │                                                    │ │
│ │ a1b2c3  Add phase-3d-architecture.md        2h    │ │ C-05 Commit Entry × n
│ │ d4e5f6  Complete Phase 3C journeys          1d    │ │ h: 36px each
│ │ g7h8i9  Add phase-3b-domain.md              2d    │ │
│ │                                                    │ │
│ │                          [View all commits →]      │ │ C-01 Button/ghost/small
│ └────────────────────────────────────────────────────┘ │
│                                          gap: space-6  │
│ ┌────────────────────────────────────────────────────┐ │
│ │ RELATIONSHIPS                            (C-09)    │ │
│ │                                                    │ │
│ │ → taurui (provides design to)              [⋮]    │ │ C-10 × n, h: 40px
│ │ ← taursec (audited by)                    [⋮]    │ │
│ │                                                    │ │
│ │ [+ Add relationship]                               │ │ C-01 Button/ghost/small
│ └────────────────────────────────────────────────────┘ │
│                                          gap: space-6  │
│ ┌────────────────────────────────────────────────────┐ │
│ │ SESSION HISTORY                          (C-09)    │ │
│ │                                                    │ │
│ │ 2026-02-15  Completed Phase 3C user jou...         │ │ C-04 Session Card/historical
│ │ 2026-02-14  Started Phase 3B domain...             │ │ h: 40px each
│ │ 2026-02-13  Wrote design brief...                  │ │
│ └────────────────────────────────────────────────────┘ │
│                                          gap: space-6  │
│ ┌────────────────────────────────────────────────────┐ │
│ │ PROJECT INFO                             (C-09)    │ │
│ │ Path: ~/projects/taurhaus                          │ │ text-mono, neutral-500
│ │ Tags: tauri-app, design                            │ │ C-12 Badge/tag
│ │ [Edit metadata]  [Remove project]                  │ │ Ghost + Destructive buttons
│ └────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────┘
```

**Content max-width**: 720px at viewports >1600px (keeps line lengths readable). At 1280px total (1000px main area), content fills available width with `space-6` horizontal padding.

**At 2560px (optional two-column):**
```
┌──────────────────────────┬─────────────────────────┐
│ Latest Session           │ Recent Activity          │
│ (wider column, ~60%)     │ Relationships            │
│                          │ (narrower column, ~40%)  │
├──────────────────────────┴─────────────────────────┤
│ Session History                                     │
│ Project Info                                        │
└─────────────────────────────────────────────────────┘
```

### Component Instances

| Element | Component | Tokens |
|---------|-----------|--------|
| Tab bar | C-08 Tab Bar | h: 40px, bg: `neutral-0`, border-bottom: `neutral-200`. Active: `text-label` `neutral-900` + `brand-600` 2px bottom border. Inactive: `text-label` `neutral-500`. |
| Project name | — | `text-heading-1`, `neutral-900` |
| Branch name | — | `text-mono-small`, `neutral-500`, inline after name with `space-2` gap |
| Activity badge | C-12 Badge/[state] | See 3F §1d |
| Description | — | `text-small`, `neutral-600` |
| Section headers | C-09 Section Header | `text-label` uppercase, `neutral-500` |
| Session card (current) | C-04 Session Card/current | bg: `neutral-0`, border: `border-default`, radius: `radius-lg`, shadow: `shadow-md`, p: `space-4` |
| → "LATEST SESSION" label | C-09 | `text-label`, `neutral-500` |
| → Date | — | `text-caption`, `neutral-400` |
| → Summary | — | `text-body`, `neutral-700` |
| → "Next Steps" heading | — | `text-heading-3`, `neutral-800` |
| → Step items | — | `text-body`, `neutral-700`, bullet `neutral-400` |
| → "Open Questions" heading | — | `text-heading-3`, `neutral-800` |
| → Action buttons | C-01 Button/ghost/small | `text-caption`, `neutral-600` |
| Commit entry | C-05 Commit Entry | h: 36px. Hash: `text-mono-small` `neutral-500`. Message: `text-small` `neutral-700`. Date: `text-caption` `neutral-500`. |
| → Divergence indicator | — | When post-session commits exist: section header appends "N new since session" in `text-caption` `brand-600`. Divider line between post/pre-session commits: 1px dashed `neutral-200`, with inline label "Session recorded" `text-caption` `neutral-400` centered on the line. |
| Relationship entry | C-10 Relationship Entry | h: 40px. Arrow: `neutral-400`. Name: `text-body-medium` `brand-600` (link). Type: `text-small` `neutral-500`. Kebab: `neutral-400`, visible on hover. |
| Session history item | C-04 Session Card/historical | h: 40px. Date: `text-caption` `neutral-500`. Summary: `text-small` `neutral-600`. Hover: `neutral-100` bg. |
| Metadata path | — | `text-mono`, `neutral-500` |
| Tags | C-12 Badge/tag | bg: `neutral-100`, text: `neutral-600` |
| Edit metadata | C-01 Button/ghost/small | `neutral-600` |
| Remove project | C-01 Button/ghost/small | `danger-600` text. Opens C-17 Confirmation Dialog. |

### Interaction Feedback (Tokenized)

| # | Action | Feedback |
|---|--------|----------|
| 1 | Switch to Files tab | Tab indicator slides (`motion-normal`). Content crossfades (`motion-normal`). Overview scroll position cached. |
| 2 | Expand session card | Card height grows (`motion-normal`). Full content (decisions, notes, session ID) appears. "Collapse" replaces "View full session". |
| 3 | Add notes | Inline editor appears below session (`motion-normal`). Save: C-01 Button/primary/small. Cancel: C-01 Button/ghost/small. |
| 4 | Expand session history item | Item grows to full card (`motion-normal`). Others remain collapsed. |
| 5 | View all commits | Section grows (`motion-normal`). Virtual scroll if >100 commits. |
| 6 | Click relationship name | Sidebar selection changes (`motion-instant`). V-02 content crossfades (`motion-normal`) to target project. |
| 7 | Remove project | C-17 Confirmation Dialog opens (`motion-slow`). Message: "Unregister [name]? This does not delete files on disk." Confirm: C-01 Button/destructive/medium. Focus on Cancel. |
| 8 | New session detected | Session card pulses with `brand-100` bg highlight (`motion-deliberate`, fades to `neutral-0` over 2s). If scrolled away, notification dot appears on "LATEST SESSION" header. |

### State Specifications

**Empty (no project selected):**
- C-11 Empty State centered in main area.
- Text: "Select a project from the sidebar" `text-body` `neutral-500`.
- Icon: arrow-left outline, 48px, `neutral-300`.

**Empty (no sessions):**
- Latest Session area: C-11 variant. Text: "No sessions yet" `text-body` `neutral-500`. Subtext: "Sessions are created via the Claude Code handoff skill." `text-small` `neutral-400`.
- Other sections render normally.

**Loading:**
- Header: project name renders immediately from sidebar data. Badge shows.
- Content: C-16 Skeleton — session card skeleton (1 rectangle 200px tall), commit list skeleton (5 bars 36px tall), relationship skeleton (2 bars 40px tall).
- Threshold: 200ms.

**Error:**
- Header renders. Content area: "Could not load project data" `text-body` `danger-600`. Subtext with specific error `text-small` `neutral-500`. "Retry" C-01 Button/ghost/small.
- For git-specific errors: session and docs sections still show if available. Git section shows inline error.

### Focus Order

Tab: Tab bar (Overview → Files) → Session card actions → Commit entries → Relationship entries → Session history items → Metadata actions.
J/K (optional): Navigate between sections.

### Accessibility

- Main area: `role="main"`.
- Tab bar: `role="tablist"`, tabs `role="tab"`, `aria-selected`. Content panels `role="tabpanel"`.
- Sections: `role="region"`, `aria-label` per section ("Latest Session", "Recent Activity", etc.).
- New session detection: `aria-live="polite"` region announces "New session imported for [project]".

---

## V-03: Document Viewer (Main Area — Files Tab)

### Purpose

This view lets the user find and read project files by showing a file tree alongside rendered content.

### Layout

```
┌──────────┬───────┐────────────────────────────────────┐
│ Overview │▸Files │                                     │ C-08 Tab Bar (shared)
├──────────┴───────┤────────────────────────────────────┤
│ ┌──[200px]──┐ ┌──[1fr, min 560px]──────────────────┐ │
│ │ File Tree │ │ File Header              (fixed)    │ │
│ │ (scrolls) │ │ phase-3d-architecture.md            │ │ text-body-medium, neutral-800
│ │           │ │ Last modified: 2h ago · 14 KB       │ │ text-caption, neutral-400
│ │ ▾ docs/   │ ├─────────────────────────────────────┤ │ h: 44px, p: space-2 space-4
│ │   brief.. │ │ Rendered Content          (scrolls) │ │ border-bottom: neutral-200
│ │   3b-do.. │ │                                     │ │
│ │  ▸3d-ar.. │ │ # Phase 3D: Information             │ │ Rendered markdown scale
│ │   3e-vi.. │ │ Architecture                        │ │ from 3F §2d
│ │ ▾ src/    │ │                                     │ │
│ │   main.rs │ │ > Defines the structural...         │ │ max-width: 720px (48rem)
│ │   lib.rs  │ │                                     │ │ p: space-4 space-6
│ │ CLAUDE.md │ │ ## Step 1: Entity Inventory         │ │
│ │           │ │ ...                                 │ │
│ └───────────┘ └─────────────────────────────────────┘ │
│  divider: 1px neutral-200                             │
└───────────────────────────────────────────────────────┘
```

- **File tree**: 200px fixed. bg: `neutral-50`. p: `space-2` top.
- **Content area**: `1fr`, min 560px. bg: `neutral-0`.
- **Divider**: 1px `neutral-200`.
- **File header**: fixed within content area. h: 44px. border-bottom: `neutral-200`.
- **Rendered content**: scrolls independently. p: `space-4` vertical, `space-6` horizontal. `max-width: 720px` for markdown readability.

### Component Instances

| Element | Component | Tokens |
|---------|-----------|--------|
| File tree item (dir) | C-06 File Tree Item | h: 32px. Chevron: `neutral-400`. Icon: folder, `neutral-500`. Name: `text-small` `neutral-700`. Indent: `space-4` per level. |
| File tree item (file) | C-06 File Tree Item | h: 32px. Icon: type-specific (md, rs, toml, img), `neutral-400`. Name: `text-small` `neutral-600`. |
| File tree item (selected) | C-06 selected state | bg: `brand-50`. Name: `brand-700`. |
| File tree item (hover) | C-06 hover state | bg: `neutral-100`. |
| File header name | — | `text-body-medium`, `neutral-800` |
| File header metadata | — | `text-caption`, `neutral-400`, separated by " · " |
| Rendered markdown | C-05 Document Renderer | H1: 24px/700. H2: 20px/600. H3: 16px/600. Body: 15px/400, lh 1.6. Code inline: 14px mono, `neutral-100` bg, `space-0.5` padding. Code block: 14px mono, `neutral-50` bg, `space-3` padding, `radius-md`. Blockquote: left border 3px `neutral-200`, `neutral-600` italic. |
| Syntax-highlighted source | C-05 variant | 14px mono, lh 1.5, `neutral-50` bg, line numbers: `neutral-400`. |

### Interaction Feedback

| # | Action | Feedback |
|---|--------|----------|
| 1 | Select file | Tree item bg → `brand-50` (`motion-instant`). Content crossfades (`motion-normal`). Content scroll resets to top. |
| 2 | Expand directory | Chevron rotates (`motion-fast`). Children slide in (`motion-fast`). |
| 3 | Collapse directory | Chevron rotates back (`motion-fast`). Children slide out (`motion-fast`). |
| 4 | Hover tree item | bg → `neutral-100` (`motion-fast`). |
| 5 | Type-ahead in tree | Focus jumps to matching file. Brief `brand-100` highlight flash. |
| 6 | Internal markdown link click | Auto-select linked file in tree. Content renders new file. |
| 7 | External link click | System browser opens. No taurhaus state change. |

### State Specifications

**Empty (no file selected):**
- Content area: C-11 Empty State. Text: "Select a file from the tree" `text-body` `neutral-500`. Or auto-select README.md/CLAUDE.md if present.

**Empty (no files):**
- Tree: C-11. Text: "No viewable files" `text-small` `neutral-500`. Subtext: "Check ignore patterns in Settings" `text-caption` `neutral-400`.

**Loading (file content):**
- Tree renders normally. Content: C-16 Skeleton — heading bar + 5 paragraph bars. Threshold: 200ms.

**Error (file not found):**
- Content: "File not found: [path]" `text-body` `danger-600`. Subtext: "It may have been deleted or moved" `text-small` `neutral-500`.

**Error (can't render):**
- Content: raw text fallback with `text-mono` `neutral-600`, bg: `neutral-50`. Warning banner at top: "Cannot render this file type. Showing raw content." `text-caption` `warning-600`, bg: `warning-50`, p: `space-2`.

### Focus Order

Tab: Tab bar → File tree → File content area.
In tree: Arrow Up/Down navigate items. Right arrow expands/descends. Left arrow collapses/ascends. Enter selects file.
In content: standard scroll. Tab navigates to links within rendered content.

### Accessibility

- File tree: `role="tree"`, items `role="treeitem"`, `aria-expanded` on directories.
- Content area: `role="article"` for rendered content.
- File header: announces file name on change via `aria-live="polite"`.

---

## V-04: Search Overlay (Command Palette)

### Purpose

This view lets the user find information across all projects via a focused search input with real-time, categorized results.

### Layout

```
┌──────────────────────────────────────────────────────┐
│                                                      │
│          ┌─────────────────────────────┐             │
│          │ 🔍 Search taurhaus...       │             │ C-13 Input/search
│          │ w: 600px, h: 44px           │             │ p: space-3 space-4
│          ├─────────────────────────────┤             │ bg: neutral-0
│          │                             │             │ border: neutral-200
│          │ DOCUMENTS              (C-09)│             │ radius-xl (top)
│          │ ┌─────────────────────────┐ │             │
│          │ │ 📄 taurhaus › docs/...  │ │             │ C-07 Search Result Item
│          │ │ ...entity inventory...  │ │             │ h: 56px
│          │ ├─────────────────────────┤ │             │
│          │ │ 📄 taurui › patterns/...│ │             │
│          │ └─────────────────────────┘ │             │
│          │                             │             │
│          │ SESSIONS              (C-09)│             │
│          │ ┌─────────────────────────┐ │             │
│          │ │ 💬 taurhaus 2026-02-16  │ │             │
│          │ └─────────────────────────┘ │             │
│          │                             │             │
│          │ ↑↓ Navigate  ↵ Open  esc   │             │ text-caption, neutral-400
│          └─────────────────────────────┘             │ p: space-2 space-3
│                                                      │ radius-xl (bottom)
│            bg: neutral-950/50%                       │ Backdrop
└──────────────────────────────────────────────────────┘
```

- **Overlay**: 600px wide, centered horizontally, ~20% from top vertically.
- **Max height**: 60% of viewport. Internal scroll for results.
- **Card**: bg `neutral-0`, radius `radius-xl`, shadow `shadow-lg`.
- **Backdrop**: `neutral-950` at 50% opacity.

### Component Instances

| Element | Component | Tokens |
|---------|-----------|--------|
| Input | C-13 Input/search | h: 44px. `text-body`, `neutral-900`. Placeholder: `neutral-400`. Icon: search, `neutral-400`. Border-bottom: `neutral-200` (separates from results). No outer border on input (card provides it). |
| Type group header | C-09 Section Header | `text-label` uppercase, `neutral-400`. p: `space-2` `space-3`. |
| Result item | C-07 Search Result | h: 56px. p: `space-2` `space-3`. |
| → Type icon | — | 16px, `neutral-400`. 📄 documents (all file types), 💬 sessions, ● commits. |
| → Project + path | — | `text-caption` `neutral-500` + `text-body-medium` `neutral-800`. |
| → Snippet | — | `text-small` `neutral-600`. Match highlight: `brand-600` `font-weight: 600`. |
| → Hover state | — | bg: `neutral-100`. |
| → Keyboard selected | — | bg: `brand-50`. |
| Keyboard hints | — | `text-caption` `neutral-400`. p: `space-2` `space-3`. Border-top: `neutral-200`. |

### Interaction Feedback

| # | Action | Feedback |
|---|--------|----------|
| 1 | Open (Cmd+K) | Backdrop fades in (`motion-slow`). Card appears (`motion-slow`, slight scale from 0.95→1). Input auto-focused. |
| 2 | Type query | Results appear (`motion-instant`). Previous results replaced. |
| 3 | Arrow Down | Highlight moves to next result. bg: `brand-50` (`motion-instant`). Auto-scroll if needed. |
| 4 | Arrow Up | Highlight moves to previous result. From first result → back to input. |
| 5 | Enter on result | Overlay dismisses (`motion-fast`). Navigate to target view. Sidebar selection updates if different project. |
| 6 | Escape | Overlay dismisses (`motion-fast`). Previous view state restored unchanged. |
| 7 | Click outside | Same as Escape. |

### State Specifications

**Just opened (no query):**
- Input focused, empty. No results section. Optionally show "Type to search across all projects" in muted text below input.

**Reopened (previous query exists):**
- Input focused with previous query text fully selected (pre-selection styling: `brand-50` bg on text). Previous results visible immediately. Typing replaces query. Escape clears query first (results disappear), second Escape dismisses overlay. Arrow Down navigates to first result.

**No results:**
- Below input: "No results for '[query]'" `text-small` `neutral-500`, centered. p: `space-6`.

**Results loading (>200ms):**
- Below input: subtle inline spinner, 16px, `neutral-400`.

**Error (index unavailable):**
- Below input: "Search index is rebuilding..." `text-small` `warning-600`, bg: `warning-50`, p: `space-2` `space-3`.

### Focus Order

Input receives focus on open. Arrow keys navigate results (roving tabindex). Enter activates. Escape dismisses. Tab is trapped within overlay (modal behavior).

### Accessibility

- Overlay: `role="dialog"`, `aria-modal="true"`, `aria-label="Search taurhaus"`.
- Input: `role="combobox"`, `aria-expanded="true"` when results visible, `aria-controls` points to results list.
- Results: `role="listbox"`, items `role="option"`, `aria-selected` on highlighted.
- Live region: `aria-live="polite"` announces result count: "5 results" or "No results".

---

## V-05: Settings

### Layout

```
┌──────────────────────────────────────────────────────┐
│ ← Back to projects                Settings            │ h: 48px, p: space-3 space-6
│                                                      │ border-bottom: neutral-200
├──────────────────────────────────────────────────────┤
│                  max-width: 640px, centered           │
│                  p: space-8 top                       │
│                                                      │
│ PROJECT SCANNING                         (C-09)      │ text-label, neutral-500
│ ┌──────────────────────────────────────────────────┐ │
│ │ Scan directories                                 │ │ text-heading-3, neutral-800
│ │ ~/projects/                          [Edit]      │ │ text-mono, neutral-600
│ │                                                  │ │   + C-01 Button/ghost/small
│ │ Global ignore patterns                           │ │
│ │ node_modules, .git, target, dist     [Edit]      │ │
│ │                                                  │ │
│ │ [Rescan all projects]                            │ │ C-01 Button/secondary/medium
│ └──────────────────────────────────────────────────┘ │ bg: neutral-0, border: neutral-200
│                                        gap: space-6  │ radius: radius-lg, p: space-4
│                                                      │
│ DISPLAY                                  (C-09)      │
│ ┌──────────────────────────────────────────────────┐ │
│ │ Activity thresholds                              │ │
│ │ Active: [7] days  Recent: [30] days              │ │ C-13 Input/text (small)
│ │ Stale: [90] days  Dormant: 90+ days              │ │
│ └──────────────────────────────────────────────────┘ │
│                                        gap: space-6  │
│ INDEX                                    (C-09)      │
│ ┌──────────────────────────────────────────────────┐ │
│ │ Status: Healthy · 47 projects · 12,403 files     │ │ text-small, neutral-600
│ │ Last rebuilt: 2 hours ago                        │ │ text-caption, neutral-400
│ │                                                  │ │
│ │ [Rebuild index]                                  │ │ C-01 Button/secondary/medium
│ └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

Settings sections use card containers (bg `neutral-0`, border `neutral-200`, radius `radius-lg`, p `space-4`). Section headers use C-09. Back link uses `text-body-medium` `brand-600`.

All input fields use C-13 with appropriate validation. Changes save automatically on blur or via explicit save button where batch editing is needed.

---

## V-06: First-Run Setup

### Layout (Step-by-step, centered)

**Step 1: Welcome**
```
┌──────────────────────────────────────────────────────┐
│                   max-width: 480px                    │
│                   centered both axes                  │
│                                                      │
│                   taurhaus                            │ text-heading-1, neutral-900
│       The house where all your projects live.        │ text-body, neutral-600
│                                                      │
│   taurhaus gives you a single view into all your     │ text-small, neutral-500
│   AI-driven projects — their code, docs, progress,   │ max-width: 400px
│   and session history.                               │
│                                                      │
│             [ Scan ~/projects/ ]                     │ C-01 Button/primary/large
│                                                      │
│        Or add a project manually                     │ text-small, brand-600 (link)
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Step 2: Project selection**
- Scrollable checkbox list. Each item: C-02 Sidebar Item variant with checkbox prepended.
- Checkbox: 18×18px, border `neutral-300`, checked: bg `brand-600`, check `neutral-0`.
- "Select all / Deselect all": `text-small` `brand-600` links.
- Count: `text-body-medium` `neutral-700` "22 selected".
- "Register selected": C-01 Button/primary/large.

**Step 3: Indexing**
- C-18 Progress Bar. Track: `neutral-200`. Fill: `brand-500`. h: 8px. radius: `radius-sm`.
- Above bar: "14 / 22 projects" `text-body-medium` `neutral-700`.
- Below bar: "Indexing: taursult" `text-small` `neutral-500`.

**Step 4: Complete**
- "22 projects registered" `text-heading-2` `neutral-800`.
- Checkmark icon, 48px, `success-500`.
- "Go to dashboard" C-01 Button/primary/large. Auto-transitions after 2s if no interaction.

---

## Project Registration Modal

Uses C-14 Modal Overlay (480px wide). Header: "Register Project" `text-heading-2`. Close: ✕ button.

| Element | Component | Tokens |
|---------|-----------|--------|
| Path input | C-13 Input/text | h: 36px. Label: `text-heading-3`. |
| Browse button | C-01 Button/icon-only | 📁 icon, `neutral-500`. |
| Auto-detected divider | — | `text-label` `neutral-400` + line `neutral-200`. |
| Name field | C-13 Input/text | Pre-filled from directory. |
| Description field | C-13 Input/text | Pre-filled from README or empty. |
| Tags | C-12 Badge/tag + C-01 Button/ghost/small "Add tag" | |
| Cancel | C-01 Button/secondary/medium | |
| Register | C-01 Button/primary/medium | Disabled until path valid. |

Validation states: Invalid path → `border-error` + "Path not found" `text-caption` `danger-600`. Not git → "Not a git repository" `text-caption` `danger-600`. Already registered → "Already registered" `text-caption` `warning-600`.

---

## Relationship Creation Modal

Uses C-14 Modal Overlay (420px wide). Header: "Add Relationship" `text-heading-2`. Close: ✕ button.

| Element | Component | Tokens |
|---------|-----------|--------|
| Target project | C-13 Input/search (combobox) | h: 36px. Label: "Target Project" `text-heading-3`. Dropdown: filtered project list, items use C-02 variant (name + activity dot). Selected: project name as chip, bg `brand-50`, text `brand-700`, `radius-sm`. |
| Relationship type | Custom select group | Predefined options: "provides design to", "depends on", "audited by", "forked from", plus custom text input. Label: "Type" `text-heading-3`. Selected option: `brand-50` bg, `brand-600` border. Unselected: `neutral-0` bg, `neutral-200` border. `text-small`. |
| Direction | Toggle | "This project → Target" / "Target → This project". Label: "Direction" `text-heading-3`. Arrow text: `text-body` `neutral-700`. Default: "This project → Target". |
| Description | C-13 Input/text | h: 36px. Label: "Description (optional)" `text-heading-3`. Placeholder: "Brief note about this relationship" `neutral-400`. |
| Cancel | C-01 Button/secondary/medium | |
| Add | C-01 Button/primary/medium | Disabled until target and type selected. Label: "Add Relationship". |

Validation: Target project required. Type required. Direction defaults to "This project → Target". Description optional. Duplicate check (same target + type + direction): "This relationship already exists" `text-caption` `warning-600`.

---

## Cross-View Consistency Check

| Shared element | Token assignment | Verified identical across |
|---------------|-----------------|--------------------------|
| Tab bar (C-08) | h: 40px, bg: `neutral-0`, active: `brand-600` bottom border | V-02, V-03 |
| Section header (C-09) | `text-label` uppercase, `neutral-500` | V-02, V-04, V-05 |
| Primary button (C-01) | fill: `brand-600`, text: `neutral-0`, h: 36px (med) / 44px (lg) | V-01, V-02, V-05, V-06, modal |
| Ghost button (C-01) | transparent, `neutral-600`, hover: `neutral-100` | V-01, V-02, V-03, V-04 |
| Empty state (C-11) | Icon: 48px `neutral-300`, text: `text-body` `neutral-500`, action: context-specific | V-01, V-02, V-03, V-04, V-06 |
| Input (C-13) | h: 36px, border: `neutral-200`, focus: `brand-500`, error: `danger-500` | V-01, V-04, V-05, modal |
| Activity dot | 8px circle, colors per 3F §1d | V-01, V-02 |
| Sidebar item (C-02) | h: 40px, selected: `brand-50` + `brand-600` left border | V-01 only (single use but consistent token application) |
| Session card (C-04) | current: shadow-md, border, radius-lg. historical: 40px flat. | V-02 only |
| Commit entry (C-05) | h: 36px, hash: mono-small, msg: small, date: caption | V-02 only |
| Search result (C-07) | h: 56px, hover: neutral-100, selected: brand-50 | V-04 only |

All shared components use identical tokens across every view they appear in. No inconsistencies.

---

## Global Keyboard Shortcut Map

| Shortcut | Action | Context | Component |
|----------|--------|---------|-----------|
| `Cmd+K` | Open/close search | Global | V-04 |
| `Escape` | Dismiss overlay / deselect | V-04, modals | — |
| `Arrow Up/Down` | Navigate list items | V-01, V-03 tree, V-04 results | Roving tabindex |
| `Enter` | Select/open focused item | V-01, V-03, V-04 | — |
| `Tab` | Move focus between regions | All | Standard |
| `/` or `Cmd+F` | Focus sidebar filter | V-01 focused | C-13 |
| `Right Arrow` | Expand directory / descend | V-03 tree | C-06 |
| `Left Arrow` | Collapse directory / ascend | V-03 tree | C-06 |

---

## Prototype–Spec Alignment Table {#prototype-spec-alignment}

The prototype (`prototype/src/Shell.svelte` + `prototype/src/app.css`) was built after this spec and reflects Proposal D "Hybrid" decisions. The table below documents every discrepancy and resolution.

| # | Element | Original Spec | Prototype (Source of Truth) | Resolution |
|---|---------|---------------|----------------------------|------------|
| 1 | **Sidebar width** | 240px (collapsed 56px) / 280px (expanded) | 252px fixed, no collapse | Use 252px fixed. Collapse is Phase 5G scope. |
| 2 | **Sidebar background** | `neutral-50` (light), `neutral-900` (dark) | `bg-brand-950` always (dark teal) | Use `bg-brand-950` in both modes. Sidebar is always dark. |
| 3 | **Panel layout** | Flush layout with 1px `neutral-200` divider | Floating panels in `bg-brand-950` frame, 6px gap (`gap-1.5`) | Use floating panel layout with `p-1.5` frame padding. |
| 4 | **Sidebar item height** | 40px | 34px (`py-1.5 px-3`) | Use 34px. Denser sidebar fits more projects. |
| 5 | **Tab bar** | 40px tall, inside main content area | 36px tab pill in titlebar (Manila Folder pattern) | Tab pill in titlebar, shares bg with main panel. |
| 6 | **Titlebar** | Not specified (assumed OS decorations) | 46px custom titlebar with logo, tab pill, controls | Use custom titlebar. All non-interactive space is draggable. |
| 7 | **Activity dot colors** | 500-level (`success-500`, `warning-500`, etc.) | 300-level (`success-300`, `warning-300`, etc.) | Use 300-level for visibility on dark `bg-brand-950` sidebar. |
| 8 | **Session card style** | `shadow-md` card with `rounded-xl` | Flat left-border treatment, keyline separators, no shadows | Use flat treatment. No cards, no shadows — keyline separators only. |
| 9 | **Content max-width** | 720px | 700px (`max-w-[700px]`) | Use 700px. |
| 10 | **Sidebar filter input** | `neutral-0` background | `bg-white/[0.05]` translucent on dark | Use translucent `bg-white/[0.05]` (appropriate for dark sidebar). |
| 11 | **Sidebar group headers** | 24px tall with chevron toggle | 10px uppercase text labels, no toggle | Use simple uppercase labels. Group toggling deferred to 5G. |
| 12 | **Color token system** | Semantic tokens (`neutral-200`, `text-body`) | Concrete Tailwind utilities (`bg-zinc-200`, `text-[13px]`) | Map spec semantic tokens to Tailwind `@theme` values from `app.css`. |
| 13 | **Frame element** | Not present (no frame concept) | `bg-brand-950` frame wraps all panels with `p-1.5` | Adopted. Core Proposal D visual identity. |
| 14 | **Inverse scoop** | Not present | CSS concave corner where tab pill meets frame | Adopted. Key visual detail for tab-to-panel transition. |

All discrepancies are resolved in favor of the prototype. The spec sections above (Application Shell, V-01) have been updated to match. Remaining view sections (V-02 through V-08) should be interpreted through these same adjustments during implementation.

---

## Handoff Summary

This document completes Phase 3 (UI Design). The outputs are:

| Phase | Document | Content |
|-------|----------|---------|
| 3B | `phase-3b-domain.md` | Entity inventory, action vocabulary, design constraints |
| 3C | `phase-3c-journeys.md` | 9 user journeys with priority scoring |
| 3D | `phase-3d-architecture.md` | View inventory, navigation model, information grouping, shared components |
| 3E | `phase-3e-views.md` | Per-view specs: layout, interactions, states, connections |
| 3F | `phase-3f-visual.md` | Color, typography, spacing, 18 components, density, motion |
| 3G | `phase-3g-specification.md` | Implementation-ready specs with all token values |

**Next: Phase 4 (Architecture)** — Rust backend modules, data models, Tauri command surface, designed to serve the UI specified here.
