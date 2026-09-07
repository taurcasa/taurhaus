# Scope — design proposal for the team-comprehension view

Creative direction for the implementation lane. Binding inputs: `synthesis.md`
(v2 — its settled list, three adjudications, and v1 scope) and the retro
addendum (declared waits as first-class chip states, restart-cursor
drill-down, the new evidence classes, endeavor-neutral vocabulary). This
document decides visual language, anatomy, and hierarchy. It deliberately does
not decide pixels, component boundaries, or IPC shape — working values below
are calibration points tied to the house grid, not specs.

A static mockup of the at-rest state with wave-1-shaped taurjobs data exists
at the session scratchpad as `view-mockup.html` (non-normative; this document
wins where they disagree).

---

## 1. The direction in one paragraph

The tab is called **Scope** — the noun the synthesis anchors on, honest for a
build, a research sweep, or a retro alike. taurhaus's identity is manila: the
active tab is a folder tab made of the main panel's own material, and the
sidebar's strongest gesture is "pulled" panel material bridging into the main
surface. Scope extends that material story downward one level. The view is
**the endeavor's paperwork laid out on the panel**: declared scope as sheets
of paper in the fixed order the planning document declares, work as material
presence on a sheet, change as light, disputes as flags, people as chips, and
the only "done" the view will ever show — a **declared** outcome — rendered as
the sheet growing a small manila folder tab and becoming a filed document.
That filed treatment is the view's one signature element; everything around it
stays quiet, dense, and calm. The walk strip above the map is the arrival
ritual — six template-compiled lines, each citing a record — and the view's
one piece of continuous prose.

Why this direction: it makes the app's existing metaphor do the honesty work.
Paper that nothing has touched looks like paper. Paper being worked has
material weight. Paper that someone *declared* finished gets filed — visibly a
human act (a labeled tab with a name and a time on it), not a computed state.
The one claim neither concept could make honest — derived finished-ness —
has no material to express itself in.

## 2. Where it lives, and why a main tab

**A main-tab sibling of Overview / Tasks / Files / Git / Mesh**, inside the
standard floating main panel. The region grid's whole job is simultaneous
comparison — which parts move, which are contested, which are silent — and
comparison needs breadth; the roster rail needs a persistent edge; the walk
needs a fixed strip above an always-visible map. None of that survives in a
popover or a takeover.

But "main tab" in taurhaus does not mean wide: the app's own paradigm is
ultrawide side-panel use, so the main panel is routinely ~560–760 px. The
layout therefore has **width classes, not a fixed composition**:

- **Wide (≳ 1000 px)**: walk strip · region grid (2–3 columns) with roster
  rail on the right edge · unplaced shelf · freshness bar.
- **Narrow (≲ ~720 px)**: the roster rail docks as a single horizontal chip
  strip directly under the walk; the grid becomes one column. Everything else
  keeps its slot.

Spatial memory is protected by **fixed document order, never by fixed
geometry**: regions render in the order the addressed headings appear in the
planning doc, the grid auto-flows and never re-sorts or re-ranks, and card
heights are stable (chip rows cap at two lines with a `+n` overflow that
prioritizes contested/blocked work, per the synthesis). Within a width class
the picture the operator left is the picture they return to; between width
classes only column count changes, order never does.

**Tab signal**: no filled badge. The rail's badge grammar reserves the filled
pill for "act on me", and adjudication C forbids manufacturing that category.
When the walk holds unacknowledged lines the tab may carry the quiet-teal
outlined treatment the sidebar already uses for workflow badges — metadata,
not an alert. It says "there is a walk to read", which is exactly all we know.

## 3. The channel system — one dimension per pixel

The core of the visual language is the adjudicated one-dimension-per-channel
rule, mapped onto material properties the app already uses:

| Channel | Dimension it carries | Treatment | May never carry |
|---|---|---|---|
| **Ground** (region surface) | Work presence, now | Line-work paper (no open mapped work in progress) vs. active ground (material fill + elevation, the mesh-node surface recipe) | Progress, completion, review state |
| **Manila** (declared only) | A recorded declaration by the scope owner | Warm manila wash + a folder tab carrying "filed by *name* · *time*" | Anything derived. Renders **only** when a source explicitly records the scope item's outcome |
| **Light** (edges) | Changed since your mark | Thin lit perimeter, brand-400 luminance + soft glow; static, no pulse | Severity, direction of change |
| **Flags** (header corner) | Recorded review/condition state | Glyph + word + responsible owner: "contested · sol", "stalled 52m · ivo (deadline pass)" | Operator requests ("needs you" waits for the typed contract) |
| **Chips** (people) | Observation, verbatim from `activitySignal` | Tool logo + 6 px dot in the existing level colors; `uncertain` dims with "last seen" wording | Assignment claims beyond "in-progress mapped task places the chip" |
| **Wait tags** (on chips) | A *declared* wait | 14 px outlined amber tag: "awaiting GO", "blocked: api-contract → sol" | Inferred waiting. Only a citable declared record renders a tag |
| **Count lines** (text) | Honest aggregates | "4/6 mapped complete · 1 in progress", "no linked task evidence", ruling tails as compact chips | A tint. "All n mapped tasks complete" is a count line, never a surface |

The gestalt readings the operator wants still exist — a filed sheet with no
flag reads done, a dense chip cluster reads hot — but every gestalt decomposes
into channels that each cite their own record on hover, and no single pixel
asserts two dimensions.

**Hue discipline inside this view** (color supports, never carries — every
flag and tag also has a glyph and a word):

- **Green** — observed motion only (activity dots; accept lines in the walk).
- **Amber** — absence of forward motion, in three claims distinguished by
  channel: an idle *dot* (observation), a *wait tag* (declaration), a
  *stalled flag* (recorded condition). One hue, one meaning — nothing moves
  here — with the channel naming which kind of claim.
- **Red** — recorded dispute only (reject / oversize rulings and their flags).
- **Blue** — uncertainty and retained readings, inherited verbatim
  (`uncertain`, stale/degraded, "last seen"). Never used for anything else in
  this view, even though the task board spends info-blue on "pending".
- **Teal** — the brand: change-since-mark light, plus the app's standard
  interactive/selection treatment. The two never blur because they differ in
  *kind*: change is **luminance** (a lit hairline + glow at 1 px), selection
  is **structure** (the house 1.5 px border + ring, exactly the mesh node
  recipe). A region can be both lit and selected and each reads.
- **Manila** — declared outcome. The only "done" color in the view, and it is
  declaration-only by construction.
- **Zinc/grey** — offline, history, folded remainder.

## 4. Anatomy at rest

```
┌──────────────────────────────────────────────────────────────┬─────────┐
│ WALK  since your mark · Sun 12:31 · 11 events first observed │ ROSTER  │
│  ▲ S5 · reject recorded — 'runner protocol' oversize … 14:12 │  ★ lead │
│  ‖ mira declared waiting — GO from lead …             13:58  │  nia    │
│  ▣ S7 filed by nia — declared complete                13:47  │  sol    │
│  ✓ S2 · 'run record schema' — accept 36/40            13:22  │  tess   │
│  … 4 quieter events                       [ Mark caught up ] │  bo     │
├──────────────────────────────────────────────────────────────┤  mira ‖ │
│ ┌─ S1 Configure ──┐ ┌─ S2 Run ══════┐ ┌─ S3 Progress ─┐      │  aya    │
│ │ paper, counts   │ │ active, chips │ │ mira ‖ GO?    │      │  ivo    │
│ └─────────────────┘ └═══════════════┘ └───────────────┘      │  ─────  │
│ ┌─ S5 Supervision ▲┐ ┌─ S6 Persist ◷┐ ┌─ S7 ░filed░───┐      │  dev ∅  │
│ └══════════════════┘ └──────────────┘ └───────────────┘      │         │
├──────────────────────────────────────────────────────────────┴─────────┤
│ UNPLACED  4 unplaced · 1 unresolved ref — of 19 open tasks  history(32)│
│ FRESHNESS ledger read 4s · written 2m · newest event 14:18 · journal … │
└────────────────────────────────────────────────────────────────────────┘
```

Top to bottom: **walk strip**, **region grid** (+ roster rail right),
**unplaced shelf**, **freshness bar**. The inspection drawer floats over the
grid's right side when something is selected. Reasoning per element follows.

### 4.1 The walk strip

The arrival ritual and the view's one reading surface — so it gets the most
generous type on the screen and the map never leaves from under it (the strip
is a band over the always-visible map, never an interstitial).

**Type and spacing** (the walk is the one place these numbers matter as
direction, so they are stated): Geist at **13 px / 1.55**, lines capped at
**~72ch** so a wide panel does not stretch a sentence across 160 characters.
Each line is a three-part row: a fixed **16 px glyph column** (severity mark,
colored per the hue table, drawn as a glyph — never color alone), the
template-compiled sentence, and a right-aligned **Geist Mono 10.5 px
tabular-nums citation** (time + record ref) that is also the line's link.
6 px between lines, no rules — the glyph column is the structure. The strip
sits on a quiet sub-surface (the validation-bar recipe: hairline border,
slightly raised fill) so it reads as a docket clipped over the drawing, not a
banner. Header microcopy states the window and its basis in one breath:
*"since your mark · Sun 12:31 · 11 events first observed"* — first-observed
time from the app-local observation journal, per adjudication B, so late
arrivals and reopens appear and the phrase never claims event-time coverage.

**Fixed severity classes**, in a declared order — gates, then outcomes, then
observations, then bookkeeping — capped at six lines with an explicit fold:

1. **Contested rulings** — reject / oversize recorded, with the ruling's
   subject scoped so verdicts about assessed historical work never read as
   current disputes.
2. **Declared waits** — a seat parked and saying so (awaiting-GO /
   blocked-on). Ranked directly after disputes because the wave-1 field
   test's worst misread was a waiting seat declared dead.
3. **Declared outcomes** — a scope item filed by the scope owner.
4. **Completions with rulings** — "complete — accept 36/40 by *name*".
5. **Stalled observations** — deadline-pass / telemetry staleness,
   source-labeled.
6. **Bookkeeping** — `budget_raised` rulings and monitor-nudge events, always
   source-labeled ("nudge → ivo, from monitor, 14:18").
7. The fold: "*n* quieter events" — count always explicit.

No generated prose; every sentence is a template over a cited record. The
strip ends in **Mark caught up** — a quiet button, the view's one ritual,
with microcopy that keeps the contract visible: *"Marks the walk read. Flags
stay until their record closes."* Acknowledgement is explicit only; no dwell.
Caught up, the strip collapses to one 24 px line: *"caught up 14:20 · quiet
since"*. When a source read fails or a scan degrades, the strip **leads**
with the app's retained-reading banner in the app's own wording — degradation
promotes to the top while the map keeps last-good states; health stays a
bottom-bar fact (§4.4).

### 4.2 The region grid

CSS grid, fixed document order, gutter on the house 6 px rhythm (suggest
10 px), cards on radius 8 (house radii 6/8/999; the folder tab uses 6). Card
anatomy, top to bottom:

- **Header row**: the address chip (`S2`, Geist Mono, 10 px, outlined) ·
  the heading's own words at 13 px/600 · flags right-aligned in the corner.
- **The heading's statement** — one quiet 11.5 px line quoted from the doc
  (its own words, not a paraphrase; endeavor-neutral by construction).
- **Chip row(s)** — members whose in-progress mapped task lives here, max two
  rows then `+n` (overflow ordering: contested/blocked first, then current
  assignments — never whoever last printed output).
- **Count lines** — the honest aggregates and ruling tails as compact chips
  ("reject · oversize · bo · 14:12"). Empty space is labeled: *"no linked
  task evidence"* — an association fact, never "no work happened".

Ground treatments (the work channel): **line-work paper** — hairline border,
no fill, no shadow — when no mapped open task is in progress; **active
ground** — the mesh-node surface recipe (translucent fill, slightly stronger
border, the node shadow) — when at least one is. In light mode this becomes
elevation doing the talking: flat outlined paper vs. a raised white card. A
region with every mapped task complete and no declaration **stays paper** with
a count line — that is adjudication A rendered: the strongest thing the grid
may say about finished-ness is a sentence, and the mockup's S1 shows it.

**Filed** (declared only): the manila wash plus the **folder tab** on the top
edge — a small tab in the manila material carrying "filed by *name*", echoing
the titlebar's manila tab down to the corner treatment. Filing never hides
flags: a filed sheet with an open contested ruling wears both, and the
contradiction is the point.

**Lit edge**: the change light from §3, tied to first-observed changes in the
region's displayed evidence since the mark. Clicking a walk line lights and
selects its region — the eye lands where the sentence pointed.

### 4.3 Roster rail and chips

Right edge, slim (~176 px; chips only), every member **including idle and
offline** — who exists is never inferred from who is placed. Rows: tool logo,
name, activity dot, and — first-class, per the addendum — the **declared-wait
tag**. A chip is placed on a region only by its member's in-progress mapped
task; an unplaceable member stays in the rail; offline members sink to a
greyed group at the bottom, in `activitySignal`'s own vocabulary (never a
live dot, "last seen" for retained readings).

The wait tag is the addendum's center: **a declaration renders beside the
observation, never instead of it**. An idle dot with a tag reading "awaiting
GO" is the honest picture of the wave-1 misread — observed idle, declared
waiting — and the tag is outlined (state), not filled (the rail grammar's
act-on-me), because a wait is a condition even when the awaited party is the
operator. Tags cite their declared record and time on hover.

**Drill-down — the seat card.** Clicking any member chip (rail or region)
opens the drawer as that seat's **restart cursor**, per the addendum: three
labeled rows — **current tip** (the seat's last recorded step), **pending
rulings** (open rulings on their work, supersession explicit), **next
action** (declared) — each with its citation, plus the declared wait if one
stands, plus the handles (§4.5).

### 4.4 Unplaced shelf and freshness bar

**Unplaced shelf** — a slim rail above the freshness bar; unknown is a place
on screen. Its fraction is scoped to the endeavor's open tasks ("4 unplaced ·
1 unresolved ref — of 19 open tasks"), history folds behind an explicit count.
Unresolved references render as dashed ghost chips carrying their bad address
("Scope: S9 — unresolved") and never become regions; adoption belongs to the
named scope owner, so the view's only verb here is inspecting the task.

**Freshness bar** — the bottom hairline strip, Geist Mono 10 px, carrying the
three-way split as three labeled clocks per source: *read* (when taurhaus
last read it), *written* (source modification), *newest event* (recorded
event time) — "ledger read 4s · written 2m · newest event 14:18". The journal
states its own window here too. Health is quiet and lives at the bottom;
failure is loud and moves to the top of the walk (§4.1). No universal
freshness threshold exists, so the bar renders facts, not verdicts.

### 4.5 Inspection drawer and handles

The drawer is the mesh canvas's anchored-card language (same enter animation,
same surface), sliding over the grid's right side, ~300 px. It is **a lens,
not a second board**: rows with citations and deep links, no detail panel of
its own. Three variants: the **region sheet** (tasks grouped by state, owner
chips, ruling tails, blockers), the **seat card** (§4.3), and the **evidence
card** (one ruling in full — verdict, by, at, subject, and its supersession
relationships rendered explicitly: a newer accept sits *beside* an earlier
reject until a source records the resolution).

Handles, from the moment that needs them: **Open in Task Board** (the
existing `navTarget` restore machinery), **Open packet** (Files), **Open
commits** (Git), and the intervention transport — **Open pane · draft
ACTION REQUIRED**, which opens the member's own pane beside the view with a
draft prefilled from the selected evidence in the team's convention. The
view reflects only ledger-recorded outcomes; no inline nudge ships in v1
(recipient/delivery/duplicate-protection are not established), so nudging is
a deep link plus a draft, honestly labeled.

## 5. Degenerate mode — the intended small-endeavor mode

No addressed headings: the grid cell is replaced by the evidence strip
(current ledger evidence, consequential rulings), with the walk, roster,
shelf-holding-everything, and freshness bar unchanged. One dismissible
adoption prompt in the sidebar's guide-card voice ("Address scope headings in
BRIEF.md to draw the map"), then silence. A two-hour sweep pays zero scope
ceremony and still gets the walk — which needs no scope at all.

## 6. Light and dark

Dark is primary (the frame world); light is the paper world — and the
metaphor gets *stronger* in light mode, where line-work vs. elevation vs.
manila reads like actual paperwork on a desk. Token direction (a `--scope-*`
family in `app.css` `@theme`; values are calibration, tune in implementation):

| Token intent | Dark | Light |
|---|---|---|
| Paper ground | no fill, border `rgba(255,255,255,0.08)` | panel white, border zinc-200, no shadow |
| Active ground | `--mesh-node-bg-dark` + node shadow | `--mesh-node-bg-light` + node shadow |
| Manila fill / border / ink | `rgba(214,193,138,0.07)` / `…0.22` / `#d6c18a` at ~80% | `#f6f0de` / `rgba(166,138,66,0.28)` / `#8a6d2f` |
| Change light | border brand-400/55 + glow `rgba(45,212,191,0.14)` | border brand-600/45 + glow `rgba(13,148,136,0.12)` |
| Flags / tags / dots | the existing semantic ramps (danger/warning/info/success) at the app's dark strengths | the light-mode strengths HoverCard already uses |

Everything else inherits: panel surfaces, selection, scrollbars, the manila
titlebar continuity.

## 7. Motion and snappiness

Render the cached snapshot immediately; explicit states ("Ledger not yet
read") instead of spinners; the standard 120 ms `content-enter` on tab
switch; drawer uses the mesh detail enter; walk collapse/expand ≤150 ms
ease-out. The change light is **static luminance** — no pulse, nothing
breathes on this surface — so reduced-motion support is nearly free and the
view stays calm at any event rate. Nothing ever moves *by itself*: change
expresses as light, never as motion, because the layout is the operator's
memory.

## 8. What no pixel claims

The review checklist for every rendered state is the may-say /
cannot-establish table the synthesis adopted; this section is the design's
own summary of it, and doubles as the field-test answer key ("name one thing
the view does NOT claim"):

- No pixel claims a scope item is fulfilled. Filed is a declaration with a
  name on it; "n/m mapped complete" is a sentence about tasks.
- No pixel claims the operator is needed. Flags render conditions with
  owners; the request category waits for the typed contract.
- No chip claims a member is executing the task that places it — assignment
  and observation are separate claims, and the dot is only ever
  `activitySignal`'s word.
- The walk claims only its window and its basis (first-observed, journal),
  states its fold, and acknowledging it resolves nothing.
- A fresh read never freshens old claims: the three clocks stay separate.
- Empty space means "no linked evidence", never "nothing happened"; unplaced
  and unresolved work stay visible, scoped so the number can still alarm.

Acceptance remains the synthesis's thirty-second field test, unchanged.
