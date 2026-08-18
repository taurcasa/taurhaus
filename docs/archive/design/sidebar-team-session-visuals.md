# Sidebar Team Session Visuals

> Archived design review. This file captures the visual-direction decision for grouped team indicators. It is retained as historical rationale; current shipped behavior lives in the sidebar/session-indicator code and tests.

## Summary

Recommendation: keep the current `14px` standalone tool-logo treatment, but change grouped mesh-team indicators so they still read as "tool runtime" rather than "generic badge".

This supersedes the earlier `T<count>` fallback from [sidebar-session-grouping.md](/home/user/projects/taurhaus/docs/archive/design/sidebar-session-grouping.md).

Two visual states:

1. `<=3` team members: render the same individual tool logos, but add a very light shared rail behind them so the row reads as one team unit.
2. `4+` team members: render stacked unique tool logos plus a compact count badge.

The row should still feel secondary to the project name. The right visual metaphor is "linked runtime markers", not "avatars" and not "notification chips".

## Current Baseline

Current implementation in [sessionIndicator.js](/home/user/projects/taurhaus/src/lib/sessionIndicator.js) and [SidebarProjectList.svelte](/home/user/projects/taurhaus/src/lib/SidebarProjectList.svelte):

- standalone session icon: `14px` box, `12px` SVG
- grouped team token: rounded pill with `T` + count
- active tint: success green
- idle tint: warning amber
- row height: `36px`
- icon strip gap: `4px` via `gap-1`

That grouped pill is operationally clear, but it breaks the existing tool-logo language. The fix should stay inside the current width budget and reuse the current activity tint semantics.

## Visual Model

### Standalone Session

Standalone sessions keep the current treatment exactly:

- one `14px` monochrome tool logo
- no connector
- same click-to-jump affordance rules
- same active/idle tint classes

### Team Session, `<=3` Members

Use individual tool logos, but place them inside a connected micro-group.

Visual recipe:

- each logo remains a `14px` hit/paint box
- logo-to-logo visual overlap stays `0`; keep readability high
- add a shared rail behind the group: `height: 8px`, `border-radius: 999px`
- rail extends from first logo center to last logo center, with `3px` inset beyond the outer centers
- rail sits behind icons at `50%` vertical alignment
- icons remain individually visible; connector is only a grouping cue

Why this is the right connector:

- a line is lighter than a shared pill background
- a link glyph would flicker as noise at `14px`
- a full outline capsule becomes too close to a badge
- the rail keeps the visual language close to the existing icon strip

### Team Session, `4+` Members

Render a compact stack of unique tool logos and a small count badge.

Visual recipe:

- show unique tool types only, not one logo per member
- max visible logos in stack: `3`
- use stable order: `Claude`, `Codex`, `Gemini`
- each logo frame: `14px`
- horizontal overlap: `4px`
- total logo-stack width:
  - `1` unique tool: `14px`
  - `2` unique tools: `24px`
  - `3` unique tools: `34px`
- count badge sits `4px` to the right of the last stacked logo
- count badge height: `12px`
- count badge min-width: `12px`
- count badge horizontal padding: `3px`
- count text: `9px`, `font-weight: 700`, tabular if available

The stack should look like a condensed legend, not a decorative pile. Keep overlap shallow so each mark stays recognizable.

## Wireframes

Legend:

- `[C]` = Claude logo
- `[O]` = Codex logo
- `[G]` = Gemini logo
- `===` = subtle connector rail
- `(5)` = count badge
- `*` = dirty marker
- `[br]` = branch chip

### Standalone Sessions

```text
[taurhaus                    ] [C] [O] [br] *
```

No connector. Independent sessions remain independent.

### Team, 2 Members

```text
[taurhaus                    ] [C===O] [br] *
```

Interpretation:

- same two logos
- one shared team rail
- no extra badge

### Team, 3 Members

```text
[taurhaus                  ] [C===O===G] [br] *
```

Still readable at row scale. No collapse yet.

### Team, 5 Members, 2 Tool Types

```text
[taurhaus                    ] [[C][O]](5) [br] *
```

Interpretation:

- five members total
- only Claude + Codex shown because those are the unique tools present
- overlap communicates grouping density
- badge communicates member count, not tool count

### Team, 7 Members, 3 Tool Types

```text
[taurhaus                  ] [[C][O][G]](7) [br] *
```

### Mixed: Team + Standalone

```text
[taurhaus                 ] [[C][O]](5) [G] [br] *
```

Rules:

- grouped mesh-team indicator first
- standalone sessions after it
- standalone logo remains visually separate

## Concrete CSS Values

### `<=3` Connector Group

Container:

```css
position: relative;
display: inline-flex;
align-items: center;
gap: 4px;
height: 14px;
padding-inline: 0;
```

Rail:

```css
position: absolute;
left: 4px;
right: 4px;
top: 3px;
height: 8px;
border-radius: 999px;
pointer-events: none;
```

Logo chips:

```css
position: relative;
z-index: 1;
width: 14px;
height: 14px;
display: inline-flex;
align-items: center;
justify-content: center;
```

### `4+` Stacked Group

Container:

```css
display: inline-flex;
align-items: center;
height: 14px;
```

Stack:

```css
display: inline-flex;
align-items: center;
margin-right: 4px;
```

Overlapped logos:

```css
width: 14px;
height: 14px;
border-radius: 999px;
margin-left: -4px;
position: relative;
```

First logo resets overlap:

```css
margin-left: 0;
```

Count badge:

```css
min-width: 12px;
height: 12px;
padding-inline: 3px;
border-radius: 999px;
display: inline-flex;
align-items: center;
justify-content: center;
font-size: 9px;
font-weight: 700;
line-height: 1;
```

## Theme Tokens

Reuse the existing activity palette from [app.css](/home/user/projects/taurhaus/src/app.css). The grouped treatment should not introduce a new color system.

### Dark Theme

Active team:

- logo color: `text-success-300` (`#86EFAC`)
- connector rail: `rgba(134, 239, 172, 0.22)`
- connector rail edge: `rgba(134, 239, 172, 0.38)`
- stacked logo backplate / overlap separator: `rgba(9, 14, 17, 0.96)`
- count badge bg: `rgba(134, 239, 172, 0.18)`
- count badge border: `rgba(134, 239, 172, 0.55)`

Idle team:

- logo color: `text-warning-300` (`#FCD34D`)
- connector rail: `rgba(252, 211, 77, 0.22)`
- connector rail edge: `rgba(252, 211, 77, 0.4)`
- stacked logo backplate / overlap separator: `rgba(9, 14, 17, 0.96)`
- count badge bg: `rgba(252, 211, 77, 0.18)`
- count badge border: `rgba(252, 211, 77, 0.65)`

### Light Theme

Active team:

- logo color: `text-success-600` (`#16A34A`)
- connector rail: `rgba(34, 197, 94, 0.16)`
- connector rail edge: `rgba(22, 163, 74, 0.26)`
- stacked logo backplate / overlap separator: `rgba(255, 255, 255, 0.94)`
- count badge bg: `rgba(240, 253, 244, 0.95)`
- count badge border: `rgba(22, 163, 74, 0.28)`

Idle team:

- logo color: `text-warning-600` (`#D97706`)
- connector rail: `rgba(245, 158, 11, 0.14)`
- connector rail edge: `rgba(217, 119, 6, 0.24)`
- stacked logo backplate / overlap separator: `rgba(255, 255, 255, 0.94)`
- count badge bg: `rgba(255, 251, 235, 0.95)`
- count badge border: `rgba(217, 119, 6, 0.26)`

## Activity Semantics

Keep the current aggregate logic from [sessionIndicator.js](/home/user/projects/taurhaus/src/lib/sessionIndicator.js):

- any grouped member active -> whole team indicator reads active
- all grouped members idle -> whole team indicator reads idle

Do not attempt split state in the row. Mixed member state belongs in the HoverCard roster.

## Comparison With Standalone Rows

Standalone session row:

- each logo means one session
- spacing between logos means independence
- click affordance may exist per item

Team-linked session row:

- connector or stack means coordination
- badge count means member count
- multiple logos in stacked state mean tool diversity, not session count
- whole group should still read at the same visual weight as `2-3` standalone logos, not a prominent badge

The sidebar should still scan as:

1. project name
2. runtime presence
3. branch / dirty metadata

The grouped team indicator must not jump ahead of the project name in contrast or size.

## Edge Cases

### Mixed Team + Standalone

Render `team group + standalone logos`.

Example:

```text
[[C][O]](5) [G]
```

Do not merge standalone sessions into the team badge.

### Single-Tool Teams, `<=3`

Example: three Codex members.

```text
[O===O===O]
```

This is acceptable because the connector does the grouping work. No extra badge needed below the collapse threshold.

### Single-Tool Teams, `4+`

Example: six Codex members.

```text
[[O]](6)
```

Only one tool logo is shown. The count badge carries the density signal.

### All Same Tool Type + Standalone Same Tool

Example: team of four Codex plus one standalone Codex.

```text
[[O]](4) [O]
```

This is still valid because the grouped token has structural cues the standalone icon does not:

- overlap stack
- count badge
- no per-session jump affordance

### All Three Tool Types Present in a Large Team

Cap the visible stack at three logos. Do not add a fourth placeholder or mini grid.

### One Team Member Only

Do not use team visuals. Render as a normal standalone session logo. A one-member "team" does not justify grouped chrome in the row.

## Minimal Implementation Impact

No backend change is required.

The grouped indicator data already carries `members`. Frontend can derive:

- unique tool types for the stack
- aggregate activity state
- count badge text

Minimal frontend adjustments:

1. extend grouped indicator shape in [sessionIndicator.js](/home/user/projects/taurhaus/src/lib/sessionIndicator.js) with `uniqueTools`
2. replace `T + count` rendering in [SidebarProjectList.svelte](/home/user/projects/taurhaus/src/lib/SidebarProjectList.svelte)
3. add a small CSS block for connector rail and stacked overlap styling in [app.css](/home/user/projects/taurhaus/src/app.css)
4. add visual fixtures for `2-member team`, `3-member team`, `5-member mixed tools`, and `team + standalone`

## Final Call

Use the lightest possible grouping signal below the collapse threshold and the lightest possible stacked summary above it.

That gives the sidebar three desirable properties at once:

- small teams still feel human-readable and tool-specific
- large teams stop flooding the row with repeated icons
- grouped mesh sessions still look like part of the existing session-indicator language rather than a separate badge system
