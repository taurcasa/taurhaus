# Team Blueprints for the Two-Frontier Era

Discussion proposal (operator + orchestrator), 2026-09-06. First-draft model
assignments with rationale — nothing here is implemented until discussed,
and every Astra placement is contingent on the calibration sign-off
(`role-first-model-routing.md`; the flake-audit calibration lane is in flight).

## What three days of Astra field data says

**Strengths** (independent evals + practitioner reports):

- **Cross-file / distributed-context reasoning**: +20% relative over Sol,
  +33% over Opus 5 on hard cross-file code review (CodeRabbit eval: 57.1% vs
  47.6% coverage). The single most role-relevant number.
- **Long-horizon terminal/agentic work**: wins Terminal Bench 4.0 (57.9 vs
  Fable's 55.8); token-efficient on sustained multi-step tasks; computer use
  is its headline training focus.
- **Cybersecurity**: OpenAI's own Critical-designation domain; consistently
  cited as a real strength.
- **Professional artifacts / UI geometric correctness**: multiple reports of
  fixed UI/UX flaws vs previous GPT models.
- Finishes a task in fewer tokens than Fable at similar nominal work.

**Weaknesses**:

- **Reasoning depth**: behind Fable 5.1 on the independent intelligence
  indices (61 vs 66 at max effort) despite OpenAI's own table claiming the
  opposite everywhere.
- **Over-engineering bias**: the most consistent practitioner criticism —
  asked for a targeted fix, it returns a sprawling PR. This collides head-on
  with our "no over-engineering" doctrine and small-change discipline.
- **Cost in practice**: 2.5× Sol per token; subscription-plan users report
  burning ~30% of a weekly limit on one task; >272K-token requests carry a
  surcharge (Fable doesn't); Fable's cache reads are cheaper — which matters
  for exactly the long orchestrator loops that replay context.
- **Monitorability**: recurrent-depth reasoning obscures the chain — raises
  the value of decorrelated review when Astra writes or decides.
- Public-version refusals in security-sensitive areas (test before relying
  on it in the security role).

**Our own calibration lane so far** (n=1, mesh e2e flake audit): single-turn
implementation, clean scope discipline, one-helper rule honored, cadence
arithmetic stated as asked; review verdict approve with five judgment-level
minors, none correctness. Consistent with the terminal/agentic strength and
mildly consistent with the thoroughness-over-minimalism bias.

## Placement principles falling out of the data

1. **Fable orchestrates by default; Astra architects by default.** The lead
   loop is retrieval-heavy and cache-read-dominated (Fable's economics + its
   reasoning-depth edge for arbitration); architecture is where Astra's
   cross-file connection strength is the differentiator. Swappable per team —
   this is the operator's "one leads, the other architects" vision with a
   data-backed default orientation.
2. **Review becomes genuinely two-family at every tier.** Claude-family
   implementations get Astra/Sol review; GPT-family implementations get
   Opus/Fable review. Astra joins the review stack specifically for
   cross-file/system-level lenses (its strongest measured skill); Opus stays
   the Claude-family review workhorse (its catch-rate number is weakest of
   the three, but its value in our loop is decorrelation plus a season of
   real caught majors — and it is priced for volume).
3. **Sol remains the implementation workhorse.** Nothing in the data dents
   it: near-Astra bug coverage at 40% of the price, and a season of proven
   lanes. Astra implements only where the task is genuinely cross-cutting or
   terminal-heavy — and always under a tight assignment contract with an
   explicit diff budget, because of the over-engineering bias.
4. **Luna owns volume.** Unchanged.
5. **Cost per accepted task is the tiebreak**, and Stage-1 telemetry now
   measures it — these blueprints are hypotheses the routing report will
   grade.

## The blueprints

### 1. Product Build Team (greenfield app, fastbreak-class)

| Role | Model / effort | Why |
|---|---|---|
| Lead / orchestrator | **Fable 5.1** high | arbitration depth, cache-read economics of the long loop |
| Architect | **Astra** high–xhigh | cross-file system design is its best measured skill |
| Implementer ×2 | **Sol** medium–high | proven workhorse at the right price |
| Heavy implementer | **Astra** high | cross-cutting/foundational slices only; strict contract + diff budget (over-engineering guard) |
| Adversarial reviewer | **Opus** high | Claude-family lens on GPT-written code (decorrelation) |
| Architecture reviewer | Fable altitude pass | frontier check on architecture-bearing changes, family-diverse from the Astra architect |

### 2. taurhaus Core Team (our repo: daemon, wire, concurrency)

| Role | Model / effort | Why |
|---|---|---|
| Lead | **Fable** high | house context depth; the deadlock class showed why reasoning depth leads here |
| Architect | **Fable** high | concurrency/protocol design wants the depth leader; **Astra as the cross-file reviewer** instead of architect here |
| Implementer | **Sol** high | as all season |
| Reviewer A | **Astra** high | cross-file lens — the +20% class, on the codebase where cross-module drift bites us |
| Reviewer B | **Opus** high | second family-diverse lens on Claude-written slices |

### 3. Security Audit Team (the /security-audit doctrine, phase boundaries)

| Role | Model / effort | Why |
|---|---|---|
| Lead auditor | **Astra** xhigh | its designation domain; terminal-driven probing |
| Counter-auditor | **Fable** high | decorrelated second opinion — mandatory given Astra's obscured reasoning |
| Fix implementer | **Sol** high | remediation stays workhorse-priced |

*Gate before adoption: verify the public-version refusal behavior doesn't
block legitimate audit prompts.*

### 4. Research / Eval Team (taureval-class studies, model comparisons)

| Role | Model / effort | Why |
|---|---|---|
| Lead / synthesis | **Fable** high | the retrieval/research edge is explicitly Fable's per the comparisons |
| Researchers ×N | **Sol** medium + **Grok 4.6** | breadth cheaply; Grok for live-web-flavored sweeps |
| Judge A + Judge B | **Fable** + **Astra** | two-family judging — taureval's own lesson (judges differ per cell) finally has a structural answer |

### 5. Batch / Processing Team (migrations, docs sweeps, data volume)

| Role | Model / effort | Why |
|---|---|---|
| Coordinator | **Sol** medium | cheap, sufficient for mechanical orchestration |
| Workers ×N | **Luna** low–medium | the batch/volume pick, per the operator's own calibration |
| Sample reviewer | **Opus** medium | spot-checks a sample, not everything |

### 6. Design-led UI Team

| Role | Model / effort | Why |
|---|---|---|
| Creative direction | **Fable** high | the sidebar program is the evidence |
| UI implementer | **Fable** (incumbent) vs **Astra** (challenger) | Astra's UI-geometric-correctness reports earn it a bake-off, not the seat |
| Visual reviewer | the other frontier family | decorrelated eyes on aesthetics |

## Role-template delta this implies (a later lane, post-discussion)

New bundled roles: Astra Architect, Astra Cross-File Reviewer, Astra Heavy
Implementer (behavioral contract carries the anti-over-engineering guard and
diff budget explicitly), Astra Security Auditor, Dual-Judge pair. Revised:
review-route notes on existing roles to name the two-family topology.
Shipping bundled role changes pays the `BUILTIN_CATALOG_REVISION` cost
(Stage 0's lesson) — priced into the lane.

## Decisions (operator questionnaire, 2026-09-06)

1. **Orientation: default + swaps.** Fable leads / Astra architects as the
   catalog default; any team may deliberately swap, and swaps are telemetry.
2. **Astra as lead: after telemetry.** The architect/reviewer seats get
   graded by the field test's routing report first; the lead experiment is
   decided on cost-per-accepted-task numbers.
3. **Heavy-implementer leash: review gate + telemetry.** The assignment
   contract states a diff budget; the reviewer FAILS oversized diffs; each
   oversize incident lands in telemetry as tier-decision evidence.
4. **Opus shrinks to one seat now** (operator override of the
   keep-two recommendation): Opus keeps the product-review seat; Astra takes
   core cross-file review solo. The next retro's routing report grades the
   bet.
5. **The 0.9.2 field test runs the Product Build blueprint.**
6. *(Adjacent thread closed)* The tab-revisit fade is **kept** as deliberate
   behavior; the regression guard's lineage comment can drop its "pending
   review" clause at next touch.

Execution order: Astra calibration lane concludes → operator tier sign-off →
role-template lane (Astra Architect / Cross-File Reviewer / leashed Heavy
Implementer / Security Auditor / Dual-Judge pair; review-route updates;
diff-budget gate + oversize telemetry event; BUILTIN_CATALOG_REVISION bump)
→ Product Build field test.
