# Lane: mesh e2e flake-class audit (+ GPT-6 Astra calibration lane)

Two purposes, stated openly: (1) retire the residual tier-2 mesh e2e timing
flake class (~1-in-4 full tier-1 runs during the 0.9.1 cycle); (2) this is the
first **calibration lane for GPT-6 Astra as implementer** — the work is judged
on its own merits by the normal review loop, and the routing tier sign-off
later cites how this lane went.

## The documented flake patterns (from the 0.9.1 cycle's adjudications)

1. **Stale-element clicks under poll re-renders**: the runtime views re-render
   on the 2s live-team-status poll, so a single WebDriver click can land on a
   just-replaced element and vanish (app log shows zero IPC after the click).
   Proven instance + proven fix shape: template-crud-ui's add-agent open is
   now click-until-open (idempotent click inside the waitUntil, target-state
   check first). See that spec's comment block for the artifact-timing gotcha
   too (post-failure cleanup pollutes failure.png).
2. **Propagation waits vs scanner cadence**: waits on coordination state that
   propagates via daemon scanner cycles (e.g. mesh-recovery's
   `waitForOfflineMemberCount` after a full team stop + reload) with budgets
   that lose under 7-worker load.

## Deliverables

1. **Audit sweep**: every action-then-wait sequence in the mesh/tier-2 specs
   (`mesh-workflow.js`, `mesh-recovery.js`, `template-crud-ui.js`,
   `templates.js`, plus any spec driving mesh runtime UI) classified against
   the two patterns. Produce a table in the lane report: site → pattern →
   fixed/already-safe/not-applicable, with one line of reasoning each.
2. **Fixes**: pattern-1 sites move to the click-until-open shape via ONE
   shared helper in `e2e/helpers/` (e.g. `clickUntil(testIdToClick,
   targetTestId, wait)`) — retrofit the existing template-crud-ui instance to
   the helper too, don't leave two dialects. Pattern-2 sites get budgets
   derived from the cadence they wait on (state the arithmetic in a comment:
   N scanner cycles + margin), not bigger magic numbers.
3. **No assertion weakened; no silent skips added.** A wait that cannot be
   made honest gets reported, not papered.
4. **Proof**: full tier-1 (`E2E_INSTALL_DAEMON=0 just test-e2e`) run
   **three times consecutively, all green** — the flake class was ~1-in-4, so
   anything less proves nothing. Report each run's spec-file summary.

## Constraints

- e2e/helpers/specs only — no product code (a suspected product bug stops and
  reports with evidence, per house discipline).
- Never touch the live daemon (17233) or ~/.claude/teams; never `just check`.
- rc=$? discipline; never pipe gates.

## Gates

- `just lint` · `bunx vitest run` (root; e2e-only changes should not move it,
  run it anyway)
- The three consecutive tier-1 greens (deliverable 4) are the acceptance.
