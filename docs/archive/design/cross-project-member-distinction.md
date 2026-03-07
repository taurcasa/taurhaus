# Cross-Project Team Member Distinction In Mesh Canvas

## Recommendation

Use **both connection styling and a very small node-level location cue**.

Recommended phase-1 treatment:

- keep local members as the current default
- render cross-project member connections with a **subtle dashed stroke** and slightly reduced opacity
- add a **small project badge or remote glyph** on the cross-project node itself
- show the exact project path/name in hover or click detail
- do **not** change layout geometry
- do **not** use alarming colors that imply failure or lower status

This is the right balance because:

- line treatment communicates relationship difference
- node treatment keeps the distinction visible even when a line is partially obscured
- the combination reads as “works elsewhere,” not “broken” or “secondary”

## Does The Data Signal Exist?

Yes.

The backend/runtime model already tracks per-member project path information, and the frontend runtime node detail already receives `projectId` / `project_id` for each member.

That means cross-project status can be derived by comparing:

- lead project path
- member project path

Conceptually:

```text
isCrossProject = normalize(member.projectPath) != normalize(lead.projectPath)
```

So this is not blocked on a new product concept. The signal already exists in the coordination data model.

## Current Problem

Today the runtime canvas makes all members look equivalent from a location standpoint.

That creates ambiguity in teams where:

- most members work in the lead project
- one or two members work in a different codebase

Example:

- lead + 4 developers in `taurhaus`
- `mesh-expert` in `mesh`

Right now the expert node and its connection visually read like just another local agent. The user has to click into details to discover that the agent is actually operating in a different repository.

That is too late. Cross-project location is a structural property, not just extra metadata.

## What Should The Primary Signal Be?

The primary signal should be the **connection line**.

Reason:

- cross-project is fundamentally about relationship to the lead’s project context
- the edge already represents that relationship
- changing the edge communicates “connected, but across a project boundary” without changing node hierarchy

The node itself should carry only a secondary cue.

That keeps the emphasis correct:

- same type of team member
- different working location

## Connection Line Treatment

Best option: **dashed line with the same overall color family**.

Why dashed works:

- widely understood as “indirect” or “non-local” connection
- readable at a glance
- does not imply error if color stays in-family
- works in light and dark themes

Recommended rule:

- local member connection: current solid line
- cross-project member connection: same hue family, dashed, slightly dimmer

Example:

```text
Local:         ─────────────
Cross-project: ┄ ┄ ┄ ┄ ┄ ┄
```

What not to do:

- red or warning colors: reads as broken
- dramatically different curve shape: adds unnecessary topology meaning
- very low opacity only: too easy to miss
- separate color per project: scales badly and creates legend problems

## Node Treatment

Node treatment should be present, but subtle.

Best option:

- small badge such as `Remote` or a compact project chip
- or a small corner glyph indicating “different project”

Recommended node cue hierarchy:

1. tiny project chip with short label if space permits
2. otherwise a small remote glyph plus project name in hover/detail

Examples:

```text
┌──────────────────────┐
│ mesh-expert      ◇   │
│ gemini-3.1-pro       │
│ mesh                 │
└──────────────────────┘
```

or

```text
┌──────────────────────┐
│ mesh-expert [mesh]   │
│ gemini-3.1-pro       │
└──────────────────────┘
```

I would avoid full background tint changes unless very restrained. Heavy tinting risks making remote members feel like a different class of agent.

## Line Style, Node Style, Or Both?

Use **both**, but with asymmetric weight:

- **edge = primary signal**
- **node = confirmation signal**

Why not line-only?

- the line can be visually lost in dense layouts
- users often scan nodes before edges

Why not node-only?

- cross-project is relational, not intrinsic
- node-only can read like a role or status variant instead of location

Using both solves discoverability without over-signaling.

## Hover And Detail Behavior

Yes, hovering or opening details should explicitly show the project.

Minimum:

- if a member is cross-project, the summary/detail should say `Project: mesh`
- if possible, also label whether it is `Lead project` or `Other project`

Example hover/detail addition:

```text
Project: mesh
Location: other project
```

That removes ambiguity and teaches the visual language.

## Scale Behavior

This should scale well for **1-2 cross-project members among 5-8 total** if the treatment stays subtle and consistent.

Expected scanning result:

- local majority forms the default visual pattern
- remote members stand out just enough via dashed edges
- the node cue prevents missed interpretation when the edge is hard to trace

If half the graph becomes cross-project, then the distinction naturally loses contrast. That is acceptable because cross-project is no longer exceptional in that scenario.

## Does The Layout Engine Need Changes?

No.

This should be a **rendering/styling change only**.

The current layout engine already computes:

- node positions
- route geometry
- node boxes

Cross-project distinction only needs an extra derived flag per member, such as:

```json
{
  "isCrossProject": true,
  "projectLabel": "mesh"
}
```

Then:

- `MeshConnection` can switch stroke dash/opacity
- `MeshNode` can render a small project cue
- `MeshNodeDetail` / future hover summary can show the exact project label

No topology or routing changes are necessary.

## Proposed Visual Sketches

### Local member

```text
             team-lead
                │
                │
        ┌────────────────┐
        │ frontend-dev   │
        │ gpt-5.4 high   │
        └────────────────┘
```

### Cross-project member

```text
             team-lead
                ┆
             ┄ ┄┆┄ ┄
        ┌────────────────┐
        │ mesh-expert ◇  │
        │ gemini-3.1-pro │
        │ mesh           │
        └────────────────┘
```

### Side-by-side comparison

```text
             team-lead
            /         \
           /           ┄ ┄ ┄
          /                  \
┌────────────────┐   ┌────────────────┐
│ frontend-dev   │   │ mesh-expert ◇  │
│ gpt-5.4 high   │   │ gemini-3.1-pro │
└────────────────┘   │ mesh           │
                     └────────────────┘
```

## Design Constraints

Keep these guardrails:

- do not use warning/error colors
- do not make remote agents feel degraded or unavailable
- do not overload the node with long project paths
- do not introduce a legend if the meaning can be taught through detail text

The distinction should feel like:

- local = default
- cross-project = elsewhere, but still fully part of the team

## Final Call

This is worth doing.

Ship the smallest clear version:

1. derive `isCrossProject` from member project path vs lead project path
2. render cross-project edges as dashed and slightly dimmer
3. add a small node cue (`[mesh]` chip or remote glyph)
4. show full project context in hover/detail
5. leave layout geometry unchanged

That gives the canvas a meaningful location signal without adding visual noise or reworking the graph system.
