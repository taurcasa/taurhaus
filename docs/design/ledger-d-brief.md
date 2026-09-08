# Lane brief: ledger increment D — boundary exports, canonical rendering and role conformance

Commissioned by the orchestrator (mesh-next program, phase 0),
2026-09-08. Implementation lane on Astra (`gpt-6-astra`, high) through
`feature-pr`; Opus lenses; orchestrator acceptance. Repository: **mesh**,
branch `feat/ledger-boundary` stacked on `feat/ledger-submission` (diff
base = that branch's closing tip), worktree `~/projects/mesh-ledger`.
Not merged, not released; role YAML is NOT edited by this lane (the
release route owns that). Runs after C2's procedure returns `complete`.

## Assignment

1. **Objective:** a wave boundary produces a reproducible, offline-
   replayable snapshot (the generated `ledger.md`, a cut manifest, and a
   compact input bundle) whose canonical compact Markdown satisfies the
   study's Markdown contract and the delivery standard's obligations,
   and every published ledger command example executes against the
   candidate binary with fixture identities.
2. **Deliverable:** commits on `feat/ledger-boundary` adding `mesh ledger
   snapshot`, offline replay of a bundle that includes the ledger segment
   and manifest (`render --input-bundle` over bundle layout v1), the
   canonical compact Markdown for `--view current` and `--view
   narrative`, projection-side audience enforcement (SD4), and the
   role-command execution tests; RESULT cites tip, tests, diff budget.
3. **First action:** read the study §7 ("Markdown contract", "Snapshots
   and wave closure"), §5 ("Time and source cuts" — the vector cut), §6
   (boundary export durability barrier), §10 rows "Boundary
   reproducibility", "Delivery obligations", "Role command validity",
   "Scope read discipline"; the intake design §7 (SD4 enforcement in D),
   §8 (narrative), and the gap matrix's renderer fixtures (seven
   malformed-width rows, the non-table paragraph, stale current-view
   cells, the negative retained joins, qualification preservation,
   amendment vs observation, authored vs delivered); then write the red
   tests: snapshot → copy the bundle to a fresh root with the original
   root removed → render byte-identical; a required artifact missing
   from the bundle → explicit failure/gap, never silence.
4. **Completion signal:** feature-pr `complete`; RESULT.
5. **Review route:** implement; Opus conformance + operational; up to
   three fix rounds; orchestrator accepts.

## Minimum deliverable

- `mesh ledger snapshot --wave <wave> --boundary <boundary-id>
  --output-dir <dir> --team --name --claude-dir`: the certifying export
  (briefly takes the ledger lock, validates and syncs the captured
  manifest/segments, releases, then reads immutable prefixes); writes
  `ledger.md` (current view), `narrative.md`, `cut.json` (the vector
  cut: ledger id/epoch/sequence/committed boundary, each source's
  identity, prefix boundary and digest, task snapshot digests and
  capture interval, referenced revisions, adapter/renderer versions,
  availability and gaps), and bundle layout v1 = layout v0 plus
  `state/ledger/v1/{manifest.json,segments/*.jsonl}` and a
  `source_facts` export that carries only permitted fields (never inbox
  bodies or restricted review content); atomic output replacement; runs
  no Git.
- Offline replay: `render --input-bundle <v1 bundle> --view current|
  narrative --format markdown|json` reproduces the snapshot byte-for-
  byte from the bundle alone (no live root, no cwd, no env); a missing
  required source or artifact is an explicit gap in the manifest and the
  banner.
- Canonical compact Markdown per the study's contract: generated banner
  (wave/incarnation, boundary or cut id, sequence, versions, coverage,
  consistency; no `now`); stable task order (explicit scope order then
  natural task id); one short row per task (task, scope, owner,
  lifecycle, acceptance evidence, operative result reference, remaining
  summary) linking to anchored blocks; one anchored block per active
  outcome/decision/remaining item with author and evidence links; a
  history count/link for superseded/withdrawn entries; unresolved
  rejections visible; the referenced completion statement beside a
  `none` declaration (T8); fixed wrapping, deterministic escaping, no
  auto-width tables, no repeated gate transcripts, bounded excerpts with
  canonical links instead of truncation; the delivery standard's fields
  reachable (commit-or-none, method/limits, red-first evidence or
  justified skip, candidate/rubric and source/landing distinction,
  numbered findings, questions apart from defects, unaltered scores).
  Replaces increment A's `<pre>` JSON blocks and increment C1's field
  dumps.
- Projection-side SD4 enforcement: every projection, history, search,
  export, snapshot and receipt detail applies the audience predicate
  before joins and rendering; an unlocked peer reviewer's view excludes
  a verdict, its summary, score, title and derived conclusions; a
  generic "restricted evidence withheld" marker without count/id/title;
  derived facts inherit the intersection of their sources' audiences.
- Role-command validity: every ledger command example that will enter
  role text (from USAGE.md) is extracted by a test, fixtures are
  substituted, and the parser AND handler paths are executed against the
  candidate binary (success and failure), so the release route can pin
  role text without guessing.
- Tests per the study §10 rows "Boundary reproducibility", "Delivery
  obligations" (golden cards for measure/no commit, diagnose/limits,
  implement/source+landing/red evidence, hero two-lens review, numbered
  findings/questions/scores, per-root zeros/unmeasured dashes),
  "Historical evidence preserved" (R38 equal-number/different-
  instrument; transcription correction supersedes only the incorrect
  claim; a later clean run does not erase an earlier FAIL; R41
  exception narrows only its item), "Scope read discipline" (late old
  event after a mark, multi-page walk, hidden history, unresolved
  ruling), and the gap matrix's renderer fixtures (malformed-width rows
  and the non-table paragraph as IMPORT cases that must render as
  explicit unavailable/unmapped input, never shifted columns). Fixtures
  binary-produced in tempdir roots; the wave-1 archive bundle as the
  historical replay case via `just ledger-archive-check`.

## Not building

- Wave-3 adoption, role YAML edits, release/pin/lock changes, the
  historical Markdown importer, segment rotation, checkpoints/caches, a
  Scope UI, any watcher or service.

## Constraints

- Diff budget ≤ 3,000 inserted lines (gross, Cargo.lock excluded); no
  new dependencies. Clippy pedantic `-D warnings`; all four gates green.
- One cargo job machine-wide; own `target/`; never `~/projects/mesh`
  master, the other mesh worktrees, the live daemon, tmux, `~/.claude*`,
  `~/.codex`, `~/.gemini`, `~/.grok`.
- Commit after every green step; never `git add -A`.
