**The Product Cutaway: see the intended product, then inspect the work changing it.**

The missing surface is a stable picture of what the team intends to build, with current evidence attached to its parts.
The operator should be able to leave nine agents working, return, and recognize the product immediately—even if every task and assignment has changed.

Three directions could solve different versions of this problem:

1. **The Product Cutaway — anchored on the intended thing.** Show the planned product as an outline divided into meaningful parts: entering a workspace, running work, inspecting results, recovering from failure. Work, artifacts, and review disputes appear inside those regions. Unrepresented scope remains visible as empty space. Zooming reveals the architecture supporting a capability, then the tasks and people changing it. Its organizing question is: “What is happening to this part of the product?” The ambitious version feels like inspecting a building under construction, with its plans, installed material, and inspection notes occupying the same space.

2. **The Commissioning Reel — anchored on meaningful transitions.** Present a deliberately paced account of the product becoming real: architecture accepted, execution path attempted, interface rejected, first usable slice demonstrated. The operator moves between the last understood state and the current state, inspecting the evidence behind each transition. Tasks become supporting material for a small number of changes worth understanding. Its organizing question is: “What became different while I was away?” Much of the raw material already exists in rulings, completion observations, documents, and git; durable “since you looked” behavior would require retained observation history.

3. **The Intervention Desk — anchored on unresolved choices.** Arrange the team’s work around the decisions holding it in place: a disputed interface, an oversized task, a missing answer, a dependency nobody owns. Each issue exposes the affected work and offers a concrete intervention. Its organizing question is: “Where would my involvement change what happens next?” Ledger dependencies and rulings provide a useful starting point, but an actual request for the operator must be explicit. A rejection alone cannot establish that the operator needs to decide.

**I would build the Product Cutaway.**
The other concepts improve awareness and intervention, but the cutaway supplies the missing reference frame: the product itself.
It can also host a restrained change notice and an intervention drawer without surrendering that anchor.

The decisive design choice is that **scope stays still while work moves through it**.
A task can complete, reopen, change owner, or receive a rejection without rearranging the picture.
The operator gradually learns where things belong.

**Start with what the existing data can honestly say.**

With no new team ceremony, taurhaus can already present a compact evidence strip:

- Which tasks the ledger records as in progress, and their assigned owners.
- Which tasks have recorded accept, reject, or oversize rulings.
- Which declared task dependencies remain unresolved.
- Where an assignment and the member’s observed runtime activity disagree.
- Which relevant observations became visible during the current viewing session.

That is substantial comprehension per pixel.
It would have made the architecture task’s completion and recorded acceptance visible without a chat relay.

It still cannot reliably answer “which part of the product?”
Task subjects can suggest an answer, but grouping them through plausible language would turn a guess into the structure of the interface.
Changed files do not solve that problem either: one shared file may support several capabilities, and a greenfield capability may have no files yet.

The cheapest useful addition is **a shared scope address between the brief and the task**.
Everything else should earn its place later.

**The ambitious view is a drawing you can inspect at several depths.**

At the highest altitude, the intended product occupies the screen.
Its regions represent capabilities or meaningful subsystems named in the brief.
A region contains a short statement of its intended behavior, a few current work annotations, and visible evidence or disputes.

In the fuller concept, the operator can look through a capability and see the architecture beneath it.
Selecting “Recover an interrupted run” might reveal persistence, execution ownership, and replay behavior.
Those relationships must come from an explicitly authored architecture source or a clearly labeled proposal; task dependencies cannot silently become architecture edges.

A further depth exposes the current implementation and its supporting material:
a document, a reviewed interface, a changed file, a captured demonstration, or a test result tied to a revision.
The desired behavior remains visible above it, so the operator can compare intention with observed evidence.

This is the three-versions-later destination.
It requires richer artifact references and, eventually, captured behavioral evidence.
The first version can preserve the same spatial idea with much less machinery.

**At rest, the screen shows a product outline annotated with work.**

The main tab contains four elements:

- A thin source bar identifying the scope document, read freshness, and unplaced work.
- Fixed scope regions occupying most of the surface.
- One restrained line for newly observed consequential changes.
- A narrow inspection drawer that opens when the operator selects a region or annotation.

Each region contains its scope name and target sentence.
Below that sit short annotations for current assignments and recorded evidence.
A ruling has a distinct mark and a readable label; color supports the distinction but never carries it alone.

An illustrative nine-member team could look like this:

```text
PRODUCT CUTAWAY                                  BRIEF.md · read 4s ago
6 declared areas · 1 without linked tasks · 7 unplaced tasks

┌ S1 · Enter a workspace ──────┐ ┌ S2 · Run work ────────────────────┐
│ Reach a usable workspace.   │ │ Execute work and retain its state.│
│ Mira · session exchange     │ │ Sol · runner   Tess · sandbox     │
│ Len  · entry screen         │ │ Bo  · review runner protocol      │
│ 2 tasks complete            │ │ T42 · REJECT recorded             │
│ 1 accept ruling             │ │ Ledger: T42 blocks 3 tasks        │
└─────────────────────────────┘ └───────────────────────────────────┘

┌ S3 · Inspect results ────────┐ ┌ S4 · Install and start ───────────┐
│ Find outputs and failures.  │ │ Start the app on a fresh machine. │
│ Aya · result browser        │ │ Ivo · packaging                   │
│ Dev · error presentation    │ │ Assigned · runtime offline        │
│ No rulings recorded         │ │ Ledger still says in progress     │
└─────────────────────────────┘ └───────────────────────────────────┘

┌ S5 · Recover interrupted work ┐ ┌ S6 · Shared foundation ──────────┐
│ Resume without losing work. │ │ Establish the product boundaries.│
│ No linked task evidence     │ │ Nia · architecture packet         │
│                             │ │ T07 complete · ACCEPT recorded    │
└─────────────────────────────┘ └───────────────────────────────────┘

Newly observed: T07 completion and an accept ruling.             Inspect
```

The names and tasks above are illustrative.
The arrangement follows the brief’s order and asserts no architectural connections.
Regions have comparable visual weight; their size does not represent effort, importance, or percentage complete.

These are fixed regions of a drawing.
Tasks do not migrate between status columns, and member activity does not determine the layout.

A narrow panel renders the same regions as compact horizontal sections.
It preserves their order, names, and evidence labels.
The main tab provides more room for simultaneous comparison.

**“Who is doing what, on which part?” should read directly.**

The region supplies the part.
The annotation supplies the assigned person and a short task subject.
A small runtime mark supplies the separately observed activity.

Thus “Run work / Tess / sandbox / active” is available in one glance.
The precise claim is that Tess owns the sandbox task and is observed active.
It does not claim that the pane is currently executing that task unless task-specific attribution supports that statement.

A crowded region shows the most relevant assignments and a readable “+3 assignments” control.
The default prioritizes contested or blocked work, followed by current ledger assignments.
It does not choose whoever most recently emitted terminal output.

The most important five-second facts are:

- Where work is concentrated.
- Which product parts have recorded disputes or dependencies.
- Which intended parts have no linked evidence.
- Who owns the work the operator may need to inspect.

**Altitude changes the question, while preserving the product context.**

| Altitude | What the operator sees | What selecting it reveals |
|---|---|---|
| Product | All declared scope regions, assignment summaries, evidence and dispute marks | A region’s inspection drawer |
| Part | Intended behavior, linked tasks, owners, rulings, declared blockers, source documents | A particular task, ruling, or artifact |
| Evidence | Exact ledger fields, ruling text and attribution, document content, relevant source links | The existing terminal, file, or git surface |
| Intervention | Selected issue and the person who can act on it | A contextual draft beside the relevant member’s pane |

The product remains visible while the drawer opens.
Inspecting one rejection should not make the operator lose the surrounding scope.

In v1, **nothing automatically changes altitude**.
An acceptance produces a quiet change notice.
A rejection or oversize ruling adds a persistent mark to its region and the attention summary.
Neither steals focus or opens a terminal.

A later opt-in attention mode could open a compact request drawer when an agent explicitly addresses a decision to the operator.
That requires a real request contract: recipient, question, affected scope, and available choices.
A model’s inference that something “probably needs you” is insufficient.

The operator can also select a temporary focus set of regions.
The header calls this “Your focus,” and the remaining scope stays visible in a subdued form.
It becomes a team milestone only when a source explicitly declares that relationship.

**The data contract adds identity, not a second planning system.**

The one new convention is a scope address carried by the existing brief and task descriptions.

An architect or lead gives each meaningful scope section in `BRIEF.md` a short, persistent identifier.
For example, a heading can read **[S2] Run work**, followed by the intended behavior already described in the brief.
A linked task description contains a line such as **Scope: S2**.

This requires no new scope file, task database, geometry editor, or agent-maintained dashboard.
The map is a projection of existing planning material.

| Contract element | Producer | Ceremony cost | Meaning |
|---|---|---|---|
| Scope ID, title, and target statement in `BRIEF.md` | Architect or lead | A short pass during the existing brief or architecture preparation | Declared intended scope |
| Scope references in task descriptions | Lead or task creator | One short line while creating or clarifying a task | Declared association with that scope |
| Additional references for shared work | Task creator | Add another ID only when materially relevant | One task affects several regions |
| Corrected references after scope changes | Whoever changes the scope or task | Part of that specific change | Keeps the association explicit |
| Read and observation timestamps | taurhaus | No team ceremony | When the view obtained its evidence |

A reasonable setup budget is five minutes for a handful of useful regions.
That is a design budget, not a measured promise.
If preparing the outline requires a workshop, the convention has grown too expensive.

IDs survive title changes.
Splitting or retiring a scope area requires an explicit decision about existing references.
The interface never silently reassigns tasks because two titles sound similar.

A task may reference multiple areas.
Its annotations appear in each, with a shared-work mark.
Team totals count unique task IDs, so a single architecture task cannot become six completed tasks through repeated display.

Tasks with no reference remain in a prominent **Unplaced work** shelf.
Unknown IDs remain visible as broken references.
Neither is quietly absorbed into a convenient region.

Existing tasks need not all be backfilled.
The first pass should place active work, consequential rulings, and immediate dependencies.
Historical work can remain visibly unplaced.

**Existing sources retain their actual authority.**

| Available source | What the cutaway may say | What it cannot establish alone |
|---|---|---|
| Task status and owner | “Ledger records T42 in progress, assigned to Sol” | Sol is currently working on it; the capability works |
| Task blocks / blockedBy | “These tasks have a declared dependency” | An architectural dependency or reliable critical path |
| Review ruling with by / at / field | “This reviewer recorded this verdict about this field” | The entire scope area is accepted |
| Runtime record and confidence | “This member is observed active/offline with this confidence” | Which task occupies every moment of that activity |
| Routing and completion telemetry | “This launch or completion observation was recorded” | Successful behavior or correct implementation |
| Git commits and changed files | “These repository changes exist” | Which capability they satisfy without an explicit association |
| Brief and architecture documents | “This is the declared intention or architectural description” | That implementation matches it |

Workflow-run trees remain available at the task or member depth.
They explain execution after the operator chooses to inspect it; they need not compete with product scope at rest.

Document and git links enrich a region when an explicit task reference supplies the connection.
Filename similarity may eventually generate a placement suggestion, but the suggestion must remain distinct from a declared association.
V1 needs no such inference.

**Evidence accumulates without turning into a completion score.**

Each region exposes three independent dimensions:

- **Work:** linked task states and assigned owners.
- **Review:** recorded verdicts, their subjects, and their provenance.
- **Observation:** freshness, runtime confidence, and any available artifact references.

There is no combined percentage.
There is also no regional “Done” state manufactured from completed tasks.

“2 tasks complete” opens those two tasks.
“1 accept ruling” opens the exact ruling, including its author, time, field, and text.
“Ledger says blocks 3 tasks” opens the three referenced tasks.

“Complete · no ruling recorded” is valid.
“Awaiting review” requires explicit evidence of a review request or review task; an absent ruling cannot establish it.
An active linked task titled “Review runner protocol” can be shown verbatim without asserting which other artifact it certifies.

Reject, accept, and oversize remain distinct.
Oversize may mean the work needs decomposition; it does not necessarily mean the implementation is incorrect.
Scores remain attached to their original rubric or ruling and never become product progress.

Multiple rulings remain inspectable.
A newer acceptance does not automatically erase an earlier rejection unless the source supplies an explicit resolution or supersession relationship.

For the architecture packet, the view can immediately say:
**“Architecture task complete; accept ruling recorded by the judge.”**
If the ruling does not identify a document revision, the evidence drawer says so.
It cannot certify the current contents of a subsequently edited packet.

**Honesty has to survive both stale sources and stale claims.**

A successful read and a recent event are different facts.
The source bar reports when taurhaus last read the ledger.
The evidence drawer separately reports source modification time and any recorded event time.

A ledger read four seconds ago can still contain a week-old assignment.
An unchanged file is not automatically stale, and an old acceptance does not automatically become invalid.
The interface should expose those facts without inventing one universal freshness threshold.

Concrete failure behavior matters:

- A failed or partial ledger read preserves the last usable snapshot and labels it with the failure and last successful read time.
- An unreadable brief preserves the last scope outline with an equally visible warning.
- A deleted scope ID leaves a retired or missing region while tasks still reference it.
- An offline member with an in-progress assignment shows both facts together.
- Low-confidence runtime activity stays visibly low confidence.
- Missing artifact revision information remains an explicit limitation of the ruling.
- A task mentioning an unknown ID stays unresolved until someone corrects the reference.

Empty space is labeled **“No linked task evidence.”**
It means the view has no association to show.
It cannot establish that no work happened.

The change line distinguishes recorded event times from local observation times.
Without persistent observation history, v1 says “Newly observed this session,” never “Everything that happened since your last visit.”

**The operator’s handles should start from the thing that needs attention.**

Selecting a disputed region exposes the relevant ruling and task before presenting actions.
The operator should not have to reconstruct an issue from a terminal transcript to nudge the right person.

The first version provides these paths:

| Handle | Concrete behavior |
|---|---|
| Inspect the dispute | Open the exact ruling beside its task and declared dependencies |
| Nudge the owner | Open the owner’s existing pane beside the cutaway, with task ID and selected context available as a draft |
| Request reassignment | Open the lead’s pane with the current assignment and requested change in the draft |
| Answer an agent | Open the relevant pane while retaining the selected issue and source text |
| Repair placement | Open the task description or brief section that owns the scope reference |
| Inspect supporting work | Navigate to the referenced document, file, or existing git view |

The terminal is the v1 intervention transport.
The operator explicitly pastes or sends through the existing input surface.
A draft being prepared does not mean it was delivered, and a request being delivered does not mean the ledger changed.

The view reflects reassignment only when the ledger records it.
It never edits status or owner by silently rewriting ledger JSON.

This makes the intervention loop concrete while keeping the feature contained:
select the affected product part, inspect the evidence, address the responsible agent, then watch for the recorded result.

**The smallest honest v1 is a scope drawing with an evidence drawer.**

One feature PR should deliver:

1. A read-only projection of addressed scope headings from the existing brief.
2. Explicit task-to-scope references read from task descriptions.
3. Fixed scope regions in a main tab, with a compact side-panel presentation.
4. Linked task states, assigned owners, recorded rulings, and declared blockers.
5. Runtime activity shown separately from assignment when available.
6. An always-visible unplaced-work count and inspectable shelf.
7. Source freshness and last-known-data behavior on read failure.
8. An inspection drawer and contextual navigation into existing terminal and document surfaces.
9. A small session-local notice for newly observed completion or ruling changes.

The display renders its available snapshot immediately.
Before a usable snapshot exists, it presents explicit states such as “Ledger not yet read” or “No declared scope.”
There are no spinners and no background model generation on the viewing path.

A missing convention produces a useful but limited surface:
current ledger evidence, consequential rulings, and unplaced work.
The product outline appears as soon as addressed scope sections and task references exist.
The UI must never fabricate one to make onboarding look complete.

V1 deliberately omits architectural edges, inferred task placement, screenshot collection, automated demonstrations, milestone parsing, persistent replay, and direct assignment mutation.
It also omits effort-weighted progress and a general graph editor.

These omissions keep the first slice to one new projection, a small reference convention, and existing navigation surfaces.
No daemon needs to maintain a second account of the project.

It is already useful because it answers three questions the current surfaces cannot answer together:
**which product part a task belongs to, who owns that work, and what the ledger’s reviewers have said about it.**

It also exposes the architecture acceptance directly and makes unrepresented scope hard to miss.
The lead may still need `ledger.md` for wave sequencing and milestone gates.
V1 reduces the comprehension burden without claiming to replace information it cannot yet read reliably.

**The later versions should deepen evidence before adding decoration.**

A second version could add persistent observation history and explicit milestone membership.
That would support a trustworthy “since you looked” view and a declared wave overlay without moving the underlying scope regions.

A third version could connect scope to authored architecture relationships and revision-bound behavioral evidence.
The operator could inspect a target capability, its supporting components, and a demonstration of what a particular revision actually does.
An explicit decision-request contract could then support controlled interruption.

Each addition must introduce the source that makes its new claim honest.

**The main risks are failures of interpretation and maintenance.**

| Risk | Design guard |
|---|---|
| A polished outline looks like the complete project | Label it declared scope, identify its source, and keep unplaced work visible |
| Accepted tasks make a capability look finished | Show task-level evidence labels; never manufacture regional completion |
| Region size or shading suggests percentage | Use stable layout and discrete annotations; avoid proportional completion fills |
| The convention becomes another document agents must maintain | Reuse the brief and task descriptions; store identity and association only |
| Scope references become careless boilerplate | Show the target sentence beside tasks during inspection and make corrections easy |
| Shared work inflates apparent progress | Mark repeated placement and deduplicate totals by task ID |
| File-based inference misplaces foundational work | Keep inference out of v1; later suggestions require explicit adoption |
| Renames, splits, or deleted sections erase context | Preserve IDs and retain missing or retired references until resolved |
| A fresh read conceals old claims | Display read freshness separately from event age and runtime confidence |
| Conflicting rulings collapse into a misleading green mark | Preserve verdict subjects and provenance; require explicit resolution |
| Rapid activity destroys the operator’s mental map | Keep scope order fixed and batch session changes into one restrained notice |
| Every rejection becomes an operator interruption | Mark attention persistently; reserve automatic expansion for explicit, opted-in requests |
| The feature grows into a second orchestrator | Use existing panes for intervention and treat the ledger as the recorded outcome |

The first field test should ask an operator to identify an affected product part, its assigned people, a recorded dispute, and the evidence behind an acceptance after a brief glance.
Then ask them to act on one issue without losing the product context.

If they can do that while also recognizing what remains unknown, the cutaway is doing its job.
