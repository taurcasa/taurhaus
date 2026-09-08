# Research brief: ledger increment B — the proposition-to-source gap matrix

Commissioned by the orchestrator (mesh-next program, phase 0), 2026-09-08.
One Astra researcher (`gpt-6-astra`, reasoning effort high), read-only,
through the research-sweep procedure, AFTER increment A's reader lands on
`feat/ledger-reader` in `~/projects/mesh-ledger`. Deliverable: a reviewed
matrix, `ledger-gap-matrix.md`, that decides how small the ledger's
authored event type can be. Payload v1 freezes only after this review.

## Assignment

1. **Objective:** for every proposition in the wave-1 hand ledger and
   rulings log (hash-pinned population), and for the decidable part of the
   wave-2 hand ledger, name its source class — retained-structured,
   retention gap, source-adapter/link gap, existing-authority schema gap,
   or new authored declaration (with the study's kind: outcome,
   remaining, decision, note) — with provenance on both sides.
2. **Deliverable:** the report at the path the procedure names, with the
   matrix (a table per stratum), three denominators reported separately
   (rows/fields, Unicode characters, propositions), the ambiguous codings
   listed for the acceptance owner, the malformed-row and non-table cases
   as fixtures, and a payload-v1 freeze recommendation: which §4 fields
   are confirmed necessary, which are unused on this evidence, which new
   ones are evidenced (each a named spec-delta).
3. **First action:** build the candidate reader (`cd ~/projects/mesh-ledger
   && cargo build`, waiting for any other cargo process first), assemble
   the wave-1 bundle in scratch (extract `taurjob-team/{config.json,state}`
   from the archive tarball beside a copy of `tasks/*.json`), run
   `target/debug/mesh ledger render --input-bundle <bundle> --format json`
   and `... ledger census ...`, and verify the census matches the study's
   §11 expectations before coding a single proposition.
4. **Completion signal:** the structured summary the procedure requires,
   `status: ok`, with the report path.
5. **Review route:** measure + design; acceptance owner = orchestrator
   (Fable), who checks the ambiguous codings; one round.

## Authority

`docs/design/ledger-append-log-research.md` §2 (taxonomy, denominators,
deep cases), §4 (the four payload kinds and reference roles), §9 rows A–B
("Every gap labeled retention, source-adapter/link, existing-authority
schema, or new authored declaration. Do not add ledger fields to
compensate for missing archived tasks."), §11 (the census recipe — reuse
it for the hand-ledger side); `docs/design/mesh-next-adjudication.md`
(the artifact IS the event; gaps 2, 4, 6); the machinery review §4
(closed-wave census, malformed rows, the 5,881-character paragraph).

## Populations (hash-pinned; refuse to proceed on a mismatch)

- Wave-1 ledger `d9272cf638e3071509f4b6767e57cd674257a43a768b5ce5df2a62726e7656bb`
  (29 task rows × 8 cells, 52,159 cell characters; §2.1 buckets), wave-1
  rulings `c08329fb30537d2e63b1af0bc212d830edd5a1d482ff5b3250053ebba07bc0ac`
  (32 second-level sections), archive tarball
  `60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1`.
- Wave-2 ledger: capture the current bytes ONCE, record size and SHA-256,
  and label it a live capture — the wave-2 team is still standing and no
  wave-2 mesh archive exists. Code only propositions decidable from
  committed artifacts; everything else is U pending the export.
- Late artifacts: `~/projects/taurjob/docs/wave-1/reviews/*.md` and the
  closure report — the source-adapter/link arm for T25–T28, never
  relabeled as structured mesh coverage.

## Method

- A proposition is one checkable claim (identity, owner, lifecycle,
  candidate hash, verdict, budget ceiling or raise, measurement, remaining
  item, decision, evidence citation, explanation). Enumerate them per cell
  or section with a line range; do not count Markdown edits or mirrors.
- Stratify as the study asks: short measure results (T3/T4/T5/T8), hero
  implementation/review, T25 rubric chronology, T26 integration (several
  meanings of PASS), T27/T28 closure (terminal state vs scope
  fulfillment), governance (R41/E5, freeze and correction).
- Code each proposition against the reader's projection JSON (cite event
  id / ruling seq / snapshot field) or the committed artifact (cite path,
  revision, line). Correctness first: zero invented candidate/GO/owner/
  acceptance claims; unknown is not zero.
- Report the three denominators separately; never publish a guessed
  "automatable" percentage; missing retention and missing typed fields
  are different causes.
- Fixtures: the seven malformed-width wave-2 rows and the non-table
  accumulation paragraph become renderer acceptance cases (what the
  generated projection must make impossible), stated as requirements.

## Not building

- No implementation, no change to the reader, no corpus edit, no ledger
  fields invented to cover missing archived tasks.
- No reading of live roots; no mesh command against `~/projects/taurjob`.

## Constraints

- Read-only everywhere except the procedure's scratch/output directory;
  `~/projects/mesh-ledger` may be built (its own `target/`) but not edited.
- One cargo job at a time machine-wide: check `pgrep -af '(^|/)cargo( |$)'`
  and wait before building.
- Never `~/.claude`, `~/.claude-account2`, `~/.codex`, `~/.gemini`,
  `~/.grok`, the live daemon (17233), the operator's tmux server.
- Labels S/D/P/I/U; measurement separate from judgment; ambiguous codings
  are output, not resolved by guessing.
