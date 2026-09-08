# Lane brief: ledger increment C2 — the submission-route intake

Commissioned by the orchestrator (mesh-next program, phase 0),
2026-09-08. Implementation lane on Astra (`gpt-6-astra`, high) through
`feature-pr`; Opus lenses; orchestrator acceptance. Repository: **mesh**,
branch `feat/ledger-submission` stacked on `feat/ledger-writer` (the
diff base is `feat/ledger-writer`, at whatever tip C1 closed on), in the
worktree `~/projects/mesh-ledger`. Not merged, not released. Runs after
C1's procedure returns `complete`.

## Assignment

1. **Objective:** a seat's ordinary completion, review or progress
   submission ingests the ledger events its artifact already carries —
   no second delivery step, no second command — under the accepted
   source-commit-first failure contract; a literal RESULT can be
   submitted from a file or stdin without any escaping layer.
2. **Deliverable:** commits on `feat/ledger-submission` implementing
   SD1 exactly as `docs/design/ledger-artifact-as-event.md` §1 fixes it
   (the adapter call from `task complete|review|progress`, and
   `--summary-file <path|->`), with tests; RESULT cites tip, tests, diff
   budget.
3. **First action:** read the intake design §1 ("Two explicit entry
   points", "SD1", "Submission failure contract — source-commit-first"),
   §7 (SD4 capture in C), the Recommendation's "Increment C boundary"
   and "SD1 implementation boundary", and C1's `ledger_writer` module;
   then write the red tests: a review submission with an attached `.md`
   carrying a valid `ledger` namespace commits the review AND admits the
   events; the same with an invalid namespace commits the review and
   returns the split receipt with a nonzero exit naming the standalone
   repair; `--summary-file -` with quotes, pipes, backslashes and
   newlines round-trips byte-exact into `completion_summary`.
4. **Completion signal:** feature-pr `complete`; RESULT.
5. **Review route:** implement; Opus conformance + operational; up to
   three fix rounds; orchestrator accepts.

## Minimum deliverable (the intake design's C boundary, nothing more)

- `--summary-file <path>` (`-` = stdin) as a mutually exclusive
  alternative to `--summary` on `task complete`, `task review`, `task
  progress`; the literal bytes become the summary (LF-normalized), the
  original bytes retained by digest; a leading front matter with a
  `ledger` namespace is stripped from the summary and handed to the
  adapter (SD5 trailing-body form for a literal RESULT).
- Attachment sniff: only `--artifact` paths ending in `.md`, verified
  in-root regular files without symlink traversal, are opened for a
  bounded front-matter sniff; every other attachment keeps today's
  string-only handling; a failed sniff warns and never fails delivery;
  only a PARSED but invalid `ledger` namespace produces a ledger error,
  after the source commit.
- Source-commit-first order: cheap syntactic pre-check → the existing
  lifecycle mutation and its delivery exactly as today → ledger
  validation and admission under the ledger lock. On ledger rejection
  or storage failure: the source receipt plus the ledger error, exit
  nonzero, saying `source committed; ledger rejected` or `source
  committed; ledger durability unknown`, naming `mesh ledger entry
  --file <the same artifact>` as the idempotent repair; never a repeat
  lifecycle mutation. No receipt store, no coordinator lock, no
  task-writer idempotency change.
- Submission-route derivation: the seat authors `{kind, slot}`; the
  adapter derives `wave`/`scope` and the primary scope/task/assignment
  references from the frozen assignment named by the init packet; an
  explicit wave/scope on that route is `invalid_input`.
- SD4 capture: `audience_ref` recorded from the authenticated submission
  context; restricted review intake without audience proof returns
  `source_incomplete / audience_proof_unavailable` for the ledger part
  while the review delivery commits. No projection enforcement (D).
- Tests (red first), binary-produced via `assert_cmd` in tempdir roots:
  the three first-action tests; no-namespace attachment → zero ledger
  writes and no complaint; non-`.md`, missing, out-of-root, symlink and
  directory attachments → string-only, never fatal; malformed front
  matter without a parsed namespace → warning only; retry of the
  standalone repair after a split receipt returns the original ledger
  receipt with zero lifecycle effects; the T8 visibility fixture at the
  submission route (an `outcome` declaring `none` beside a completion
  summary renders the statement beside the declaration).

## Not building

- Boundary snapshot/export, offline replay of a bundle with the ledger
  segment, canonical compact Markdown, role examples (D); projection/
  export audience enforcement (D); messaging-journal or DM submission;
  any store, watcher, hook or scan.

## Constraints

- Diff budget ≤ 1,600 inserted lines (gross, Cargo.lock excluded); no
  new dependencies. Clippy pedantic `-D warnings`; `just check-quick`,
  `just lint`, `just test`, `just ledger-archive-check <archive>` green.
- One cargo job machine-wide (poll, wait ≤30 min); own `target/`; never
  `~/projects/mesh` master, the other mesh worktrees, the live daemon,
  tmux, `~/.claude*`, `~/.codex`, `~/.gemini`, `~/.grok`.
- Commit after every green step; never `git add -A`.
