# Cross-judgment — Fable ("The Site") on Astra's Product Cutaway

## 1. STEALS

1. **Addressed headings in BRIEF.md as the scope source** (`[S2] Run work` +
   `Scope: S2` in the task description). This kills my `scope.yaml`. A second
   file is a second thing that can silently disagree with the brief; a heading
   ID lives inside the document humans already read and edit, so scope changes
   and address changes are the same edit. It also generalizes for free: every
   endeavor shape has a brief with headings; not every one has an architect who
   would own a yaml artifact.
2. **The may-say / cannot-establish table.** I wrote the rule ("if it can't
   cite, it can't tint"); Astra wrote the contract — per source, the exact
   sentence the view may utter and the claim that source cannot carry alone.
   That table should ship in the design doc verbatim and become the review
   checklist for every rendered state.
3. **The three-way freshness split** — read freshness vs source modification
   time vs recorded event time. "A ledger read four seconds ago can still
   contain a week-old assignment" is a lie my walk window could have told; my
   concept conflated read-recency with event-recency in the strip header.
4. **Ruling supersession as an explicit relationship.** My "landed requires no
   open contested ruling" never defined what closes a ruling. Astra's rule — a
   newer accept does not erase an earlier reject without an explicit
   resolution — is the missing definition and prevents the worst green-mark lie.
5. **Multi-scope task references deduplicated by task ID.** My single-valued
   `area:` tag was under-expressive for cross-cutting work; Astra's repeated
   placement with a shared-work mark, totals counted by unique ID, is strictly
   better and costs nothing.

## 2. ATTACKS

1. **"Newly observed this session" fails exactly when reabsorption matters.**
   Astra's central honesty stance — a durable since-you-looked claim needs
   retained observation history — proves too much. Rulings carry `by`/`at`;
   completions and staleness/deadline actions are recorded with timestamps.
   For those event classes the ledger *is* the retained history; a durable
   read-mark over them needs one stored timestamp, not an event log. Failure
   scenario: the operator closes the app overnight; on morning open, either
   everything observed this session reads as "new" (firehose) or the notice is
   empty (silence). The longest absence — the field test's actual wound, the
   ruling learned via chat relay — is the case the session-local design cannot
   serve by construction.
2. **"One restrained line for newly observed consequential changes" hides an
   editorial policy.** One line, undefined selection rule, no fold count.
   Three rulings and two completions arrive in an hour: which one is the line?
   Whatever picks it is a ranking algorithm Astra refuses to name — precisely
   the confident quiet choice the rest of the document forbids. A change
   channel needs declared severity classes, a cap, and a visible fold ("9
   quieter events"), or it is a curator pretending to be a fact.
3. **Blessing permanently unplaced historical work rots the drift signal.**
   "Historical work can remain visibly unplaced" plus an always-on unplaced
   count means a mature project shows "63 unplaced tasks" forever; the number
   becomes wallpaper and stops meaning "recent drift". The fraction that
   governs trust must be scoped to open/active tasks of this endeavor, with
   history foldable — otherwise the honesty device trains the operator to
   ignore it.
4. **Refusing aggregated region state forfeits the five-second read Astra
   claims.** Their own list — where work is concentrated, which parts have
   disputes, which parts have no evidence — is preattentive or it is not
   five-second. A Level-0 region rendered as four text annotations ("2 tasks
   complete", "1 accept ruling"…) across six-plus regions is a sixty-second
   reading task. Astra's strongest epistemic instinct (never combine
   dimensions) is right; the conclusion (no tints) is wrong — see divergence 3
   for the version that keeps both.

## 3. THE DIVERGENCES, revised

1. **Scope source — concede.** Addressed BRIEF.md headings win (steal 1). I
   keep two shards of mine: the ghost-block rendering for unknown IDs (the
   synthesis already adopts it), and the role-contract hook — the architect
   role's definition of done says "scope headings are addressed", pointed at
   the brief, not at a file.
2. **Reabsorption — hold, with a new argument.** The walk over a durable local
   read-mark ships, compiled only from record-timestamped facts (rulings `at`,
   completions, `deadline.task.staled`); that is exactly as honest as Astra's
   own evidence table, because every line cites a record field, not an
   observation. Concede Astra's half fully: observation-class facts (activity
   edges, offline transitions) stay session-local and never enter the walk.
   Acceptance criterion: the walk must survive an app restart and an overnight
   absence — the test the session-local design fails by design.
3. **Region tints — third way.** Hold that tints are load-bearing (attack 4),
   but concede Astra's principle harder than the synthesis did: my "landed"
   tint combined work and review in one channel. Revised: **each visual
   channel carries one dimension.** Tint encodes work only (untouched /
   active / under-review / settled); the corner flag layer encodes review
   (open contested ruling); the chip layer encodes observation. "Landed" as a
   gestalt still appears — settled tint with no flag — but no single pixel
   asserts two dimensions, and hover cites each channel's record separately.
4. **Intervention transport — concede.** Terminal-as-transport, draft beside
   the member's pane, the view reflecting only ledger-recorded outcomes. My
   "aim the existing nudge machinery" was conditioned on the wire being
   trivial; it is not — nudges are daemon-owned (`deadline.nudge.sent`), so an
   inline verb means a new daemon op, which is not v1. What rides along from
   mine: the draft is prefilled from the selected evidence in the team's
   `ACTION REQUIRED:` convention — Astra's transport, my payload.
5. **Opening move — hold.** Walk first. State-first optimizes the short
   absence, which needs no help; the felt failure was a long absence. Astra's
   own restrained change line concedes a verbs channel exists — theirs is an
   under-built walk (attack 2). One concession shapes the build: the walk is a
   strip over the always-visible map, never an interstitial; the drawing is on
   screen from frame one, and a caught-up operator never sees more than one
   quiet line.

## 4. GENERALITY VERDICT

The merged v1's *machinery* survives all three shapes; its *vocabulary* and
two aggregation scopes do not.

- **(a) Greenfield build** — native case; survives as specified.
- **(b) Research sweep** — headings become questions (`[Q3] Does X hold under
  Y?`); regions-as-questions, reports as artifacts, review rulings intact.
  Breaks: builder vocabulary ("landed", "intended behavior", blueprint/taped
  metaphors — mine as much as Astra's) and ceremony weight: a two-hour sweep
  should never be nagged into scope addressing. Fix: endeavor-neutral state
  names (untouched / active / under review / contested / stalled / settled),
  "target sentence" becomes "the heading's own statement", and the degenerate
  no-headings rendering (evidence strip + walk) is the *intended* mode for
  small endeavors, with a quiet, dismissible adoption prompt.
- **(c) Evaluation/retro** — regions are rubric categories or case groups.
  Breaks: (1) verdicts are the *deliverable*, so "review" as a dimension is
  ambiguous between rulings-about-the-graders' work and grades-that-are-output;
  fix is no schema change — always render a ruling's subject (`field`/`ref`),
  which Astra's provenance rule already requires, so the two kinds cannot
  blur. (2) A retro is entirely about historical work, so "unplaced historical
  work stays on the shelf" floods it; fix is attack 3's scoping — fractions
  count this endeavor's tasks only.

Smallest global fix: strip "product" from every operator-facing string. The
anchor is **the declared scope of the endeavor**; regions are whatever the
brief's addressed headings declare — capabilities, questions, or cases.

## 5. ONE THING THE SYNTHESIS GETS WRONG

Its region-state call contradicts itself in one sentence: it adopts "Fable's
tints… defined strictly" — including *landed*, which is work-complete **and**
no open contested ruling — and then rules "no tint may combine dimensions."
Landed, as both Fable and the synthesis define it, combines two. As written,
implementers must violate one clause. The resolution is divergence 3:
one dimension per channel — tint = work, flag = review, chips = observation —
so "landed" survives as a gestalt of two honest channels instead of one
dishonest tint.
