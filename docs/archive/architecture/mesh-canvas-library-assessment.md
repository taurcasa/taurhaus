# Mesh Canvas Library Assessment

Date: 2026-03-06  
Task: #396  
Baseline assessed: current workspace version of `src/lib/components/MeshCanvas.svelte` after commit `4d9de65`

## Current baseline

The current Mesh canvas is a hand-built SVG/HTML hybrid:

- Layout is deterministic and topology-specific. `MeshCanvas.svelte` computes one lead node, one or two centered agent rows, and manual lead-side anchor fan-out for connections.
- Edges are custom cubic Bezier paths in [`src/lib/components/MeshConnection.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshConnection.svelte).
- Nodes are regular HTML buttons in [`src/lib/components/MeshNode.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshNode.svelte), layered over an SVG connection plane.
- Detail popovers are not part of the canvas scene. They are positioned by DOM measurement plus clamped anchor math and rendered by the runtime host in [`src/lib/components/MeshRuntimeView.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshRuntimeView.svelte).
- Tests in [`src/lib/components/MeshCanvas.test.js`](/home/user/projects/taurhaus/src/lib/components/MeshCanvas.test.js) focus on row packing, connection count, lead anchor fan-out, and detail-anchor clamping.

This matters because taurhaus is not solving a general graph editor today. It is solving a branded, small-cardinality, lead-to-agents orchestration view with tightly controlled visual composition.

## Summary table

`Bundle size` below uses npm unpacked size listings from official package pages as of 2026-03-06.

| Option | Layout algorithms | Edge routing | Bundle size | Svelte 5 fit | Interactivity | Customization | Animation | Maintenance burden | Migration cost |
|---|---|---|---|---|---|---|---|---|---|
| Pure SVG + current manual layout | Excellent for current fixed star/hierarchical topology, poor once topology generalizes | Manual only; current bug class lives here | `0` new deps | Excellent | Minimal, all custom | Excellent | Excellent for bespoke motion | Highest ongoing ownership | None |
| Cytoscape.js | Strong. Preset, grid, circle, breadthfirst, concentric, plus extensions like dagre/fcose | Strong built-in curve styles and edge handling | `~5.7 MB` core, more with layout extensions | Medium. Imperative scene graph inside Svelte | Strong out of the box | Medium-high, but style model is Cytoscape-first | Good for graph transitions, weaker for bespoke card choreography | Much lower for graph math, higher adapter cost | High |
| D3-force + D3-dag | Good primitives, but you still compose the whole solution | Mostly manual; D3 helps math, not full edge UX | `~477 kB` + `~507 kB` | Medium-high. Easy to embed, but low-level | Low-medium unless paired with more D3 modules | High | High if we build it | Medium-high | Medium |
| ELKjs | Excellent hierarchical layout for directed graphs | Good routing metadata from layout output, but not a renderer | `~7.9 MB` | High as a pure layout engine, low as a complete canvas answer | None by itself | High if we keep our renderer | High if we animate layout diffs ourselves | Lower for layout math only | Low-medium if used as helper, high if treated as full solution |
| Svelte Flow | Good node-editor canvas, but layout is delegated to dagre/ELK/etc. | Good interactive edge framework, not automatic “diagramming” routing magic | `~313 kB` | High. Native Svelte library, built for Svelte 5 era | Excellent out of the box | High for node/edge components | Good | Medium | Medium-high |
| React Flow | Similar to Svelte Flow technically | Similar to Svelte Flow | Not evaluated further | Poor in this repo | Strong, but wrong framework | High | Good | Medium | Very high |
| AntV G6 | Strong graph-focused layouts and behaviors | Strong graph rendering feature set | `~7.2 MB` | Medium. Imperative graph engine in Svelte | Strong | Medium-high | Good | Lower for graph mechanics, higher for integration | High |

## Recommendation

Stay with the current SVG/HTML approach for the Mesh runtime canvas.

The current view is still a constrained product surface:

- one lead
- a modest number of agents
- deterministic topology
- highly branded cards
- external detail overlays
- deliberate, not exploratory, interaction

That is not where Cytoscape, G6, or even Svelte Flow deliver their best value. Those libraries pay off when the product needs one or more of these:

- arbitrary graph topologies
- user-driven pan/zoom as a primary interaction
- drag-repositioned nodes
- dynamic subgraphs
- general-purpose auto-layout for heterogeneous structures
- built-in selection, handles, minimaps, and connection editing

The current bug pressure is narrower than that. The connection-removal issue and recent anchor fan-out fix point to a local weakness in our own layout/edge bookkeeping, not to a mismatch between product needs and rendering architecture.

## Why not migrate now

### Cytoscape.js

Cytoscape is the strongest “full graph engine” candidate in this list. If taurhaus were moving toward a true topology viewer with pan/zoom, selectable subgraphs, alternate layouts, or arbitrary team structures, it would be a serious option.

It is still the wrong tradeoff today:

- It wants to own the graph scene. Our current implementation deliberately splits SVG edges from HTML node cards and measures DOM nodes for detail-anchor placement.
- Styling would move from direct HTML/CSS/Tailwind ownership into Cytoscape’s own styling model for much of the scene.
- The hardest part of our UI is not graph theory. It is preserving the exact branded node cards, external overlays, and product-specific motion language.
- For the current star layout, Cytoscape replaces simple deterministic math with a heavier imperative integration layer.

Inference from the docs: Cytoscape would reduce layout/edge math ownership, but it would introduce a substantial Svelte integration adapter and likely force a rewrite of node rendering and overlay anchoring.

### D3-force / D3-dag

D3 is attractive if the goal is “keep control, outsource some math.” That is real, but limited:

- `d3-force` is ideal for simulations, not this fixed lead-to-agents composition.
- `d3-dag` is more relevant because layered DAG layout is closer to the current topology.
- Neither gives us a finished runtime canvas. We would still own rendering, interaction, edge styling, overlay measurement, and most state synchronization.

This is the best low-level helper family, not the best migration target.

### ELKjs

ELK is the most interesting helper option if the layout complexity increases without a desire to give up our renderer.

Strengths:

- Excellent hierarchical layout for directed graphs.
- Better future path than force simulation if we add multiple groups, deeper hierarchy, or more than two visual tiers.
- Can be used as a pure layout engine while keeping our current HTML/SVG rendering model.

Weaknesses:

- It does not solve the full problem. We still own rendering, event handling, animation, and the DOM-overlay system.
- For the current topology, ELK is likely overkill and heavier than the benefit justifies.

### Svelte Flow

Svelte Flow is the best “framework-native” alternative if taurhaus eventually wants a node-editor style canvas.

Why it is not the recommendation today:

- Its core value is built-in node-canvas interaction: drag, zoom, pan, selection, connection editing, minimap, controls.
- The Mesh runtime canvas does not currently need most of that.
- Layout is still delegated to external helpers like dagre or ELK. It does not remove the layout-engine decision.
- Migrating from our HTML-over-SVG composition to a flow-canvas abstraction is still invasive.

If the roadmap shifts toward interactive graph manipulation, Svelte Flow becomes the best migration candidate ahead of Cytoscape because it fits the repo’s framework and keeps component customization straightforward.

### React Flow

React Flow is effectively out of scope for this codebase. The only reason to mention it is that Svelte Flow exists and is the framework-aligned sibling.

### AntV G6

G6 is the other credible full-graph platform. It has a richer graph-tool feel than we need and shares the same main downside as Cytoscape: it wants to become the scene engine, while our product value is still in controlled composition rather than general graph manipulation.

## What to improve if we stay with SVG

The current pain points can be addressed without a library rewrite.

### 1. Extract layout into a pure mesh-layout module

Move all geometry math out of `MeshCanvas.svelte` into a pure helper, for example:

- `computeMeshLayout({ width, height, lead, agents, mode, initSteps })`
- `computeConnectionAnchors(layout)`
- `computeDetailAnchor({ selectedNodeRect, canvasRect, fallbackMetrics })`

This lowers regression risk and makes the existing layout independently testable.

### 2. Separate topology rules from rendering rules

Right now row packing, lead anchor fan-out, and add-node placement are all coupled in one component. Split them into explicit phases:

- topology phase: which row each node belongs to
- geometry phase: exact coordinates and widths
- connection phase: anchor assignment and path endpoints
- overlay phase: detail-card anchoring

That would make removal/reflow bugs much easier to reason about.

### 3. Promote connection generation to a first-class tested contract

The recent bug is exactly the kind of failure that happens when connection generation is treated as a derived afterthought. Add focused tests for:

- remove from 7+ agents down to 6/5
- remove from middle vs edge positions
- reorder and resume transitions
- selected node overlay after reflow
- small container widths and row wraps

### 4. Introduce stable semantic anchor slots on the lead

The current fan-out fix ranks agents by x-position and derives lead anchors from width. Make that explicit and reusable:

- `anchorSlotIndex`
- `anchorSlotCount`
- `leadAnchorX`

That gives a stable connection model during add/remove transitions and future animation work.

### 5. Add pan/zoom only if product need appears

Do not adopt a graph library just to get pan/zoom “for free.” If the runtime canvas needs it later, lightweight zoom/pan on the existing SVG/HTML scene is cheaper than a wholesale scene-engine migration.

### 6. Keep node cards as HTML, not canvas primitives

The current cards, hover states, accessibility, and detail-overlay integration are easier to maintain in HTML/CSS than in a graph-engine rendering model. Preserve that advantage unless the product genuinely needs general graph tooling.

## Migration trigger points

Re-open the library decision if any of these become true:

- teams regularly exceed 10-12 visible members
- topology stops being a simple lead-to-agents graph
- product requires user drag, pan/zoom, or subgraph exploration
- multiple alternate layouts become a user-facing feature
- edge routing must avoid many overlapping nodes or cross-cluster paths

If that happens:

1. First choice: `Svelte Flow + ELKjs`
2. Second choice: `Cytoscape.js`

Reasoning:

- `Svelte Flow + ELKjs` is the better fit for a Svelte application that wants a customizable node UI and framework-native component model.
- `Cytoscape.js` is stronger when the graph engine itself becomes the product.

## Rough scope estimate if migration becomes necessary

### Incremental helper adoption: ELKjs only

Estimated scope: `1-2` days

- Keep `MeshNode`, `MeshConnection`, and runtime host structure.
- Replace row/position math with ELK-generated coordinates.
- Add layout-to-UI translation and animation interpolation.

This is the only migration path that looks safely incremental.

### Full canvas migration: Svelte Flow

Estimated scope: `4-7` days

- Replace `MeshCanvas` scene model
- Rebuild node components in Svelte Flow
- Rework edge rendering and status styling
- Rebuild detail overlay anchoring against flow coordinates/viewport transforms
- Update tests and interaction contracts

### Full graph-engine migration: Cytoscape.js or G6

Estimated scope: `6-10` days

- Replace the canvas renderer entirely
- Rebuild nodes/edges/selection behavior
- Re-solve overlay placement and brand styling
- Rework animation behavior to match current product feel

This is rewrite territory, not an incremental swap.

## Final call

For the taurhaus Mesh runtime canvas as it exists today, a graph library migration would be premature.

The current architecture problem is not “we chose SVG.” It is “our custom layout and connection logic needs a cleaner internal contract.” Fixing that contract is substantially cheaper than migrating to a scene engine that is optimized for broader graph problems than taurhaus currently has.

If the product evolves into a true interactive graph surface, re-open the decision with `Svelte Flow + ELKjs` as the leading migration path.

## Sources

- Current implementation:
  - [`src/lib/components/MeshCanvas.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshCanvas.svelte)
  - [`src/lib/components/MeshConnection.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshConnection.svelte)
  - [`src/lib/components/MeshNode.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshNode.svelte)
  - [`src/lib/components/MeshRuntimeView.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshRuntimeView.svelte)
  - [`src/lib/components/MeshCanvas.test.js`](/home/user/projects/taurhaus/src/lib/components/MeshCanvas.test.js)
  - [`docs/screenshots/mesh-missing-connection-after-removal.png`](/home/user/projects/taurhaus/docs/screenshots/mesh-missing-connection-after-removal.png)
- Library references:
  - Cytoscape.js docs: https://js.cytoscape.org/
  - Cytoscape npm: https://www.npmjs.com/package/cytoscape
  - d3-force docs: https://d3js.org/d3-force
  - d3-force npm: https://www.npmjs.com/package/d3-force
  - d3-dag docs: https://erikbrinkman.github.io/d3-dag/
  - d3-dag npm: https://www.npmjs.com/package/d3-dag
  - ELKjs npm: https://www.npmjs.com/package/elkjs
  - Eclipse ELK docs: https://eclipse.dev/elk/
  - Svelte Flow docs: https://svelteflow.dev/
  - Svelte Flow / xyflow site: https://xyflow.com/
  - Svelte Flow 1.0 release note: https://xyflow.com/blog/svelte-flow-release
  - React Flow docs: https://reactflow.dev/
  - AntV G6 docs: https://g6.antv.antgroup.com/
  - AntV G6 npm: https://www.npmjs.com/package/@antv/g6
