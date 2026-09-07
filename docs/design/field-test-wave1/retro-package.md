# Wave-1 field test — final retro package

Assembled by the orchestrator, 2026-09-07. Inputs: the team retro
(`taurjob/docs/wave-1/retro.md`, 8 attributed seat contributions), the
lead's member judgments (`member-judgments.md`), two blind machinery audits
(`machinery-audit-fable.md`, `machinery-audit-astra.md`), the routing
report (`routing-report-wave1.txt`), and the pre-registered box score
(`team-blueprints-frontier-era.md`, written before first launch).

## Box score — pre-registered predictions graded

| # | Prediction (pre-registered) | Outcome | Grade |
|---|---|---|---|
| 1 | Demoable slice without operator product code; >~3 machinery interventions/day is a red flag | Slice delivered and smoke-PASSed (115e1e6: configure → real run with live progress → results). Operator wrote zero product code; in-team interventions stayed at direction/approvals plus ONE substantive call (the R41 theater freeze). The machinery/orchestration column absorbed three incidents (workspace kill, tmux socket, altitude misdiagnosis) — charged to the orchestrator, not the team | **PASS**, with the incident cost on the machinery ledger |
| 2 | Every launch lands in sidecars; report shows accepted > 0 per row; all-unruled = review theater | Every row accepted > 0 (24/27 touches accepted with sequenced rulings) — the review contract was honored. But 9 launches unattributed, sidecars missing for 2 tasks, reviewer work invisible in rows | **PARTIAL** — wave-level claim holds, per-launch claim does not |
| 3 | 1–3 oversize incidents; zero beside sprawling diffs indicts reviewers | ZERO incidents — and diffs were genuinely bounded (forecast/actual per batch, decomposition, reviewers auditing budget bases). But ~15 budget raises went unrecorded, 2 post hoc; enforcement path never exercised | **MISS in the interesting direction** — the leash worked as a ratchet, not a trap; raises must become telemetry (`budget_raised`) or the leash is unfalsifiable |
| 4 | Two-family review holds; fully-overlapping lenses would trigger a rethink | Held everywhere; lens non-overlap proven repeatedly (altitude finding surviving a judge ACCEPT → T28-12; Opus retracting 3 own findings against the judge's 10 on T9). Opus single seat earned its place — including via a self-retracted grounding failure | **PASS** — Decision 4 confirmed; no rethink triggered |
| 5 | Lead delegates, never implements | Zero product code by the lead all wave; transcript is contracts and rulings. One authorized, recorded exception (the T7 altitude substitution) | **PASS** |
| 6 | Wall-time per accepted task recorded; first Astra-vs-Sol cost datum | Recorded per row — but BOTH audits independently rule the comparison unanswerable: task shapes incomparable, review latency inside wall time, tokens not collected | **RECORDED, COMPARISON BLOCKED** — Decision 2 stays gated on token accounting |

**Verdict: the field test achieved its purpose.** The machinery's genuine
wins (two-family review caught ≥12 named product defects; the ledger made a
mid-wave team kill fully recoverable; hash-first handoffs; the leash's
accounting discipline) and its genuine failures (shared-checkout
contamination ×5, ceremony at ~77–81% of commits, dependency-waits
misread as dead seats, instrument recursion without a brake) are now
legible, attributed, and mostly cheap to fix.

## Corrections to the record

- **The altitude seat was never dead.** Archived inboxes show it pre-read
  and waiting for "T7 go"; the standard prohibited the acknowledgment whose
  absence was read as death; the nudge preceded the go by 59 seconds. The
  orchestrator's "miswired seat" diagnosis was wrong; the real defect is
  that WAITING is not a visible state. (Astra audit §3; owned by the
  orchestrator.)
- **The operator's R41 freeze was load-bearing**: the lead's own
  self-assessment concedes the machinery had no internal brake ("the
  operator, not I, called the theater"). The depth gate below mechanizes it.

## Wave-2 changes — convergent cheap set (union of lead proposals + both audits)

1. **Isolation, tooled**: per-seat worktrees from first assignment;
   pathspec-only commits for docs seats; pre-commit guard against
   unqualified stages; lease primitive for the build slot and display;
   per-seat CARGO_TARGET_DIR. (Retro C1; both audits' #1.)
2. **Waiting is a state**: `awaiting-GO` / `blocked-on(artifact, owner)`
   first-class; idle monitor respects them and its nudges land in
   telemetry; two failed nudges escalate as launch-health, never repeat.
3. **Budget contract**: one counting semantics per assignment (ceiling,
   what counts, exemptions) stated at assignment; `budget_raised` rulings
   with old/new/reason, surfaced beside oversize_diffs in the report.
4. **Instrument discipline**: rubric+instrument frozen before any
   certification run; acceptance plans authored by a seat that will not
   judge against them; the depth gate — N review rounds or M amendment
   commits on a non-product artifact forces an operator go/no-go.
5. **Restart cursor**: every long wait persists cwd, code tip, rubric tip,
   tool hash, active root, pending rulings, next action (the retro's
   unanimous consensus proposal).
6. **Role-text fixes**: replace the v3-architect-codex open-slot hack with
   a first-class `fable-altitude-reviewer`; scope judge isolation to real
   dual cells (or ship the evidence-auditor role it actually played); apply
   Astra's seven tabled role-text replacements; fix the
   onboarding-vs-assignment acknowledgment contradiction.
7. **taurhaus product fixes**: mesh ownership on accept/start (E10);
   lifecycle-verb contradictions (start/progress/complete semantics);
   validate member cwd+model at create/recreate; sidecars for every task;
   label monitor-vs-deadline event classes; unify the lead inbox names.
8. **Kickoff pre-seeding**: the checkout/build/display rules ship in the
   team's CLAUDE.md on day one instead of being rediscovered by incident.

## Structural lanes (each needs its own lane, post-decision)

Transactional task lifecycle enforcement · safe workspace lifecycle (the
directory-move class) · telemetry v2 (tokens per role/model/task, review
work attributed, budget raises) · shared evidence runner · **generated
operating views** — which is precisely the team-comprehension view already
designed in `docs/design/team-comprehension-view/`; the field test
independently demanded it (257 ledger commits, 21k-char cells, board/ledger
drift are its exact target).

## The open roster decision (operator's call)

- Fable audit: nine seats roughly right; architect is a part-time seat —
  give it standing load or say so.
- Astra audit: **seven-seat trial** — lead, architect (transitioning into
  GPT reviewer after the freeze), Fable structural reviewer, Opus product
  reviewer, design lead, one Sol implementer, one Astra heavy; second
  implementation lane added only when a third independent slice is ready;
  standalone judge seat retired (its evidence-auditor function kept as a
  reviewer role).
- Lead's judgment ranks the architect last ("staff part-time or fold into
  a reviewer") and the retro names the seat-identity churn as an Ugly.

Orchestrator recommendation: **adopt the seven-seat trial** — it is the
lead's and Astra's position and answers the Fable audit's own finding; the
second heavy seat returns the moment parallel slices justify it.
