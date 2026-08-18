# Mesh Canvas Layout Engine Concept

Date: 2026-03-06  
Task: #413

## Problem

`MeshCanvas.svelte` still computes:

- row packing
- node positions
- lead-side anchor fan-out
- bezier bend values

inside one component-local path. The result is a recurring bug class:

- missing connection after reflow/removal
- tangled curves after row collapse
- center agent path degenerating into an effectively invisible vertical line

The core issue is not SVG. The core issue is that routing is represented as a late `bend` tweak instead of a first-class layout result.

## Current failure mode

Today the connection model is effectively:

1. place nodes
2. sort agents by `x`
3. assign lead anchors from that sort
4. derive one scalar `bend`
5. let `MeshConnection.svelte` infer the full cubic path from `from`, `to`, and `bend`

That is too weak a contract. A single scalar cannot robustly encode:

- row-aware routing
- stable lane ordering through add/remove transitions
- guaranteed separation for near-center agents
- non-degenerate curves across 1-8 agents

## What the layout engine should be

A small, pure layout module that computes **node placement and connection routing together**.

It should not render SVG. It should output a deterministic layout model for the existing SVG/HTML renderer.

Suggested contract:

```ts
type MeshLayoutInput = {
  width: number
  height: number
  mode: 'setup' | 'runtime' | 'initializing'
  lead: MeshMember
  agents: MeshMember[]
}

type NodeBox = {
  id: string
  x: number
  y: number
  width: number
  height: number
  row: 0 | 1
  column: number
}

type ConnectionRoute = {
  id: string
  fromId: string
  toId: string
  start: { x: number, y: number }
  end: { x: number, y: number }
  control1: { x: number, y: number }
  control2: { x: number, y: number }
  laneIndex: number
  row: 0 | 1
  status: 'setup' | 'active' | 'idle' | 'offline' | 'initializing'
}

type MeshLayoutOutput = {
  lead: NodeBox
  agents: NodeBox[]
  connections: ConnectionRoute[]
  addNode: { x: number, y: number } | null
}
```

The important shift is that `MeshConnection` should receive full route geometry, not invent the route from a single bend number.

## Design principles

### 1. One coordinated pass

Node placement and connection routing must share one coordinated model. Routing depends on row assignment and slot ordering, so it should never be a later patch step.

### 2. Stable semantic lead slots

The lead needs explicit connection slots, not implicit “whatever x-sort produced today.”

Each visible agent gets:

- `row`
- `column`
- `slotIndex`
- `slotCount`

That makes anchor placement stable across transitions and avoids the “center agent invisible” class.

### 3. Row-aware route lanes

Connections should route through row lanes, not free-form bends.

For this topology, that means:

- one corridor for single-row layouts
- two corridors for two-row layouts
- monotonic left-to-right slot assignment within each corridor

### 4. Renderer stays simple

`MeshConnection.svelte` should become a dumb renderer that draws a path from already-computed control points.

## Recommended custom engine

### Topology phase

Given `N` agents:

- `1-6` agents: one row
- `7-8` agents: two rows

Return explicit row membership and column order.

### Geometry phase

Compute:

- canvas width and height
- node widths and gaps
- lead box
- agent boxes
- add-node position in setup mode

This replaces the current mix of `buildRow`, `fitHorizontalLayout`, and local inline geometry.

### Routing phase

For each connection:

1. assign the lead slot from stable row/column order
2. assign a lane index inside the row corridor
3. compute `start`, `end`, `control1`, `control2`

For the current visual style, the path can remain cubic Bezier. The difference is that the control points are now explicit layout output.

### Invariants the engine should guarantee

- every visible agent has exactly one route
- every route starts at a distinct lead slot
- control points remain ordered left-to-right with their target order
- no route collapses into a near-zero horizontal bend unless that is intentionally allowed
- route bounds stay inside the SVG viewBox
- row collapse from `8 -> 5` or `7 -> 6` preserves non-crossing ordering

## Comparison

| Option | What it improves | What it still leaves unsolved | Fit for taurhaus |
|---|---|---|---|
| Status quo patching | fixes the latest visible bug | recurring routing regressions because the contract is still `bend`-based | poor |
| Small custom engine | solves the real problem directly with minimal surface area | still owned in-repo | best |
| `dagre` helper | can compute layered node positions | branded slot fan-out and Bezier routing still need custom post-processing | medium-low |
| `ELK` helper | can compute layered layout and richer edge routing metadata | too much machinery for a fixed 1-8 node star layout; would still need style translation | medium |

## Lightweight library assessment

### `dagre`

`dagre` is useful when the main problem is ranking nodes into layers in a directed graph. That is not the hard part here.

For taurhaus it would help with:

- top-to-bottom layering
- general row ordering

It would not solve the product-specific parts:

- stable lead slot semantics
- non-crossing branded fan-out
- center-line degeneracy rules
- preserving the exact current SVG curve style

So `dagre` would still require a custom routing layer after layout. At that point the dependency saves less than it appears.

### `ELK`

`ELK` is the strongest helper if taurhaus later needs:

- more than two tiers
- more heterogeneous graph shapes
- automatic edge routing for more complex topologies

For the current mesh canvas it is still the wrong default:

- the topology is fixed and tiny
- the desired output is a branded, symmetric, hand-tuned look
- the current bugs come from under-specified internal geometry, not from lack of a general graph solver

`ELK` is a credible fallback if the topology grows. It is not the right first move for the 1-8 agent case.

## Recommendation

Build a **small custom mesh layout engine** and keep the current SVG renderer.

Do not keep patching `bend`.

Do not add `dagre` or `ELK` as phase one dependencies.

Reasoning:

- the topology is highly constrained
- taurhaus needs deterministic branded geometry more than general graph layout
- the bug source is the weak route contract, not missing graph-library power
- a pure local engine is cheaper to test and easier to reason about than adapting a general layout library and then overriding its routing style

## Rough implementation approach

### Step 1: extract a pure module

Create something like:

```text
src/lib/components/meshLayout.js
```

Move layout logic out of `MeshCanvas.svelte` into:

- `computeMeshTopology(input)`
- `computeMeshBoxes(topology, input)`
- `computeMeshRoutes(boxes, input)`
- `computeMeshLayout(input)`

### Step 2: replace `bend` with explicit route geometry

Change `MeshConnection.svelte` from:

- `from`
- `to`
- `bend`

to:

- `start`
- `end`
- `control1`
- `control2`

or simply a precomputed `d` path string.

### Step 3: test invariants as pure geometry

Add pure tests for:

- `1..8` agents
- `7 -> 5` and `8 -> 6` row collapse
- centered agent cases
- narrow container widths
- ordering stability after removal from the middle

Those tests should assert route invariants, not just rendered path counts.

### Step 4: keep animation and styling in the renderer

Status color, glow, dash, and draw animation remain in `MeshConnection.svelte`.

Only geometry moves into the engine.

## Future-proofing boundary

If taurhaus later needs arbitrary team shapes, more than two rows, or cross-cluster routing, revisit `ELK` as a helper behind the same layout interface.

That is the right abstraction boundary:

- custom engine now
- optional library-backed provider later

## Final call

The smallest correct move is:

1. stop expressing route geometry as `bend`
2. extract a pure `meshLayout` engine
3. compute node boxes and connection routes together
4. keep the current SVG/HTML rendering model

That directly targets the repeated bug class without over-solving the problem.

## Sources

- Current implementation:
  - [`MeshCanvas.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshCanvas.svelte)
  - [`MeshConnection.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshConnection.svelte)
  - [`MeshCanvas.test.js`](/home/user/projects/taurhaus/src/lib/components/MeshCanvas.test.js)
- Prior assessment:
  - [`mesh-canvas-library-assessment.md`](/home/user/projects/taurhaus/docs/architecture/mesh-canvas-library-assessment.md)
- Official library references reviewed on 2026-03-06:
  - Dagre: https://github.com/dagrejs/dagre
  - ELK: https://eclipse.dev/elk/
