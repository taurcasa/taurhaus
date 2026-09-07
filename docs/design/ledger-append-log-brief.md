# Research brief: the ledger as an append-log with rendered projections

Commissioned by the operator, 2026-09-07. One Astra researcher/architect
at high effort. Deliverable: `docs/design/ledger-append-log-research.md`
— a design study ready for implementation when wave 2 closes. Research
only; no implementation.

## The problem, measured

283 of 694 commits on the taurjob repository (40.8% of its entire
history) touch a ledger file — the lead hand-rendering state into
markdown, with git as a poor man's event log. Wave-1 audits separately
flagged 257 ledger revisions and 21–24k-character cells. The operator
wants this solved elegantly: appendable events, a fold to current
state, `ledger.md` as a GENERATED artifact — and explicitly NO service,
no watcher (the ledger is pull-only; it needs none of the messaging
study's delivery machinery).

The seed design is phase-0 item 5 in `mesh-core-team.md`: `entry` /
`amend`-superseding-a-prior-id-with-reason / tombstone-with-reason,
never in-place edits; deterministic fold; snapshots committed only at
real boundaries. Improve on it freely.

## Ground truth to mine

- **The real ledgers**: `~/projects/taurjob/docs/wave-1/ledger.md` and
  `rulings.md` (closed wave — mine deeply), and the wave-2 ledger as it
  currently stands (READ-ONLY — a live wave is writing it; read the
  current bytes, never assume stability, quote sparingly). Build a
  content taxonomy: what share of ledger content merely re-renders
  facts that already exist as structured mesh records (tasks,
  assignments, rulings, completions, budget raises, monitor records)
  versus genuinely novel prose (honest outcomes, `remaining`, evidence
  citations, decisions). That coverage number decides how small the new
  event type can be.
- **Existing structured substrate**: `~/projects/mesh` (READ-ONLY) —
  workflow journal, protocol index, task mutation journal (byte-offset
  readers), rulings schema, task metadata. The overhaul study's warning
  binds: do not turn an existing journal into an omnibus store; a new
  authority gets its own versioned log. Decide explicitly: separate
  small ledger log per team, or rows in the future message journal, or
  something else — with reasons.
- **The corpus**: `mesh-task-threads-research.md` (projection/fold
  discipline, supersession vocabulary), `mesh-messaging-overhaul-
  research.md` (journal envelope discipline — sequence vs wall clock,
  segment/manifest/recovery patterns, evidence labels S/D/P/I/U),
  `docs/team-delivery-standard.md` (the honest-outcome and result-
  artifact obligations any rendering must preserve).

## Design dimensions

- **Event schema**: entry shape (id, sequence, author, task/scope
  references, kind, structured fields vs prose body, evidence refs);
  amend/supersede relations; tombstones; authorship and authority —
  who may amend whose entries, how ruling-like authority applies, and
  the rule that a ledger event NEVER mutates task lifecycle state.
- **Fold semantics**: deterministic current-state fold; point-in-time
  replay; ordering discipline; how conflicts become impossible by
  construction rather than resolved.
- **Render**: `mesh ledger render` (verb surface sketch — commands will
  be cited in role texts, so follow the house rule that mesh commands
  in role text must be execution-validatable); markdown output that
  preserves the delivery standard's obligations and stays human-
  diffable; snapshot policy at boundaries; what the wave-closure report
  consumes.
- **Consumers**: routing report/telemetry (accepted = completion WITH
  ruling — can ledger events sharpen that join?), the Scope view's
  walk/history surfaces, retro tooling.
- **Migration**: wave 2 finishes on the hand ledger untouched; wave 3
  adopts. Is importing historical md ledgers worth anything, or do old
  waves stay as documents? Smallest testable increment — strongly
  consider: an offline prototype that generates a wave-1-ledger
  equivalent purely from the EXISTING structured records, measuring
  what fraction of the real ledger it reproduces; the gap IS the
  event-type specification.
- **What NOT to build**: no service, no watcher, no summarizer seat, no
  second task database, no change to mesh 0.2.29 during the live wave.

## Constraints

- Read-only everywhere: never write to ~/projects/taurjob,
  ~/projects/mesh, ~/.claude/teams, ~/.claude-account2 (live wave
  root), or any live state; never touch the daemon (17233) or the
  operator's tmux server. Scratch probing only via the locked mesh
  binary in tempdirs (`--claude-dir <tmp>`).
- Label claims with the corpus vocabulary (S/D/P/I/U); separate
  measurement from judgment; state confidence honestly; open questions
  are first-class deliverables.
