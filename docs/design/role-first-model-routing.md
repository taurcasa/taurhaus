# Role-First Model Routing

Future-development design note (pre-decision; no implementation lane exists).
Origin: operator design conversation, September 2026, reviewed and amended by
the orchestrator. The roadmap at the end is the binding part: no stage starts
before its prerequisites hold.

## Core idea

Taurhaus treats **roles as the durable organizational and safety layer** and
**models as schedulable execution resources underneath them**.

Today a role does two jobs at once: it defines authority/behavior, and it
guarantees capability by pinning a model. The second job is valuable — it
prevents an orchestrator from handing an architecture task to a cheap model
because it "looks easy" — but the binding can become policy instead of a
constant without weakening the contract:

```
Role (durable)
  -> capability policy
    -> model selection (runtime)
      -> session/process (disposable)
```

This is already taurhaus's shape: the persistent entity is the team member and
its role; sessions are relaunched, quarantined, effort-switched, and moved
across accounts (protocols 22–24) while the member endures.

**Key principle: roles define the contract; models satisfy the contract.**

## Why roles stay first-class

Orchestrator, architect, developer, reviewer, security reviewer, test
specialist — these are not model categories. They are responsibility,
authority, allowed decisions, required context, quality gates, escalation
rules, definition of done, review requirements, and a capability floor. The
role catalog v2 schema already carries most of this; the missing piece is the
capability policy.

## Separating role semantics from model guarantees

A role's policy block, sketched:

```yaml
role: architect
authority: [architecture decisions, interface design, system constraints]
minimum_capability: frontier          # a TIER, resolved through the ModelCatalog
allowed_models: [fable-5.1, <frontier-class peer>]
preferred_model: fable-5.1
model_selection: fixed                # or adaptive
reasoning_effort: { default: high, allowed: [high, very_high] }
cross_family_review_required: true
downgrade_below_minimum: forbidden    # structural, not advisory
escalation: { target: orchestrator_or_human }
```

```yaml
role: developer
minimum_capability: strong_coding
allowed_models: [gpt-5.6-sol, fable-5.1, ...]
model_selection: adaptive
routing_objective: minimize_cost_per_accepted_task
reasoning_effort: { default: medium, allowed: [medium, high] }
escalation: { on_failed_review: next_tier, on_repeated_failure: architect }
```

Model names above are illustrative; the catalog is the authority.

### Amendment 1 — capability tiers live on the ModelCatalog

`minimum_capability: frontier` as a free-standing label rots the way every
duplicated fact rots. The tier is a **field on ModelCatalog entries** — the
backend-owned registry that already carries efforts and deprecation hints —
and roles reference tiers. When a cheaper model reaches a tier, the catalog
changes once; no role file is rewritten, no preset is migrated.

### Amendment 2 — the floor is structural

"Downgrade below the floor: forbidden" is enforced the way the writer boundary
is enforced: the launch-render path **cannot** emit a below-floor model for a
floored role, pinned by a boundary-style test, with the attempted violation
logged. Never orchestrator vigilance; never a soft warning.

## Two-stage routing (plus the assigner's third input)

```
Orchestrator chooses the ROLE
        |
Role policy constrains ALLOWED models + effort band
        |
Runtime router selects model + effort inside that envelope
```

The orchestrator never composes arbitrary `{role, model, effort}` triples; the
role remains the guardrail. **Amendment 3:** effort is a three-input decision —
the policy sets the allowed band, the *assigner* may pin an effort per task
(instance difficulty is visible to the lead, not to policy — this is the
shipped optional-effort-on-assignment mechanic), and the router defaults
otherwise.

Fixed and adaptive roles coexist: `model_selection: fixed` keeps today's
behavior for high-authority roles (orchestrator, architect); commodity
implementation becomes cost-aware. Migration is per-role, never all-or-nothing.

## The objective: cost per accepted task

Not token cost, not per-run cost. The metric that naturally absorbs retries,
review failures, escalation runs, human interventions, regression cleanup,
latency, and context reconstruction. A cheaper model needing two retries and a
heavyweight review costs more than one frontier pass.

### Amendment 4 — "accepted" must be a trustworthy event

The fastbreak retro's core finding was acceptance theater. The denominator is
a **ledger-recorded completion with a review verdict** (the W-B task ledger's
completion packet + rulings), never a member's self-report. Human-cost signals
fold in from streams we already log: nudges, deadline actions, wake
injections — the retro program's box-score metrics.

## Cross-family review as policy

Promote the shipped convention (Opus ↔ Codex, visual dual review) to data:

```yaml
review_policy:
  implementation:      { required: true, reviewer_family: different_from_implementer }
  architecture_change: { required: true, reviewer_family: different_from_author,
                         reviewer_minimum_capability: strong,
                         altitude_pass: { required: true, minimum_capability: frontier } }
```

The goal is not that one family is more correct; it is decorrelating blind
spots. This cycle's evidence: the cross-family flip has repeatedly found
orthogonal defect classes the same-family loop missed.

## Escalation

Ladder: failed review → next capability tier inside the role's envelope;
repeated failure or unresolved disagreement → architect/orchestrator
arbitration; exhaustion → human.

### Amendment 5 — escalation is bounded, and it is a handoff

Two hard lessons already paid for:

- **Oscillation** (the effort-reliability lane's emergent bug): any automatic
  ladder needs a per-tier attempt budget, a budget that clears **only on the
  acceptance signal**, and a terminal state that routes to a human. Reuse the
  `task_effort` state machine's shape verbatim.
- **Context is not free** (why the account-switch handoff manifest exists):
  sessions are disposable for *identity*, not for *context economics*. An
  escalation is a handoff — the new session receives the manifest (prior
  transcript pointer, ledger state, entry-point framing), and the router
  prices context reconstruction as a real cost.

## Learning from history — explicitly gated

Record per task: characteristics → role → selected model → effort →
tokens/time → review result → retries → escalations → human intervention →
final outcome. Over time this *may* justify a learned policy ("Sol is
excellent on contained Rust changes, escalates on ambiguous cross-cutting
refactors").

### Amendment 6 — telemetry first, learning last, maybe never

taureval's lesson stands: n=1 is ±1-case noise. A learned router before the
dataset exists fits noise with confidence. The gate is written into the
roadmap below: hundreds of accepted tasks per routing cell AND documented
misrouting by the handwritten ladder. Catalog changes (new models, tier moves)
partially reset the dataset.

## Placement

- **Policy fields** live in the git-backed role template schema — every policy
  change gets history, diff, and revert for free, which is the audit story for
  "who decided this tier."
- **Routing decisions** execute daemon-side (the daemon owns launches,
  protocols 16–19) and emit structured `launch.model.*`-family events carrying
  their inputs — the audit trail for the non-downgrade guarantee.
- **Authority split** stays as today: orchestrator owns decomposition,
  assignment, sequencing, operational completion; architect owns design and
  technical arbitration; reviewers own independent challenge; developers own
  bounded implementation with evidence. Routing must not re-centralize this.

---

## Roadmap

Each stage names what ships, the prerequisites that must already hold, and the
exit gate that must pass before the next stage may start. Stages 0–2 change no
runtime behavior. The current fixed-role/fixed-model discipline **remains the
default until each gate passes**.

### Stage 0 — declare the status quo (schema groundwork)

**Ships:** `model_selection: fixed` made explicit on every role;
capability-policy fields (`minimum_capability`, `allowed_models`, effort band)
added to the role schema as carried-but-unenforced data; `capability_tier`
field on ModelCatalog entries, tier assignments authored by the operator.

**Prerequisites:** 0.9.1 released (no schema churn mid-release); role catalog
v2 and template git storage (shipped); catalog tier assignments reviewed by
the operator — tiers are human-owned policy, not inferred. **Reviewed
2026-09-04; the signed-off table:**

| Tier | Models (rank within tier, highest first) |
|---|---|
| `frontier` | `fable` (Fable 5.1) |
| `strong` | `gpt-5.6-sol` · `opus` (Opus 5) · `claude-opus-4-6`/`-thinking` (agy) · `gemini-3.1-pro-high` · `grok-4.6` · `gpt-5.5` |
| `efficient` | `gpt-5.6-luna` (preferred for batch/volume work — speed and throughput over peak intelligence) · `gpt-5.4` · `gpt-5.4-mini` · `gemini-3.x-flash-*` · `gemini-3.1-pro-low` · `gpt-oss-120b-medium` · `grok-4.5` |
| *untiered* | `gpt-5.6-terra` (no current place — see rule below) · deprecated entries (`sonnet`, `haiku`; replacement pointers make them unroutable anyway) |

Three rules the review produced:

- **Untiered = unroutable.** A model with no tier is never selected by
  adaptive routing; it remains explicitly pinnable by a human. This is how a
  model with "no current place" stays available without entering any ladder.
- **Within-tier rank is data.** Operator calibration: `gpt-5.6-sol` ranks
  above `opus` on intelligence while both sit in `strong` — the router's
  "cheapest meeting threshold" and the escalation ladder need the ordering,
  not just the tier. Entries carry a rank (or ordered position) within tier.
- **Capability rank ≠ system role.** Opus below Sol on raw intelligence does
  NOT demote Opus in the system: its adversarial-review value is
  *decorrelation*, which lives in the review policy (`reviewer_family:
  different_from_implementer` + a floor), not in the tier ladder.
  Consequently the architecture-review policy expresses the shipped practice
  exactly: reviewer floor `strong` with family diversity, **plus** a
  `frontier` altitude pass on architecture-bearing changes — not a `frontier`
  reviewer requirement that would exclude the cross-family lens.

**Exit gate:** role import/export and preset round-trips carry the new fields
losslessly (adapter tests); `cli_renderers`/launch goldens byte-identical —
proving zero behavior change; a doc-pin test ties tier vocabulary to the
catalog.

### Stage 1 — telemetry: make cost-per-accepted-task measurable

**Ships:** the task ledger records task characteristics (type, size class,
blast-radius flag from the assignment contract), assigned role, the model +
effort **actually launched** (read from `RenderedLaunch` — the single render
authority, not the request), relaunches/retries, review verdicts, escalations,
and human interventions; tokens/time where the harness exposes them (time as
the proxy elsewhere). One reporting surface (a `just` recipe over the ledger)
computes cost-per-accepted-task per (role, model).

**Prerequisites:** Stage 0 fields (attribution); W-B task ledger with
completion packets and rulings (shipped); the box-score human-cost events
(shipped in the retro program).

**Exit gate:** after one real team wave (the operator's field test qualifies),
the report renders a filled table for that wave with no hand-collection.

### Stage 2 — floors become structural

**Ships:** the launch-render path rejects a below-floor model for a floored
role; a boundary-style pin test makes the violation impossible to reintroduce
silently; `launch.model.floor_*` audit events with decision inputs; the roster
builder UI cannot offer a below-floor model for a floored role and says why.

**Prerequisites:** Stage 0 tiers exist and are operator-reviewed; an
implementation decision on where floor data reaches the daemon (likely the
pushed-settings pattern from protocol 21 — resolve during the lane, and note
that a wire change may cost a protocol bump).

**Exit gate:** pin test green; goldens updated deliberately (the only
permitted diff is the new validation); a seeded below-floor role template is
rejected end to end with the audit event emitted.

### Stage 3 — bounded escalation (first adaptive behavior, narrow)

**Ships:** on a ledger-recorded review rejection, the daemon escalates the
member to the next tier inside the role's envelope: per-tier attempt budget,
budget clears only on ledger acceptance, terminal state notifies the
lead/human; every escalation rides the handoff manifest (transcript pointer,
ledger entry, prior-tier framing).

**Prerequisites:** Stage 1 telemetry live (to judge whether escalation pays);
Stage 2 floors (the ladder's envelope is enforced, not assumed);
machine-readable review verdicts in the ledger (shipped, W-B); the
`task_effort` attempt-budget pattern and the switch handoff manifest (both
shipped).

**Exit gate:** a paid-lane experiment (W4-style) demonstrates one full cycle —
rejection → escalation with manifest → acceptance — and one budget-exhaustion
path routing to the lead, with the ledger showing both faithfully.

### Stage 4 — adaptive selection inside the envelope (handwritten ladder)

**Ships:** developer-class roles may set `model_selection: adaptive`; the
runtime router picks the cheapest allowed model via a handwritten ladder keyed
on task characteristics; the assigner's per-task pin overrides within the
band; orchestrator/architect roles stay `fixed`.

**Prerequisites:** Stage 3 escalation live — **this is the load-bearing
coupling: adaptivity is only safe once a wrong cheap pick self-corrects
boundedly and automatically**; Stage 1 has several waves of data so the ladder
is sanity-checked against reality rather than intuition; context-reconstruction
cost included in the accounting.

**Exit gate:** over at least one full wave, adaptive roles' measured
cost-per-accepted-task ≤ the fixed baseline **and** review-major rates are not
worse. If either fails, adaptivity reverts to fixed and the ladder is revised.

### Stage 5 — learned routing (gated; may never trigger)

**Ships:** the ladder augmented or replaced by a policy learned from the
Stage-1 dataset.

**Prerequisites (hard gates):** hundreds of accepted tasks per routing cell;
documented cases where the handwritten ladder misroutes; metric definitions
stable across at least two retros; a drift rule for catalog changes (new
model/tier resets affected cells).

**Exit gate:** the learned policy beats the ladder on cost-per-accepted-task
in a held-out comparison without degrading review outcomes. Absent that, the
ladder stays — this stage is allowed to never happen.

---

## Pre-registered: Astra 6.0 arrival (expected September 2026)

A second frontier-class model is rolling out. Its arrival is the first real
test of the "catalog changes, roles don't" claim — no role file or preset
should need editing. On-arrival checklist (a small-change lane):

1. **Catalog entry** on the harness that serves it (expected: the codex
   harness with a new model slug; if it needs a new CLI, it takes the full
   "Add a new CLI tool" path instead and this stops being small).
   Effort vocabulary confirmed against the real CLI, goldens extended.
2. **Tier sign-off by the operator** — anticipated `frontier` beside
   `fable`, but the rule stands: tiers are reviewed on the real model, never
   pre-assigned.
3. **What a second frontier model unlocks** (policy options, each its own
   deliberate change, not automatic):
   - *Frontier family diversity for altitude passes*: architecture-bearing
     work implemented by Fable can take its altitude pass from Astra and
     vice versa — closing the current same-family-at-the-top correlation gap.
   - *Arbitration independence*: "disagreement between agents → top-tier
     arbitrator" can require the arbitrator's family to differ from both
     disputants.
   - *Role de-overloading*: architect, lead, and final arbitrator no longer
     have to be the same model because it's the only frontier one.
4. **Field calibration before reliance**: run it as an implementer or
   reviewer on a few contained lanes first; the tier is confirmed by observed
   work, and Stage-1 telemetry (once live) records the evidence.

## What this preserves

The operator's current safety property — high-impact work cannot be silently
downgraded — survives every stage, upgraded from convention to structure:
floors are catalog-resolved, render-enforced, test-pinned, and audited. Roles
remain the organizational contract; models become a budgeted resource; and the
system's intelligence spend becomes something the orchestrator allocates
rather than something the roster hardcodes.
