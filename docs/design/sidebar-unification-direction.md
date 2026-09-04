# Sidebar unification — chosen direction (approved)

Operator-approved graft of the two concepts from
[`sidebar-utility-unification.md`](sidebar-utility-unification.md): **Concept B's
device written in Concept A's law.** The mockups are archived as artifacts
("Quiet Unification", "Drawer Rail"); this document is the binding summary.

## The device (from Concept B — "The Drawer")

The sidebar is the drawer; **anything the main panel is currently showing is
rendered in the main panel's own surface material — "pulled."** Material means
exactly one thing: shown-ness.

1. **Pulled project row**: the selected row is made of the panel surface
   (theme-following), flush to the rail's right edge with the house inverse
   scoops implying continuation into the panel. Name goes semibold; branch line
   and marks flip to panel-legible tones.
2. **Pulled footer key**: when a utility surface (Projects / Accounts /
   Settings) occupies the panel, its footer key pulls instead — and the
   selected project row **demotes to a quiet "held" state** (soft fill, keeps
   its selection handle). This fixes the shipped falsehood where the selected
   row keeps glowing while Settings occupies the panel. At most one element is
   ever made of panel material.
3. **Guide-card group headers**: sentence-case tab chip sitting on a hairline
   (drawer guide cards), carrying the group's project count. The 10px ALL-CAPS
   register dies in the list.
4. **Key-echo surface headers**: every surface opens behind one sticky doorway
   header that repeats the exact icon+name key that opened it, plus back
   affordance, optional meta slot, one optional action, and the Esc hint.

## The law (from Concept A — "Quiet unification")

- **Rail tone ramp** (every interactive white on brand-950): idle white/.30 →
  signaled idle /.55 → hover /.60 + fill .05 → open = pulled material. Keyboard
  focus = the filter's brand-500/.70 ring, rail-wide.
- **Geometry grid**: 28px control hits · 16px utility glyphs stroke 1.5 ·
  14px mark boxes/badges/pills · 12px tool marks · 36px rows · 44px footer ·
  48px doorway header · radii 6/8/999 only · surface widths 640 (reading) and
  1024 (board).
- **Badge grammar**: one 14px pill geometry, 10px tabular-nums. **Outline =
  count** (team size with group-activity-tone border; workflow runs in quiet
  teal). **Filled = act on me** (accounts magnitude: danger = spent or signed
  out, warning = pressure). Concept B's text badges ("91%", "Sign in") are
  rejected — magnitude number only, tones carry severity.
- **Icon registers**: utility icons are outline nouns (16px, stroke 1.5, round
  joins); tool marks are filled brand geometry 12-in-14 where **shape =
  identity, color = activity** (verbatim shipped semantics). The `+` dies; the
  Projects key takes the folder glyph.
- **Two-signals law**: selection (handle + fill / pulled) and tmux foreground
  (paired 2px edge lines) remain two distinct signals; they can disagree, so
  they never merge. On a pulled row the edge lines run to the rail edge.

## Conflict rulings (the graft decisions)

| Conflict | Ruling |
|---|---|
| Footer arrangement | **B**: one key cluster bottom-left (Projects · Accounts · Settings), daemon readout right — deliberately un-key-like, a vital sign, not a door. |
| Badge on the Accounts key | **A's grammar**: filled magnitude pill (number), warning/danger tones. No text badges. |
| Doorway height/back | 48px (A's grid) with **B's key-echo** (icon+name) and A's labeled back (chevron + "Back", 12px) + Esc hint (B). Meta slot + one ghost action max. |
| Surface widths | 640 reading (Projects, Settings) / 1024 board (Accounts) — A's tokens. |
| Group headers | **B's guide cards** with counts. |
| Selected-row treatment | **B's pulled/held**, replacing A's brand-bar-only selection; the 3px handle survives inside both states. |
| Add-project surface | Takeover (both concepts ruled it; argued from interaction cost — the rail stays watchable during scan-triage, the scan list gets panel height). Three workflows survive as segmented control. |
| Settings sections | Sentence-case 12px titles (both agreed); card language kept. |

## Fixed constraints (unchanged from the brief)

Aesthetic overhaul only: no feature changes, no IPC changes. Activity
semantics/confidence from `activitySignal.js` are consumed, never re-derived.
Team rail/stack layouts, thresholds, HoverCard triggers, dirty dots, workflow
pills, daemon five-state palette, accounts hover board + magnitude signal,
keyboard shortcuts — all keep their meaning. Test ids preserved except renames
priced with their test updates (the modal→takeover rename is the expected one).
Both themes; the rail stays `bg-brand-950` in both.
