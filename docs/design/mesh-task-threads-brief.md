# Research brief: task-scoped threads in mesh

Commissioned by the operator, 2026-09-07. One Astra researcher at xhigh.
The deliverable is a design memo at
`docs/design/mesh-task-threads-research.md` — evidence-based, honest
about confidence, ending in a recommendation and the smallest testable
increment. This is research, not implementation.

## The question

Should mesh grow task-scoped threads — all communication about a
specific task collected in one place that every participating agent can
read — instead of (or alongside) today's per-member inboxes? Direct
messages would remain for private and cross-task communication either
way.

The operator's two competing intuitions, both taken seriously:

1. **Against**: every check of a growing thread re-reads more tokens;
   the team spends more, and a wall of thread history may confuse more
   than it informs.
2. **For**: shared visibility could mean *fewer* messages overall —
   less duplication, fewer overlapping or contradictory instructions,
   no re-explaining context that is already on the thread.

## Ground truth to mine (this is the point of the brief)

- `~/projects/taurjob/docs/wave-1/mesh-archive/` — the complete inbox
  state of a real nine-seat wave. Quantify the actual traffic: how many
  envelopes, what share is task-scoped vs broadcast vs DM-like, how much
  content is duplicated across recipients, where overlapping or
  contradictory messages occurred (the retro's ack-contradiction and the
  split lead inbox — team-lead.json's 34 envelopes vs lead-taurjob.json's
  2 — are documented instances).
- `docs/design/field-test-wave1/token-accounting.md` — the real token
  economics, including the finding that the lead's notice traffic
  dominated (≈1.29B cache reads) and that Claude-family verbosity is
  input-coupled (≈0.95 in/out correlation): more input to a Claude seat
  directly produces more output spend. Any thread design changes the
  input side of every seat — model that.
- `docs/design/field-test-wave1/retro-package.md` and both machinery
  audits — the documented communication failures a thread model claims
  to fix. For each, say concretely whether threads would have prevented
  it (the T9 awaiting-GO nudge, the dependency-parked lanes, the
  reviewer-work invisibility) or made it worse.
- Mesh itself: `~/projects/mesh` (READ-ONLY) and its USAGE.md. Note
  what already exists before proposing new storage: tasks carry
  metadata, assignment cards, RESULT messages, rulings, idle-monitor
  records with structured sources, a task mutation journal, and a
  protocol index. Take seriously the possibility that "threads" are a
  READ PROJECTION over records mesh already writes, not a new write
  model — and say what, if anything, is missing for that projection to
  be complete.

## Design dimensions the memo must cover

- **Read economics under explicit assumptions**: cost both models on the
  measured wave-1 traffic under (a) naive full-thread re-reads and
  (b) cursor-disciplined delta reads (mesh already has restart-cursor
  conventions). State the break-even: at what thread length and check
  frequency does shared visibility beat targeted delivery, per family
  (Claude's input-coupling vs GPT's flatter curve).
- **Confusion, both directions**: overlapping-message reduction vs
  wall-of-history cost. What does a seat actually need at uptake time —
  and is that a thread, or a well-formed assignment card (the delivery
  standard's position today)?
- **Scope of visibility**: participants-only vs whole team; where the
  lead sits; how broadcast changes; what remains a DM and whether the
  DM/thread boundary is enforceable or advisory.
- **Interaction with the new machinery**: canonical lead inbox,
  awaiting-GO markers, launch-health records, source-tagged nudges —
  all recently landed. Do threads subsume, complement, or fight them?
- **Consumers beyond agents**: taurhaus telemetry and the planned Scope
  view read mesh state; a thread projection could be exactly what a
  task's walk/history surface wants. Note the fit without designing UI.
- **Compaction**: agents compact; a thread that outlives a context
  window needs a summary story. Who writes it, when, and is it honest?

## Deliverable shape

`docs/design/mesh-task-threads-research.md`: findings with numbers from
the archive; the costed comparison; failure-case walkthrough; a
recommended position (adopt / adapt / decline, with the projection-vs-
storage call made explicitly); the smallest testable increment and what
evidence its trial would produce; open questions. State confidence
honestly and separate measurement from judgment.

## Constraints

- Read-only everywhere: never write to ~/.claude/teams,
  ~/.claude-account2/teams, ~/projects/mesh, ~/projects/job-hunt, or any
  live team state. Scratch probing of mesh behavior only via the locked
  binary in tempdirs (`--claude-dir <tmp>`).
- Never touch the live daemon (port 17233) or the operator's tmux
  server. No credentials anywhere.
- The wave-1 archive contains real team communication: quote sparingly,
  aggregate freely, never move its contents outside the two repos.
