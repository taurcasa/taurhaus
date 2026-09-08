# Lane brief: one record-generated assignment card (mesh implementation)

Commissioned by the orchestrator (mesh-next program, phase 0, overhead
item 2 — reclassified a correctness change), 2026-09-08. Implementation
lane on Astra (`gpt-6-astra`, reasoning effort high) through
`feature-pr`; Opus lenses; orchestrator acceptance. Repository: **mesh**,
branch `feat/assignment-card` off master `6789201c` in a sibling worktree
(`~/projects/mesh-card`). Not merged to master, not released. Runs AFTER
ledger increment C1 (cargo serialization and review bandwidth), unless
the operator reorders.

## Assignment

1. **Objective:** the assignment notice a seat receives is generated once
   from the task record with every contract field, the assignment token,
   effort and reason, references and the operative wait; a GO release is
   a separate compare-and-commit command with its own bounded body; no
   context leaks across assignment generations; the lead can read the
   assignment token from the assign output.
2. **Deliverable:** commits on `feat/assignment-card` implementing the
   accepted design `docs/design/assignment-rendering-design.md` (the
   matrix, the literal template, the lifetime table, the spec-delta
   list), with tests; RESULT cites tip, tests, diff budget.
3. **First action:** read the design's Result section, "Context writers
   and assignment lifetime", "Recorded execution and conflict routing",
   "GO surface decision", the spec-delta table and the acceptance
   fixtures; then write the red tests: generation-1 → generation-2 with
   an old wait/references/budget that must NOT survive; assign output
   carrying `assignment_id`; the one-template render of a held
   assignment; `task go` with a stale generation rejected.
4. **Completion signal:** feature-pr `complete`; RESULT.
5. **Review route:** implement; Opus conformance + operational; up to
   three fix rounds; orchestrator accepts.

## Minimum deliverable (the design's spec-deltas, nothing more)

- `metadata.review_route` (fifth line) accepted at create/assign; the
  combined-card validation requires the five nonempty lines for a NEW
  assignment while legacy records stay inspectable ("not recorded").
- Optional typed `metadata.assignment_context` with the design's subkeys
  and lifetimes: `assignment_context` added to `ASSIGNMENT_CLEAR_KEYS`;
  task-scoped subkeys (`checkout`, `constraints`, …) reconstructed
  from the whitelist on reassign; generation-scoped subkeys
  (`references[]`, `wait`, `budget`, `previous_assignment_id`,
  `assignment_change_reason`) never copied; snapshot with the
  `task_assigned` event.
- Assign output prints a labelled `assignment_id` beside the message id;
  `--json` returns both.
- One renderer, the literal template, used by the directed assignment
  notice, `task get` (lossless: no 160-char truncation of the contract)
  and the reassignment/resume variants; the `Execution:` line reads the
  recorded `awaiting_go` token and declared blocks only.
- `task go <id> --assignment <uuid>`: verifies lead authority, current
  owner, matching generation and frozen references under the mutation
  lock; releases only that generation's wait; records the release;
  sends the bounded release body through the existing directed delivery;
  `task update --go` untouched (warns that it is unguarded).
- The message lint exempts identified generated assignment/release
  messages from the `deliverable:` substring check.
- Tests per the design's acceptance fixtures (1)–(8), binary-produced
  via `assert_cmd` in tempdir roots.

## Not building

- Taurhaus consumers, Scope, role-text edits, transport/scheduler/native
  push, subscriptions, a summarizer, the onboarding card, any migration
  of live teams, release/pin changes.

## Constraints

- Diff budget ≤ 1,800 inserted lines (gross, Cargo.lock excluded); no new
  dependencies. Clippy pedantic `-D warnings`; `just check-quick`, `just
  lint`, `just test` green.
- One cargo job machine-wide; own `target/`; never `~/projects/mesh`
  master, live daemon, tmux, `~/.claude*`, `~/.codex`, `~/.gemini`,
  `~/.grok`.
- Commit after every green step; never `git add -A`.
