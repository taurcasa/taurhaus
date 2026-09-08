# Lane brief: info-only stage 1 — expectation-aware suppression on the existing paths (mesh implementation)

Commissioned by the orchestrator (mesh-next program, phase 0, overhead
items 3 and 4, stage 1), 2026-09-08. Implementation lane on Astra
(`gpt-6-astra`, reasoning effort high) through `feature-pr`; Opus
lenses; orchestrator acceptance. Repository: **mesh**, branch
`feat/info-only-stage1` off master `6789201c` in a sibling worktree
(`~/projects/mesh-info`). Not merged to master, not released. Queued
after ledger increment C1 and the assignment-card lane.

## Assignment

1. **Objective:** an `INFO ONLY:` (or explicit-info) message never costs
   a mesh-driven wake on the tmux paths, a mislabeled correction cannot
   be sent as info, the lead can see undrained info per member, and the
   expectation decision is one committed vector table both binaries
   test against — with no new store and no change to Claude's native
   mailbox behavior (stage 2).
2. **Deliverable:** commits on `feat/info-only-stage1` implementing
   stage 1 of `docs/design/info-only-and-notice-dedup-design.md` (§1,
   §2 stage 1, §5 stage-1 fixtures, §6 receipts, §8 stage-1 row) with
   tests; RESULT cites tip, tests, diff budget.
3. **First action:** read the design's §1 (expectation table, lint,
   vector table), §2 "stage 1", §5 fixtures, §8 compatibility; then
   write the red tests: an INFO body containing "ignore the earlier" is
   rejected at send; an `info` message in a mixed batch is not injected
   while the action message is; `ACTION REQUIRED … --priority low` still
   succeeds and is labeled; the vector table drives both the daemon
   filter and the terminal action-key selection.
4. **Completion signal:** feature-pr `complete`; RESULT.
5. **Review route:** implement; Opus conformance + operational; up to
   three fix rounds; orchestrator accepts.

## Minimum deliverable

- Optional `responseExpectation: action|info` on the inbox message
  (typed field; prefix fallback at every reader seam; old-binary strip
  degrades to prefix provenance), the `--expectation` send/broadcast
  flag, and provenance in the workflow `message_sent` echo.
- The vector table `tests/fixtures/expectation-vectors.json` mapping
  (prefix, orchestration intent, explicit flag, priority, generated
  kind) → (expectation, wake eligibility, provenance), and one
  normalization function consumed by: the daemon batch filter
  (`daemon.rs` ~1523), terminal action-key selection (~999), the cron
  gate (~1894). Generated kinds (assignment, nudge, completion, cron)
  are `action` by class; cron's terminal injection unchanged.
- The send-time lint in `lint_and_prepare_send_message`: an
  INFO-prefixed or explicit-info body carrying the design's bounded
  supersession/withdrawal/release keyword list is rejected with the
  re-prefix guidance; explicit expectation/prefix conflicts rejected;
  no override flag. Info never supersedes; a generated-info declaration
  is rejected before append.
- Per-member pending-info count on `mesh who` and the explicit read
  surfaces (no wake, no read mark).
- Receipts: `wake_not_requested` with reason on the existing wake
  telemetry; `superseded`/`failed`/`outcome_unknown` retained.
- Tests: every stage-1 row of the design's §5 fixture table,
  binary-produced via `assert_cmd` in tempdir roots; the vector-table
  conformance test; a copy of the vector table plus its conformance
  test is the taurhaus mirror's contract (the taurhaus side is a
  separate lane).

## Not building

- Stage 2 (deferred array, merged reads, capability/version
  transition); any transport, scheduler, native adapter or journal; the
  taurhaus mirror lane; role-text edits; release/pin changes.

## Constraints

- Diff budget ≤ 1,500 inserted lines (gross, Cargo.lock excluded); no
  new dependencies. Clippy pedantic `-D warnings`; `just check-quick`,
  `just lint`, `just test` green.
- One cargo job machine-wide; own `target/`; never `~/projects/mesh`
  master, live daemon, tmux, `~/.claude*`, `~/.codex`, `~/.gemini`,
  `~/.grok`.
- Commit after every green step; never `git add -A`.
