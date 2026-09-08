# Mesh-next session handover

Written 2026-09-08 by the outgoing orchestrator session (default
account) for the incoming session (second account). You are taking over
the mesh-next program at the start of phase 0. Your account has no
memory of the prior session — this file and the repo are the bridge.

## Read in this order

1. `docs/design/mesh-core-team.md` — THE program document: phase-0
   overhead items (ledger + onboarding are co-first), the
   contract-repair gate, the shelf team roster + architect charter, the
   launch checklist.
2. `docs/design/mesh-next-adjudication.md` — the wave-2 team's user
   review, adjudicated. **BINDS over the studies where it modifies
   them.** The big one: the artifact a seat already commits IS the
   ledger event (front-matter ingestion) — do not build a
   per-fact-entry-file flow.
3. The studies, as reference depth (each carries a dated wave-2
   addendum): `mesh-task-threads-research.md`,
   `mesh-messaging-overhaul-research.md`, `ledger-append-log-research.md`,
   `native-push-probe-report.md`, and
   `field-test-wave2/machinery-review-astra.md` (findings M1–M3).

## State at handover

- Corpus FROZEN. Phase 0 not yet started.
- **M1–M3 contract-fix lane**: branch `fix/telemetry-contracts`
  (worktree `~/projects/taurhaus-contractfix`, LANE-SPEC.md inside).
  The OUTGOING session owns landing and merging it — check
  `git log origin/main` / merged PRs before assuming anything; if its
  PR is merged, the measurement-gated items unblock.
- **Unblocked immediately** (no dependency on the fix): ledger offline
  reader + gap-coding (ledger study increments A/B), the
  artifact-as-event authoring design, the combined assignment
  rendering design. **Gated on the fix**: every telemetry-based
  measurement (onboarding replay census, notice dedup, info-only turn
  counts).
- The taurjobs wave-2 TEAM IS STILL STANDING (account2 root,
  `taurjob` tmux window) finishing a PDF-picker bug lane. That thread
  belongs to the operator and the outgoing session — hands off unless
  the operator redirects it to you. The wave-2 mesh state archive does
  not exist yet; the machinery review §8 lists what analysis waits for
  it.
- Version state: app+daemon 0.9.7 installed; mesh locked 0.2.29
  (protocol 1/schema 1); daemon protocol 24; catalog revision 8.
  0.9.8 is the next train (carries the M-fix and, later, the Scope
  view — see `docs/design/team-comprehension-view/`).

## Standing rules this session learned the hard way (not in CLAUDE.md)

- **Never touch**: the live daemon (port 17233) and its data dir; the
  operator's tmux server (read-only diagnosis only when the operator
  reports breakage); `~/.claude/teams` and `~/.claude-account2/teams`;
  `~/.codex`, `~/.gemini`, `~/.grok`, `~/.zshrc`; the operator's
  untracked drafts (`docs/pi-*.md`, `scripts/install-imagemagick7.sh`);
  `~/projects/job-hunt` (read-only); `~/projects/mesh` is read-only
  for lanes (orchestrator merges only).
- **Fixtures must be PRODUCED by the locked mesh binary**
  (`~/.local/bin/mesh`, tempdir `--claude-dir`), never hand-written —
  hand-written fixtures are how M1/M2 shipped. Same rule for any mesh
  command cited in role text: execute it against the real binary first.
- **Model split**: Fable orchestrates; implementation lanes run codex
  `gpt-6-astra` at high (`-c model_reasoning_effort=...`); Opus for
  cross-family review lenses; never Sonnet. Workflow runners invoke
  codex with `env -u TMUX` and `< /dev/null` (stdin trap).
- **Serialize cargo-heavy gates** — one at a time machine-wide (WSL
  vsock died under three concurrent). `just check` runs are
  lead-serialized; prefer the per-lane gates in CLAUDE.md. Check exit
  codes directly, never through a pipe. Poll that PR checks EXIST
  before `gh pr checks --watch` (it exits 0 on "no checks reported"),
  and verify MERGED before any cleanup.
- **Detached work pattern**: long codex research/fix runs go through
  `setsid nohup env -u TMUX zsh -c '... codex exec ... < /dev/null'`
  with a log file and a completion marker; they survive session death.
  /tmp is wiped on WSL restart — durable artifacts go in the repo.
- **No escaping layers for authored prose**: files + heredocs, never
  long argv strings.
- **Attention-budget rule** (operator): role/design text adds at most a
  handful of new rules per change; generalize instead of adding; point
  at a tool's own docs instead of duplicating them.
- Research/design lanes label claims S/D/P/I/U, separate measurement
  from judgment, and record honest confidence — the corpus sets the
  bar; hold new documents to it.

## Phase-0 execution shape

Run items as bounded ad-hoc lanes (small-change workflow or detached
Astra runs), NOT as a standing team — mesh-core launches only after
phase 0, the wave-3 retro input, and an explicit operator go. Mesh
0.2.29 stays locked until a deliberate lock-flow release
(`update-mesh-lock` → `bundle-mesh` → `mesh-verify-lock` →
`install-mesh`, lock files committed BEFORE builds). Measure every
claimed saving against the threads study's baselines with the
estimator disclosed.

## First actions for the incoming session

1. Read items 1–2 above in full.
2. Check `fix/telemetry-contracts` state (merged? still open?).
3. Start the ledger offline reader + gap-coding lane (increment A/B) —
   it needs only the wave-1 archive already in the taurjob repo and the
   frozen design.
4. Keep the operator posted the way this program always has: verdicts
   with evidence, honest remaining, no theater.
