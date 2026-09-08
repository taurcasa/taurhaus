# Lane brief: ledger increment A — offline existing-record reader

Commissioned by the orchestrator (mesh-next program, phase 0), 2026-09-08.
Implementation lane on Astra (`gpt-6-astra`, reasoning effort high) through
the `feature-pr` procedure; Opus cross-family review lenses; the
orchestrator (Fable) is the acceptance owner. Repository: **mesh**, worktree
`~/projects/mesh-ledger`, branch `feat/ledger-reader` off master
`6789201c5511b51be704fe30c6e4d025f3e64f8c` (the locked 0.2.29 commit).

**This lane does not merge to master and does not release.** Mesh stays
locked at 0.2.29: taurhaus's `scripts/resolve-mesh-binary.sh` rebuilds from
`~/projects/mesh` and `mesh-verify-lock` fails on any commit past the pin,
so master moves only through the deliberate lock-flow release.

## Assignment

1. **Objective:** a tested, read-only mesh reader that turns an exported
   team's existing structured records (workflow journal, protocol index,
   task-mutation journal, task snapshots, team config) into a versioned
   projection, a deterministic Markdown rendering, and a census plus
   source-gap report — reproducing the ledger study's §2.3–2.4 counts and
   negative cases on the wave-1 archive without ever reading the hand
   ledger.
2. **Deliverable:** commits on `feat/ledger-reader` adding a `ledger` module
   and the offline verbs `mesh ledger render --input-bundle <dir>
   --format markdown|json` and `mesh ledger census --input-bundle <dir>
   --format markdown|json`, their tests, and the `just ledger-archive-check
   ARCHIVE` recipe; the RESULT cites the tip hash and the census numbers.
3. **First action:** read `docs/design/ledger-append-log-research.md` §2.3,
   §2.4, §3, §5 ("Time and source cuts"), §7 (offline bundle reads and the
   Markdown contract), §9 row A, §10 and §11 in
   `~/projects/taurhaus`, then write the red archive test
   (`tests/ledger_wave1_archive.rs`, `#[ignore]`, env-gated) that pins the
   expected census below.
4. **Completion signal:** the feature-pr ledger returns `complete` with all
   four gates green; RESULT lists tip hash, census numbers, diff-budget
   count, and honest remaining.
5. **Review route:** implement; acceptance owner = orchestrator; reviewers =
   the procedure's Opus conformance and operational lenses; no hero
   surface; one review round by default, up to three fix rounds.

## Authority (precedence order)

1. `docs/design/ledger-append-log-research.md` — the design of record for
   this reader. Binding rules for this increment: reuse source IDs and
   summaries; deduplicate a ruling's task-array copy and its workflow echo
   by `(task_id, seq)` and report a byte disagreement as a source conflict,
   never as two rulings; the task-mutation journal carries changed-field
   names only (correlation, never values); protocol-index rows fold
   latest-by-`recordId` over the selected prefix; no last-verdict shortcut;
   never substitute current task JSON into a historical render; the same
   cut and renderer version produce byte-identical output; no wall-clock
   `now` in any output.
2. `docs/design/mesh-next-adjudication.md` — binds where it modifies the
   study. Nothing here changes A, except: the reader defines no ledger
   event kind and no per-fact entry.
3. `docs/design/field-test-wave2/machinery-review-astra.md` §4 and the
   ledger rows of §6 — the closed-wave evidence; its malformed-row and
   non-table cases are hand-ledger pathologies that belong to increment B's
   comparison, not to this reader.

## Minimum deliverable

- **Bundle layout v0** (documented in the module and in `USAGE.md`): one
  exported team as a directory — `config.json` (team config),
  `state/workflow_events.jsonl`, `state/protocol_index.jsonl`,
  `state/task_mutations.jsonl`, `tasks/<id>.json`, optional
  `telemetry/*.jsonl`. This is the seed of increment D's snapshot export;
  label it v0 and keep the layout version in the manifest. The wave-1
  archive supplies it after extracting `taurjob-team/{config.json,state}`
  from the tarball beside the `tasks/` snapshots (see Inputs).
- **Cut manifest**: every file present in the bundle with its SHA-256,
  byte length, and row count (for JSONL), marked `consumed: true|false`;
  the journal timestamp window; layout version; reader version = crate
  version plus git commit. Sources the reader does not interpret
  (`telemetry/`, and anything else present) are listed, digested, and
  marked not consumed. The manifest is embedded in the JSON projection and
  in the Markdown banner.
- **Adapters** for the four consumed sources into typed source facts with
  source identity (file, row position, event id / record id / ruling seq).
  Unparseable row inside a consumed file: fail closed with the file and
  line named — never warn-and-skip, never rewind (the existing offset
  readers' recovery semantics are explicitly not the contract here).
- **Projection JSON** (`projection_schema: "ledger-reader/0"`): per task —
  id, subject, owner at snapshot, snapshot status, last workflow status
  and whether the two agree, assignment history (assignment ids, owners,
  supersessions), the contract fields present in metadata (`first_step`,
  `deliverable`, `completion_signal`, `work_kind`, `lane_id`,
  `criticality`, effort), rulings (seq, kind, by, at, field, value, ref,
  note) with their echo-join status, completion events (event id, at,
  summary), review requests, progress reports, blocks; plus coverage: task
  numbers retained, sources present, known gaps.
- **Markdown rendering** per the study's Markdown contract: generated
  banner (cut identity, coverage, consistency), one compact row per task
  in natural task-id order, anchored per-task blocks with retained
  completion statements and rulings by source id, `unavailable` where the
  bundle carries nothing (never a guess), fixed wrapping, deterministic
  escaping. No history concatenation, no prose invented from timestamps.
- **Census and source-gap report** (`mesh ledger census`): the counts
  below, the accepted-core joins, the negative cases, and a gap list that
  names for each missing kind of fact whether the cause is the archive
  window (retention) or the absence of a typed field in mesh's records
  (schema) — in the study's vocabulary, and nothing about the hand ledger.
- **Tests** (red first): unit and CLI tests whose fixtures are produced by
  the crate's own binary in tempdir roots via `assert_cmd` (join a lead and
  a member, create a task with the contract flags, assign, rule, complete,
  then export the team directory and tasks into bundle layout v0 and
  render); the determinism test (two renders, identical bytes); the
  pull-purity test (render with `env_clear`, no `HOME`, no `--claude-dir`;
  the bundle's digests are unchanged afterwards and no file is created
  anywhere but the requested output); the fail-closed test (a corrupted
  row names its file and line); the echo-join tests (task-array ruling vs
  workflow echo: identical → one fact, different bytes → one conflict);
  and the archive test below.
- **`just ledger-archive-check ARCHIVE`** in the mesh `justfile`, running
  `MESH_LEDGER_WAVE1_ARCHIVE={{ARCHIVE}} cargo test --test
  ledger_wave1_archive -- --ignored --nocapture`. The archive test asserts
  the population digests first (tarball
  `60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1`,
  workflow JSONL
  `907075d8fbaa2d9cbaa5424ecbc17880bf7aff135821134585d8d6b56c3dbe38`,
  protocol-index JSONL
  `0f26b0e3fb87db07544bc274f2ab1d0fcd5af51a077147d9dd7ef8ced363849c`,
  task-mutation JSONL
  `c5f3ce38628b2cb1cad7a0e2fbc235adffa2777bfeb3c717cbedcfbc038be194`)
  and fails with "population changed" otherwise; when the env var is unset
  it fails with a message naming the recipe — it never silently passes.

## Expected census on the wave-1 archive (the advance condition)

All values are the study's S-measured counts on that hash-identified
population (§2.3, §2.4, §11). Pin them in the archive test; derive any
further expectation from the archive bytes by inspection, never from the
hand ledger.

- Journal rows: workflow 321, protocol index 53, task mutations 98.
- Task snapshots 11 (ids 1–11); task rulings 23 in total.
- Workflow event types: 11 `task_created`, 12 `task_assigned`,
  1 `assignment_superseded`, 72 `task_state_updated`, 7 `task_completed`,
  7 `review_requested`, 5 `progress_reported`, 23 `ruling_recorded`, plus
  messaging and administrative events (count them; do not pin a guess).
- Journal window 2026-09-06 19:34:32.013 to 20:11:26.180 UTC.
- Snapshot status agrees with the last workflow state for all 11 tasks.
- Exactly one budget-related ruling: task 11 seq 1, value `budget-430`,
  no typed `budget_raised` field; zero `budget_raised`-field rulings; zero
  `idle_monitor_records` in any snapshot (schema newer than the archive —
  report as a retention fact, not a defect).
- Accepted cores, each a completion event joined to a seq-2 architect
  ruling with verdict accepted and the candidate ref: T3
  `4f28477b-4cba-4c3b-a337-81c7bd886e25` / `8a7375b`; T4
  `521117ca-8671-4682-8a79-bf3f04c71cfa` / `c563ff3`; T5
  `77faa1e4-7a91-459a-8d2f-7fa35ff4595d` / `79d3657`; T8
  `3c5d02db-f5a4-4529-9c05-84778c5ccf20` / `bfc2cca`.
- Negative cases the projection must show as they are retained: T1 and T2
  still `in_progress` with correction/rejection rulings; T6's retained
  completion candidate (report its ref); T7 completed with no qualifying
  verdict on its own task; T9 completed while the verdict it produced sits
  on task 2 (no typed link — do not invent one); T10 carrying acceptance
  evidence (`b825b54`) while still `in_progress`. These are fixtures, not
  discrepancies to repair.

## Not building

- No writer, no `ledger init/entry/amend/tombstone`, no ledger log,
  manifest durability, CAS, or authority checks (increment C).
- No live-root reads: the offline verbs take `--input-bundle` only and
  refuse to resolve `--claude-dir`/`--team`/`--name`; no fallback to a
  live root, no daemon, watcher, or scheduled wake.
- No hand-ledger parsing, no Markdown import, no comparison against
  `ledger.md` (increment B does the comparison outside this code).
- No changes to the existing workflow/task/protocol enums, readers,
  writers, or their recovery semantics; no new event variants.
- No inbox bodies, control-auth, activity, operational, or projection
  files consumed; no `telemetry/` interpretation.
- No role-text or `USAGE.md` command examples beyond the bundle-layout
  note: the study's execution-validation gate says no verb enters role
  YAML before increment D.
- No new runtime dependency except `sha2` (digests). No `tar`, `flate2`,
  `serde_yaml`; the archive test extracts the tarball with the system
  `tar` into a tempdir, test-only.
- No release, install, lock change, or merge to master.

## Inputs (read-only)

- The wave-1 archive: `~/projects/taurjob/docs/wave-1/mesh-archive/` —
  `taurjob-team-archive.tar.gz` (contains `taurjob-team/config.json`,
  `taurjob-team/state/*.jsonl`, inboxes and other state), `tasks/*.json`
  (11 snapshots), `team-config.json`, `telemetry/*.jsonl`. Read only;
  extract only into a tempdir. `~/projects/taurjob` is another team's live
  repository: never write there, never run mesh against it.
- Mesh's own source for the record shapes: `src/workflow.rs` (event
  enum, line 38 onward), `src/task_lifecycle.rs` (rulings), `src/types.rs`
  (`Task`, line 181), `src/protocol_index.rs`, `src/task_journal.rs`.

## Constraints

- Diff budget: at most **2,200 inserted lines** across `src/`, `tests/`,
  `justfile`, `USAGE.md` (gross insertions from `git diff --stat
  master...HEAD`, `Cargo.lock` excluded), counted before the first commit
  and reported in the RESULT. Exceeding it without a recorded raise is a
  review failure. Generalize; do not scaffold for increments C–E.
- Clippy runs pedantic with `-D warnings` (`Cargo.toml` lints); `just
  check-quick` is `fmt-check` + `clippy`, `just lint` is `clippy`, `just
  test` is `cargo test`.
- Cargo serialization: one cargo job at a time machine-wide. Before every
  `cargo`/`just` invocation check `pgrep -af '(^|/)cargo( |$)'` for a
  process that is not yours; if one runs, wait (30 s polls, at most 30
  min), then proceed and note the wait. Never kill a process you did not
  start. Use this worktree's own `target/`.
- Never touch: `~/projects/mesh` (the master checkout), the live daemon
  (port 17233) and its data dir, the operator's tmux server,
  `~/.claude`, `~/.claude-account2`, `~/.codex`, `~/.gemini`, `~/.grok`.
  Every mesh invocation in tests passes an explicit tempdir `--claude-dir`
  and an empty child environment.
- Claims in the RESULT and in code comments carry the study's labels
  (S/D/P/I/U) where they are claims about the archive; measurement and
  judgment stay separate; unknown is not zero.
- Commit after every green step; never `git add -A`; conventional
  subject lines naming the mechanism, not the surface.
