# Wave-1 machinery review — blind, evidence-first

You are one of two frontier models independently auditing the TAURHAUS SIDE
of the first real field test: a 9-member managed team (product-build
roster) built the first slice of a greenfield app ("taurjob") over one
wave. You audit the MACHINERY — roles, review routing, coordination,
telemetry, overhead — not the product's quality. The team's own retro runs
separately; you must not contact any team member or write anywhere except
your single output document.

## Evidence (read-only; ground every claim in it)

- ~/projects/taurjob — the repo the team built: BRIEF.md, the product code,
  git history, docs/wave-1/ (ledger.md — the lead's running state of
  record; reviews/ incl. the altitude review; mesh-archive/ — the complete
  mesh state at mid-wave: team-config.json roster with roles/models/efforts,
  tasks/ (the full task ledger with rulings), telemetry/ (routing sidecars:
  launches, completions, staleness)).
- ~/projects/taurhaus/src-tauri/resources/templates/roles/*.yaml — the role
  texts the members ran under (the contract each seat was given).
- ~/projects/taurhaus/docs/design/team-blueprints-frontier-era.md — the
  binding Decisions this roster implements (review gates, the
  heavy-implementer diff-budget leash, Opus single seat, orientation).
- ~/projects/taurhaus/docs/team-delivery-standard.md — the assignment and
  message conventions members were expected to follow.
- Routing report (quantitative rollup):
  /tmp/claude-1000/-home-mstie-projects-taurhaus/dcb9f91d-188a-46eb-a9d2-3f77b554b6ec/scratchpad/routing-report-wave1.txt
  (being generated; if absent when you start, read it last).

Context you should know, stated neutrally: mid-wave the team was killed by
a workspace-directory move (an orchestration-side mistake, already charged
to the machinery column), then resumed from restored state; the roster
config also shows one seat (altitude-reviewer) initialized as claude/fable
with no model against a codex role definition — it never acknowledged a
task; the lead performed that altitude pass itself under an authorized
fallback. One added seat (judge-astra-1) had no counterpart or rubric and
was repurposed mid-wave as a design reviewer.

## Questions (rank findings by consequence; cite evidence for each)

1. ROLES: For each seat, did observed behavior match its role text? Where
   did role instructions get ignored, fought, or prove unnecessary? Which
   seats demonstrably earned their place (distinct value visible in
   artifacts/rulings) and which were redundant or idle? Was 9 seats the
   right size for this wave?
2. REVIEW ROUTING: Did the two-family review route catch real defects
   (name them) or stamp? Did the Opus-single-seat bet (Decision 4) hold?
   Did the altitude pass add value beyond the product review?
3. THE LEASH: Did the heavy-implementer diff-budget/oversize mechanism
   fire? Did it shape behavior (smaller diffs, decomposition) or get
   ignored? Is the telemetry attribution usable?
4. OVERHEAD: Estimate the ceremony share — ledger upkeep, rulings,
   contract messages, reviews-of-reviews, retro-of-everything — versus
   product output. Where was the machinery worth it, where was it theater?
   Be numeric where the ledger/telemetry allows (task counts, ruling
   counts, review rounds per merged artifact).
5. COORDINATION: Stalls, silent waits, dependency chains that idled seats,
   deadline/nudge behavior, inbox friction — what does the evidence show?
6. ADJUSTMENTS: Concrete, ranked — role-text edits (quote the line you'd
   change and its replacement), roster composition changes, model/tier
   placement changes, convention changes, and anything taurhaus-the-product
   should build or fix to make the next wave better. Distinguish "cheap,
   do before wave 2" from "structural, needs a lane".

## Output

One markdown document, max ~250 lines, findings ranked most-consequential
first, every finding carrying its evidence reference (file/task-id/ruling).
End with a ten-line executive summary the operator can read alone.
