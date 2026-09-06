# Brainstorm brief: the team-comprehension view for taurhaus

You are one of two frontier models brainstorming independently. Diverge
boldly; a safe answer that resembles the existing UI is a failed answer.
The constraint list below binds the v1 slice, NOT the concept — dream
first, then land one version.

## The product

taurhaus is a desktop tool (Tauri 2 + Svelte 5 + Rust, dense-but-calm dark
UI, floating-panel layout) whose thesis is "a single clear view into all
projects — code, docs, progress, history — so you never lose context." It
orchestrates managed AI teams ("mesh teams"): a lead agent plus role-based
members (architects, implementers, reviewers, judges) working in tmux panes
on real repos, coordinated through a task ledger with review rulings.

Existing surfaces: a runtime canvas (member nodes, activity states,
workflow-run trees), a kanban task board, per-project overview/files/git
tabs. What does NOT exist: any view anchored on the THING BEING BUILT.

## The problem (real, from today's field test)

The operator ran a 9-member team building a greenfield app. Their words:
"it's hard for me to follow what's going on and to find the proper altitude
I want to follow. Since work is going fast it has to be high enough."
Concretely observed today:

- The lead kept a hand-written markdown ledger because no app view shows
  wave state (milestones, what landed, what's blocked, what's under review).
- The operator learned that the architecture packet landed and got an
  altitude review only via a chat relay from another agent.
- The task board shows task SUBJECTS; the canvas shows WHO is busy — but
  nothing shows WHICH PART of the product is moving, done, or contested.
- Reviews and rulings (accept/reject/oversize) happen in the ledger,
  invisible until someone reads JSON.

The wish, verbatim: "a diagram of the architecture — what we want to build,
or the total project scope — then we see tasks working on which parts of
the project and who they are assigned to… a great impression quickly about
project progress and who is doing what, if somehow possible in a visual
way."

## Data actually available today (ground your concept in this)

- Mesh task ledger per team: id, subject, description, status
  (pending/in_progress/completed/stale), owner (member), blocks/blockedBy,
  metadata: review rulings (verdict/score/ruling with by+at+field), effort,
  deadlines.
- Routing telemetry sidecars: per-task launch attribution (member, role,
  model, capability tier), effort switches, nudges, staleness events,
  completion observations with ruling presence.
- Member runtime records: activity (working/active/idle/offline with
  confidence), pane identity, project binding.
- Claude workflow runs: live phase/agent trees for members running
  orchestrated workflows.
- Git (in-process libgit2): commits, branches, changed files per project.
- Team documents by convention: BRIEF.md, a ledger.md, architecture
  packets and review documents as markdown in the repo.
- Does NOT exist: a machine-readable scope/architecture map, or any
  task↔component mapping. If your concept needs one (it probably does),
  specifying its CHEAPEST HONEST SOURCE is a first-class part of your
  answer: a convention the team roles adopt (e.g. the architect maintains
  a scope file; tasks carry area tags), derivation from files-touched, an
  LLM-derived map, or a hybrid — and what happens when it drifts.

## Constraints for the v1 slice (not the dream)

- Snappy, dense-but-calm, no loading spinners; fits a side panel or a main
  tab in the existing floating-panel layout.
- HONEST WHEN STALE: this team has been burned by dashboards that lie.
  Staleness, unknowns, and confidence must be visible states, never
  papered over. A progress number nobody can trace to evidence is banned.
- No over-engineering: v1 must be buildable as one contained feature PR;
  the concept may sketch the 3-versions-later dream separately.
- The operator is a builder, not a PM: they intervene (nudge, reassign,
  answer a decision request) — a view that only informs but offers no
  handle on the moment that needs them is half a view.

## Deliverables (write a single markdown document)

1. TWO or THREE genuinely distinct concepts, one tight paragraph each —
   different anchors (e.g. scope-map-anchored vs timeline-anchored vs
   attention-anchored), not variations of one idea.
2. Flesh out the ONE you believe in: the anatomy of the view (what is on
   screen at rest), the altitude mechanics (what the highest level shows,
   what one click deeper reveals, when the operator is pulled down
   automatically), how "who is doing what, on what part" reads in five
   seconds, the data contract (exists today vs must be produced — by whom,
   at what ceremony cost), staleness/honesty handling, and the operator's
   handles (what they can DO from the view).
3. The smallest honest v1: what ships first, what it deliberately omits,
   and why it is already useful alone.
4. Risks: the ways this view could mislead, become ceremony nobody feeds,
   or rot — and the design guards against each.

Length: whatever the ideas deserve; typical strong answers run 150-300
lines. Do not describe implementation code; this is concept and contract.
