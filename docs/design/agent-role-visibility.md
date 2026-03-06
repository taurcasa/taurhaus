# Agent Role Visibility In Mesh Runtime Canvas

## Recommendation

Show a **small role summary on deliberate hover/focus**, and keep **click for actions**.

This is worth doing, but only as a narrow runtime aid:

- surface a compact role card when the pointer rests on a node for a short delay or when the node receives keyboard focus
- keep the existing click interaction dedicated to the actionable node detail panel
- do **not** show full template instructions in runtime
- do **not** frame roles as model capability labels
- do **not** make runtime role visibility the primary fix for poor role adoption

Role visibility helps runtime comprehension. It will not, by itself, solve role setup friction.

The better product direction is:

1. add lightweight runtime role visibility
2. separately simplify role assignment and editing in setup/runtime flows

## Perspective Shift

The core value of a role is **context steering**, not abstract capability declaration.

The underlying models are general-purpose. A `UI specialist`, `architect`, or `reviewer` role is not valuable because it proves the agent *can* do UI, architecture, or review work. The value is that the role:

- steers what work gets assigned to that agent
- accumulates domain-specific context over time
- preserves a domain identity even after compaction or handoffs
- helps the lead remember what kind of work that agent should keep doing next

So runtime role visibility should answer:

- what context domain is this agent operating in?
- what kind of work has this agent been shaped to continue?
- what behavioral boundaries should I expect from it?

It should **not** answer:

- what can this model theoretically do?

## Current State

Today the runtime canvas already exposes:

- member name
- generic role chip (`Lead` or `Agent`)
- tool and model
- project binding
- description
- runtime actions in the click-open node detail (`Resume`, `Stop`, `Capture`, `Focus`)

What is missing is the **template-derived runtime identity** and its context meaning.

That gap matters because the template system already carries richer semantics:

- role name
- focus area / instructions
- behavioral contract
- accumulated domain expectation
- tool/model defaults

At runtime, those semantics disappear. The user sees that an agent exists, but not *what lane it is meant to stay in*.

## Is Role Info Useful At Runtime?

Yes, in a constrained way.

Role info is useful in four concrete runtime moments:

1. **Task routing**
   When the team lead is deciding who should take a task, they need to know which agent already carries the right context domain. `UI specialist` or `architect` is useful because it signals likely retained context, not because it lists exclusive abilities.

2. **Behavior diagnosis**
   If an agent is acting unexpectedly, the lead needs to know whether that behavior is off-role or consistent with the context the agent has been given.

3. **Trust and recall**
   Teams are often created once, then revisited later. Runtime role visibility helps the lead remember why `architect-1` exists and what domain it should continue owning.

4. **Role quality feedback**
   If role identity is visible during real work, weak or vague role definitions become obvious. That creates pressure to improve the context boundaries and role summaries.

It is less useful for:

- line-by-line debugging
- reading full instructions during active work
- replacing setup/configuration screens

So the right goal is **runtime orientation around context domain**, not template inspection.

## On Hover Vs On Click

Use **hover/focus for summary**, **click for actions**.

That split matches the current interaction model better than combining everything in one surface.

### Why not combine role info into the click panel only?

Because click is already an action surface. If role information lives only there, the user has to open an action menu just to answer a read-only question. That creates friction and adds accidental action risk.

### Why not make hover show the full current detail panel?

Because hover-triggered action panels are noisy and error-prone. The current canvas is operational, not exploratory art. Hover should stay non-destructive and lightweight.

### Recommended interaction

- **Hover or keyboard focus on node**: small summary card after roughly 180-250ms
- **Click on node**: existing action detail panel
- **If click panel is open**: suppress hover summary for that node
- **On touch/pointer-coarse devices**: skip hover card entirely; rely on click panel

This keeps runtime scanning fast while preserving deliberate action handling.

## What Role Info Matters

The useful runtime subset is:

- **Role name**
  Example: `Codex Architect`

- **Focus area**
  A short label or sentence describing the domain this agent is meant to stay in
  Example: `Owns structural decisions and architecture review.`

- **Context lane summary**
  A short sentence that helps the lead understand what this agent has likely been accumulating context around
  Example: `Keeps long-lived context around module boundaries, reviews, and design tradeoffs.`

- **Behavioral boundary summary**
  One short sentence describing what the agent should independently handle versus escalate
  Example: `Handles pattern decisions; escalates new feature direction.`

- **Optional recent work theme**
  If taurhaus ever wants a richer runtime signal, recent task themes would be more valuable than capability tags
  Example: `Recent work: watcher architecture, stall detector review`

- **Tool + model**
  Keep visible because they still shape expectations, but they are not the meaning of the role

Optional:

- **Role source marker**: built-in vs custom

Do **not** show by default:

- generic capability tags like `architecture • review • docs`
- full instructions blob
- full behavioral contract lists
- constraints schema
- project binding rules
- raw role IDs unless needed in debug mode

## Proposed UI

### Hover/focus summary card

Small, read-only, non-interactive except maybe for one safe link such as `View role` in a later phase.

```text
┌──────────────────────────────────┐
│ frontend-dev                     │
│ Codex Architect                  │
│ Focus: architecture decisions    │
│ Carries structural context       │
│ across reviews and refactors.    │
│ Handles pattern choices;         │
│ escalates direction changes.     │
│ Codex · gpt-5.4 high             │
└──────────────────────────────────┘
```

Rules:

- anchor near the node, lighter than the click detail panel
- no action buttons
- dismiss immediately on pointer leave
- suppress while another node action panel is open

### Click detail panel

Keep action-first behavior, but add a compact role section near the top.

```text
┌──────────────────────────────────────┐
│ frontend-dev                [Agent]  │
│ Active                                │
│ Codex · gpt-5.4 high                  │
│ Project: taurhaus-web                 │
│                                      │
│ Role                                 │
│ Codex Architect                      │
│ Focus: architecture decisions        │
│ Carries structural context across    │
│ reviews and refactors.               │
│ Handles pattern choices; escalates   │
│ direction changes.                   │
│                                      │
│ [Resume] [Stop] [Capture] [Focus]    │
└──────────────────────────────────────┘
```

This avoids forcing the user to discover role semantics only via hover.

## Does This Encourage Role Adoption?

Yes, but only modestly.

Visible runtime roles create a real feedback loop:

- if a role helps during active management, users see immediate value
- if a role summary is vague, that weakness becomes obvious
- useful runtime surfacing makes role definition feel less like hidden metadata and more like an operational memory aid

But the effect is limited by setup friction. If role authoring and assignment still feel heavy, runtime visibility alone will not cause broad adoption.

## Alternative: Simplify Role Setup Instead

This is not an either/or decision. But if forced to choose a single investment, **simplifying setup has larger impact on adoption**.

Reason:

- runtime visibility increases value perception for users who already use roles
- setup simplification increases the number of users who will use roles at all

The strongest product sequence is:

1. simplify role assignment in setup and hot-add
2. surface the selected role cleanly at runtime
3. later add richer runtime context signals if usage justifies it

High-value simplifications would be:

- clearer preset-to-runtime mapping
- a dedicated short `focus` or `context summary` field instead of deriving meaning from long instructions
- fewer editable knobs during hot-add
- better defaults for role assignment and naming

## Implementation Boundaries

If this moves from exploration to implementation, keep scope narrow.

- Reuse current node metadata flow rather than inventing a second role data pipeline.
- Add explicit runtime role summary fields instead of shipping full template text into the canvas.
- Keep hover summary purely informational.
- Keep click panel authoritative for actions.
- Avoid a new modal or side panel for phase 1.

The sidebar HoverCard pattern is a good precedent: brief contextual information on hover, deeper action or navigation elsewhere.

## Practical Data Contract

Phase 1 runtime canvas likely needs a compact role payload per member such as:

```json
{
  "roleId": "codex-architect",
  "roleName": "Codex Architect",
  "focusArea": "Architecture decisions and structural review",
  "contextSummary": "Carries long-lived context around module boundaries, reviews, and refactors.",
  "behaviorSummary": "Handles pattern choices independently; escalates direction changes.",
  "tool": "codex",
  "model": "gpt-5.4 high"
}
```

That is enough for runtime visibility without leaking the full template schema into the canvas layer.

## Final Call

Runtime role visibility is **worth doing**, but only as a small clarity feature.

Ship:

- hover/focus role summary
- compact role section inside the click detail panel
- explicit focus area, context summary, and behavioral boundary
- no generic capability tags

Do not treat it as the main adoption fix. If the product wants more role usage, simplify role setup first or in parallel.
