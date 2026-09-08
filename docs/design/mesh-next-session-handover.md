# Mesh-next session handover

Rewritten 2026-09-08 ~05:45 UTC by the phase-0 orchestrator session
(second account) after taking the program over from the default-account
session. The incoming session has no memory of either; this file, the
execution ledger and the repo are the bridge.

## Read in this order

1. `docs/design/mesh-next-phase0-execution.md` — the running record:
   every lane, model, verification, adjudication, ruling, incident.
2. `docs/design/mesh-core-team.md` — the program document (phase-0
   status block at the top; charter; launch checklist).
3. `docs/design/mesh-next-adjudication.md` — binds over the studies.
4. The accepted phase-0 designs (each Astra-authored, one Opus fix round,
   orchestrator-verified): `ledger-artifact-as-event.md` (SD1–SD6 =
   increment C's intake contract), `onboarding-card-design.md`,
   `assignment-rendering-design.md`, `info-only-and-notice-dedup-design.md`,
   and `ledger-gap-matrix.md` (PARTIAL; increment B open).
5. The studies, as reference depth, each with dated phase-0 addenda.

## State at handover

- **Contract-repair gate CLOSED**: PR #148 (M1–M3, binary-produced
  fixtures) merged to main. Telemetry measurements wait ONLY on the
  wave-2 mesh state archive, which does not exist yet and belongs to the
  operator / the standing taurjob wave-2 team.
- **Ledger increment A complete** on mesh branch `feat/ledger-reader`
  (`ace395e`, `~/projects/mesh-ledger`), orchestrator-verified against the
  study's census; NOT merged (lock rule below).
- **Ledger increment C1 IN FLIGHT** (or landed — check): `feature-pr` on
  the stacked branch `feat/ledger-writer` in the same worktree, base
  `feat/ledger-reader`, brief `ledger-writer-brief.md` (payload v1
  provisionally frozen; T8 fixture). Check `git -C ~/projects/mesh-ledger
  log feat/ledger-reader..feat/ledger-writer` and the workflow result;
  verify directly (build, run the standalone verbs in a tempdir, check
  the study §10 tests) before accepting.
- **Queued lane briefs** (mesh, sibling worktrees, after C1):
  `assignment-card-lane-brief.md`, `info-only-stage1-lane-brief.md`;
  then ledger C2 (submission-route integration + `--summary-file`, per
  the intake design's C boundary) and D (boundary exports, role
  conformance). Onboarding-card implementation is two lanes (taurhaus +
  mesh) and touches the live compaction path — ordering is the
  operator's.
- **Increment B (gap matrix) OPEN**: 51/3,777 coded; continuation needs
  the closed wave-1/wave-2 exports; resume point under
  `docs/design/evidence/ledger-gap-matrix/`.
- **Item 2 reclassified**: a field-preserving combined assignment card is
  LARGER than the two bodies (−3%/−5%); correctness change only.
- Version state unchanged: app+daemon 0.9.7 (main now carries #148 for
  0.9.8); mesh locked 0.2.29 (`6789201c`); daemon protocol 24.

## Standing rules (all still in force; additions marked NEW)

- Never touch: the live daemon (17233) and data dir; the operator's tmux
  server; `~/.claude/teams`, `~/.claude-account2/teams`; `~/.codex`,
  `~/.gemini`, `~/.grok`, `~/.zshrc`; the operator's untracked drafts
  (`docs/pi-*.md`, `scripts/install-imagemagick7.sh`); `~/projects/job-hunt`;
  `~/projects/taurjob` (read-only); `~/projects/mesh` master checkout
  (orchestrator merges only, inside a lock-flow release).
- **NEW — mesh master stays at the lock commit.** `scripts/resolve-mesh-
  binary.sh` rebuilds from `~/projects/mesh` when its commit differs from
  `mesh.lock.json` and `mesh-verify-lock` then fails, so any commit on
  master past `6789201c` breaks taurhaus platform builds until a
  deliberate lock-flow release. Every mesh lane lives on a branch in a
  sibling worktree.
- Fixtures PRODUCED by the locked binary (or the crate's own binary in
  its tests), never hand-written; execute every mesh command cited.
- Model split: Fable orchestrates/adjudicates; `gpt-6-astra` at **high**
  implements and researches (workflow args `implementer: 'codex',
  codexModel: 'gpt-6-astra', effort: 'high'`); Opus is the review lens;
  xhigh only for an architect/auditor seat; never Sonnet; Sol overflow.
  Codex runs use `env -u TMUX` and stdin from a file.
- Serialize cargo machine-wide; check exit codes directly; verify MERGED
  before cleanup.
- **NEW — fix-round procedure.** A design/research lane's output gets an
  Opus defect lens (Agent, model opus, read-only, re-executes probes);
  the orchestrator writes `.check-logs/mesh-next-phase0/d<N>-opus-
  findings.md` with BINDING directions per finding, runs ONE fix round
  on the document in place, verifies the fix directly against the
  directions (no second lens), then promotes the document to
  `docs/design/` with its evidence sidecars. Use
  `.check-logs/mesh-next-phase0/research-sweep-fix.js` (no resume) for
  fix rounds. **A lane that returns `unavailable` is finished** — the
  long procedure's resume turns once re-ran an exhaustive attempt that
  overwrote a fix round's output; confirm the producing runner is dead
  (pidfile + `pgrep`) before editing its output.
- Attention budget: a handful of new rules per change; point at a tool's
  own help; label claims S/D/P/I/U; separate measurement from judgment.

## First actions for the incoming session

1. Read items 1–2 above.
2. Check C1 (workflow result / branch state); verify and adjudicate it
   the way the execution ledger shows for A.
3. Launch the queued mesh lanes in order (assignment card, info-only
   stage 1), then C2 and D from the intake design's boundaries.
4. Keep the operator posted: verdicts with evidence, honest remaining.
   Decisions that are the operator's: the lock-flow release, the
   mesh-core team go, the wave-2 archive export, the onboarding-card
   implementation ordering.
