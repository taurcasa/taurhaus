# Sidebar Icon Refinement

## Summary

Two things should change in the sidebar session indicators.

1. The current brand marks are too detailed at `12px` inside a `14px` frame. They are technically accurate, but not optically crisp enough for a high-density scanning surface.
2. The team count badge is undersized and currently participates too strongly in the activity tint, which makes it feel like a second status token instead of a supporting count.

Recommendation:

- keep the current full brand SVGs for larger surfaces
- introduce a `sidebar-small` icon variant set for Claude, Codex, and Gemini
- increase sidebar icon frame from `14px` to `15px` for grouped team treatments only
- keep standalone session frames at `14px` unless a later pass shows the same clarity issue there
- enlarge the team count badge from `12px` to `14px`
- make the count badge mostly neutral, with only a subtle activity-aware border / text tint, not a fully tinted fill

This produces clearer marks without making the indicator strip visually louder.

## Current Problem

Current rendering:

- rail logos: `12px` SVG inside `14px` frame
- stack logos: `12px` SVG inside `14px` frame
- count badge: `12px` height, `9px` text, tinted fill

That works for simple shapes, but these brand marks are not simple shapes.

Observed issues:

- Claude starburst loses inner articulation and becomes noisy at `12px`
- Codex knot reads as a dense blob unless contrast is perfect
- Gemini sparkle holds up best, but still feels optically smaller than Claude/Codex
- the count badge circle is too small relative to the stacked logo cluster
- the tinted badge fill competes with the logo tint instead of supporting it

## Recommendation

### 1. Add Sidebar-Specific Small Icons

Do not keep shrinking the full brand shapes and hoping they read better.

Use two icon tiers:

- `default` icon set: existing full SVGs, used in larger surfaces
- `sidebarSmall` icon set: simplified versions tuned for `12-13px` rendering

The small variants should follow this rule:

- preserve silhouette first
- preserve brand rhythm second
- drop inner detail third

Per tool:

- Claude: reduce the starburst to fewer larger petals / wedges
- Codex: reduce the knot to fewer loops with more open negative space
- Gemini: keep the 4-point sparkle, but simplify inner tapering and slightly enlarge the core

These should remain filled shapes using `currentColor`. Do not switch to delicate stroke-only logos; stroke icons will degrade faster at this size in mixed theme/contrast conditions.

### 2. Increase Grouped-Team Icon Frames Slightly

For grouped team treatments only:

- frame: `15px`
- glyph box: `13px`

Why only grouped indicators:

- grouped rails and stacks are the most visually compressed
- the count badge already adds a secondary element, so the logos need more help there
- changing every standalone session icon at once would be a broader visual shift than this task needs

Do not go to `16px` in the current `36px` row. That starts to crowd the project name and branch chip.

### 3. Make the Count Badge Larger but Quieter

Increase badge size:

- current: `12px` height, `9px` text
- recommended: `14px` height, `10px` text, `min-width: 14px`, `padding-inline: 4px`

Make it optically subordinate:

- neutral surface fill
- subtle activity-aware border
- activity-aware text tint

Do not use a full green/amber badge background by default. That gives the count equal visual weight to the logos and overstates the importance of the number.

The count is metadata, not the primary runtime signal.

## Size Comparison

### Current

- standalone icon frame: `14px`
- grouped logo frame: `14px`
- grouped glyph: `12px`
- count badge: `12px`
- count text: `9px`

### Recommended

- standalone icon frame: `14px` for now
- grouped logo frame: `15px`
- grouped glyph: `13px`
- count badge: `14px`
- count text: `10px`
- stack overlap: keep `4px`
- rail gap: keep `4px`

Why this is enough:

- `+1px` frame / glyph growth materially helps clarity at this scale
- `+2px` badge height makes the count stop looking pinched
- keeping spacing unchanged avoids a ripple effect on row width budgeting

## Count Badge Activity Decision

### Recommended

Count badge should participate in activity state lightly, not fully.

Use:

- neutral fill tied to sidebar surface
- tinted text
- tinted inner ring / border

This preserves the activity relationship without creating a second glowing status blob.

### Rejected Alternative

Full tinted fill for the count badge.

Why reject it:

- makes the badge visually louder than the logos in some themes
- turns the stack into "logos + another pill" rather than one coherent grouped unit
- reduces numeral clarity in idle amber state, where the fill is softer and the text is already warm

## Proposed Dark Tokens

Grouped logo frame:

- icon ring / backplate: `rgba(9, 14, 17, 0.96)`
- icon foreground active: `#86EFAC`
- icon foreground idle: `#FCD34D`

Count badge:

- neutral fill: `rgba(255, 255, 255, 0.05)`
- neutral shadow line: `rgba(255, 255, 255, 0.04)`
- active text: `#86EFAC`
- active border: `rgba(134, 239, 172, 0.38)`
- idle text: `#FCD34D`
- idle border: `rgba(252, 211, 77, 0.42)`

Rail background:

- active rail: `rgba(134, 239, 172, 0.18)`
- idle rail: `rgba(252, 211, 77, 0.22)`

## Proposed Light Tokens

Grouped logo frame:

- icon ring / backplate: `rgba(255, 255, 255, 0.96)`
- icon foreground active: `#16A34A`
- icon foreground idle: `#D97706`

Count badge:

- neutral fill: `rgba(255, 255, 255, 0.92)`
- neutral shadow line: `rgba(19, 78, 74, 0.06)`
- active text: `#16A34A`
- active border: `rgba(22, 163, 74, 0.24)`
- idle text: `#D97706`
- idle border: `rgba(217, 119, 6, 0.24)`

Rail background:

- active rail: `rgba(22, 163, 74, 0.16)`
- idle rail: `rgba(217, 119, 6, 0.18)`

## Wireframes

Legend:

- `[c]` / `[o]` / `[g]` = current tiny detailed logos
- `[C]` / `[O]` / `[G]` = refined small variants with larger optical footprint
- `(5)` = current count badge
- `( 5 )` = refined count badge

### Current Rail

```text
[taurhaus                    ] [c===o] [br]
```

Problems:

- logos feel busy
- rail reads better than the marks themselves

### Recommended Rail

```text
[taurhaus                    ] [C===O] [br]
```

Changes:

- simplified small variants
- slightly larger grouped icon frames
- no extra chrome added

### Current Stack

```text
[taurhaus                  ] [[c][o][g]](5) [br]
```

Problems:

- icons blur together before the overlap has a chance to read as intentional
- badge looks undersized and over-tinted

### Recommended Stack

```text
[taurhaus                ] [[C][O][G]]( 5 ) [br]
```

Changes:

- clearer small icon variants
- slightly larger logo frames
- larger badge
- neutral badge fill with lighter activity participation

## Concrete CSS Direction

Grouped logo frame:

```css
width: 15px;
height: 15px;
```

Grouped glyph:

```css
width: 13px;
height: 13px;
```

Refined count badge:

```css
min-width: 14px;
height: 14px;
padding-inline: 4px;
font-size: 10px;
font-weight: 700;
line-height: 1;
font-variant-numeric: tabular-nums;
```

Activity treatment:

```css
background: var(--sidebar-team-count-neutral);
color: var(--sidebar-team-count-text-active-or-idle);
box-shadow:
  inset 0 0 0 1px var(--sidebar-team-count-border-active-or-idle),
  0 0 0 1px var(--sidebar-team-count-shadow);
```

## Alternatives Considered

### Keep existing logos, only enlarge frame

Rejected.

Reason:

- helps slightly, but the root problem is detail density, not just frame size

### Convert all logos to monoline stroke icons

Rejected.

Reason:

- too easy to lose brand recognition
- too fragile in amber idle state and light theme

### Keep badge fully tinted, just enlarge it

Rejected.

Reason:

- improves count legibility but keeps the visual hierarchy wrong

## Final Call

The right refinement is not "make everything louder."

It is:

- simplify the tiny brand marks specifically for sidebar use
- give grouped logos one extra pixel of breathing room
- enlarge the count badge slightly
- reduce the badge's fill emphasis while keeping light activity-aware text and border cues

That keeps the sidebar readable at a glance and preserves brand recognition without making the indicator strip dominate the row.
