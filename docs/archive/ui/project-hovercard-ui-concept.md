# Project HoverCard UI Concept

## Purpose

This document translates the approved HoverCard vision from [project-hovercard-vision.md](/home/user/projects/taurhaus/docs/ui/project-hovercard-vision.md) into an implementation-ready UI concept.

The core rule stays the same:

**The HoverCard is a non-interactive project preview that answers "Should I open this project right now?"**

The card should feel dense but calm, consistent with taurhaus's dark teal frame and Geist typography. It should read as a verdict first and supporting evidence second.

## Recommended Card Spec

- Width: `312px` default
- Compact width: `288px` when viewport is narrow
- Max width: `min(312px, calc(100vw - 24px))`
- Padding: `px-4 py-3.5`
- Border radius: `rounded-xl`
- Border: `1px`
- Shadow: softer spread than the current `shadow-xl`, but still elevated above sidebar content
- Pointer behavior: `pointer-events-none`
- Scroll behavior: avoid internal scrolling in normal cases; collapse content before allowing height growth

Reason for width change:

- `280px` is tight once the card includes a verdict line, one evidence line, and a relationship/risk cue without looking cramped.
- `312px` still preserves tooltip character and keeps the card visually subordinate to the full project view.

## Layout Wireframe

### Default structure

```text
+------------------------------------------------------+
| [project name....................] [branch] [dirty] |
| Active work in progress                              |
| Claude is working now · active 12m                   |
|                                                      |
| Latest change                                        |
| Session: Implement IPC error envelope fix            |
| Open question: Should retry stay frontend-side?      |
|                                                      |
| Related context                                      |
| Depends on mesh-daemon · other side active           |
+------------------------------------------------------+
```

### Section hierarchy

```text
HoverCard
├─ Header row
│  ├─ Project name
│  ├─ Branch chip
│  └─ Optional dirty chip
├─ Verdict block
│  ├─ Attention label
│  └─ One-line why-now explanation
├─ Evidence stack
│  ├─ Motion row
│  ├─ Latest change row
│  └─ Optional unresolved item row
└─ Optional relationship/risk row
```

### Exact vertical rhythm

- Outer padding: `px-4 py-3.5`
- Header to verdict gap: `mt-2`
- Verdict to evidence gap: `mt-3`
- Between evidence rows: `gap-1.5`
- Before relationship row: `mt-3 pt-2.5 border-t`

The card should normally fit within `148px` to `196px` height. It should not grow beyond `232px` in normal desktop use.

## Visual Tokens

## Typography

- Project name: `text-[14px] font-semibold tracking-[-0.01em] leading-[1.15] font-sans`
- Branch chip text: `text-[10px] font-mono leading-none`
- Status chip text: `text-[10px] font-medium leading-none`
- Verdict line: `text-[13px] font-medium leading-[1.25]`
- Evidence label: `text-[10px] uppercase tracking-[0.08em] font-medium`
- Evidence body: `text-[12px] leading-[1.35]`
- Secondary evidence / timestamps: `text-[11px] leading-[1.3]`
- Fallback path / technical git hint if needed: `text-[10px] font-mono`

## Surface tokens

### Dark mode

- Card background: `bg-brand-950/96`
- Card border: `border-white/[0.08]`
- Card shadow: `shadow-[0_14px_34px_rgba(0,0,0,0.34)]`
- Internal divider: `border-white/[0.07]`
- Project name: `text-zinc-100`
- Verdict text: `text-zinc-100`
- Evidence body: `text-zinc-300`
- Secondary text: `text-zinc-400`
- Muted text: `text-zinc-500`
- Quiet fallback text: `text-zinc-500`

### Light mode

- Card background: `bg-white/96`
- Card border: `border-brand-900/10`
- Card shadow: `shadow-[0_12px_28px_rgba(15,23,42,0.14)]`
- Internal divider: `border-brand-900/10`
- Project name: `text-zinc-900`
- Verdict text: `text-zinc-900`
- Evidence body: `text-zinc-700`
- Secondary text: `text-zinc-500`
- Muted text: `text-zinc-400`
- Quiet fallback text: `text-zinc-400`

## Accent tokens

### Attention states

- Active: `text-success-400` dark, `text-success-600` light
- Waiting / blocked: `text-warning-300` dark, `text-warning-600` light
- Recent but quiet: `text-info-300` dark, `text-info-600` light
- Dormant / low urgency: `text-zinc-400` dark, `text-zinc-500` light
- Dirty chip accent: `bg-warning-400/12 text-warning-300 border-warning-400/20` dark, `bg-warning-50 text-warning-600 border-warning-500/20` light
- Relationship chip accent: `bg-brand-400/10 text-brand-300 border-brand-400/20` dark, `bg-brand-50 text-brand-700 border-brand-600/15` light

### Branch chip

- Dark: `bg-white/[0.04] text-zinc-400 border border-white/[0.08]`
- Light: `bg-zinc-50 text-zinc-500 border border-zinc-200`

## Recommended outer container classes

### Dark

```text
fixed z-[90] w-[312px] max-w-[calc(100vw-24px)] rounded-xl border border-white/[0.08] bg-brand-950/96 shadow-[0_14px_34px_rgba(0,0,0,0.34)] px-4 py-3.5 pointer-events-none backdrop-blur-[6px]
```

### Light

```text
fixed z-[90] w-[312px] max-w-[calc(100vw-24px)] rounded-xl border border-brand-900/10 bg-white/96 shadow-[0_12px_28px_rgba(15,23,42,0.14)] px-4 py-3.5 pointer-events-none backdrop-blur-[6px]
```

## Content Mapping

| Data field | Visual representation | Notes |
|------|------|------|
| `project.name` | `text-[14px] font-semibold truncate text-zinc-100` dark / `text-zinc-900` light | Single line only |
| `project.branch` | small chip, `text-[10px] font-mono px-1.5 py-1 rounded-md` | Hide if empty |
| `project.isDirty` | dirty chip with warning tint | Show only if true |
| `project.activityState` | not shown raw as a standalone row; used to derive verdict tone and accent color | Avoid `Active · Dirty · Recent` style dump |
| live session present | first evidence row with tool name + state + compact timing | Preferred evidence source |
| live tool icon | optional 11px icon before motion copy | Use only if it helps scan; not required for every state |
| session working/idle/unattributed | sentence fragment in motion row | Example: `Claude is waiting on input` |
| session duration | right-side secondary text or inline suffix | Example: `· active 12m` |
| latest session summary | primary "Latest change" row body | Preferred over commit message when fresh |
| `latestSession.open_questions[0]` | secondary unresolved line | Prefix with `Open question:` |
| `latestSession.next_steps[0]` | fallback unresolved line | Prefix with `Next:` if no open questions |
| latest commit message | fallback "Latest change" body | Used when no fresh handoff exists |
| latest commit hash | optional muted suffix only if needed for ambiguity | Do not lead with hash |
| git ahead/behind | optional compressed note in motion or latest-change block | Only if fetched cheaply |
| relationship summary | final row with compact chip + one line | Hide when low-signal |
| no live session | motion row becomes quiet-state evidence | Example: `No live agent session` |
| no latest change | muted evidence text | Example: `No session handoff or recent commit yet` |

## Detailed Layout Spec

### 1. Header row

Structure:

- Left: project name
- Right cluster: branch chip, dirty chip

Classes:

- Row: `flex items-start gap-2`
- Name container: `min-w-0 flex-1`
- Name: `truncate`
- Right cluster: `flex items-center gap-1.5 shrink-0 pt-0.5`

Behavior:

- Project name is always one line.
- Branch chip truncates to `max-w-[88px]`.
- Dirty chip never wraps.

### 2. Verdict block

Structure:

- Line 1: verdict text
- Line 2: optional why-now explanation

Examples:

- `Active work in progress`
- `Waiting on user input`
- `Recent change, no live session`
- `Quiet project, no recent handoff`

Classes:

- Block: `mt-2`
- Verdict: `text-[13px] font-medium leading-[1.25]`
- Why-now: `mt-1 text-[11px] leading-[1.3]`

Color:

- Verdict line uses urgency color
- Why-now line uses secondary text tone

### 3. Evidence stack

Evidence rows should use a label/value pattern.

Structure:

- Label in uppercase microcopy
- One or two lines of body text

Classes:

- Stack: `mt-3 grid gap-1.5`
- Row: `rounded-lg px-2.5 py-2 border`
- Label: `text-[10px] uppercase tracking-[0.08em]`
- Body: `mt-0.5 text-[12px] leading-[1.35]`
- Subline: `mt-0.5 text-[11px]`

Dark row surface:

- `border-white/[0.06] bg-white/[0.03]`

Light row surface:

- `border-brand-900/8 bg-brand-50/35`

#### Motion row

Purpose:

- show whether work is moving right now

Examples:

- `Claude is working now`
- `Codex is waiting on input`
- `Gemini shows project activity without attribution`
- `No live agent session`

Timing suffix examples:

- `· active 12m`
- `· idle 34m`
- `· last active yesterday`

#### Latest change row

Purpose:

- answer what changed since last visit

Preferred source order:

1. fresh latest session summary
2. latest commit message
3. empty-state fallback

Examples:

- `Session: Implement session-indicator fix`
- `Commit: normalize IPC error envelope handling`
- `No session handoff or recent commit yet`

#### Unresolved item row

Show only if available.

Examples:

- `Open question: should retry stay frontend-side?`
- `Next: verify mesh daemon startup logs on Windows`

This row should collapse entirely when absent.

### 4. Relationship / risk row

Purpose:

- surface dependency context only when it affects project priority

Structure:

- top border separator
- compact chip
- one evidence line

Classes:

- Container: `mt-3 pt-2.5 border-t`
- Chip: `inline-flex items-center rounded-md px-1.5 py-1 text-[10px] font-medium border`
- Body: `mt-1 text-[11px] leading-[1.3]`

Examples:

- Chip: `Depends on`
- Body: `mesh-daemon is also active`

- Chip: `Referenced by`
- Body: `taurhaus-shell changed recently`

Hide the entire section when there is no strong relationship signal.

## Interaction Details

## Trigger timing

- Enter delay: `100ms`
- Exit delay: `70ms`
- Re-hover on adjacent project rows: no extra dwell once card is already visible; update content immediately on row switch

Reasoning:

- `80ms` is acceptable, but `100ms` gives slightly better protection against accidental flicker without feeling delayed.
- Faster exit keeps sidebar scanning responsive.

## Motion

- Enter transition: opacity + `translateY(2px)` + scale `0.985 -> 1`
- Duration: `120ms`
- Easing: `ease-out`
- Exit transition: opacity only
- Duration: `70ms`

Suggested class approach:

```text
transition-[opacity,transform] duration-120 ease-out
```

or a dedicated tooltip keyframe if the implementation wants exact control.

Reduced motion:

- disable scale/translate
- keep fast opacity fade or no animation

## Positioning rules

- Prefer right side of hovered sidebar row
- Horizontal gap from row: `10px`
- If right side would overflow viewport, place card on the left
- Vertical anchor: align card center to row center
- Clamp top and bottom to `12px` viewport inset
- Never allow card edge to touch window frame

Additional rule for narrow side-panel layouts:

- if both left and right placements are tight, reduce width to compact mode before changing vertical behavior

## Non-interactive behavior

- Keep `pointer-events-none`
- Do not include buttons, links, or hover-only internal affordances
- The card disappears entirely when the row loses hover for the configured delay

## Edge Cases

### No sessions, no latest session, no recent commits

Show:

- Verdict: `Quiet project`
- Why-now: `No live work or recent handoff`
- Motion row: `No live agent session`
- Latest change row: `No session handoff or recent commit yet`

Do not show empty separators or blank sections.

### New project with branch but little history

Show:

- branch chip if available
- quiet verdict
- latest change fallback text

Do not imply missing data is an error.

### Long project names

- Keep to one line with `truncate`
- `title` attribute optional for native full-name tooltip
- Never allow the name to wrap into the evidence stack

### Long branch names

- Truncate chip at `88px`
- Preserve left edge so branch family remains recognizable

### Very long session summaries or commit messages

- Clamp to 2 lines in latest-change body
- Unresolved row clamps to 2 lines
- Prefer sentence truncation over card height expansion

### Multiple live sessions

Do not list them all.

Resolution:

- pick the most relevant session for the motion row
- prioritization: active > unattributed project activity > waiting
- optionally append count suffix: `+2 more`

### Unknown activity state

Do not print `Unknown` as the primary verdict.

Fallback:

- derive verdict from available evidence
- if nothing else exists, use `Project status unavailable`

### Relationship noise

If relationships exist but none affect urgency, hide the section entirely.

## Responsive Behavior

## Typical desktop

- Width: `312px`
- Full layout with header, verdict, two or three evidence rows, optional relationship row

## Narrow side-panel or small app window

Trigger condition:

- viewport width below roughly `1180px`, or placement space below `312px + 24px`

Behavior:

- width reduces to `288px`
- branch chip max width reduces to `72px`
- relationship body clamps to 1 line
- unresolved row hides before core rows do

Priority for compression:

1. hide unresolved row
2. clamp relationship row harder
3. reduce width to `288px`
4. clamp why-now text to 1 line

Do not reduce text sizes below the defined spec.

## Very tall ultrawide layout with sidebar far from main content

Behavior:

- keep default width
- retain right-side placement preference
- do not increase card size just because space exists

The card should remain compact and preview-like even on large monitors.

## Content Priority Rules

When not all data is available, preserve this order:

1. project identity
2. verdict
3. motion row
4. latest change row
5. unresolved row
6. relationship row

This ensures the card still works even if some IPC calls fail or are still loading.

## Implementation Notes For Phase 3

- Reuse existing sidebar trigger architecture in [Sidebar.svelte](/home/user/projects/taurhaus/src/lib/Sidebar.svelte), but allow separate enter and exit timing constants.
- Replace the current commit-list and session-metadata layout in [HoverCard.svelte](/home/user/projects/taurhaus/src/lib/HoverCard.svelte) with a verdict-first stack.
- Prefer latest-session narrative over historical project-activity totals.
- Keep the card strictly presentational and non-interactive.

## Summary

The new HoverCard should feel like a calm operational headline:

- identity
- verdict
- proof

Not a mini overview, not a debug panel, and not a second sidebar. If implemented to this spec, the card should be faster to scan, visually quieter, and more useful during rapid project selection.
