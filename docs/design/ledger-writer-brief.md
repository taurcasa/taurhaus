# Lane brief: ledger increment C1 — the isolated ledger writer, fold and standalone verbs

Commissioned by the orchestrator (mesh-next program, phase 0), 2026-09-08.
Implementation lane on Astra (`gpt-6-astra`, reasoning effort high)
through `feature-pr`; Opus cross-family lenses; the orchestrator is the
acceptance owner. Repository: **mesh**, worktree `~/projects/mesh-ledger`,
branch `feat/ledger-writer` stacked on `feat/ledger-reader` (base for the
diff is `feat/ledger-reader`). Not merged to master, not released.

## Assignment

1. **Objective:** the ledger authority the study specifies — one
   separately versioned append-only journal per team incarnation with a
   single validated writer, CAS/idempotent admission, a pure
   deterministic fold, durable manifest publication — reachable through
   the standalone verbs, with the Markdown-with-front-matter and JSON
   intake adapters exactly as the accepted intake design defines them.
2. **Deliverable:** commits on `feat/ledger-writer` adding the `ledger`
   writer/fold module, `mesh ledger init | entry | amend | tombstone |
   history | render --view current|narrative` in live-root mode
   (explicit `--team/--name/--claude-dir`; the offline `--input-bundle`
   mode from increment A stays), the adapters, and the tests below;
   RESULT cites the tip hash, the test inventory, and the diff-budget
   count.
3. **First action:** read `docs/design/ledger-artifact-as-event.md`
   (§1–§6, §9, the Recommendation's C boundary, the Round-1 fix record)
   and `docs/design/ledger-append-log-research.md` §4–§6 and §10 in
   `~/projects/taurhaus`; then write the first red tests: entry → amend →
   render current and cut-at-A; two racing creators of one key; retry
   with the same event id after a lost response.
4. **Completion signal:** feature-pr returns `complete`; RESULT with tip,
   tests, budget, honest remaining.
5. **Review route:** implement; acceptance owner orchestrator; Opus
   conformance + operational lenses; up to three fix rounds.

## Authority (precedence)

1. `docs/design/ledger-artifact-as-event.md` — the intake contract
   (SD1–SD6): namespace `ledger: {adapter_version: 1, events: [...]}`,
   forbidden authored envelope fields, `{kind, slot}` authored on the
   submission route (wave/scope derived) and full `entry_key` on the
   standalone route from the named packet/assignment record, the
   reference vocabulary with mesh-computed digests and the canonical
   normalization, SD5 body forms, SD6 expected-head transport, the error
   table and validation order, idempotency over the event's own
   normalized content, SD3 `retention` defaults.
2. `docs/design/ledger-append-log-research.md` §4 (envelope, four
   kinds, limits 4 KiB body / 32 KiB record / 64 refs), §5 (admission,
   fold, time and source cuts), §6 (storage layout
   `state/ledger/v1/{lock,manifest.json,segments/000001.jsonl}`,
   commit sequence, failure table, single segment for v1), §7 (structured
   errors and exit classes 0/2/3/4/5), §10 (the test table).
3. `docs/design/mesh-next-adjudication.md`; increment A's reader
   (`src/ledger.rs`) for the source adapters the fold joins.
4. **Payload v1 — provisional freeze (orchestrator, 2026-09-08, on the
   gap matrix's checked subset; the exhaustive proposition census stays
   U until the closed wave exports exist):** the study's §4 envelope
   and four kinds with the field spellings and conditions of the intake
   design's payload table (`outcome`: body, scope_disposition,
   limitations, remaining_status, remaining_entry_ids; `remaining`:
   item_id, description, consequence, revisit_condition, disposition,
   optional owner/target_task; `decision`: question, decision,
   consequence, authority_ref, applicability, optional prior_decision;
   `note`: body, references, optional qualifies), NO new kinds, NO
   budget/GO/verdict/monitor fields in the ledger. Reference vocabulary
   per the intake design plus two additions from the gap matrix:
   reference roles gain `baseline`, `gated`, `reviewed`, `landed`,
   `red_base`, `red_features` (SD-SOURCE-TUPLE); `retention` gains
   `retain_until: <boundary-id>` beside `state` (SD-RETENTION-REF).
   Evidence `assessment` is per scoped reference, never document-wide
   (SD-CLAIM-ASSESSMENT). Tombstone, the `withdrawn` disposition and
   the epoch-reset transition are implemented as specified even though
   the corpus did not exercise them. The gap matrix's budget and
   diff-confirm deltas belong to the ruling/task authority, not to C1.

## Minimum deliverable

- Writer library: stable `lock` file; manifest with schema/ledger/epoch,
  segment ids, committed byte/sequence boundary and prefix digest;
  append-then-sync, manifest to temp + atomic replace + directory sync;
  uncommitted-suffix quarantine on recovery; `committed_corruption`
  stop; `durability_unknown` on sync failure. No rotation in v1 (keep
  the manifest segment-aware).
- Admission: `entry` (unused event id and key), `amend`/`tombstone`
  (current-head precondition, reason), idempotent retry returning the
  original receipt before head checks, `idempotency_conflict` on changed
  normalized content; single-winner races under the lock; batch
  admission (SD2b) all-or-nothing under one manifest update.
- Authority (v1, minimal): the authenticated actor is an active member
  of the team (existing mesh actor path); own-entry amend/tombstone by
  the original author; lead override with reason and retained original
  attribution; `unauthorized` otherwise. The policy digest recorded in
  the manifest at `init` is the team config digest; no policy-change
  machinery beyond the study's reserved slot.
- Pure fold: consecutive-sequence verification, roots/heads/withdrawn,
  `--at-sequence`, source-fact join through increment A's adapters
  over the live root's own journals/snapshots (no live mutation; reads
  under the existing shared locks).
- Adapters: Markdown front matter per the design's grammar (literal
  block scalars only for prose; section selector; trailing body for
  dedicated files; 16 KiB front matter, 1 MiB artifact, 16 events);
  `--json <file>`; the standalone verbs only (the submission-route call
  and `--summary-file` are increment C2).
- Render: `--view current` (the compact projection + anchored blocks
  per the study's Markdown contract, byte-identical for the same cut)
  and `--view narrative` (per task and per committed day, superseded
  claims expandable); `--format markdown|json`; same cut identity as A.
- `history <entry-id>`; structured JSON errors; exit classes.
- Tests, red first, one per study §10 row that C1 owns: deterministic
  fold and past replay; no competing heads; retry correctness; authorship
  and authority; small event and honest limits (UTF-8, pipes, control
  chars, oversize rejected without truncation, `remaining: unknown` ≠
  none); durable committed prefix (crash before/during append, after
  line sync, around manifest replace — with an injectable fs fault
  seam); corruption and cursors; pull purity (render creates nothing,
  touches no task/inbox); role-command validity (every verb example in
  the design executed against the candidate binary in a tempdir).
  Fixtures come from the crate's own binary via `assert_cmd` in tempdir
  roots; never hand-written journal bytes except the deliberately
  corrupted-row cases, which must say so.

## Not building (C2 and D own these)

- The submission-route integration (`task complete/review/progress`
  attachment sniff, `--summary-file`), `audience_ref` capture from the
  submission context, and the source-commit-first split receipt — C2.
- Boundary snapshot/export bundles, offline replay of a bundle with the
  ledger segment, role-text examples, release/pin changes — D.
- Segment rotation, checkpoints/caches, historical Markdown import,
  any watcher/service/summarizer, any change to existing workflow/task
  enums or readers.

## Constraints

- Diff budget: at most **4,000 inserted lines** (gross, Cargo.lock
  excluded) across `src/`, `tests/`, `USAGE.md`; new dependencies:
  `serde_yaml` (or an equivalent YAML 1.2 core parser) and `unicode-
  normalization` only, each justified in the commit; no `tar`,
  `flate2`, database or async runtime.
- Clippy pedantic `-D warnings`; `just check-quick`, `just lint`, `just
  test`, `just ledger-archive-check <archive>` stay green.
- One cargo job at a time machine-wide (poll `pgrep -af '(^|/)cargo( |$)'`,
  30 s, up to 30 min); this worktree's own `target/`.
- Never touch `~/projects/mesh` (master), the live daemon, tmux,
  `~/.claude*`, `~/.codex`, `~/.gemini`, `~/.grok`; every test root is a
  tempdir with explicit `--claude-dir` and an empty environment.
- Commit after every green step; never `git add -A`.
