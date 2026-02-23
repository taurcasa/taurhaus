# Lookbook Brief: Session History — Archived Task Timeline

> Generated via `/lookbook` skill during Phase 5H implementation. This is the design brief governing the session history view for archived/completed tasks.

## Problem

The task board shows active work ("what's happening now"), but completed tasks from previous sessions disappear when they go stale. The backend now archives completed tasks (with `archived_at` timestamp) instead of deleting them. This data exists but has no UI.

A flat list of old completed tasks has no value — without context, "task #3 from two weeks ago" is meaningless. The user needs **session grouping** to understand what work was done when, and **drill-down interaction** to explore the commits and files associated with each session.

## Your Project Context

**Established design language:**
- Dark teal frame (`brand-950`), floating panels, Manila Folder tabs
- Color tokens: success (green), info (blue), warning (amber), danger (red), zinc (neutral)
- Typography: Geist sans, dense 11-13px scale for content
- Dark mode via `$derived` tokens — no inline ternaries

**Existing components to reuse:**
- `TaskBoard.svelte` — current 3-column Kanban (In Progress / Pending / Completed)
- `TaskDetailPanel.svelte` — 360px slide-over with session info, commits, files, dependencies
- `taskHelpers.js` — `statusBadgeClass()`, `statusLabel()`
- `TOOL_ICONS` in TaskBoard — monochrome SVG logos (Claude, Codex, Gemini)
- `MarkdownRenderer.svelte` — for task descriptions
- Commit hash pills, file path splitting — all exist in TaskDetailPanel

**Existing backend:**
- `enrich_from_session()` resolves session → time range → commits → files changed
- Archived tasks in SQLite: `archived_at IS NOT NULL`, linked via `session_id`
- Task scanner types: `UnifiedTask`, `TaskDetail`, `SessionInfo`

## Recommended Pattern: Sub-Tab + Accordion Session Groups

### Navigation: Sub-Tab within Tasks Tab

Add a sub-tab switcher within the Tasks tab header: **"Active"** | **"History"**.

- **Active** (default): Shows the current Kanban board (unchanged)
- **History**: Shows the session history accordion (new)

This follows the Manila Folder tab convention from `Shell.svelte`. Sub-tabs keep each view focused — the active board isn't cluttered with historical data, and the history view has full space.

The sub-tab pills sit in the Tasks tab header bar (where the "Tasks" heading and count are now). Small, unobtrusive — 11px uppercase, same style as the Kanban column headers.

### Layout: Accordion Session Groups

The history view is a vertical list of session groups, sorted reverse-chronological (newest first). Each session is a collapsible accordion section.

**Level 0 — Session headers (always visible):**

```
┌─────────────────────────────────────────────────────────┐
│  ▶  Feb 20, 2026 — 2h 15m     5 tasks  ·  12 commits   │
│     [Claude icon] [Codex icon]                          │
├─────────────────────────────────────────────────────────┤
│  ▼  Feb 18, 2026 — 1h 42m     3 tasks  ·  7 commits    │
│     [Claude icon]                                       │
│  ┌─────────────────────────────────────────────────────┐│
│  │  Tasks                                              ││
│  │  ✓  Add task scanner backend              [Claude]  ││
│  │  ✓  Build TaskBoard UI component          [Claude]  ││
│  │  ✓  Write unit tests                      [Gemini]  ││
│  │                                                     ││
│  │  Commits                                            ││
│  │  abc1234  Add task scanner types              30m   ││
│  │  def5678  Implement Claude parser             1h    ││
│  │  ghi9012  Build TaskBoard component           1h    ││
│  │  ...4 more                                          ││
│  │                                                     ││
│  │  8 files changed                                    ││
│  └─────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────┤
│  ▶  Feb 15, 2026 — 3h 05m     8 tasks  ·  15 commits   │
│     [Claude icon] [Gemini icon]                         │
└─────────────────────────────────────────────────────────┘
```

**Session header anatomy:**
- Chevron (expand/collapse indicator)
- Date + duration (primary — 13px semibold)
- Task count pill + commit count (secondary — 11px muted)
- Source tool icon(s) — which CLI tools contributed to this session
- Full header row is the click target (not just the chevron)

**Level 1 — Expanded session detail:**
- **Tasks sub-section**: Compact list with checkmark icon, subject, source tool icon. Clicking a task opens the existing `TaskDetailPanel` slide-over.
- **Commits sub-section**: Compact list with hash pill (monospace, styled like TaskDetailPanel), commit message, relative time. Max 5 visible, "+N more" truncation for sessions with many commits.
- **Files changed summary**: "N files changed" as a compact footer line. Could expand to show the file list on click (future enhancement).

### Interaction

- **Multi-expand**: Any number of sessions can be open simultaneously. This is the accordion default — single-expand forces the user to lose their place.
- **Task click → TaskDetailPanel**: Reuses the existing slide-over. The detail panel shows full task info including commits, files, dependencies.
- **Commit click**: Future enhancement — could navigate to file view or show diff. For MVP, commits are display-only.
- **Keyboard**: Arrow keys between session headers, Enter to toggle expand/collapse, Tab into expanded content.
- **Expand/collapse animation**: CSS `grid-template-rows: 0fr → 1fr` transition (200ms ease-out). Avoid `max-height` hacks.

### Data Flow

**New IPC command**: `get_archived_tasks(projectPath)` — returns archived tasks grouped by session_id, with session metadata (time range, commit count, file count).

**Response shape:**
```typescript
interface ArchivedSession {
  session_id: string
  started_at: string
  ended_at: string
  duration_ms: number
  tasks: UnifiedTask[]        // archived tasks in this session
  commit_count: number        // pre-computed count
  file_count: number          // pre-computed count
  sources: string[]           // which CLI tools contributed
}

interface ArchivedSessionsResult {
  sessions: ArchivedSession[]
  errors: string[]
}
```

**Commit detail**: Fetched on-demand when a session is expanded (not pre-loaded for all sessions). Reuses `enrich_from_session()` backend.

**Fetch strategy**: Load on mount when History sub-tab is selected. No auto-refresh needed (archived data is static). Cache across sub-tab switches.

### States

- **Empty**: "No completed work yet — finished tasks appear here after sessions end." Centered, muted, with subtle icon (same pattern as TaskBoard empty state).
- **Loading**: Skeleton with 3-4 collapsed session headers (pulse animation).
- **Error**: Inline warning (same pattern as TaskBoard error indicators).
- **Populated**: Session list with all collapsed by default.

## Why It Works

**Accordion pattern** (TaurUI `patterns/information-display/accordion.md`): "Content is naturally divided into labeled sections and the user needs only 1-2 at a time. Sections are self-contained. The user's task is focused retrieval: find the section I need, read it, done."

**Information density** (TaurUI `foundations/information-density.md`): Professional tool for expert users. Dense summaries in collapsed state (scan session headers), full detail on demand (expand what you care about). The user doesn't need all sessions open — they're investigating "what did I do last Tuesday?"

**Drill-down** (TaurUI `patterns/information-display/drill-down.md`): The hierarchy is shallow — just session → items. Two levels is within the "sweet spot." No breadcrumb needed because the parent (session header) stays visible above the expanded content.

## Reference Apps

- **Linear**: Issue detail as slide-over panel. Activity feed shows timestamped events grouped by day. Dense, keyboard-first. Study the way activity entries are compact (one line per event) with expand-on-click.
- **Tower Git Client**: "Commit history visualization and branch management UI are best-in-class." Commit list with detail panel — scan, click, see details.
- **GitHub**: Commit history grouped by date. Hash, message, author, relative time. Dense, scannable, navigable.
- **Warp Terminal**: "Block-based output grouping — each command + output is a block." Session-as-block metaphor maps directly.

## Alignment Notes

- **Reuse `TaskDetailPanel`** for task click-through. Same component, same props.
- **Reuse `TOOL_ICONS`** from TaskBoard for source indicators.
- **Reuse `$derived` token pattern** — all dark mode tokens follow the same convention.
- **Commit hash pills** — same `<code>` styling as TaskDetailPanel.
- **File path display** — same dir/name split and contrast pattern as TaskDetailPanel.
- **Sub-tab pills** — follow Manila Folder tab sizing (11px uppercase, same active/inactive pattern as Kanban column headers or the shell tab pills).

## Implementation Checklist

- [ ] DB query for archived tasks grouped by session
- [ ] New IPC command `get_archived_tasks`
- [ ] Frontend IPC wrapper + mock data
- [ ] Sub-tab switcher in TaskBoard (Active | History)
- [ ] SessionHistory component with accordion layout
- [ ] Session header rendering (date, duration, counts, tool icons)
- [ ] Expanded session detail (tasks list, commits list, file count)
- [ ] Task click → TaskDetailPanel integration
- [ ] Empty, loading, error states
- [ ] Dark/light mode styling
- [ ] Keyboard navigation
- [ ] Tests (backend + frontend)
- [ ] Visual review (8 categories, dual scoring)

## Further Reading

- `patterns/information-display/accordion.md` — Full accordion behavior spec
- `patterns/information-display/drill-down.md` — Drill-down navigation
- `patterns/information-display/master-detail.md` — If the view evolves into list + detail layout
- `foundations/information-density.md` — Density spectrum for collapsed vs expanded states
- `lookbook/developer-tools.md` — Tower, Warp block grouping
- `lookbook/productivity.md` — Linear activity feed, Asana timeline
