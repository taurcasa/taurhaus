# Review brief: wave 2 through the machinery lens

Commissioned by the operator, 2026-09-08. One Astra reviewer at high
effort. Deliverable: `docs/design/field-test-wave2/machinery-review-astra.md`.
Research/review only; no implementation.

## The assignment

Review the COMPLETE wave 2 of the taurjobs field test from the
machinery and mesh-improvement perspective — deliberately complementary
to the team's own retro, which runs concurrently (the lead is
collecting per-member accounts; do not duplicate that work, and do not
message the team). Three deliverable parts:

1. **The good, the bad, and the ugly, machinery edition**: what the
   orchestration machinery (mesh 0.2.29, taurhaus 0.9.5→0.9.7,
   workflows, telemetry) did well, poorly, and badly this wave, with
   evidence. Include the launch itself: the pane-split failure, the
   account2 team-root failure, the mid-wave 0.9.7 install — the first
   wave launched on a non-default Claude root.
2. **First field test of the round-5 machinery — measure it.** Wave 2
   is the first wave running: awaiting-GO assignment markers with
   `--go` release, budget_raised rulings as telemetry, launch-health
   records after ignored nudges, source-tagged monitor nudges, the
   canonical lead inbox, the deadline pass respecting declared waits,
   worktree isolation as standing rule. For each mechanism: was it
   USED, did it WORK, what did it cost, what did the team work around?
   The provided `routing-report-wave2.txt` shows 9 budget_raises
   recorded and ZERO deadline/monitor nudges and zero staled tasks —
   establish WHY zero (waits respected? no idleness? mechanism unused?)
   rather than assuming success.
3. **Grade the evidence against the planned mesh overhaul.** The corpus
   is committed in docs/design/: mesh-task-threads-research.md,
   mesh-messaging-overhaul-research.md (+ operator decisions addendum),
   native-push-probe-report.md, ledger-append-log-research.md (+ review
   amendment), mesh-core-team.md (phase-0 overhead package). Does
   wave-2 evidence confirm, refute, or reprioritize each phase-0 item
   and each overhaul design decision? Recommend concrete changes to the
   corpus where wave 2 taught something new. Note specifically: the
   ledger question (what share of wave-2 commits are ledger
   maintenance; how do its cells compare to wave-1's pathology), the
   onboarding-duplication question, and the assignment double-render
   question.

## Evidence

- `~/projects/taurjob` READ-ONLY — docs/wave-2/ (ledger, closure
  report, reviews, rulings, the commission), git history and diffs for
  the whole wave. The team is writing its retro CONCURRENTLY: capture
  file bytes once, note the capture point, expect churn, and treat any
  retro content that appears as a separately labeled source.
- `docs/design/field-test-wave2/routing-report-wave2.txt` (provided,
  committed beside this brief) — telemetry for the wave window.
- The corpus documents named above, plus the wave-1 baseline record
  (docs/design/field-test-wave1/, and taurjob docs/wave-1/) for
  comparisons.
- Live team state (~/.claude-account2, the daemon, tmux) is OFF-LIMITS
  even read-only; the mesh state archive does not exist yet — name
  what analysis must wait for it rather than approximating.

## Constraints

- Read-only everywhere except the single deliverable file; no commits;
  never touch the live daemon (17233), the operator's tmux server, any
  team root, or ~/projects/mesh (read-only reference allowed).
- Evidence labels S/D/P/I/U per the corpus convention; measurements
  with method and population; separate measurement from judgment;
  confidence stated honestly; quote team prose sparingly.
