# Research brief: the artifact IS the ledger event — authoring intake design

Commissioned by the orchestrator (mesh-next program, phase 0), 2026-09-08.
One Astra researcher/architect (`gpt-6-astra`, reasoning effort high),
read-only, through the research-sweep procedure. Deliverable: a design
document, `ledger-artifact-as-event.md`, ready for increment C to implement
without redesign. Research and design only; no implementation, no edits
to the corpus, no mesh change.

## Assignment

1. **Objective:** specify how mesh ingests a ledger event FROM an artifact a
   seat already commits — front-matter on a review file, NOTES, or RESULT
   artifact, or a RESULT message as submission body — so that authoring a
   ledger fact costs no second delivery step, no escaping layer, and no
   new ritual on ordinary RESULTs.
2. **Deliverable:** the report at the path the procedure names, sections
   Result / Evidence / Recommendation, every claim labeled S/D/P/I/U with
   file:line or command output; a parser-precise front-matter contract; a
   worked example per qualifying artifact class drawn from REAL wave-1 or
   wave-2 artifacts (quoted sparingly, hash-identified); open questions as
   first-class output.
3. **First action:** read `docs/design/mesh-next-adjudication.md`
   ("Accepted with redesign") and `docs/design/ledger-append-log-research.md`
   §4, §5, §7 with its review-pass amendment, and the operator constraint
   at the end of `docs/design/ledger-append-log-brief.md`; then inventory
   the artifact classes in `~/projects/taurjob/docs/wave-1/reviews/` and
   `~/projects/taurjob/docs/wave-2/reviews/` (counts, sizes, whether each
   already carries front-matter or a RESULT-first line).
4. **Completion signal:** the structured summary the procedure requires,
   `status: ok`, with the report path.
5. **Review route:** design; acceptance owner = orchestrator (Fable
   altitude review), decorrelated Opus defect lens; one round.

## Why (measured)

Eight of nine wave-2 seats called a per-fact entry file a regression — a
second delivery obligation duplicating the RESULT, review file, or NOTES
they already produce (`~/projects/taurjob/docs/wave-2/concept-review/summary.md`,
items 4 and "would hurt" 1). The adjudication adopted the fix: ledger
ingestion derives events from committed artifacts; a dedicated entry file
remains ONLY for facts that exist in no artifact. The lead's condition: the
generated rendering keeps a narrative view. The operator's constraint: no
escaping layer for authored prose — files or heredoc stdin, never long
argv; JSON intake only for programmatic emitters that write no prose.

## Ground truth to mine (all read-only)

- The seats' own words: the nine files under
  `~/projects/taurjob/docs/wave-2/concept-review/` and `summary.md`.
- Real artifacts: `~/projects/taurjob/docs/wave-1/reviews/*.md` (review,
  acceptance, certification, evidence files), `docs/wave-2/reviews/*`, any
  NOTES the asset seat produced (find it), the RESULT bodies retained in
  the wave-1 archive inboxes (`docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz`,
  extract into a tempdir only) and the retained `task_completed.summary`
  fields in the archived workflow journal — measure how much of an outcome
  already lands in the completion summary today (S), because the study's
  default is "if the completion summary suffices, the ledger author does
  nothing".
- The delivery standard `docs/team-delivery-standard.md` (results and
  reviewer artifacts, budget counting, review manifests) — obligations the
  intake must preserve, not restate.
- Mesh 0.2.29 (`~/projects/mesh`, read-only): `task complete --summary`,
  `task ruling --ref/--note`, `task progress`, and how `metadata` is
  round-tripped (`src/types.rs:181`, `src/task_lifecycle.rs`), so the
  design names which existing command already carries a fact.
- Precedents for markdown+front-matter intake in this house: session
  handoffs (`ARCHITECTURE.md`, "Session handoffs"), workflow prompt files.

## Design dimensions (decide each with reasons; label I)

1. **Qualifying artifacts and how mesh finds them.** Explicit path
   argument only (`mesh ledger entry --file <artifact.md>`); no directory
   scanning, no watcher, no git hooks. Which classes qualify (review file,
   NOTES, RESULT artifact, dedicated entry file, JSON emitter) and what
   distinguishes them at the parser: one front-matter contract, or a
   `ledger:` block inside an artifact's front-matter?
2. **The front-matter contract.** Minimal required keys per payload kind
   (`outcome`, `remaining`, `decision`, `note` — no new kinds), how they
   map onto the study's envelope (`entry_key`, `references`, `payload`),
   what the writer assigns (sequence, `committed_at`, author from the
   authenticated actor — never a front-matter `author:`), and how the
   bounded `body` (4 KiB) relates to a 20 KB review file: which part of
   the artifact becomes the event body (a designated summary field, the
   first section, nothing) and how the rest is referenced.
3. **Artifact identity and the commit chicken-and-egg.** References need
   an immutable identity (repo, revision, path, blob/content digest). An
   artifact is edited, committed, then ingested — or ingested from the
   working tree by content digest and bound to a revision later? Decide
   the sequence a seat actually follows, with the no-ceremony constraint,
   and what mesh verifies at ingest (digest of the bytes it read; git
   revision optional, never fabricated).
4. **Idempotency and amendment through the artifact.** Where the client
   event id lives (front-matter key; who generates it; what a retry of the
   same bytes returns), and whether re-ingesting a changed artifact is an
   `amend` (previous head resolved from the `entry_key`, reason required)
   or a rejected conflict. Keep the study's CAS semantics intact.
5. **RESULT message as submission body.** When the RESULT already lands
   as `task_completed.summary`, what — if anything — is left to ingest;
   specify the "do nothing" default and the exact condition that requires
   a ledger event (outcome qualification, remaining items, decisions,
   evidence assessments the completion cannot carry).
6. **Validation and errors** the author sees (missing key, oversize body,
   unresolvable reference, unauthorized scope declaration), mapped to the
   study's error vocabulary; no silent truncation.
7. **Independent-review visibility.** A review artifact ingested as an
   event must not expose a peer verdict to an unlocked reviewer through
   the projection (adjudication gap 1); state the audience rule the
   intake records.
8. **The narrative view** (lead's condition) as a renderer requirement:
   what the generated rendering must offer so the lead does not write the
   narrative elsewhere — without a summarizer seat or per-event snapshots.
9. **Role-text impact under the attention-budget rule:** at most a
   handful of new rules for a seat; point at the verb's own `--help` and a
   worked example instead of duplicating doctrine. Every example is marked
   future until increment D execution-validates it — there is no `ledger`
   verb in 0.2.29 (S: `mesh ledger --help` exits 2).
10. **Measurement plan:** authored bytes per fact today (RESULT message +
    ledger row edit) versus under this design (front-matter lines added to
    an artifact that exists anyway), on the wave-2 population; disclose
    the estimator; no savings claim without it.

## Not building

- No implementation, no test, no mesh or taurhaus change, no corpus edit
  (the study's next revision folds this document in).
- No new payload kinds, no fields beyond the study's §4 unless a gap is
  evidenced — and then as a named spec-delta.
- No per-fact entry-file flow, no summarizer, no watcher, no git hook, no
  scanning.

## Constraints

- Read-only everywhere: `~/projects/taurjob`, `~/projects/mesh`,
  `~/projects/taurhaus`; never `~/.claude`, `~/.claude-account2`,
  `~/.codex`, `~/.gemini`, `~/.grok`, the live daemon (17233) or the
  operator's tmux server. Scratch probing only via the locked binary
  `~/.local/bin/mesh` in tempdir roots (`--claude-dir <tmp>`, empty
  environment); execute any mesh command you cite before citing it.
- Write only under the procedure's scratch/output directory.
- Labels S/D/P/I/U on every claim; measurement separate from judgment;
  confidence stated honestly; the wave-2 team is still standing — quote
  its live documents sparingly and by hash.
