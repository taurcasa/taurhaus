# GPT-6 Astra calibration memo

Author: Fable (orchestrator). Date: 2026-09-06. Status: recommendation —
awaiting operator tier sign-off.

The judging protocol the operator approved: the Opus defect lens ran
independently inside every lane round; this memo is the orchestrator's
judgment of fit against our decision history, the pre-registered bias watch,
and a concrete tier recommendation. Briefs were orchestrator-verbatim
(LANE-SPEC + dispositions), executed through direct `codex exec` for purity.

## Evidence base (four exhibits)

**Exhibit 1 — flake-audit implementation** (mesh e2e flake class, high
effort): competent, evidence-driven implementation; commits carried root
causes, not just fixes.

**Exhibit 2 — review absorption**: adversarial findings were absorbed
without defensiveness; fixes addressed the finding's mechanism rather than
its surface.

**Exhibit 3 — test-strategy review** (as reviewer, high effort): an 81-line
strategy review whose top recommendation — machine-readable run accounting
where "green means everything selected actually ran" — caught three real,
distinct defect classes on its first two runs: a bail-truncated audit run
that had shipped a broken spec unseen; a "known ~2/10 flake" that is
actually a 3/3 deterministic product issue; and 22 silently-skipping
conditional tests nobody had ever counted. A review that designs an
instrument which immediately finds real defects is the strongest reviewer
evidence we have from any model in this program.

**Exhibit 4 — reform lane, two rounds** (implementing its own review, high
effort): all seven phase-1 deliverables landed red-first in one turn per
round (wizard walks 9→1, contract gate, accounting, capture eviction, bail
semantics, smoke, cost measurement). The decisive moment came in round 2:
the orchestrator's dispatch asserted a root-cause hypothesis (seed-state
divergence) with a prescribed method (app-data tree diff). Astra ran the
method, **falsified the hypothesis with preserved evidence** (both trees
archived under `.check-logs/reform-round2/`), flagged it loudly ("read this
before merging"), attributed the true cause to *its own* round-1 commit's
exiting CLI blockers, and fixed that instead — inside scope, in one turn.
Disproof-over-compliance with honest self-attribution is precisely the
behavior a frontier seat requires and the behavior cheaper models most
reliably fail.

## Residual profile (the Opus lens, decorrelated)

Across the reform rounds, Opus's residuals against Astra's work were
structural hygiene only — manifest authority, entry-shape uniformity, guard
placement: 3 minors + 4 nits, **zero correctness defects**. For comparison,
the same lens against Sol's frontier-roles lane found correctness-class
residuals in the same week (a mesh command asserted but never executed; an
attribution inversion). The lens itself stays justified: it found real
findings every round, which is exactly the decorrelated-review value we
priced in for a recurrent-depth model whose reasoning we cannot inspect.

## Bias watch (pre-registered vs observed)

- **Over-engineering**: the one standing concern from public field data.
  Observed: mild richness appetite, but every accounting field earned its
  place, and when the dispatch said "minimal, no new machinery" it complied
  exactly. Verdict: manageable under instruction; keep the diff-budget
  leash on heavy-implementer seats (Decision 3) — the telemetry now exists
  to grade it.
- **Reasoning opacity**: unchanged; mitigated by the two-family lens, which
  stays mandatory (Decision 4's routing).
- **Cross-file strength** (+20% rel. over Sol in public evals): consistent
  with observed behavior — its system-level reads (bail semantics,
  green≠ran, wizard-walk waste) were cross-cutting insights no Sol lane had
  surfaced.

## Recommendation

**Move `gpt-6-astra` from untiered to `frontier`**, ranked after `fable`.
Concretely: `capability_tier: Frontier` on the catalog entry, and the
signed-off table in `role-first-model-routing.md` gains astra in the
frontier row — both changes only on operator sign-off, per the table's
authority rule.

What this does and does not change:

- It makes Astra routable wherever policy asks for frontier capability —
  the architect, cross-file-review, and leashed heavy-implementer seats it
  already holds by role.
- It does **not** touch Decision 2: the orchestrator/lead seat still waits
  for the field test's cost-per-accepted-task numbers. Fable remains the
  default lead; swaps remain telemetry.
- Cost asymmetry stands (≈2.5× Sol, costlier cache reads, >272K surcharge):
  routing should keep Sol on parallel implementation lanes where Sol's
  residual profile is acceptable, and spend Astra where judgment density is
  the bottleneck — exactly the shape the blueprints already encode.

## Watch items for the field test

1. Oversize incidents on the leashed seats (predicted 1–3 in wave 1;
   zero beside sprawling diffs indicts the reviewer, not the leash).
2. Whether Astra's architect packets survive contact with implementers
   without mid-task re-architecting (orientation evidence, Decision 1).
3. Cost-per-accepted-task vs the Sol rows — the Decision 2 gate.
