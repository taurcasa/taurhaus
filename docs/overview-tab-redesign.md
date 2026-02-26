# Overview Tab Redesign Assessment

**Date**: 2026-02-26
**Reviewers**: Claude Opus 4.6 (self) + Gemini 3.1 Pro (cross-review)
**Screenshots**: `Screenshot 2026-02-26 025555.png`, `Screenshot 2026-02-26 025633.png`

---

## Current Layout (top to bottom)

1. Project header — name, branch, activity state
2. Quick Actions — Claude/Codex/Gemini/Terminal launch buttons
3. Last Commit — single commit row
4. Latest Session — handoff summary (often empty)
5. Recent Activity — commit list (repeats Last Commit as first row)
6. README — rendered markdown (H1 stripped)
7. Relationships — auto-detected connections
8. Session History — past sessions (often empty)
9. Project Info — path, created, edit/remove

## Problems Identified

Both reviewers independently flagged the same issues:

### P1: "Last Commit" is redundant
The first row of Recent Activity IS the last commit. Showing it separately adds cognitive load — users read the same commit twice before realizing it's duplicated.

**Fix**: Delete "Last Commit" section entirely. Recent Activity covers it.

### P2: README buried below operational data
When the README was at the top, the project header served as its H1 — the page read like a cohesive document. Now it's pushed below buttons, commits, and sessions. Users must scroll significantly to understand what the project even is.

**Fix**: Move README directly below the header. The project name IS the README's H1.

### P3: Quick Actions occupy prime real estate
The Claude/Codex/Gemini/Terminal buttons are occasional-use actions. The sidebar already provides per-tool launch (context menu) and tool indicator clicks. Four outlined buttons taking the top slot is disproportionate to their usage frequency.

**Fix**: Merge into the header. Right-align as compact icon-only buttons (with tooltips) opposite the project name. Saves ~60px vertical space.

### P4: Empty sections are dead weight
"No sessions imported yet" and "No connections detected yet" messages in a dense developer tool are wasted space. They appear for most projects.

**Fix**: Hide sections entirely when they have no data. Only render when there's actual content to show.

### P5: Arbitrary section ordering
Current flow: actions → status → activity → content → metadata. This is dashboard-widget ordering, not an orientation flow. The primary question is "what is this project?" — the README answers that.

## Proposed Layout

Flow: **Identity** → **Context** → **State** → **Metadata**

### 1. Project Header (keep, absorb quick actions)
- Project name, branch, dirty indicator, activity state (unchanged)
- Quick action icons right-aligned in the header row (compact, icon-only with tooltips)

### 2. README (move to top)
- Directly below header, no section label needed
- H1 still stripped (project name in header serves that role)
- This immediately grounds the user in the project's purpose

### 3. Recent Activity (merged, no separate "Last Commit")
- 5-7 commits, "View all" link to Git tab
- Compact commit rows (hash + message + time)

### 4. Sessions (combined, conditional)
- Merge "Latest Session" + "Session History" into one section
- **Only render when data exists** — no empty state message
- Latest session handoff at top, older sessions below

### 5. Relationships (conditional)
- **Only render when connections exist**
- No "No connections detected yet" message

### 6. Project Info (keep at bottom)
- Path, created date, edit/remove

## Additional UI Notes (from Gemini review)

- Markdown table rendering could use subtle header row background or faint row borders
- Section header vertical rhythm: balance padding above/below labels
- Sidebar vertical alignment of text/pill/dot could be tighter

## Impact

Before: fragmented dashboard of widgets, README buried, redundant data
After: cohesive document — identity first, state second, metadata last
