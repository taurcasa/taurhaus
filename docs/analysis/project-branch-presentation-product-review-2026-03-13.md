# Project-Branch Presentation — Product Review

**Date**: 2026-03-13
**Task**: #1274
**Reviewer**: product-check-1 (operator/product perspective)
**Design doc**: `project-branch-presentation-design-options-2026-03-13.md` (#1270)
**Implementation**: `SidebarProjectList.svelte` (#1272)

---

## What Was Implemented

**Proposal B (Second Line)** from the design doc, implemented cleanly:

### Behavior

| Scenario | Row height | Branch display |
|----------|-----------|---------------|
| Default branch (main/master/develop) | 36px | Hidden — no branch indicator |
| Non-default branch | 50px | Second line: `⑂ feature/branch-name` |
| No branch data | 36px | Hidden |

### Code structure (SidebarProjectList.svelte)

- **Helper functions** (lines 53-68): `normalizedBranch()`, `isDefaultBranch()` (checks main/master/develop, case-insensitive), `branchLine()` returns null for defaults
- **Row height** (line 123): `h-[50px] py-1.5` for two-line, `h-[36px]` for single
- **Layout** (lines 130-229): Outer flex container uses `items-start` (not `items-center`), inner `<span class="min-w-0 flex-1">` wraps both lines vertically
- **Branch line** (lines 222-227): `data-testid="sidebar-branch-line"`, `text-[10px] font-mono`, `text-white/35` selected / `text-white/20` unselected, ⑂ glyph prefix, `truncate` for overflow

---

## Product Assessment

### What works well

**1. Information hierarchy is correct.**
The project name owns the first line — full width minus indicators. The branch is clearly secondary on a separate line. You never have to wonder which text is the project name and which is the branch. This was the core problem and it's solved.

**2. Default branch suppression is the right call.**
Hiding "main"/"master"/"develop" eliminates the most common visual noise. Most projects sit on their default branch; showing a pill that says "main" on every row adds zero information. Only non-default branches appear, which is exactly when the user needs to know.

**3. The ⑂ glyph is a good choice.**
It's semantically correct (Unicode branch symbol), visually compact, and distinctive enough to scan without being heavy. Combined with the monospace font and subdued opacity (`text-white/20`), it reads as metadata rather than primary content.

**4. Full branch name is visible without hovering.**
Unlike Proposal C (icon-only), the user can see the actual branch name at a glance. For workflows where 2-3 projects are on feature branches simultaneously, this is valuable — you can tell them apart without hovering.

**5. The `truncate` on the branch line is correct.**
Even at full row width, some branch names will exceed ~35 chars. The fallback to HoverCard for the full name is already in place (HoverCard.svelte shows branch info).

### Concerns

**1. Variable row height disrupts visual rhythm.**
The 36px → 50px height change creates an uneven grid. In a sidebar with 10 projects where 3 are on feature branches, the alternating heights make the list feel less scannable as a whole. This is the primary trade-off of Proposal B and it's real.

**Severity: Low-Medium.** Acceptable because:
- Feature branches are typically a minority (1-3 out of 10+ projects)
- The 50px height is consistent across all feature-branch rows
- The sidebar's primary scan target is project names, which stay on line 1 at the same position regardless of row height

**2. Layout shift on branch change.**
When a user switches branches (e.g., `git checkout feature/foo` → `git checkout main`), the row height will jump from 50px to 36px, shifting everything below it. This could be momentarily disorienting.

**Severity: Low.** Branch switches happen infrequently (maybe a few times per day), and the shift is small (14px). The file watcher debounce (2s for .git internals) means it won't happen instantly during rapid operations.

**3. Two-line rows draw disproportionate visual weight.**
Feature-branch rows are visually heavier (more text, taller) than default-branch rows. This makes them "pop" — which could be seen as a feature (draws attention to the unusual state) or a bug (unearned visual prominence).

**Severity: Neutral.** For a project management tool, drawing attention to "you're on a feature branch" is arguably correct behavior. Feature branches are temporary and deserve a visual signal.

**4. The `py-1.5` vertical padding on two-line rows.**
Two-line rows use `py-1.5` (12px total vertical padding) while single-line rows have no explicit vertical padding (height is fixed at 36px with content centered). This means the internal spacing model differs between the two row types.

**Severity: Negligible.** The visual result looks fine — the second line fits naturally within the 50px. This is an implementation detail, not a user-facing concern.

**5. Selection indicator alignment.**
The selection bar (`w-[3px] h-3.5 bg-brand-400 rounded-full`) uses `mt-2` which aligns it with the first line. On two-line rows, it's top-aligned rather than vertically centered. This is actually correct — it should point at the project name, not the branch.

**Severity: None.** Working as intended.

---

## Comparison with Design Doc Predictions

| Design doc concern | Actual result |
|-------------------|--------------|
| "Variable row heights disrupts uniform 36px grid" | Confirmed but acceptable — minority of rows affected |
| "~14px extra per feature branch" | Confirmed: 50px - 36px = 14px |
| "Visual weight shift" | Confirmed but arguably a feature for branch awareness |
| "Layout jump on branch switch" | Confirmed but low severity — infrequent event |
| "Project name always has full first-line width" | Confirmed — working exactly as designed |
| "Branch gets full row width" | Confirmed — ~35 chars before truncation |
| "Information hierarchy is clear" | Confirmed — strong visual separation |
| "Default branches disappear" | Confirmed — only non-default shown |

---

## Edge Cases to Monitor

1. **Many feature branches at once** (e.g., 8+ out of 10 projects): The variable-height rows would dominate the sidebar. If this becomes common in practice, consider whether the extra 14px per row is worth it or whether a capped-pill fallback (Proposal A's `max-w-[72px]`) should kick in at high density.

2. **Very long branch names** (e.g., `feature/user-authentication-timeout-handling-with-retry-logic`): The `truncate` handles this, but at `text-[10px] font-mono` (~6px/char), the branch line can show ~35 chars before truncating. This is usually enough to identify the branch.

3. **Branch name that matches default** but with prefix (e.g., `fix/main-page-auth`): `isDefaultBranch()` correctly only hides exact matches of "main", "master", "develop" — this case is handled correctly.

---

## Verdict

**Ship it.** The implementation matches Proposal B faithfully, the information hierarchy is correct, and the trade-offs (variable row height, visual weight) are acceptable for the expected use case (1-3 feature branches among 10+ projects).

No blocking issues found. The two concerns worth watching over time are:
1. If many projects on feature branches becomes common, the variable height adds up
2. Layout shift on branch change is minor but noticeable

Neither warrants blocking or rework now.
