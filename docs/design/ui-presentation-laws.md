# UI presentation laws

Distilled from the Scope-view design rounds (2026-09-07, operator-driven).
Binding for all taurhaus UI work; `claude-design-lead` and
`frontend-design-skill-developer` carry a contractual reference here. When
a law conflicts with a specific brief, the brief wins and says so.

## The five laws of what carries information

1. **People get pictures.** Members render as chips: role glyph as the
   face, shape encodes model family, ring encodes observed activity,
   badges encode declared waits. Faces are naturally visual; never a
   two-letter code where a glyph system exists. Same-role duplicates are
   not distinguished at a glance — identity lives on hover.
2. **States get plain words.** Short colored words — "signed off",
   "sent back", "waiting on you" — never abstract marks the eye must
   learn (no bare triangles, squares, dash patterns as state carriers).
3. **Structure gets material.** Tint, elevation, spines, fixed order —
   felt, not decoded. The manila filed-tab is the canonical example: the
   only "done" the surface can draw is a declared one.
4. **Machine addresses leave the surface.** Ids (S2, task numbers,
   hashes) are for tasks, rulings, and citations; at-a-glance surfaces
   show names and move addresses to hover or detail.
5. **Exceptions become sentences; calm stays compact.** A troubled item
   grows a second line stating what's wrong in plain language; healthy
   items stay one line. Trouble is thereby also *taller* — form echoes
   priority.

## The cockpit law

**Instruments for scanning, words for warnings.** Recurring, comparable
facts (counts, presence, progress-as-facts) render as instruments: task
dots (one dot = one recorded task, filled = done — never a percentage
bar), gauge numerals with tiny lowercase labels, chip clusters.
Warnings and requests stay sentences. Instruments always carry a small
label or hover title — an unlabeled instrument is an abstract mark (law
2 violation).

## Honesty constraints (from the Scope adjudications)

- No derived completion states: aggregates may count recorded facts
  ("16 of 32 tasks done"); nothing may claim a scope is fulfilled unless
  a source explicitly declares it.
- Unknowns are rendered space ("3 tasks don't belong anywhere yet"),
  never omissions.
- Staleness and confidence are visible states, inherited from
  `activitySignal` vocabulary verbatim.

## Voice

- **Chrome speaks teammate-across-a-desk sentences**: "sent back", not
  "rejection recorded"; "nothing's moved for 52 minutes", not "stale
  threshold exceeded"; "you last looked", not "last session timestamp".
- **Names are referenceable labels**, not conversational phrases: scope
  headings, role names, and anything tasks cite must survive being
  quoted inside a ruling ("Run work", never "What came back").
- **A whole-view surface states the whole**: the endeavor's own
  objective sentence rendered verbatim (no chrome prefix — never assume
  "building"; research endeavors don't build) plus honest aggregates.

## Chrome vs content

Every string is one of two kinds, and a design deliverable must say
which: **chrome** (generic app copy, product-assumption-free, ships with
taurhaus) or **content slots** (team-authored text — objectives, scope
headings — rendered verbatim). Demo data in mockups is realistic content,
never lorem, and never migrates into chrome.

## Process

- **Translate, never transcribe**: the backing document is law for what
  the surface must honor, never copy for what it must show. A deliverable
  that reads like its brief has failed the brief.
- Genuine open questions ship as switchable variants in one mockup
  (density, chip styles) so the operator compares by looking, not by
  imagining.
- Screenshot the work and fix what the look shows before delivering;
  judge against the five-second read, not against the spec text.
