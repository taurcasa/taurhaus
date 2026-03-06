# Project HoverCard Vision

## Purpose

The project HoverCard should answer one question fast:

**Should I open this project right now?**

That makes it a decision-support surface, not a miniature overview page and not a debugging panel. The current HoverCard mixes all three roles. It shows useful signals, but it also spends space on information that is either already visible elsewhere or too detailed for a transient hover interaction.

The redesigned HoverCard should optimize for:

- immediate triage
- low reading cost
- strong signal hierarchy
- zero dependence on deep hover dwell time

## Information Architecture

The popup should be organized into four layers, in priority order.

### 1. Decision header

This is the part the user should understand in under a second.

Include:

- project name
- branch
- primary attention state
- one-line reason the project deserves attention now

Examples of attention reasons:

- Live agent session active
- Dirty changes on current branch
- Recent handoff with open questions
- Dependency relationship recently touched
- Quiet / no recent work

This top section should behave like a verdict, not raw metadata.

### 2. Current motion

The second layer explains whether work is actively moving.

Include:

- whether there is a live session
- which tool is active
- whether the session is working, waiting, or unattributed activity
- one compact timing signal

This section should describe momentum, not internals. The point is to show whether the project is currently alive, blocked, or idle.

### 3. Latest meaningful change

Hover is a good place for "what changed since I last looked?".

Include one concise change summary drawn from available data:

- latest session summary, if present and fresh
- otherwise latest commit message

If the latest session has structured follow-up data, show only the most important unresolved item:

- first open question, or
- first next step

This gives the card narrative value without turning it into session history.

### 4. Relationship / risk context

Only show this if it materially changes prioritization.

Examples:

- this project depends on another active project
- another project depends on this one
- recent dependency/reference activity exists

This should be a compact contextual line or badge group, not a list.

## User Journey Rationale

The sidebar is for scanning. Hover is for confirming intent before clicking.

That means the HoverCard should support these user journeys:

### Scan to choose

The user moves across several projects and wants to know which one is worth opening. The card should quickly expose urgency, motion, and latest change.

### Reorient after context switch

The user has been away from a project and needs a fast reminder. The card should summarize "what is going on now" without requiring a full project load.

### Judge interruption cost

The user sees a project with a live session and wants to know whether opening it will interrupt active work, resume a waiting agent, or just inspect a quiet state.

### Spot dependency-driven relevance

The user may not intend to open a project, but relationship context can explain why it is suddenly important because another project now depends on it or references it.

These flows all favor summary language over technical detail.

## What To Drop

The following information does not belong in the default hover experience:

- PID
- tmux session, window, or pane coordinates
- truncated session id
- multi-row per-session operational stats
- aggregate historical totals like total active time across all sessions
- three separate recent commits

Reasons:

- They are slow to parse.
- They do not help the open-or-skip decision.
- They duplicate detail that belongs in overview, session history, or debugging surfaces.
- They make the popup feel like an implementation artifact instead of a product surface.

If engineering needs low-level runtime details, they should live in a debug-only surface, context menu action, or dedicated inspector.

## What To Add

The current HoverCard has useful data access, but not enough prioritization. The next design should add or elevate:

- a clear attention label
- a one-line "why now" explanation
- latest session summary as the preferred narrative source
- first unresolved item from session handoff data when available
- ahead/behind and dirty state if git divergence is cheap to fetch and materially affects action choice
- compact relationship context when it changes project importance
- explicit empty-state language for quiet projects

Recommended source priority:

1. Live session state
2. Latest session handoff summary
3. Dirty/diverged git state
4. Latest commit
5. Relationship signal

This ordering matches the user's likely decision cost: live work and unresolved handoffs matter more than historical activity totals.

## Presentation Principles

### Make the first line decisive

The card should lead with an interpretation, not a data dump.

Bad:

- Active · Dirty · 3 sessions · 2h 18m

Better:

- Active work in progress
- Waiting on user input
- Quiet project, no recent handoff

### Prefer one strong sentence over many weak rows

Hover is transient. Dense lists increase dwell time and reduce scan efficiency.

### Show at most one item per category

One live-session summary.
One latest-change summary.
One dependency/risk cue.

Anything beyond that should require click-through.

### Keep technical detail out of the default layer

The visual language should feel product-level, not operator-console-level.

### Preserve sidebar-to-hover continuity

The sidebar already tells the user activity grouping and presence of sessions. The HoverCard should deepen that signal, not restate it verbatim.

### Optimize for non-hover-perfect behavior

Users should still succeed if they only dwell for a moment. The most important content must appear in the header area without requiring careful reading or scrolling.

## Proposed Content Model

For the next phase, the conceptual content stack should be:

1. Project name + branch
2. Attention verdict
3. Live work summary
4. Latest meaningful change
5. Optional dependency/risk context

Notably absent:

- raw runtime identifiers
- long commit lists
- historical aggregates
- anything that looks like a debug panel

## Data Fit With Existing IPC

The current frontend already has access to most of what this vision needs:

- `project.activityState`, `project.isDirty`, `project.branch`
- live session snapshot data from sidebar/session state
- `get_latest_session` / `list_sessions`
- `get_recent_commits`
- `get_relationships`
- potentially `get_git_status` for ahead/behind if the extra fetch cost is acceptable

That means the main work in later phases is not data acquisition. It is:

- choosing the right summary priority
- shaping a compact presentation
- avoiding duplicated detail from the overview tab

## Recommendation For Phase 2

The UI concept should treat the HoverCard as a two-beat surface:

1. verdict
2. evidence

Verdict tells the user whether the project deserves attention.
Evidence gives just enough justification to trust that verdict.

If the next concept preserves that discipline, the implementation can become substantially simpler than the current card while also feeling more useful.
