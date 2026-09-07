# Wave-1 machinery audit — blind auditor (Fable)

Scope: taurhaus-side machinery only (roles, routing, leash, overhead, coordination).
Evidence: ~/projects/taurjob (git history, docs/wave-1/ledger.md, rulings.md,
wave-closure-report.md, team-recreation.md, reviews/, mesh-archive/), the role
YAMLs, team-blueprints-frontier-era.md, team-delivery-standard.md, and the
routing report. I did NOT read docs/wave-1/retro/ (the team's own retro) —
kept blind by design. Product quality is out of scope except as machinery signal.

Base numbers (cited throughout): wave ran ~13.5 h wall (first commit 18:53
Sep 6 → 08:22 Sep 7), 606 commits (~491 docs-only, ~120 code-bearing), product
src 12,802 lines (+615 test-harness lines), hand-written process prose ~11.5k
lines added, committed evidence trees ~233k lines. 28 tasks (T1–T28), 41 rulings
(R1–R41), 16 escalations (E1–E16), 60 review files. Routing report: 27 task-
touches, 24 accepted, 3 completed_unruled, 4 relaunches, 0 nudges/staled/
oversize/effort-switches; tokens not collected.

## Findings, ranked by consequence

**F1 [ROUTING] — The two-family review route caught real, named defects at every
tier; it is the machinery's clearest win.** Not stamping. Concrete catches, each
with its artifact: E12 — M2 checkpoint A was "architectural theater" (no
dispatcher, in-memory store, `cancel_run` with no process to signal; altitude,
`reviews/T19-A-structural.md` findings 1–2, HIGH); R30 — `tasks.error_code`
stored Debug-lowercase so Results *never showed a real failure reason on
production data* (found in T27-5, latent through two acceptance passes);
R28/R29 — live matrix-run rows 19/57 (oversize output mislabelled; cumulative
counters overwritten); R31 — three hand-projected task-row writers as the common
cause (altitude structural, T26-structural 36ce66c); R34 — recovery skipped
leader-less process groups, a research child could survive an app crash
unrecorded (judge, reproduced on 485449f); four generations of scanner
false-cleans (judge: non-UTF-8 skip, `rglob` unreadable-dir omission, unscanned
live index cells/schema SQL, cross-page freelist sentinel, item-10 clean-flag
bypass); R32 — any withholding marked a read failed (design live pass, drift 2);
R37 — freed SQLite pages held reader output on disk → secure_delete/TRUNCATE, a
real shipped privacy fix. T9: judge rejected the design packet with 10 findings;
product-reviewer accepted all 10 **and retracted 3 of its own confirmed-sound
entries** (ledger T9 row) — direct evidence the second family lens changes
verdicts, not just adds prose.

**F2 [ROLES/COORD] — The altitude seat is the machinery's worst config defect:
it failed to materialize as specified twice, in two different ways.** Original
team: initialized cli_tool=claude, model absent, against the codex role
definition; never acknowledged a task; lead substituted the T7 pass (E5, task
7 ruling: "altitude-reviewer inactive, no uptake after nudge"). Recreation spec
said "codex default … deliberately NOT the old claude/no-model variant"
(team-recreation.md), yet the seat again came up as Fable and the operator had
to ratify it post hoc (ledger "Post-recreation correction"; E8 "the Codex
altitude seat did not materialize"). Root cause is the role text itself:
`v3-architect-codex.yaml:18` — "The preset-pinned role_id stays
v3-architect-codex for compatibility; switch the open slot by editing
defaults.cli_tool, defaults.model…" — an open-slot compat hack that yields
incoherent (claude tool / codex role / no model) seats. Once running as Fable,
the seat was one of the highest-value on the team (E12, R31, R37, lease finding
9, T26-rerun-structural b3dd570), so the failure is initialization machinery,
not the lane.

**F3 [OVERHEAD] — Ceremony share: ~81% of commits, ~43% of authored lines; the
ledger alone was committed 257 times (42% of all commits).** 491 of 606 commits
touched only docs; hand-written process prose ~11.5k lines vs ~15.4k added
code+test lines. Rulings.md is 1,095 lines; the integration plan was amended in
46 commits; T26's ledger row lists ~26 review artifacts/verdicts for one task.
The ledger self-diagnosed its failure mode: row 39 grew to 21,661 characters in
one table cell, "every condition in it was correct and none was reachable"
(closure finding 12). Where it was worth it: the ledger was the recovery
substrate that let a killed team resume mid-wave with nothing lost (42-line
team-recreation.md; wave continued to a certified close) — that alone justifies
a state-of-record, not this format. Where it was theater: the mesh board
drifted from the ledger for hours (Board reconciliation note: "mesh showed 19
completed for hours"; two overdue closures), so the wave effectively ran two
task systems and hand-reconciled them.

**F4 [OVERHEAD] — The scanner/certification recursion was the single largest
sink and had no internal stop condition; the operator had to impose the freeze
(R41).** The privacy scanner went v3→v4→v5→v6→7ef2218 (~8 iterations, items
1–11), with ~10 judge/altitude/product rounds *on the instrument itself*, ~500
lines of rulings (R33–R38), and release-grade forensics (six roots, per-root
zeros, field-set fingerprints) for a wave-1 demo slice on synthetic documents.
It did catch a genuine product privacy defect (R37) and produced durable
evidence rules — but the depth was chosen by the loop, not by a cost-benefit
ruling, which contradicts the operator's standing "efficiency before rigor"
doctrine. The machinery needs a depth gate it can trigger itself (see A8).

**F5 [LEASH] — The diff-budget leash shaped behavior but its teeth never
engaged; the ratchet is invisible to telemetry.** oversize_diffs = 0 across all
rows (routing report), matching zero `oversize_diff` rulings in the ledger. But
budgets were raised at least 15 times: T27 700→1550 in 8 steps, T28 900→1200 in
6, T1 400→450, T11 400→430, T26-fix-1 180→250, and twice explicitly **post
hoc** (T2a "exceeded without prior approval, lead-approved post hoc"; T6 "224
lines vs 200 budget recorded as overage, revised budget 260 granted" — both
docs lanes, where the oversize contract technically doesn't bind). The heavy
lanes did show the intended behavior: per-batch forecast/actual accounting
(T28-9 "forecast 70–85, actual 90"), stop-and-request-raise before crossing,
batch decomposition (8 and 11 batches), and the judge auditing the budget basis
("task-owned consolidated lines… ≈225/250"). The pre-registered box score
(prediction 3) anticipated exactly this ambiguity: zero incidents is a pass
only "with real reviewed diffs behind it" — which there were — but the current
telemetry cannot distinguish a respected leash from a lenient ratchet because
raises are not events. Attribution verdict: the column works; it is
under-inclusive.

**F6 [ROLES] — The roster inverted the blueprint's implementation economics
without a decision record: Astra heavy seats, not Sol, wrote most of the
product.** Blueprint 1 prescribes "Implementer ×2 Sol… Heavy implementer Astra:
cross-cutting/foundational slices only" and principle 3 says "Sol remains the
implementation workhorse… Astra implements only where genuinely cross-cutting."
The team ran 1 Sol + 2 Astra heavy (team-recreation.md: "remove the second Sol
implementer / add a second heavy seat"), and the heavy seats owned M0, M1, M1b,
M3a-1, M3a-2, M3b, plus both closure lanes — i.e. nearly all product code, at a
tier the blueprint itself prices at 2.5× Sol per token. It worked (0 oversize,
both heroes accepted), but Decision 2's cost-per-accepted-task gate cannot be
graded: tokens aren't collected and wall-time medians are incomparable across
task sizes (Sol median 234m because it drew M2 + T26 integration; heavy 34m
across 11 smaller touches). The wave's central tier question is unanswered by
its own telemetry.

**F7 [ROUTING] — Decision 4 (Opus single seat) held; the lenses demonstrably do
not overlap, so the "one lens suffices" reversal is not triggered.** Distinct,
non-overlapping catch profiles: judge (GPT) owned evidence-integrity (scanner
false-cleans, R34, the flake race, item 10/11 counter audits); altitude (Fable)
owned structure (E12, R31, lease finding 9, R37); product (Opus) owned
acceptance semantics and the meta-findings that are this wave's most durable
export (A5 "fixtures must reproduce what the writer emits", "a check that
cannot fail" with five instances, per-root zeros reporting, the unreachable-
rubric measurement, R35a exact-equality correction). The altitude pass added
value *beyond* product review on every architecture-bearing surface — R31 and
the lease findings are not findable by a product lens. Cost side: Opus's
routing row (2 tasks, 5m17s median) is absurdly disconnected from its actual
output (~20 review artifacts + the 377-line plan) — see F9.

**F8 [COORD] — The dominant coordination defect class was the shared checkout:
five escalations, each spawning an after-the-fact rule.** T12 collision (a
committed review briefly deleted from the tree), E13 (committed red scaffold
broke master's gate), E14 (partial commits broke svelte-check on master,
twice from the same lane), E15 (Xvfb capture grabbed another lane's window),
E16 (product-reviewer's `git add -A` swept another lane's uncommitted code onto
master in four commits). Every rule that fixed these (per-seat worktrees,
pathspec-only commits, separate `git status` read, per-seat CARGO_TARGET_DIR,
own-display capture) was invented mid-wave via incident. taurhaus already knows
these rules from its own repo; the field test paid to rediscover them. Host
contention belongs here too: three concurrent cargo builds tripped the OOM
guard and SIGTERMed a gate; disk hit 96%.

**F9 [TELEMETRY] — Routing telemetry misses most of what reviewers do and some
of what the monitor does; three concrete attribution holes.** (a) Review work
lands as rulings on the *author's* task, so reviewer rows collapse (Opus: 2
tasks touched; v3-architect-codex: 4 touches, 1m59s median — against dozens of
review artifacts); wall-time medians for review seats are noise. (b) nudges=0
and staled=0 in the report while the ledger records the idle monitor nudging
the closure lanes "repeatedly" and E5's nudge of the dead altitude seat — the
monitor's nudges aren't counted in the nudge column. (c) The archive is missing
sidecars for completed tasks 3 and 4, and 9 launches sit in _unattributed.jsonl
(roster boots). Box-score prediction 2 ("every member launch lands in the
sidecars") is only mostly met.

**F10 [ROLES] — judge-astra-1 ran under the wrong contract all wave and still
earned its seat; the dual-judge text was pure dead weight.** Its role is the
GPT half of a dual-judge pair with verdict isolation ("Never see the other
judge's verdict before recording your own", judge-astra.yaml:40) — no Judge
Fable, no cell, no rubric ever existed (brief; E4 "no bake-off this wave").
Repurposed, it became the team's best evidence auditor (F1's scanner chain,
T9's 10-finding reject, T26 verdict discipline: "REJECT as an unqualified PASS
record"). The isolation text was inert ceremony occupying its compaction
summary. Ship the role it actually played.

**F11 [ROLES] — Seat-by-seat verdict (Q1), and 9 was approximately right.**
Earned clearly: both heavy seats (all milestones + closure lanes, disciplined
accounting), implementer-1/Sol (M2 hero + the entire T26 integration/scanner
grind; but also the wave's two worst implementation incidents, E12 theater and
E14 partial commits — its "NO FAKE FEATURES" role text did not prevent E12,
review did), product-reviewer (F7; also caused E15/E16 process incidents, both
self-caught/disclosed), altitude-as-Fable (F2), judge (F10), design (T2
contract; REQUEST CHANGES → 11-item setup fix list into T22; six drifts →
T28-6; R26 geography catch; three live passes closing the design record; one
convention violation E11, edits under review, self-reverted), lead (routing,
41 rulings, smoke passes that found shipped-copy defects, zero product code —
box-score prediction 5 held: delegates, doesn't implement). Weakest
utilization: **architect** — high-value early (T1 frozen contract, R15/R15a),
then effectively idle after the architecture froze (~2 task touches, no
standing load; its acceptances of T3/T4/T5/T8/T10 were same-family and
correctly recorded as such). A 9th seat is defensible for this wave shape;
the architect seat as staffed is a part-time seat.

**F12 [COORD] — Deadlines were never used; the idle monitor was the only
uptake mechanism and it both spammed and failed.** No assignment carried a
deadline (deadline columns all zero — consistent with the standard's "optional
overrides, never default"). The idle monitor: nudged legitimately-idle batch
lanes repeatedly until an operator note invented the block-between-batches
convention (ledger "Closure lanes" note); nudged the dead altitude seat to no
effect because the seat's launch was broken (E5) — a nudge cannot fix
launch-level failure, and nothing escalated the distinction. E10 is the
matching mesh bug: `accept`/`start` succeeded for a non-owner after
reassignment; only `complete` refused — ownership is unverifiable except via
`mesh task get`.

**F13 [COORD] — The kill/recreate recovery worked and should be credited as a
machinery capability, with the workspace mistake already charged.** Full state
recovered from ledger + archive; 42-line recreation doc; wave resumed to a
certified close. Fallout was bounded and visible: E7 (carried task records
naming the dead `~/projects/taurjobs` path; idle monitor escalated; paths
corrected by ruling). Residue in the archived config: design seat cwd still
`/home/mstie/projects/taurjobs` and `model: "external"` — recreation did not
validate member cwd or model fields (taurhaus fix, A7b).

## Adjustments (Q6), ranked

**Cheap — do before wave 2:**

- **A1 (from F2)**: Replace the open-slot hack with a first-class role. Delete
  `v3-architect-codex.yaml:18` "Candidates: Fable 5.1 (preferred) or GPT-5.6
  Sol (fallback). The preset-pinned role_id stays v3-architect-codex for
  compatibility; switch the open slot by editing defaults.cli_tool,
  defaults.model, and defaults.reasoning_effort." Replace with a
  `fable-altitude-reviewer` role: "This is the Claude-family altitude seat:
  Fable 5.1, high effort." Presets must not ship a role whose id, cli_tool and
  model can disagree; the builder should refuse a member with a role/tool
  mismatch or a missing model.
- **A2 (from F10)**: In `judge-astra.yaml`, scope isolation: replace "Never see
  the other judge's verdict before recording your own." with "When the
  assignment names a paired judge and cell, never see the other judge's verdict
  before recording your own; on solo assignments you are an independent
  evidence auditor — apply the rubric the assignment names." Better: ship the
  evidence-auditor role this seat actually played and keep the pair for real
  dual-judge cells.
- **A3 (from F5)**: Make the ratchet measurable. In
  `astra-heavy-implementer.yaml`, after "Exceeding the assignment's diff budget
  without prior lead approval is a review FAILURE, not a style note." add: "A
  budget raise is telemetry: the lead records it as a `budget_raised` ruling
  with old ceiling, new ceiling and reason; a task's raises appear beside
  oversize_diffs in the routing report." Zero code oversights + fifteen silent
  raises is currently indistinguishable from leash success.
- **A4 (from F8)**: Pre-seed the checkout rules instead of rediscovering them:
  the kickoff CLAUDE.md template (or the lead role text) carries from day one —
  code lanes work only in per-seat worktrees; docs seats commit by pathspec
  after a separately-run `git status --porcelain`; per-seat CARGO_TARGET_DIR;
  capture only from your own display with geometry asserted; stagger gate runs
  at ≥3 concurrent builds.
- **A5 (from F12/F9)**: Batch/review lanes get a first-class "blocked awaiting
  routed input" state the idle monitor respects, and monitor nudges land in the
  telemetry nudge column. A nudge that fails twice on a seat with no task
  uptake should escalate as a launch-health incident, not repeat.
- **A6 (from F11)**: Give the architect standing load after the freeze
  (structural review rotation or explicit on-call), or declare it a part-wave
  seat in the blueprint. Keep both heavy seats and the judge; keep design.
- **A7 (taurhaus product, all cheap)**: (a) fix mesh `accept`/`start` ownership
  check (E10); (b) validate member cwd exists and model is set at
  create/recreate (F13); (c) write per-task telemetry sidecars for every
  completed task (tasks 3/4 missing); (d) `runtimeCompactSummary` +
  MODEL SLOT template-management prose reaches runtime members' instructions —
  strip catalog-management text from what a member is actually launched with.

**Structural — needs a lane:**

- **A8 (from F4)**: A machinery-side depth gate: when an instrument or gate
  artifact (not the product) accumulates N review rounds or M amendment
  commits, the lead must obtain an operator go/no-go before continuing.
  Wave 1's stop (R41) was operator-imposed; the loop had no internal brake.
  This is the "efficiency before rigor" doctrine as a mechanism.
- **A9 (from F3)**: Rebuild the state-of-record: mesh as the single canonical
  store with the ledger generated from it (or per-task narrative files with a
  thin index). Targets: 257 ledger commits, 21k-char cells, board/ledger drift.
  The recovery property (F13) must be preserved — that is the requirement, not
  the current format.
- **A10 (from F6/F9)**: Token accounting per (role, model, task) — Decision 2
  (Astra-as-lead) and the Sol-vs-heavy inversion are both blocked on cost data
  the wall-time proxy cannot supply. Attribute review rulings as work units
  with wall time to the reviewer's row while at it.

## Executive summary

1. The review machinery is real: two-family routing caught ≥12 named shipped-or-would-have-shipped defects, including a Results surface that never showed true failure reasons (R30) and a crash-orphan process leak (R34).
2. The Opus-single-seat bet (Decision 4) held — the three lenses' findings don't overlap; keep Opus product + Fable altitude + the Astra judge, all three earned their seats.
3. The altitude seat's *configuration* failed twice (claude/no-model against a codex role; the "codex this time" recreation didn't materialize) — the open-slot role hack in v3-architect-codex is the root cause; fix before wave 2.
4. The diff-budget leash produced the intended accounting and decomposition but never fired: 0 oversize rulings against ~15 budget raises, 2 post hoc — add `budget_raised` telemetry or the leash is unfalsifiable.
5. Ceremony ran ~81% of commits and ~43% of authored lines; the ledger alone took 257 commits and drifted from the mesh board — but the same ledger made the mid-wave kill fully recoverable, so reform the format, keep the function.
6. The scanner/certification recursion (8 instrument versions, ~10 reviews of the instrument) found one real privacy defect and much durable method — at a depth no wave-1 slice justifies; the machinery needs a self-triggered depth gate.
7. The blueprint's implementation economics were silently inverted: Astra heavy seats wrote most of the product at the priciest tier; tokens aren't collected, so the wave cannot grade its own central cost question (Decision 2 still blocked).
8. The worst coordination class was the shared checkout — five escalations rediscovering worktree rules taurhaus already knew; pre-seed them at kickoff.
9. Telemetry attribution is directionally usable but blind to review work, monitor nudges and budget raises; reviewer wall-time rows are currently noise.
10. Nine seats was roughly right for this wave; the architect is a part-time seat after the freeze — give it standing load or say so in the blueprint.
