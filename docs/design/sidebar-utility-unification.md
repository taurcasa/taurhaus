# Sidebar unification — aesthetic overhaul brief

The three surfaces reached from the sidebar's bottom-left strip — **Manage/Add
projects**, **Accounts home**, **Settings** — grew in separate milestones and it
shows; the operator has widened the scope to the **entire sidebar panel**: one
unified look and feel for the whole left rail (project rows, group headers,
activity indicators, team-session grouping visuals, filter, notices, daemon
status, footer strip) plus the three utility surfaces. Aesthetic overhaul only:
no feature changes, no IPC changes, existing behavior and test ids preserved.

## Whole-sidebar scope (operator addition)

Every visual in the sidebar encodes information; the redesign may restyle any of
it but must preserve what each visual *says*:

- Activity states + confidence come from `src/lib/activitySignal.js`
  (`working`/`active`/`idle`/`uncertain`/`offline`) — presentation only, never
  re-derive.
- Team-linked session grouping uses a connector rail + stacked tool logos
  (rationale archived in `docs/archive/design/sidebar-session-grouping.md` and
  `sidebar-team-session-visuals.md`); the grouping information must survive.
- Tool identity comes from `src/lib/toolLogos.js` accents/logos.
- Dirty markers, session counts, group headers, the filter row, sidebar notices,
  and the HoverCard trigger behavior all keep their meaning.
- Current states are screenshotted under `src/test/visual/__screenshots__/`
  (`sidebar/`, `shell/`, `shellPopups/`, `settings/`, `account/`,
  `claudeAccount/`) — study them before designing.

## Current state (the drift, verified in code)

| | Manage projects | Accounts | Settings |
|---|---|---|---|
| Footer control | lone `+` icon, **left** side, no active state | people icon + magnitude badge, right cluster, tone driven by usage signal | gear, right cluster, has open/active tone |
| Surface | overlay **modal** (`AddProjectModal`, three workflows inside) | main-panel **takeover**, sticky backdrop-blur header, `←` text glyph (12px), 16px title, `max-w-5xl` | main-panel **takeover**, no sticky header, chevron+text back link (13px), 20px title, `max-w-[640px]` |
| Icon optics | `w-7 h-7` button, `text-white/20` idle | `h-7 min-w-7` + badge overflow | `w-7 h-7`, different active treatment |

Also in the strip: the daemon status readout (dot + text) sits between the lone
`+` and the right cluster.

## Functional requirements (fixed)

1. All three surfaces stay reachable from the bottom-left strip; Accounts keeps
   its hover usage board and magnitude badge (the ambient signal is a shipped,
   deliberate feature — restyle, don't remove); Settings keeps `settingsOpen`
   toggle semantics; keyboard shortcut hints stay.
2. The daemon status readout stays in the footer (restyle/replace allowed if
   legibility is preserved — it is a health signal, not decoration).
3. Add-project's three workflows (create / manual path / scan) all survive.
   Whether the surface stays a modal or becomes a takeover is a **design
   decision** — argue it from interaction cost, not from symmetry alone.
4. Existing `data-testid`s and a11y labels remain (renames allowed only with the
   matching test updates priced in).
5. Both themes. The sidebar strip itself is always on `bg-brand-950` (dark
   regardless of theme); the opened surfaces follow the app theme.

## Design mandate (freedom)

- One engagement-pattern language for the strip: grouping, order, alignment,
  idle/hover/open tones, badge treatment, icon set consistency (stroke width,
  size, metaphor quality — is `+` the right glyph for "manage projects"?).
- One surface language for what opens: a shared header convention (title scale,
  back affordance, sticky or not, width rhythm) that Settings, Accounts, and
  the projects surface all speak. The two existing takeovers must stop
  disagreeing; the projects surface must stop looking like a stranger.
- Respect the house paradigms: snappy, dense-but-calm, floating-panel frame,
  one dark teal, manila-tab continuity. Geist. Tailwind v4 `@theme` tokens,
  dark mode via `$derived` tokens.

## Deliverables (per concept)

1. A direction summary: the unifying idea in a few sentences, then the concrete
   decisions (strip composition; surface header convention; add-project surface
   verdict; icon optics spec at the token level — sizes, tones, states).
2. A single self-contained HTML mockup file (no external assets beyond the CDN
   allowlist; system-ui fallback for Geist is fine) showing: the full sidebar
   in the new language (project rows across activity states, a team-grouped
   cluster, group headers, filter, notices, daemon status, footer strip with
   idle/hover/open/badge states), and all three utility surfaces in the new
   shared language — both themes, side by side. Mockups are for judging
   fidelity of the *idea*; they need not be pixel-production.
3. A component inventory: file-by-file list of what the implementation lane
   would touch, with churn size estimates.
