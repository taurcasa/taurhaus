# The Site — a team-comprehension view for taurhaus

*Concept document. Fable, 2026-09-06.*

Design premise, stated before any pixels: the view must pass the **thirty-second
reabsorption test**. The operator looks away for an hour — a meeting, lunch, their
own coding — and on return must trust that thirty seconds in front of this view
restores the full working picture: what landed, what is contested, what stalled,
who needs them. Everything else in this document is derived from that moment.
Trust to look away is the product; the ambient view is just what the trusted
view looks like while you happen to be watching.

Two consequences fall out immediately:

1. **Reabsorption needs constancy plus salient change.** Humans re-orient by
   spatial memory: the same picture in the same places, with only the *changed*
   parts lit. A view that re-sorts, reflows, or re-ranks itself between glances
   destroys the very memory it should be leaning on. So the view must be a
   *place*, not a feed — and change must express as light, never as motion.
2. **A dashboard answers "what is the state"; the returning operator asks "what
   happened."** State is the noun; the operator needs the verbs since they left.
   So the view opens with a short, evidence-linked *story*, and only then settles
   into the ambient picture.

---

## 1. Three concepts

**A. The Site Plan** *(scope-anchored)* — The view is a stable map of the thing
being built: five to nine named areas of the product (auth, API, data layer,
UI shell…), drawn as blocks in a fixed arrangement, each tinted by its ledger
state — blueprint outline (unstarted), live (crew present), taped (under
review), flagged (contested ruling or stale), filed (landed). Member chips sit
*on the block they are working*, carrying their existing activity dot. Tasks,
rulings, and commits are one click below each block. This is the operator's
verbatim wish — "a diagram of what we want to build, then tasks working on which
parts and who is assigned" — made honest by a cheap scope convention.

**B. The Watch Log** *(time/handover-anchored)* — Borrowed from ships, not
dashboards: every watch officer coming on duty reads the log and signs it.
The view is a compiled narrative of typed ledger events since your personal
read-mark — completions with their ruling verdicts, rejections, effort switches,
staleness events, decision requests — rendered as six terse lines, each a link
to its evidence, ending in a "caught up" signature that advances the mark. No
map, no board: the unit of the view is *the interval you were away*. Its whole
claim: reabsorption is a reading act, so build the reading.

**C. The Interrupt Line** *(attention-anchored)* — The view is sorted by one
question only: *does this need the operator?* A short queue of moments —
a rejected packet awaiting a call, a stale task past its deadline, a blocked
chain whose head is idle — each with the one or two verbs that resolve it
(nudge, reassign, answer, waive). Everything healthy is compressed into a single
calm band: "6 members working, 4 tasks in review, nothing needs you." Its claim:
altitude anxiety is really *interrupt* anxiety; solve triage and the operator
stops needing to follow everything.

**Which I believe:** A is the skeleton — it is the only concept anchored on the
thing being built, which is the named hole in the product, and the only one that
answers "which *part* is moving" at all. But A alone, drawn naively, is a
dashboard: a state display with no arrival ritual and no verbs. So the fleshed
concept below is **the Site Plan as the place, with the Watch Log as its opening
move and the Interrupt Line as its flags** — B and C are not discarded, they are
the reabsorption moment and the handles of A. What I explicitly reject is pure B
(a log gives verbs but no altitude — you cannot *see* that the data layer hasn't
moved all afternoon in prose) and pure C (a triage queue is silent exactly when
the operator most wants reassurance, and silence from a tool that has lied
before reads as absence, not health).

---

## 2. The Site, fleshed

### 2.1 The reabsorption moment (designed first)

You open the Site tab after 74 minutes away. Two things happen, in order:

**The walk.** A strip across the top — the *site walk* — shows at most six
lines compiled from typed ledger events since your read-mark, in fixed severity
order (decisions > contested > landed > stalled > everything-else folded):

> **since 13:05** · 2 things need you
> ● Decision: reviewer-2 rejected the wave-2 API packet — *oversize* (14:12)
> ● Data layer: no movement for 52m; owner idle since 13:31
> ✓ Auth landed — accept, 38/40, reviewer-1 (13:47)
> ✓ Task board wiring landed — accept, 34/40 (14:02)
> · 9 quieter events — *and mark caught up*

Every line is assembled from a typed event by a fixed template — completion +
its ruling scalar, a `verdict: reject` ruling entry, a staleness observation
from the routing telemetry, a decision request — so every sentence is traceable
to a record. **No line is generated prose in v1.** Clicking a line lights and
opens the block it belongs to. "Mark caught up" advances the read-mark; the
strip collapses to one quiet line until events accumulate again.

**The light pass.** On the map below, blocks whose state changed since the
read-mark carry a thin lit edge (brand accent); flags (contested, stale,
decision) sit as small high-contrast corner markers. Nothing moves, nothing
reflows — the map is the same map you left, with three edges lit and two flags
up. Your eyes do the diff, because the layout let them.

Thirty seconds: read six lines, glance at three lit edges, click the one flag
that names you. Done.

### 2.2 Anatomy at rest

A main tab (sibling of Overview / Tasks / Files / Git), living in the standard
floating main panel. Top to bottom:

- **Walk strip** (collapsed to one line when caught up: "caught up 14:20 ·
  quiet since").
- **The map**: a grid of area blocks in the *fixed order the scope file
  declares* — never resorted, never reflowed. Each block: area name, a small
  `landed n / mapped m` fraction, the member chips currently working it
  (tool logo + activity dot straight from `activitySignal` — `uncertain`
  renders dimmed with "last seen" wording, never green), and its state tint:
  - *unstarted* — line-work only, blueprint style, no fill;
  - *moving* — panel fill, live chips;
  - *in review* — a taped top edge (hatched);
  - *contested* — amber/red corner flag (an open `reject`/`oversize` ruling);
  - *stalled* — desaturated, captioned "no movement 52m" (from telemetry
    staleness, not a local guess);
  - *landed* — quiet manila fill, visually "filed", echoing the app's manila
    tab identity. Landed means: zero open mapped tasks **and** no open
    contested ruling — never a threshold, never a percentage.
- **The unmapped tray**: a slim bottom rail holding tasks with no area tag and
  members whose in-progress task is unmapped. Unknown is a *visible place on
  screen*, not an omission. The tray shows "34 of 41 tasks mapped" — the one
  fraction that governs how much the map may be trusted, always adjacent to it.
- **Roster rail** (right edge, chips only): every member, including idle and
  offline ones, so "who exists" never has to be inferred from "who is placed."

### 2.3 Altitude mechanics

- **Level 0 — the Site.** Areas, chips, flags, walk. Answers: which parts are
  moving / done / contested, who is where, what changed, what needs me.
- **Level 1 — the block sheet.** Click a block: a floating sheet (same anchored
  card style the mesh canvas already uses) lists that area's tasks grouped by
  state, each row carrying owner chip, ruling tail as compact chips
  ("reject · oversize · r2 · 14:12" / "accept 38/40"), blockers, and evidence
  links — open in Task Board (the board's existing `navTarget` restore
  machinery makes this a one-hop deep link), open packet in Files, open commits
  in Git. The sheet is a *lens over existing surfaces, not a second board*: it
  never grows its own detail panel.
- **Level 2 — the existing surfaces.** The Site aims the Task Board, Files, and
  Git tabs; it rebuilds none of them.
- **Being pulled down:** the Site never steals focus. The pull is: the tab
  badge counts decision-class flags; opening the view after an absence
  auto-presents the walk; clicking a walk line opens its block sheet. That is
  the whole automation — an operator mid-thought is never yanked.

### 2.4 The five-second read

Fixed scan path, by deliberate contrast budget: **flags first** (few, small,
highest contrast), **lit edges second** (what changed), **chips third** (who is
where — a chip cluster on one block *is* the assignment picture), **the calm
rest last** (manila = done, line-work = not started, desaturated = stalled).
"Who is doing what, on what part" is literally chips-on-blocks; "how is it
going" is the tint field; "what needs me" is the flag count. Nothing on Level 0
requires reading a sentence except the walk, and the walk is optional once
you're caught up.

### 2.5 The data contract

| Need | Source | Status / ceremony cost |
|---|---|---|
| Task state, owner, blocks/blockedBy, effort, deadlines | Mesh task ledger | Exists |
| Rulings (verdict, score, by, at, ref) | Task metadata rulings (ledger; W-B formalizes the sequenced array) | Exists / landing |
| Member activity + confidence | Runtime records → `activitySignal` | Exists |
| Staleness, nudges, completion observations | Routing telemetry sidecars + deadline pass | Exists |
| Commits, changed files | libgit2 in-process | Exists |
| **Area list** | `scope.yaml` (or a fenced block in BRIEF.md): flat list of `{id, name, one-liner, optional path globs}` — maintained by the **architect role**, added to that role's `required_artifacts` and definition of done. | **New convention.** Edited at exactly the moments the architect already works (packet time). Minutes per wave. |
| **Task → area mapping** | An `area: <id>` key in task metadata, set by the **lead at assignment** — one token inside a write that already happens. The operator can also set/fix it from the Site (see handles). | **New convention.** Near-zero marginal ceremony; enforced by visibility (the unmapped tray), not by validation. |
| **Decision requests** | v1 derives "needs you" from what exists: open `reject`/`oversize` rulings, telemetry staleness past deadline, blocked chains whose head is idle. A *typed* operator-decision convention (`needs: operator` metadata key, set by lead or member) is a cheap follow-up that rides the same metadata channel. | Derived now; one small convention later |
| **Read-mark** | Local per-project timestamp in the app DB, advanced by "mark caught up" and by dwell on the open view. | Trivial, app-local |

**Drift is a rendered state, not a failure.** A task tagged with an unknown
area id renders a ghost "proposed area" block (line-work + question mark)
instead of erroring — which is simultaneously the nag that tells the architect
the scope file is behind reality. Untagged tasks pool in the tray. The
`mapped m / total` fraction keeps the map's own trustworthiness on screen at
all times. Files-touched derivation (commits → area path globs) is deliberately
**not** the primary mapping in v1 — it is the future *cross-check* that flags a
task whose commits land outside its declared area ("spill") — because derived
mappings are exactly the kind of confident-looking guess this team has been
burned by.

### 2.6 Honesty and staleness

- **Every visual state is a rendering of a record, and hover shows the
  derivation.** Hovering a landed block: "6/6 mapped tasks completed · last
  ruling accept 38/40 by reviewer-1 · 13:47." Hovering a stalled caption names
  the telemetry observation. If it can't cite, it can't tint.
- **No synthetic progress numbers.** Only `n/m` fractions whose members are
  clickable, with the unmapped remainder printed beside any aggregate.
- **Activity honesty is inherited, not reimplemented**: `uncertain`, retained
  (`stale`/`degraded`) and `offline` readings render in `activitySignal`'s own
  vocabulary — dimmed, "last seen", never a live dot. A chip is placed on a
  block only by its member's *in-progress mapped task*; a member the system
  cannot place stays in the roster rail rather than being guessed onto the map.
- **The walk states its own window** ("since 13:05") and folds what it cut
  ("9 quieter events"), so it never pretends to completeness.
- **When the daemon bridge is degraded**, the map keeps last-good tints but the
  walk strip leads with the retained-reading banner in the same wording the
  rest of the app uses. A stale map that says it is stale is a map; a stale map
  that doesn't is the thing this team was burned by.

### 2.7 Operator handles

From the moment that needs them, without leaving altitude:

- **Flag verbs.** A stalled flag offers *nudge* (the existing deadline-pass
  nudge machinery, aimed manually); a contested flag offers *open the ruling*
  (block sheet, ruling tail, packet link) and — once the decision convention
  exists — *answer*, prefilled with the team's `ACTION REQUIRED:` message
  convention. Reassignment stays one hop away (deep link into the existing
  roster surface) until it earns an inline verb.
- **Mapping verbs.** Drag a task from the unmapped tray onto a block (or use a
  picker) to set its `area` tag — the same gesture that fixes drift also
  bootstraps a team mid-flight that adopted the Site late.
- **Mark caught up.** The signature at the end of the walk; the view's one
  ritual, and the thing that makes the next reabsorption honest.

---

## 3. The smallest honest v1

One contained feature PR:

- The **Site tab** for a project with an active team: walk strip (template-
  compiled from ledger completions + rulings + telemetry staleness, fixed
  severity order, capped at six), the **fixed-order block grid** with the six
  tints, member chips via `activitySignal`, corner flags for
  contested/stalled, the **unmapped tray** with the mapped-fraction, and the
  roster rail.
- **Scope source**: read `scope.yaml` if present; with no scope file the view
  renders one honest block — "unscoped" — holding everything, plus a single
  line telling the operator what file the architect role should produce. The
  degenerate state is itself the adoption prompt.
- **Mapping**: read `area` from task metadata; write it via the tray picker.
- **Handles**: block sheet with deep links (Task Board `navTarget`, Files,
  Git); tray tagging; mark-caught-up. Nudge ships only if the aim-the-existing-
  machinery wire is genuinely small; otherwise it deep-links.
- **Deliberately omitted**: files-touched derivation and spill detection; any
  LLM anywhere; dependency edges between areas; area weighting/sizing; time
  scrubbing; auto-focus pulls; the typed decision-request convention; in-app
  scope editing.

**Why it is already useful alone:** it retires the lead's hand-written
ledger.md as the only wave-state view; it makes rulings visible without reading
JSON (the field test's exact wound — the operator learned about the
architecture packet's altitude review from a chat relay); and even
half-mapped, the walk strip alone passes a weaker form of the thirty-second
test on day one, because completions-with-verdicts and staleness need no scope
file at all.

## 4. Risks and guards

- **Ceremony rot** — nobody feeds the scope file or the tags. *Guards:* the
  mapping cost is one token inside an existing write; rot is rendered (tray
  fills, fraction falls), never silent; the architect role's contract carries
  the scope file; the operator can tag by drag; and the walk strip stays
  useful at zero mapping, so the view never becomes worthless enough to close.
- **Lying by aggregation** — a block reads landed while an unmapped critical
  task burns. *Guards:* block states aggregate only mapped tasks and say so
  (`n/m`); the unmapped fraction is structurally adjacent to every aggregate;
  landed additionally requires no open contested ruling.
- **Scope outgrown by the product** — the map ossifies at wave-1's shape.
  *Guards:* unknown area ids become ghost blocks that prompt the architect;
  the scope file is a flat, append-friendly list; blocks never encode layout
  the operator would resist changing (fixed *order*, not authored geometry, in
  v1).
- **Walk becomes noise at high event rates.** *Guards:* fixed severity
  classes, hard cap of six lines, explicit folding with a count, and the
  read-mark resets the window — the strip's size is bounded by design, not by
  hope.
- **Misplaced chips** — attribution errors put a member on the wrong block.
  *Guards:* placement only via the in-progress task's own tag (evidence, not
  inference); unplaceable members stay in the rail; confidence and retained
  states are inherited from `activitySignal` verbatim.
- **The Site becomes a second task board** and forks detail UX. *Guards:* the
  block sheet is capped at rows-with-links; every deeper action deep-links
  into the existing board/files/git surfaces via machinery that already exists.

## 5. Three versions later (the dream, kept separate)

An architect-authored *diagram* rather than a grid — positions and
area-to-area edges aggregated from `blocks`/`blockedBy` across tagged tasks,
so contention shows as a red edge between blocks. Files-touched heat under
each block with spill detection as the drift alarm. A time scrubber that
replays the site at any past read-mark ("show me Tuesday"). The typed
decision-request lane merged with an answer composer. And only then, last and
least: an LLM-narrated walk — with every sentence still required to carry its
citation, because the walk's authority comes from the records, and that rule
is the whole reason the operator will trust it enough to look away.
