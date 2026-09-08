# Mesh-next: adjudication of the wave-2 team's user review

Orchestrator, 2026-09-08, under the architect-charter rules. Input:
taurjob `docs/wave-2/concept-review/summary.md` (nine seat files, by
attribution, dissents preserved). This document BINDS the corpus as its
final pre-freeze revision; where it modifies a study, this document
governs until the study's next revision folds it in.

## Accepted as designed (survey confirms)

- **Task record as carrier, contract with creation** (9/9, each seat
  citing its own incident) — the top-ranked change stands as the
  program's first deliverable priority.
- **Directed delivery; evidence reads strictly optional** (9/9) — with
  the survey's sharpening adopted as a binding invariant: *the card is
  never a required preflight, and "on the card" never substitutes for
  "delivered."* The four moments where the card earns its read (cold
  resume, moved tip/crossed instruction, wait release, closure
  recovery, plus cross-owner handoff) become the card's design targets.
- **State-aware nudges** (confirmed by exactly the seats that were
  wrongly nudged).
- **Lead re-freeze of moved tips** — adopted WITH the judge's
  condition: typed deltas (product / test-only / captures-only) so a
  confirm is a read, not a ruling, and capture updates don't bottleneck
  on the lead.
- **Answered-step flag** — adopted with all six survey conditions as
  requirements: sender-visible before send; repeat ≠ revision
  (deliver marked, never drop); lead-authorized follow-ups exempt;
  stage- and assignment-generation-aware; covers completed/re-issued
  tasks; capture deltas don't trip it.

## Accepted with redesign (the survey's one structural correction)

**The ledger authoring model changes: the artifact IS the event.**
Eight of nine seats identified a per-fact entry file as a regression —
a second delivery obligation duplicating the RESULT, review file, or
NOTES they already produce. Adopted resolution, consistent with the
operator's no-escaping constraint: ledger ingestion derives events from
the artifacts seats already commit — front-matter ON the review file /
NOTES / result artifact is the entry; the RESULT message may be a
submission body. A dedicated entry file remains ONLY for facts that
exist in no artifact (a scope-fulfillment declaration, a standalone
decision). Mesh still validates, sequences, and canonicalizes at
ingest; the journal contract, CAS/amend semantics, four payload kinds,
and pull-only rendering are unchanged. The lead's condition is
honored: the generated rendering keeps a narrative view, or the lead
will write the narrative elsewhere — renderer requirement, recorded.

## Gaps the survey establishes — added to the design as requirements

1. **Independent-review visibility on the card** (5/9): the projection
   must enforce the restricted-review audience rule at the card level —
   an unlocked reviewer's card excludes peer verdicts and their
   derivatives. The user summary's "structurally impossible" claim was
   too broad and is retracted; the threads study's audience rules now
   explicitly cover every projection surface.
2. **Evidence retention**: a retain-until/archived-vs-referenced
   distinction on evidence references; closure cleanup gates on it
   (three seats deleted evidence roots ten minutes before the retro
   commission).
3. **Budget accounting as a tool**: the record carries the counting
   rule and script name (`tools/budget.py` class); phase-0 adjacent.
4. **Source identity tuple**: baseline / gated / reviewed / landed
   identities with red_base/red_features — folds into the ledger
   study's typed references.
5. **Assignment prose rendered from the record; ids at creation.**
6. **Diff-confirms as a record form** (attach confirmations to original
   scope; landing manifest separate from candidate).
7. **Decision latency with an owner and age**, distinct from delivery
   latency (the 2050 wait, the 97-minute pick).
8. **Build-host coordination is explicitly OUT of the messaging
   program** and is recorded as its own future program (5/9 named it
   their top pain; solving it via messaging would be scope theft).

## Documentation practice adopted

Two seats read "the studies govern" as a hidden reading burden. Fix:
any user-facing surface must be self-sufficient for the behavior it
describes; a governing-appendix pattern is not acceptable for review
surfaces. (This document exists partly because of that rule.)

## Declined

Nothing in the survey is declined outright. Native-channel conditions
(opt-in per seat; no queue reordering mid build-tail) and receipt
conditions (no acknowledgment duty) were already design intents and
are now explicit requirements.

## Status

With this adjudication, the mesh-next corpus is FROZEN for the
mesh-core charter, pending only the phase-0 contract-repair gate
outcomes (M1–M3 lane) and the operator's go. The charter's authority
list gains this document after the ledger study.
