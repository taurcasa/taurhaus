# Sidebar Session Indicator Grouping

> Archived design review. This file captures the grouping threshold and rationale used during the sidebar session-indicator redesign. It is kept as historical product reasoning; current behavior is defined in the implementation and its tests.

## Summary

Recommendation: keep the current per-session logos for `1-3` live sessions, and collapse **team-linked** sessions into a single grouped indicator at `4+` live sessions.

Standalone sessions should remain individual.

Preferred row rule:

- `1-3` live sessions: individual logos
- `4+` live sessions where multiple sessions share one mesh team: show one team group token instead of individual team logos
- mixed rows: `team group + standalone logos`
- keep the existing `36px` row height
- use the existing HoverCard, not inline expansion, for member detail

## Current Constraints

The current sidebar row in [SidebarProjectList.svelte](/home/user/projects/taurhaus/src/lib/SidebarProjectList.svelte) is fixed at `36px` high and renders:

- project name
- `14px` tool indicators for each live session
- optional branch chip
- optional dirty marker

The current indicator logic in [sessionIndicator.js](/home/user/projects/taurhaus/src/lib/sessionIndicator.js) is intentionally session-level:

- one icon per live session
- per-session active vs idle tint
- optional direct click target for tmux jump

That is good at low counts, but it scales poorly for team-heavy projects.

## When It Gets Cluttered

The current design is fine through `3` live sessions and starts breaking down at `4`.

Practical threshold:

- `1`: excellent
- `2`: clear
- `3`: acceptable
- `4`: borderline
- `5+`: visually busy enough that grouping is justified

Why `4` is the break point:

- four `14px` indicators plus gaps begin to compete with the project name
- branch and dirty markers lose breathing room
- the row stops reading as "project first, status second" and starts reading as "icon strip"
- the sidebar becomes visually inconsistent with the rest of the app's cleaner density

This is not really a problem for standalone sessions. It is a team-scaling problem.

## Recommendation

### Group Team Sessions, Not All Sessions

If multiple live sessions belong to the same mesh team, collapse them into one grouped token.

If a session is standalone, keep it individual.

That gives the row the right abstraction level:

- standalone session = one visible session
- mesh team = one coordinated unit with multiple members

The row is a scanning surface, not a roster surface.

### Trigger Rule

Use grouping when both are true:

1. the project row has `4+` live sessions
2. at least `2` of those sessions share the same team identity

That avoids grouping too early while still protecting the row once team density becomes the dominant visual signal.

### Mixed Rows

If a project has both team-linked and standalone sessions, render them separately:

- grouped team token first
- standalone icons after it

Example:

- `3 team members + 1 standalone Codex` -> `[team 3] [codex]`
- `5 team members + 2 standalone sessions` -> `[team 5] [claude] [gemini]`

Do not merge standalone sessions into the team token. That hides important ownership and navigation differences.

## Why Not Inline Expansion

Do not expand the grouped token inline on hover.

Reasons:

- the row only has `36px` height
- inline expansion would add hover jitter to a dense scanning surface
- it would create more motion and layout instability than the problem warrants
- the app already has a HoverCard pattern for per-project detail

The HoverCard is the right place for member-level detail.

Recommended behavior:

- row stays compact
- grouped token communicates presence + size + aggregate state
- HoverCard shows the member list, tool mix, and per-member state

## What Information Is Lost

Grouping does remove some row-level granularity.

Current individual icons show:

- per-session tool identity
- per-session active/idle color
- per-session click-to-jump affordance

That loss is acceptable at `4+` team-member counts because the alternative is a row that is visually overloaded.

What the grouped token still needs to preserve:

- this is a team, not a generic badge
- member count
- aggregate activity state

Recommended aggregate state semantics:

- if any grouped member is `active`, the token reads as active
- else if any grouped member is `idle`, the token reads as idle
- if states are mixed, HoverCard shows the split

I would not try to encode per-tool composition in the token itself. That makes the grouped control visually noisy again.

## Row Height Impact

Grouping should keep the current `36px` row height unchanged.

That is one of the strongest arguments for doing it.

Without grouping, large teams force one of three bad outcomes:

- crowded horizontal rows
- smaller and less legible indicators
- taller project rows

The third option is the worst. Taller rows reduce sidebar scan density for every project, not just the team-heavy ones.

## Proposed Group Token

The token should look like an operational runtime marker, not a notification badge.

Suggested structure:

- compact rounded pill
- small team/mesh glyph on the left
- count on the right
- active or idle tint from aggregate state

Example concepts:

```text
[mesh 5]
```

```text
[team 5]
```

```text
[◈ 5]
```

I would avoid a mini collage of multiple tool logos. That repeats the clutter problem inside the grouped token.

## Wireframes

Legend:

- `[C]` = Claude session
- `[O]` = Codex session
- `[G]` = Gemini session
- `[T5]` = grouped team token with 5 members
- `[br]` = branch chip
- `*` = dirty marker

### 1 Live Session

Current behavior is already correct.

```text
Before
[taurhaus                           ] [C] [br] *

After
[taurhaus                           ] [C] [br] *
```

### 3 Live Sessions, Same Team

Still acceptable as individual logos.

```text
Before
[taurhaus                    ] [C] [O] [G] [br] *

After
[taurhaus                    ] [C] [O] [G] [br] *
```

### 5 Live Sessions, Same Team

This is where the current layout becomes busy.

```text
Before
[taurhaus         ] [C] [O] [O] [G] [C] [br] *

After
[taurhaus                        ] [T5] [br] *
```

### 8 Live Sessions, Same Team

This strongly needs grouping.

```text
Before
[taurhaus ] [C] [C] [O] [O] [O] [G] [G] [C] [br] *

After
[taurhaus                        ] [T8] [br] *
```

### Mixed: Team + Standalone

#### 3 Team Members + 1 Standalone

Because the row total is `4`, grouping is already justified.

```text
Before
[taurhaus                 ] [C] [O] [G] [O] [br] *

After
[taurhaus                    ] [T3] [O] [br] *
```

#### 5 Team Members + 1 Standalone

```text
Before
[taurhaus      ] [C] [O] [O] [G] [C] [G] [br] *

After
[taurhaus                    ] [T5] [G] [br] *
```

#### 5 Team Members + 2 Standalone

```text
Before
[taurhaus   ] [C] [O] [O] [G] [C] [G] [O] [br] *

After
[taurhaus               ] [T5] [G] [O] [br] *
```

## HoverCard Role

The existing HoverCard in [HoverCard.svelte](/home/user/projects/taurhaus/src/lib/HoverCard.svelte) already prioritizes live-session summary. It should absorb the lost per-member detail rather than the row trying to do both summary and roster work.

Recommended addition when a grouped token is present:

```text
Live team: taurhaus-team
- team-lead     Claude   active
- architect     Codex    idle
- frontend-dev  Codex    active
- reviewer      Gemini   idle
+ 1 more
```

That is enough to preserve understanding without requiring inline expansion.

## Data Needed To Implement It

The current sidebar projection appears to know about sessions, tools, and activity state, but not explicit team grouping.

Minimal extra metadata per session would be something like:

```json
{
  "cli_tool": "codex",
  "state": "idle",
  "project_path": "/projects/taurhaus",
  "group_kind": "mesh_team",
  "group_id": "taurhaus-team",
  "group_label": "taurhaus-team",
  "member_name": "architect"
}
```

That is enough to:

- detect team-linked sessions
- count group members
- separate grouped vs standalone sessions
- populate HoverCard detail

## Final Call

The right design is:

1. keep individual logos for `1-3` live sessions
2. collapse team-linked sessions into one token at `4+` live sessions
3. keep standalone sessions individual
4. render mixed rows as `team token + standalone icons`
5. keep the `36px` row height
6. use HoverCard for expanded member detail, not inline expansion

That keeps the sidebar visually clean at scale without hiding meaningful information for small projects.
