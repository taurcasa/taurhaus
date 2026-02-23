# Lookbook Brief: Task Management — Kanban Board + Task Detail Panel

> Generated via `/lookbook` skill during Phase 5H implementation. This is the design brief governing the TaskDetailPanel component.

## Your Project Context

**Established design language:**
- Dark teal frame (`brand-950`), floating panels, Manila Folder tabs
- Color tokens: success (green), info (blue), warning (amber), danger (red), zinc (neutral)
- Typography: Geist sans, dense 11-13px scale for content
- Dark mode via `$derived` tokens — no inline ternaries

**Existing components to reuse:**
- `TaskBoard.svelte` — current 3-column Kanban (In Progress / Pending / Completed)
- `taskHelpers.js` — `statusBadgeClass()`, `statusLabel()`
- `HoverCard.svelte` — reference for rich overlay/detail patterns
- `CodeViewer.svelte` — reference for file content display
- `MarkdownRenderer.svelte` — could be used for task descriptions

**Current Kanban anatomy:**
- Three equal columns with colored header dots (green/blue/zinc)
- Task cards: tool icon + subject + description + blocked_by + owner
- Cards are static — no click interaction, no expansion, no detail view

## Recommended Pattern: Kanban + Slide-Over Detail Panel

The **Pipeline/Kanban + Master-Detail hybrid** is the right fit. The board is the "master" (spatial overview of all tasks across states), and clicking a card opens a slide-over detail panel from the right.

**Why slide-over, not modal:**
- User stays in context — other columns remain visible behind the panel
- Can click another task card to switch detail without closing
- Follows Linear's pattern (the benchmark for modern project tools)
- No overlay dimming — feels lightweight, not interruptive

**Why not inline card expansion:**
- Would disrupt column layout (cards below shift, column heights become unpredictable)
- Narrow column width (~33%) can't accommodate rich detail (commits, file lists)
- Breaks the scanning pattern — the board becomes asymmetric

**Panel anatomy** (from TaurUI master-detail pattern):

```
┌─ In Progress ─┐  ┌─ Pending ─────┐  ┌─ Completed ──┐  ┌─ Detail Panel ─────────┐
│                │  │                │  │               │  │                         │
│  [Card A]      │  │  [Card C] ◂───│──│───────────────│──│  Task: Card C           │
│  [Card B]      │  │  [Card D]     │  │  [Card F]     │  │  Status: Pending        │
│                │  │  [Card E]     │  │  [Card G]     │  │  Source: Claude          │
│                │  │               │  │               │  │                         │
│                │  │               │  │               │  │  ── Description ──       │
│                │  │               │  │               │  │  Full text here...       │
│                │  │               │  │               │  │                         │
│                │  │               │  │               │  │  ── Session ──           │
│                │  │               │  │               │  │  abc-1234 · 2h active    │
│                │  │               │  │               │  │                         │
│                │  │               │  │               │  │  ── Commits (3) ──       │
│                │  │               │  │               │  │  f0a5339 Add SQLite...   │
│                │  │               │  │               │  │  d9331db Add event...    │
│                │  │               │  │               │  │                         │
│                │  │               │  │               │  │  ── Files Changed ──     │
│                │  │               │  │               │  │  command_center.rs       │
│                │  │               │  │               │  │  lib.rs                  │
│                │  │               │  │               │  │  TaskBoard.svelte        │
│                │  │               │  │               │  │                         │
│                │  │               │  │               │  │  ── Dependencies ──      │
│                │  │               │  │               │  │  Blocked by: #1, #3      │
└────────────────┘  └────────────────┘  └───────────────┘  └─────────────────────────┘
```

**Detail panel sizing:**
- Fixed width: 340-380px (matches sidebar width precedent)
- Board columns compress proportionally when panel is open
- Panel slides in from right edge with a brief transition (150ms)
- Close via X button, Escape key, or clicking outside

## Progressive Disclosure in Detail Panel

Each section appears **only when data exists**. No "No commits yet" placeholder sections cluttering the view — sections that have no data simply aren't rendered. This follows the "dense but calm" principle: show what we know, don't manufacture noise.

**Always present:**
- **Header**: Subject (large), status badge, source tool icon + label
- **Description**: Full text (not truncated like on the card). Use `MarkdownRenderer` if descriptions contain markdown.

**Present when session is linked:**
- **Session**: Session UUID (truncated), running/ended status, time active, tool that created it

**Present when commits exist:**
- **Commits**: Hash (short), message (truncated), relative time. Each commit row clickable → could navigate to Overview tab's commit detail in the future. Shows "during this session" qualifier so the user understands the scope.

**Present when files can be derived from commits:**
- **Files Changed**: File paths grouped by commit or deduplicated. Clickable → opens in Files tab.

**Present when dependencies exist:**
- **Dependencies**: Visual blocked-by / blocks chain. Task IDs are clickable → selects that task and shows its detail.

**Present when owner exists:**
- **Owner**: Agent name, possibly with avatar if available

**For tasks with minimal data** (e.g., a Gemini TODO.md checkbox):
The panel shows just the header + description section. It's a short panel — that's fine. The panel height is content-driven, not fixed.

## Card Design Refinements

Current cards work well. Minor refinements for the clickable interaction:

- **Cursor**: `cursor-pointer` on cards
- **Selection state**: Selected card gets a left border accent (2px, brand-500) and slightly elevated background — makes it clear which card the detail panel refers to
- **Hover**: Already implemented (`cardHover` token). Keep as-is.
- **Active/pressed**: Brief scale down (0.98) on click for tactile feedback

## Why It Works

From TaurUI foundations:

- **Information density** (foundations/information-density.md): Expert users in dense desktop tools benefit from "anchor-then-scan" — the board is the anchor (spatial overview), the detail panel is the scan (deep dive). Both visible simultaneously.
- **Master-detail** (patterns/information-display/master-detail.md): "The user stays in one place. Browsing and reading happen simultaneously. The list provides context while the detail provides depth."
- **Status & Progress** (patterns/visualization/status-progress.md): Pipeline/Kanban — "The user sees all entities across all stages simultaneously, making bottlenecks and stalled items immediately visible."

## Reference Apps

1. **Linear** — The primary reference. Issue detail slides in from the right as a panel. Keyboard navigable (J/K to move between issues). Fast transitions. The gold standard.
2. **Trello** — Card detail as modal overlay. Less ideal than Linear's slide-over (loses board context), but good reference for card detail content structure.
3. **Asana** — Task detail as right panel in board view. Shows subtasks, activity, custom fields. Good reference for section organization within the detail.
4. **Things 3** — Detail appears below or alongside the list. Minimal, progressive disclosure. Good reference for the "sparse task" case.

## Alignment Notes

- **Reuse `$derived` dark mode tokens** — detail panel follows same pattern as Shell.svelte and current TaskBoard
- **Detail panel background**: Same as main content panel (`mainBg` — zinc-950 dark / white light). Not a separate surface.
- **Keyline separator** between board and detail panel, matching existing `keyline` token
- **Close button** style: Match the existing pattern from `SearchOverlay.svelte` or `Settings.svelte`
- **Typography scale**: Header 15px semibold (matches "Tasks" header), section headers 11px uppercase tracking (matches column headers), content 13px (matches card text)
- **Status badge in detail**: Reuse `statusBadgeClass()` from `taskHelpers.js`
- **Commit rows**: Similar density to the Recent Activity section in Overview tab (hash + message + time)

## Checklist (from TaurUI new-view checklist)

- [ ] **Empty detail state**: No task selected → show instructional text in detail area, or don't show panel at all (panel only appears on click)
- [ ] **Loading state**: If detail data (commits, session) requires async fetch, show skeleton in relevant sections
- [ ] **Error state**: If commit fetch fails, show inline error in that section only
- [ ] **Keyboard navigation**: Arrow keys to move between cards? Escape to close panel? Tab to move focus into panel?
- [ ] **Volume testing**: 0 tasks, 1 task, 50 tasks per column. Does the board scroll? Do columns handle overflow?
- [ ] **Sparse task**: Task with only subject + status (Gemini TODO item) — panel should look intentional, not empty
- [ ] **Rich task**: Task with description + 10 commits + 15 files + blocked_by chain — panel should scroll, not overflow
- [ ] **Selected card visibility**: If the selected card scrolls out of view in its column, is that confusing? (Probably acceptable — Linear does this)
- [ ] **Panel animation**: 150ms slide-in, `prefers-reduced-motion` respected
- [ ] **Responsive**: Panel hides below a minimum board width (probably not an issue for desktop-only app)

## Further Reading

- `~/projects/taurui/patterns/information-display/master-detail.md` — Full master-detail pattern with anti-patterns
- `~/projects/taurui/patterns/visualization/status-progress.md` — Pipeline/Kanban section + state semantics
- `~/projects/taurui/lookbook/productivity.md` — Linear, Asana, Trello references
- `~/projects/taurui/foundations/information-density.md` — Density strategies for expert desktop tools
- `~/projects/taurui/foundations/interaction-states.md` — Selection state, hover, active press states
