# Project-Branch Presentation — Design Review

**Date**: 2026-03-13
**Task**: #1273
**Reviewer**: design-taurhaus
**Surface**: Sidebar project list — two-line branch presentation (Proposal B implementation)
**Source**: `src/lib/SidebarProjectList.svelte`

---

## Pre-scoring Check

> "If this screen were deleted, would users notice and complain?"

Yes. The sidebar is the primary navigation surface. Every interaction starts
here. The branch display is part of how users identify and orient to their
projects — especially when working across multiple feature branches.

---

## Verdict

**APPROVE**

Score: **45 / 50** (all categories 9/10)

---

## Rubric

### 1. Product Value — 9/10

**Can a non-builder state what this is for?**
Yes — "it shows which git branch each project is on."

**Does it answer the primary user question?**
Yes. The original problem was: long feature branch names crush project names
into unreadable truncations. The implementation solves this completely. Project
names now always get the full first line. Branch names get the full second line.
Neither competes with the other.

**Would a user know what to do next?**
Yes — click the project row to open it. The branch context is informational,
not interactive.

**Observations:**
- Default branches (main/master/develop) are correctly hidden — this removes
  noise from the common case where branch info carries zero signal.
- The 50px row height for feature-branch projects creates a subtle visual flag
  that draws appropriate attention to "this project is on a non-default branch."
- The ⑂ glyph prefix gives instant semantic context without taking meaningful
  space.

### 2. Comprehension & Copy — 9/10

- Branch names are rendered verbatim — no transformation, no abbreviation, no
  system language. Users see exactly what `git branch` would show.
- The ⑂ glyph is the standard branch/fork symbol. Developers recognize it
  immediately. No label needed.
- Opacity distinction (unselected `text-white/20`, selected `text-white/35`)
  correctly signals that the branch is secondary context, not primary content.
- No copy reads like system or model language — everything is user-facing data.

### 3. Layout & Hierarchy — 9/10

**Structural analysis of the implementation:**

```
<button h-[36px] | h-[50px] py-1.5>
  <span flex items-start gap-2>
    [selection indicator (mt-2, conditional)]
    <span flex-1 min-w-0>
      <span flex items-center gap-2>   ← first line
        [project name (14px, truncate, flex-1)]
        [tool indicators (shrink-0)]
        [dirty dot (shrink-0)]
      </span>
      [branch line (10px mono, conditional)]  ← second line
    </span>
  </span>
</button>
```

- Two-tier hierarchy is structurally enforced: project name at 14px on line 1,
  branch at 10px mono on line 2. No ambiguity about which is primary.
- `items-start` on the outer flex correctly top-aligns the selection indicator
  with the project name. The `mt-2` offset on the indicator vertically centers
  it against the text baseline — precise detail work.
- For 36px rows (no branch): button's default vertical centering handles it.
  For 50px rows: `py-1.5` (6px top/bottom) provides balanced padding for the
  two text lines (~36px content height).
- `pl-0.5` on the branch line creates a 2px indent relative to the project
  name — a subtle but effective hierarchy cue (branch is "under" the name).
- Foreground active bars (`absolute top-0 / bottom-0`) work correctly at both
  row heights.
- `min-w-0` on flex children is correct for enabling truncation in flex layouts.

**One minor note:** The old `<button>` had `flex items-center gap-2` directly.
The new one delegates layout to an inner `<span>`. This is a valid pattern —
the button just provides the interactive surface, the inner span owns layout.
Browser default button centering handles the 36px case.

### 4. State Coverage — 9/10

| State | Handled | Notes |
|-------|---------|-------|
| Default branch (main/master/develop) | Yes | Pill hidden, 36px row |
| Feature branch | Yes | Second line shown, 50px row |
| Selected project | Yes | Branch brightens to `text-white/35` |
| Unselected project | Yes | Branch dims to `text-white/20` |
| Very long branch name | Yes | `truncate` class clips with ellipsis |
| Empty/null branch | Yes | `normalizedBranch()` returns '' → `branchLine()` returns null |
| Whitespace-only branch | Yes | `trim()` in normalizer handles it |
| Case variants (Main, MASTER) | Yes | `toLowerCase()` in `isDefaultBranch()` |
| Dirty indicator | Yes | Stays on first line, not displaced |
| Foreground active | Yes | Absolute-positioned bars work at both heights |
| Context menu highlight | Yes | Background tint unchanged |
| Tool indicators | Yes | First line, same position as before |

No missing edge cases identified.

### 5. Token & Pattern Consistency — 9/10

- `text-[10px]` + `font-mono`: matches existing branch pill and HoverCard
  branch chip styling.
- `text-white/20` and `text-white/35`: consistent with the sidebar's opacity
  palette (headers use `/35`, project names use `/75`, dimmed elements use
  `/20`-`/30`).
- `truncate` + `min-w-0`: same overflow pattern used on project names — no new
  pattern introduced.
- `h-[50px]` is a new arbitrary value. Acceptable — `h-[36px]` is already
  arbitrary, and the 14px delta matches the second line's content height.
- `mt-0.5` and `pl-0.5` are standard Tailwind spacing tokens, not custom
  values.
- `data-testid="sidebar-branch-line"`: follows existing testid conventions
  (`sidebar-selection-indicator`, `sidebar-foreground-indicator`).
- No hardcoded colors — all values use opacity-based `white/` variants against
  the dark teal sidebar.

---

## Summary

The implementation is a faithful and well-executed rendering of Proposal B.
The core problem — long branch names crushing project names — is fully solved.
The structural approach (default branches hidden, feature branches on a
dedicated second line) is the right trade-off for this tool's use case.

Code quality is clean: defensive normalization, proper flex truncation patterns,
state coverage across all interactive states, and a testid for the new element.
No issues requiring changes.
