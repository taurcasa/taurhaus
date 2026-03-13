# Project-Branch Presentation — Design Options

**Date**: 2026-03-13
**Task**: #1270
**Author**: design-taurhaus

---

## Problem

Long feature branch names crowd out project names in the sidebar. The branch
pill is `shrink-0` with no max-width, so it claims as much horizontal space as
it needs. Short default branches ("main", "master") are fine, but a typical
feature branch like `feature/clear-overhaul` or `fix/user-auth-timeout-handling`
consumes most of the 252px sidebar width.

**Screenshot reference**: `Screenshot 2026-03-13 164615.png` — the `claude_config`
row shows a `feature/clear-overhaul` pill that visibly compresses the project name.

### Current Layout Anatomy (left → right)

```
[3px selection bar?] [project name (flex-1, truncate)] [tool logos (shrink-0)] [branch pill (shrink-0)] [dirty dot (shrink-0)]
```

- Row: `h-[36px]`, `px-3`, `gap-2`, `overflow-hidden`
- Project name: `text-[14px] truncate flex-1` — gives up space first
- Branch pill: `text-[10px] font-mono shrink-0 px-1.5 py-0.5 rounded` — never shrinks
- Effective content width: ~208px (252px minus padding/gaps)

### Budget Math

At `10px` Geist Mono (~6px per char):
- "main" = ~24px + 12px padding = ~36px — fits comfortably
- "feature/clear-overhaul" = ~132px + 12px = ~144px — leaves ~64px for project name + indicators

The project name at 14px Geist needs at least ~80px to show a meaningful prefix
(e.g., "claude_con..."). With 3 tool logos (~48px), the name is crushed to near-zero.

### User Need

Branch context matters most when it's *not* the default branch — that's when
users need to know "am I still on that feature branch?" The default branch is
the expected state and carries little information value.

---

## Proposal A: Capped Pill

**Philosophy**: Show it, but shorter. Minimal change.

### Design

Add a max-width cap to the branch pill. Default branches fit naturally; long
branches truncate with ellipsis. Full name is already visible in HoverCard on
hover.

```
Sidebar row (short branch — no change):
┌────────────────────────────────────────────┐
│ taurhaus          ⬡ ⬡ ⬡    main     •    │
└────────────────────────────────────────────┘

Sidebar row (long branch — truncated):
┌────────────────────────────────────────────┐
│ claude_config       ⬡    feature/cl…  •    │
└────────────────────────────────────────────┘
```

### Implementation

Single CSS change on the branch `<span>` at line 197 of `SidebarProjectList.svelte`:

```diff
- class="text-[10px] font-mono shrink-0 px-1.5 py-0.5 rounded ..."
+ class="text-[10px] font-mono shrink-0 max-w-[72px] truncate px-1.5 py-0.5 rounded ..."
```

72px cap shows ~10 monospace chars — enough for "main", "master", "develop",
and the meaningful prefix of most feature branches ("feature/cl…", "fix/login…").

### Pros

- **Zero layout disruption**: row height unchanged, element order unchanged
- **Trivial implementation**: 2 Tailwind classes added
- **Default branches unaffected**: "main"/"master"/"develop" all fit under 72px
- **Full text accessible**: HoverCard already shows branch (already has its own
  `max-w-[88px] truncate` in HoverCard.svelte line 466)

### Cons

- **Reduced scannability**: truncated branches are harder to distinguish at a
  glance — "feature/cl…" vs "feature/co…" requires hover
- **Ellipsis adds visual noise**: many truncated pills in a row can feel cluttered
- **Doesn't address density**: pill still occupies ~72px even when the stem is
  only a few chars longer than the cap

### Design Intent

Every row stays one-line, fixed-height. Branch remains visible as a contextual
hint. Users who need the full name hover to reveal it. This is the safe,
incremental option.

---

## Proposal B: Second-Line Branch

**Philosophy**: Give branches their own space. Project name never competes.

### Design

Default branches ("main", "master", "develop") are hidden — they carry no
information value. Non-default branches drop to a dedicated second line below
the project name, using the full row width.

```
Sidebar row (default branch — pill hidden):
┌────────────────────────────────────────────┐
│ taurhaus            ⬡ ⬡ ⬡            •    │
└────────────────────────────────────────────┘

Sidebar row (feature branch — two lines):
┌────────────────────────────────────────────┐
│ claude_config         ⬡               •    │
│  ⑂ feature/clear-overhaul                  │
└────────────────────────────────────────────┘
```

Row height: 36px for default-branch rows, ~50px for feature-branch rows.
Second line: `text-[10px] font-mono text-white/20`, left-aligned with a subtle
branch glyph prefix (⑂ or similar).

### Implementation

Restructure the row to a `flex-col` wrapper when branch is non-default:

```svelte
{#if project.branch && !isDefaultBranch(project.branch)}
  <span class="text-[10px] font-mono text-white/20 truncate w-full pl-0.5 -mt-0.5">
    ⑂ {project.branch}
  </span>
{/if}
```

Where `isDefaultBranch()` checks against `["main", "master", "develop"]`.

### Pros

- **Project name always has full first-line width**: no competition with branch
- **Branch gets full row width**: even very long names like
  `feature/user-auth-timeout-handling-v2` can display ~35 chars before truncating
- **Information hierarchy is clear**: primary (name) on line 1, secondary
  (branch) on line 2
- **Default branches disappear**: reduces clutter for the common case — most
  projects are on "main"

### Cons

- **Variable row heights**: disrupts the uniform 36px grid, makes the sidebar
  feel less predictable
- **More vertical space**: each feature-branch project takes ~14px extra — if
  5 projects are on feature branches, that's 70px of added scroll
- **Visual weight shift**: two-line rows draw more attention, which may not
  match their importance
- **Layout jump on branch switch**: switching from main → feature branch changes
  the row height, causing a subtle layout shift in the list

### Design Intent

The sidebar's primary job is project identification. Branch context is secondary.
This proposal enforces that hierarchy structurally: names always win, branches
only appear when they add information (non-default), and they get their own
dedicated space rather than stealing from the name.

---

## Proposal C: Icon Marker + Hover Reveal

**Philosophy**: Hide it until needed. Maximum density.

### Design

Default branches: no branch indicator at all (clean row). Non-default branches:
show only a tiny branch-divergence glyph (⑂) as an inline marker. The full
branch name is revealed on hover via HoverCard (which already exists and already
shows branch info) or when the project is selected.

```
Sidebar row (default branch — no indicator):
┌────────────────────────────────────────────┐
│ taurhaus            ⬡ ⬡ ⬡            •    │
└────────────────────────────────────────────┘

Sidebar row (feature branch — icon only):
┌────────────────────────────────────────────┐
│ claude_config         ⬡             ⑂ •    │
└────────────────────────────────────────────┘

HoverCard (on hover — already exists):
┌──────────────────────────────┐
│ claude_config                │
│ ⑂ feature/clear-overhaul    │
│ ...session details...        │
└──────────────────────────────┘
```

The ⑂ glyph is ~14px wide (same size as a tool logo), styled
`text-[10px] text-white/25` — subtle enough to blend with existing indicators
but visible enough to signal "not on default branch."

### Implementation

Replace the branch pill with a conditional icon:

```svelte
{#if project.branch && !isDefaultBranch(project.branch)}
  <span class="text-[10px] text-white/25 shrink-0" title={project.branch}>⑂</span>
{/if}
```

HoverCard already shows branch info (line 465-468 of HoverCard.svelte) — no
changes needed there. Optionally, expand the HoverCard branch chip to remove
its current `max-w-[88px]` truncation so the full name is always visible on
hover.

### Pros

- **Maximum density**: branch indicator is 14px vs 36-144px for a text pill
- **Project names get full width**: zero competition from branch text
- **Clean visual rhythm**: uniform row heights, no text-heavy pills breaking
  the pattern
- **Default branches invisible**: same benefit as Proposal B — no clutter for
  the common case
- **Leverages existing UI**: HoverCard already shows branch context; this just
  leans into that pattern more fully

### Cons

- **Requires hover to see branch name**: not visible at a glance — users must
  hover or select to know *which* branch
- **Less discoverable**: new users may not realize ⑂ means "non-default branch"
  without learning the icon
- **Weakens branch awareness**: users who switch branches frequently lose the
  constant visual reminder of which branch each project is on
- **Breaks parity with existing mental model**: users accustomed to the branch
  pill will miss it

### Design Intent

Branch information is consulted, not monitored. Most of the time, users know
which branch they're on. The rare moment they need to check, they hover. This
trades a small convenience loss for a significant density and readability gain
across every single row in the sidebar.

---

## Comparison Matrix

| Criterion                      | A: Capped Pill  | B: Second Line   | C: Icon + Hover  |
|-------------------------------|-----------------|------------------|------------------|
| Project name readability       | Good            | Best             | Best             |
| Branch visibility at a glance  | Good (truncated)| Best (full text) | Poor (icon only) |
| Row height consistency         | Unchanged       | Variable         | Unchanged        |
| Implementation complexity      | Trivial (CSS)   | Moderate (layout)| Low (conditional)|
| Sidebar density                | Same            | Worse            | Best             |
| Default-branch clutter removed | No              | Yes              | Yes              |
| Requires hover for full info   | Yes (long only) | Rarely           | Always (non-def) |

---

## Recommendation

**Proposal A (Capped Pill)** is the safest path — near-zero risk, immediate
improvement, ships in minutes. It's the right fix if the goal is to stop the
bleeding without rethinking the layout.

**Proposal B (Second Line)** is the strongest design if branch context is
genuinely important to the workflow. It respects both the project name and
the branch name. The variable row height is a real cost but is bounded (only
affects projects on non-default branches, which are typically a minority).

**Proposal C (Icon + Hover)** is the most opinionated. It produces the cleanest
sidebar but requires a bet that users don't actively monitor branch names — they
only check occasionally. For a power-user tool where most projects live on "main"
and only 1-2 are on feature branches at any time, this bet is probably correct.

A hybrid approach is also viable: start with **A** as an immediate fix, then
evaluate whether **B** or **C** is warranted based on how many projects
typically show non-default branches in practice.
